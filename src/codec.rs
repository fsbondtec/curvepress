/// Header framing, ties the pipeline stages together, and computes `max_error`
/// over the FULL lossy pipeline (point-drop + quantization).
///
/// Binary layout (little-endian):
/// ```text
/// [4]  magic "CPRS"
/// [1]  version (= 1)
/// [1]  flags: bits[0-1]=algo, bit[2]=ts_mode, bits[3-7]=reserved
/// [1]  quant_bits (u8)
/// [1]  reserved (= 0)
/// [8]  val_min   (f64 LE)
/// [8]  val_range (f64 LE)
/// [4]  n_kept    (u32 LE)
/// [4]  n_input   (u32 LE)
/// [8]  t0        (i64 LE)
/// [8]  interval  (i64 LE, meaningful only for Regular, else 0)
/// [.]  payload   (value varint stream, then timestamp stream)
/// ```
use crate::error::CpError;
use crate::quantize::{dequantize, quantize};
use crate::rdp::{rdp_n_simplify, rdp_simplify};
use crate::radial::radial_filter;
use crate::varint::{read_varint, write_varint, zigzag_decode, zigzag_encode};
use crate::vw::vw_simplify;
use crate::{Algo, Config, Stats, TsMode};

const MAGIC: &[u8; 4] = b"CPRS";
const VERSION: u8 = 1;
const HEADER_LEN: usize = 4 + 1 + 1 + 1 + 1 + 8 + 8 + 4 + 4 + 8 + 8; // = 48

// ─── helpers ────────────────────────────────────────────────────────────────

fn write_i64_le(buf: &mut Vec<u8>, v: i64) {
    buf.extend_from_slice(&v.to_le_bytes());
}
fn write_f64_le(buf: &mut Vec<u8>, v: f64) {
    buf.extend_from_slice(&v.to_le_bytes());
}
fn write_u32_le(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn read_i64_le(data: &[u8], off: usize) -> Option<i64> {
    data.get(off..off + 8).map(|b| i64::from_le_bytes(b.try_into().unwrap()))
}
fn read_f64_le(data: &[u8], off: usize) -> Option<f64> {
    data.get(off..off + 8).map(|b| f64::from_le_bytes(b.try_into().unwrap()))
}
fn read_u32_le(data: &[u8], off: usize) -> Option<u32> {
    data.get(off..off + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()))
}

/// Resolve the effective search/normalization range.
///
/// `value_range` from `Config` is an optional override. When it is <= 0
/// (the default is 1.0, but explicit 0.0 means "auto"), fall back to the
/// measured data range.
fn effective_value_range(cfg_value_range: f64, measured: f64) -> f64 {
    if cfg_value_range > 0.0 {
        cfg_value_range
    } else {
        measured.max(1.0)
    }
}

// ─── compress ───────────────────────────────────────────────────────────────

