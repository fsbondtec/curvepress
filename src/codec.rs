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
/// [.]  payload   (value varint stream, then timestamp stream)
/// ```
use crate::error::CpError;
use crate::quantize::{dequantize, quantize};
use crate::rdp::{rdp_n_simplify, rdp_simplify};
use crate::varint::{read_varint, write_varint, zigzag_decode, zigzag_encode};
use crate::vw::vw_simplify;
use crate::{Algo, Config, Stats};

const MAGIC: &[u8; 4] = b"CPRS";
const VERSION: u8 = 1;
const HEADER_LEN: usize = 4 + 1 + 1 + 1 + 1 + 8 + 8 + 4 + 4; // = 32

// ─── helpers ────────────────────────────────────────────────────────────────

fn write_f64_le(buf: &mut Vec<u8>, v: f64) {
    buf.extend_from_slice(&v.to_le_bytes());
}
fn write_u32_le(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn read_f64_le(data: &[u8], off: usize) -> Option<f64> {
    data.get(off..off + 8).map(|b| f64::from_le_bytes(b.try_into().unwrap()))
}
fn read_u32_le(data: &[u8], off: usize) -> Option<u32> {
    data.get(off..off + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()))
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

    // Measure actual data range (auto-detect, always used).
    let v_min_data = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let v_max_data = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let eff_vrange = (v_max_data - v_min_data).max(1.0);

    // 1. Point reduction (normalize_axes always true).
    let kept_mask: Vec<bool> = match cfg.algo {
        Algo::Rdp => rdp_simplify(
            timestamps_ns, values,
            cfg.epsilon,
            true,
            eff_vrange,
        ),
        Algo::Vw => vw_simplify(
            timestamps_ns, values,
            cfg.n_out,
            true,
            eff_vrange,
        ),
        Algo::RdpN => rdp_n_simplify(
            timestamps_ns, values,
            cfg.n_out,
            eff_vrange,
            true,
            eff_vrange,
        ),
    };

    // Extract kept points.
    let (kept_ts, kept_val): (Vec<i64>, Vec<f64>) = timestamps_ns
        .iter()
        .zip(values.iter())
        .enumerate()
        .filter_map(|(i, (&t, &v))| if kept_mask[i] { Some((t, v)) } else { None })
        .unzip();

    let n_kept = kept_ts.len();

    // 3. Determine quantization epsilon.
    //
    // RDP: cfg.epsilon is the explicit geometric tolerance — use it directly.
    //
    // VW / RDP-n: cfg.epsilon is not used by the simplifier, so using it for
    // quantization would be arbitrary.  Instead, measure the actual maximum
    // vertical deviation of every dropped point from the piecewise-linear
    // interpolation of the kept points (using original values).  That is the
    // true reconstruction error introduced by the simplification step, and
    // quantizing with it adds at most half that value on top — giving a
    // self-consistent total error of ~1.5 × effective_epsilon.
    //
    // Fallback: if all points were kept (n_kept == pf_ts.len()) the measured
    // error is 0 — we fall back to cfg.epsilon so the caller still controls
    // quantization granularity.
    let quant_epsilon = match cfg.algo {
        Algo::Rdp => cfg.epsilon,
        Algo::Vw | Algo::RdpN => {
            let measured = max_reconstruction_error_of_dropped(
                timestamps_ns, values, &kept_ts, &kept_val,
            );
            if measured > 0.0 {
                measured
            } else if cfg.epsilon > 0.0 {
                cfg.epsilon
            } else {
                // VW with no epsilon: use near-lossless quantization.
                (eff_vrange / 1_000_000.0).max(f64::EPSILON)
            }
        }
    };

    // Quantize kept values.
    let q = quantize(&kept_val, quant_epsilon);

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

    // Timestamp stream: always Irregular — t0 as plain varint, then plain varint deltas.
    write_varint(&mut payload, kept_ts[0] as u64);
    for i in 1..n_kept {
        let delta = kept_ts[i] - kept_ts[i - 1];
        write_varint(&mut payload, delta as u64);
    }

    // 6. Assemble header.
    let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    let algo_bits = match cfg.algo { Algo::Rdp => 0u8, Algo::Vw => 1, Algo::RdpN => 2 };
    out.push(algo_bits); // ts_mode bit = 0 (always Irregular)
    out.push(q.n_bits as u8);
    out.push(0u8); // reserved
    write_f64_le(&mut out, q.val_min);
    write_f64_le(&mut out, q.val_range);
    write_u32_le(&mut out, n_kept as u32);
    write_u32_le(&mut out, n as u32);
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

    let _flags = data[5];
    // ts_mode is always Irregular (ts_mode bit is always 0 in streams produced
    // by this library). Legacy Regular-encoded streams are not supported.
    let quant_bits = data[6] as u32;

    let val_min = read_f64_le(data, 8).ok_or(CpError::Corrupt)?;
    let val_range = read_f64_le(data, 16).ok_or(CpError::Corrupt)?;
    let n_kept = read_u32_le(data, 24).ok_or(CpError::Corrupt)? as usize;
    let _n_input = read_u32_le(data, 28).ok_or(CpError::Corrupt)?;

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

    // Decode timestamp stream (always Irregular: t0 as plain varint, then plain varint deltas).
    let timestamps: Vec<i64> = {
        let t0 = read_varint(data, &mut pos).ok_or(CpError::Corrupt)? as i64;
        let mut ts = vec![t0];
        for _ in 1..n_kept {
            let delta = read_varint(data, &mut pos).ok_or(CpError::Corrupt)? as i64;
            let last = *ts.last().unwrap();
            ts.push(last + delta);
        }
        ts
    };

    Ok((timestamps, values))
}