pub fn compress_inner(
    timestamps_ns: &[i64],
    values: &[f64],
    cfg: &Config,
) -> Result<(Vec<u8>, Stats), CpError> {
    let n = timestamps_ns.len();
    if n != values.len() {
        return Err(CpError::BadInput("timestamps and values have different lengths".into()));
    }
    if n == 0 {
        return Err(CpError::BadInput("empty input".into()));
    }
    // Validate timestamps monotonicity.
    for i in 1..n {
        if timestamps_ns[i] <= timestamps_ns[i - 1] {
            return Err(CpError::BadInput(format!(
                "timestamps not strictly increasing at index {i}"
            )));
        }
    }
    // Validate values.
    for (i, &v) in values.iter().enumerate() {
        if !v.is_finite() {
            return Err(CpError::BadInput(format!(
                "non-finite value at index {i}: {v}"
            )));
        }
    }

    // Measure actual data range for fallback.
    let v_min_data = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let v_max_data = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let measured_range = v_max_data - v_min_data;
    let eff_vrange = effective_value_range(cfg.value_range, measured_range);

    // 1. Optional radial pre-filter.
    let prefilter_mask: Vec<bool> = if let Some(r) = cfg.radial_prefilter {
        radial_filter(timestamps_ns, values, r)
    } else {
        vec![true; n]
    };

    // Extract pre-filtered points.
    let (pf_ts, pf_val): (Vec<i64>, Vec<f64>) = timestamps_ns
        .iter()
        .zip(values.iter())
        .enumerate()
        .filter_map(|(i, (&t, &v))| if prefilter_mask[i] { Some((t, v)) } else { None })
        .unzip();

    // 2. Point reduction.
    let kept_mask: Vec<bool> = match cfg.algo {
        Algo::Rdp => rdp_simplify(
            &pf_ts, &pf_val,
            cfg.epsilon,
            cfg.normalize_axes,
            eff_vrange,
        ),
        Algo::Vw => vw_simplify(
            &pf_ts, &pf_val,
            cfg.n_out,
            cfg.normalize_axes,
            eff_vrange,
        ),
        Algo::RdpN => rdp_n_simplify(
            &pf_ts, &pf_val,
            cfg.n_out,
            eff_vrange,
            cfg.normalize_axes,
            eff_vrange,
        ),
    };

    // Extract kept points from pre-filtered set.
    let (kept_ts, kept_val): (Vec<i64>, Vec<f64>) = pf_ts
        .iter()
        .zip(pf_val.iter())
        .enumerate()
        .filter_map(|(i, (&t, &v))| if kept_mask[i] { Some((t, v)) } else { None })
        .unzip();

    let n_kept = kept_ts.len();

    // 3. Quantize kept values.
    let q = quantize(&kept_val, cfg.epsilon);

    // 4. Compute max_error over ALL original input points (full lossy pipeline).
    //    Reconstruct via linear interpolation between quantized kept points.
    let recon_kept = dequantize(&q.codes, q.val_min, q.val_range, q.n_bits);
    let max_error = compute_max_error(timestamps_ns, values, &kept_ts, &recon_kept);

    // 5. Encode payload.
    let mut payload = Vec::new();

    // Value stream: q[0] raw, then zigzag deltas.
    write_varint(&mut payload, q.codes[0] as u64);
    for i in 1..n_kept {
        let delta = q.codes[i] as i64 - q.codes[i - 1] as i64;
        write_varint(&mut payload, zigzag_encode(delta));
    }

    // Timestamp stream.
    match cfg.ts_mode {
        TsMode::Regular => {
            // Regular: store t0, interval, then delta-of-original-indices (zigzag+varint).
            let t0 = kept_ts[0];
            let interval = if n >= 2 {
                timestamps_ns[1] - timestamps_ns[0]
            } else {
                1_000_000_000i64 // 1s default if only one point
            };
            write_i64_le_buf(&mut payload, t0);
            write_i64_le_buf(&mut payload, interval);
            // Map kept timestamps to original indices.
            let mut orig_idx = 0usize;
            let mut prev_orig_idx = 0usize;
            let mut first = true;
            for &kt in &kept_ts {
                while timestamps_ns[orig_idx] != kt {
                    orig_idx += 1;
                }
                let delta = orig_idx as i64 - prev_orig_idx as i64;
                if first {
                    write_varint(&mut payload, orig_idx as u64);
                    first = false;
                } else {
                    write_varint(&mut payload, zigzag_encode(delta));
                }
                prev_orig_idx = orig_idx;
            }
        }
        TsMode::Irregular => {
            // Irregular: t0, then zigzag deltas between consecutive kept timestamps.
            write_i64_le_buf(&mut payload, kept_ts[0]);
            for i in 1..n_kept {
                let delta = kept_ts[i] - kept_ts[i - 1];
                write_varint(&mut payload, zigzag_encode(delta));
            }
        }
    }

    // 6. Assemble header.
    let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    let algo_bits = match cfg.algo { Algo::Rdp => 0u8, Algo::Vw => 1, Algo::RdpN => 2 };
    let ts_bit = if cfg.ts_mode == TsMode::Irregular { 0u8 } else { 1u8 << 2 };
    out.push(algo_bits | ts_bit);
    out.push(q.n_bits as u8);
    out.push(0u8); // reserved
    write_f64_le(&mut out, q.val_min);
    write_f64_le(&mut out, q.val_range);
    write_u32_le(&mut out, n_kept as u32);
    write_u32_le(&mut out, n as u32);
    let t0 = kept_ts[0];
    let interval = match cfg.ts_mode {
        TsMode::Regular if n >= 2 => timestamps_ns[1] - timestamps_ns[0],
        _ => 0,
    };
    write_i64_le(&mut out, t0);
    write_i64_le(&mut out, interval);
    out.extend_from_slice(&payload);

    let bytes_raw = n * 16;
    let bytes_compressed = out.len();
    let stats = Stats {
        n_input: n,
        n_kept,
        bytes_raw,
        bytes_compressed,
        ratio: bytes_raw as f64 / bytes_compressed as f64,
        max_error,
        quant_bits: q.n_bits,
    };

    Ok((out, stats))
}

/// Write an i64 little-endian into a payload Vec (without the outer `out` Vec).
fn write_i64_le_buf(buf: &mut Vec<u8>, v: i64) {
    buf.extend_from_slice(&v.to_le_bytes());
}

// ─── decompress ─────────────────────────────────────────────────────────────

pub fn decompress_inner(data: &[u8]) -> Result<(Vec<i64>, Vec<f64>), CpError> {
    if data.len() < HEADER_LEN {
        return Err(CpError::Corrupt);
    }
    if &data[0..4] != MAGIC {
        return Err(CpError::Corrupt);
    }
    if data[4] != VERSION {
        return Err(CpError::Corrupt);
    }

    let flags = data[5];
    let ts_mode = if flags & (1 << 2) != 0 { TsMode::Regular } else { TsMode::Irregular };
    let quant_bits = data[6] as u32;

    let val_min = read_f64_le(data, 8).ok_or(CpError::Corrupt)?;
    let val_range = read_f64_le(data, 16).ok_or(CpError::Corrupt)?;
    let n_kept = read_u32_le(data, 24).ok_or(CpError::Corrupt)? as usize;
    let _n_input = read_u32_le(data, 28).ok_or(CpError::Corrupt)?;
    let _t0_header = read_i64_le(data, 32).ok_or(CpError::Corrupt)?;
    let _interval_header = read_i64_le(data, 40).ok_or(CpError::Corrupt)?;

    if n_kept == 0 {
        return Err(CpError::Corrupt);
    }

    let mut pos = HEADER_LEN;

    // Decode value stream.
    let first_code = read_varint(data, &mut pos).ok_or(CpError::Corrupt)? as u32;
    let mut codes = vec![first_code];
    for _ in 1..n_kept {
        let delta = zigzag_decode(read_varint(data, &mut pos).ok_or(CpError::Corrupt)?);
        let prev = *codes.last().unwrap() as i64;
        codes.push((prev + delta) as u32);
    }

    let values = dequantize(&codes, val_min, val_range, quant_bits);

    // Decode timestamp stream.
    let timestamps: Vec<i64> = match ts_mode {
        TsMode::Irregular => {
            let t0 = i64::from_le_bytes(
                data.get(pos..pos + 8).ok_or(CpError::Corrupt)?.try_into().unwrap(),
            );
            pos += 8;
            let mut ts = vec![t0];
            for _ in 1..n_kept {
                let delta = zigzag_decode(read_varint(data, &mut pos).ok_or(CpError::Corrupt)?);
                let last = *ts.last().unwrap();
                ts.push(last + delta);
            }
            ts
        }
        TsMode::Regular => {
            let t0 = i64::from_le_bytes(
                data.get(pos..pos + 8).ok_or(CpError::Corrupt)?.try_into().unwrap(),
            );
            pos += 8;
            let interval = i64::from_le_bytes(
                data.get(pos..pos + 8).ok_or(CpError::Corrupt)?.try_into().unwrap(),
            );
            pos += 8;
            // Read first index.
            let first_idx = read_varint(data, &mut pos).ok_or(CpError::Corrupt)? as i64;
            let mut ts = vec![t0 + first_idx * interval];
            let mut prev_idx = first_idx;
            for _ in 1..n_kept {
                let delta = zigzag_decode(read_varint(data, &mut pos).ok_or(CpError::Corrupt)?);
                prev_idx += delta;
                ts.push(t0 + prev_idx * interval);
            }
            ts
        }
    };

    Ok((timestamps, values))
}

// ─── interpolate ────────────────────────────────────────────────────────────