// ─── interpolate ────────────────────────────────────────────────────────────

/// Linear interpolation for a single query timestamp `t`.
///
/// - `t` outside `[ts[0], ts[last]]` is clamped (flat extrapolation).
pub fn interpolate_point(
    ts: &[i64],
    val: &[f64],
    t: i64,
) -> Result<f64, CpError> {
    if ts.is_empty() {
        return Err(CpError::BadInput("empty ts".into()));
    }
    let n = ts.len();
    if t <= ts[0] {
        return Ok(val[0]);
    }
    if t >= ts[n - 1] {
        return Ok(val[n - 1]);
    }
    // Binary search: find j such that ts[j] <= t < ts[j+1].
    let j = ts.partition_point(|&x| x <= t) - 1;
    let span = (ts[j + 1] - ts[j]) as f64;
    let frac = (t - ts[j]) as f64 / span;
    Ok(val[j] + frac * (val[j + 1] - val[j]))
}

// ─── max_error ──────────────────────────────────────────────────────────────

/// Maximum vertical deviation of **dropped** points from the piecewise-linear
/// interpolation of the kept points (using original, pre-quantization values).
///
/// This is used as the quantization epsilon for VW and RDP-n: it is the actual
/// simplification error, so quantizing with it keeps the total pipeline error
/// self-consistent at ~1.5× this value.
///
/// Returns 0.0 when all points were kept.
fn max_reconstruction_error_of_dropped(
    pf_ts: &[i64],
    pf_val: &[f64],
    kept_ts: &[i64],
    kept_val: &[f64],
) -> f64 {
    let n_kept = kept_ts.len();
    if n_kept == 0 || n_kept == pf_ts.len() {
        return 0.0;
    }

    // Build a lookup set of kept timestamps so we can skip them quickly.
    let kept_set: std::collections::HashSet<i64> = kept_ts.iter().cloned().collect();
    let mut max_err = 0.0_f64;
    let mut j = 0usize;

    for (&t, &v) in pf_ts.iter().zip(pf_val.iter()) {
        if kept_set.contains(&t) {
            continue; // this point is kept — no error contribution
        }
        // Advance j so that kept_ts[j] <= t < kept_ts[j+1].
        while j + 1 < n_kept - 1 && kept_ts[j + 1] <= t {
            j += 1;
        }
        let span = (kept_ts[j + 1] - kept_ts[j]) as f64;
        let frac = (t - kept_ts[j]) as f64 / span;
        let recon = kept_val[j] + frac * (kept_val[j + 1] - kept_val[j]);
        let err = (v - recon).abs();
        if err > max_err {
            max_err = err;
        }
    }
    max_err
}

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
        let cfg = Config { algo: Algo::Rdp, epsilon: 1.0, n_out: 0 };
        let (data, stats) = compress_inner(&ts, &val, &cfg).unwrap();
        let (ts_out, val_out) = decompress_inner(&data).unwrap();
        assert!(ts_out.len() < ts.len());
        assert_eq!(ts_out.len(), val_out.len());
        assert_eq!(ts_out.len(), stats.n_kept);
    }

    #[test]
    fn roundtrip_vw() {
        let (ts, val) = sine_series(500);
        let cfg = Config { algo: Algo::Vw, epsilon: 0.0, n_out: 50 };
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

    #[test]
    fn interpolate_point_linear() {
        let ts = vec![0i64, 10_000, 20_000, 30_000];
        let val = vec![0.0f64, 10.0, 20.0, 30.0];
        assert!((interpolate_point(&ts, &val, 5_000).unwrap() - 5.0).abs() < 1e-9);
        assert!((interpolate_point(&ts, &val, 0).unwrap() - 0.0).abs() < 1e-9);
        assert!((interpolate_point(&ts, &val, 30_000).unwrap() - 30.0).abs() < 1e-9);
    }

    #[test]
    fn interpolate_point_clamps() {
        let ts = vec![10_000i64, 20_000];
        let val = vec![5.0f64, 10.0];
        assert!((interpolate_point(&ts, &val, 0).unwrap() - 5.0).abs() < 1e-9);
        assert!((interpolate_point(&ts, &val, 99_999).unwrap() - 10.0).abs() < 1e-9);
    }
}