/// Linear interpolation onto a regular grid.
///
/// - Grid points outside `[ts[0], ts[n-1]]` are clamped (flat extrapolation).
/// - Output count = `floor((t_end - t_start) / interval_ns) + 1`.
pub fn interpolate_inner(
    ts: &[i64],
    val: &[f64],
    t_start: i64,
    t_end: i64,
    interval_ns: i64,
) -> Result<Vec<f64>, crate::error::CpError> {
    use crate::error::CpError;
    if ts.is_empty() {
        return Err(CpError::BadInput("empty ts".into()));
    }
    if interval_ns <= 0 {
        return Err(CpError::BadInput("interval_ns must be > 0".into()));
    }
    if t_end < t_start {
        return Err(CpError::BadInput("t_end < t_start".into()));
    }
    let n_out = ((t_end - t_start) / interval_ns) as usize + 1;
    let mut out = Vec::with_capacity(n_out);

    let n = ts.len();
    let mut j = 0usize; // cursor into ts

    for k in 0..n_out {
        let t = t_start + k as i64 * interval_ns;

        // Clamp to data range.
        if t <= ts[0] {
            out.push(val[0]);
            continue;
        }
        if t >= ts[n - 1] {
            out.push(val[n - 1]);
            continue;
        }

        // Advance j so that ts[j] <= t < ts[j+1].
        while j + 1 < n - 1 && ts[j + 1] <= t {
            j += 1;
        }

        let span = (ts[j + 1] - ts[j]) as f64;
        let frac = (t - ts[j]) as f64 / span;
        out.push(val[j] + frac * (val[j + 1] - val[j]));
    }
    Ok(out)
}

// ─── max_error ──────────────────────────────────────────────────────────────

/// Compute the maximum absolute error over ALL original input points.
///
/// For each original point, the reconstructed value is obtained by linear
/// interpolation between the nearest kept (and quantized) neighbors.
/// Points before the first kept point or after the last use flat extrapolation.
fn compute_max_error(
    orig_ts: &[i64],
    orig_val: &[f64],
    kept_ts: &[i64],
    kept_recon: &[f64],
) -> f64 {
    if kept_ts.is_empty() {
        return 0.0;
    }
    let n_kept = kept_ts.len();
    let mut max_err = 0.0_f64;
    let mut j = 0usize;

    for (&t, &orig) in orig_ts.iter().zip(orig_val.iter()) {
        let recon = if t <= kept_ts[0] {
            kept_recon[0]
        } else if t >= kept_ts[n_kept - 1] {
            kept_recon[n_kept - 1]
        } else {
            // Advance j.
            while j + 1 < n_kept - 1 && kept_ts[j + 1] <= t {
                j += 1;
            }
            let span = (kept_ts[j + 1] - kept_ts[j]) as f64;
            let frac = (t - kept_ts[j]) as f64 / span;
            kept_recon[j] + frac * (kept_recon[j + 1] - kept_recon[j])
        };
        let err = (orig - recon).abs();
        if err > max_err {
            max_err = err;
        }
    }
    max_err
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Algo, Config};

    fn sine_series(n: usize) -> (Vec<i64>, Vec<f64>) {
        let ts: Vec<i64> = (0..n as i64).map(|i| i * 1_000_000).collect();
        let val: Vec<f64> = ts.iter().map(|&t| (t as f64 * 1e-6 * 0.1).sin() * 100.0).collect();
        (ts, val)
    }

    #[test]
    fn roundtrip_rdp() {
        let (ts, val) = sine_series(1000);
        let cfg = Config { algo: Algo::Rdp, epsilon: 1.0, ..Default::default() };
        let (data, stats) = compress_inner(&ts, &val, &cfg).unwrap();
        let (ts_out, val_out) = decompress_inner(&data).unwrap();
        assert!(ts_out.len() < ts.len());
        assert_eq!(ts_out.len(), val_out.len());
        assert_eq!(ts_out.len(), stats.n_kept);
    }

    #[test]
    fn roundtrip_vw() {
        let (ts, val) = sine_series(500);
        let cfg = Config { algo: Algo::Vw, n_out: 50, ..Default::default() };
        let (data, stats) = compress_inner(&ts, &val, &cfg).unwrap();
        let (ts_out, val_out) = decompress_inner(&data).unwrap();
        assert_eq!(ts_out.len(), 50);
        assert_eq!(val_out.len(), 50);
        assert_eq!(stats.n_kept, 50);
    }

    #[test]
    fn corrupt_magic_returns_error() {
        let mut data = vec![0u8; 64];
        assert!(matches!(decompress_inner(&data), Err(CpError::Corrupt)));
        data[0..4].copy_from_slice(b"CPRS");
        // Still corrupt (version mismatch).
        data[4] = 99;
        assert!(matches!(decompress_inner(&data), Err(CpError::Corrupt)));
    }
}
