use curvepress::{compress, compress_with_stats, decompress, interpolate, Algo, Config, TsMode};

// ─── helpers ─────────────────────────────────────────────────────────────────

fn sine_ts(n: usize) -> (Vec<i64>, Vec<f64>) {
    let ts: Vec<i64> = (0..n as i64).map(|i| i * 1_000_000).collect();
    let val: Vec<f64> = ts.iter().map(|&t| (t as f64 * 1e-6 * 0.05).sin() * 100.0).collect();
    (ts, val)
}

/// Fracture (Abrisskurve): slow ramp → sharp peak → steep drop to ~0.
fn fracture_ts() -> (Vec<i64>, Vec<f64>) {
    let n = 500usize;
    let ts: Vec<i64> = (0..n as i64).map(|i| i * 1_000_000).collect();
    let mut val = vec![0.0f64; n];
    // Slow rise: indices 0..300 ramp from 0 to 100.
    for i in 0..300 {
        val[i] = i as f64 / 3.0;
    }
    // Sharp peak at index 300: value 150.
    val[300] = 150.0;
    // Steep drop: indices 301..310 drop from 100 to 1.
    for i in 301..310 {
        val[i] = 100.0 - (i - 300) as f64 * 11.0;
    }
    // Clamp negatives to 0.
    for v in &mut val {
        if *v < 0.0 {
            *v = 0.0;
        }
    }
    // Remainder stays near 0 with tiny noise (still finite).
    for i in 310..n {
        val[i] = 0.5 * ((i as f64 * 0.3).sin().abs());
    }
    (ts, val)
}

// ─── Round-trip tests ────────────────────────────────────────────────────────

#[test]
fn roundtrip_rdp_kept_points_exact() {
    let (ts, val) = sine_ts(2000);
    let cfg = Config { algo: Algo::Rdp, epsilon: 1.0, ..Default::default() };
    let (data, stats) = compress_with_stats(&ts, &val, &cfg).unwrap();
    let (ts_out, val_out) = decompress(&data).unwrap();

    assert!(ts_out.len() < ts.len(), "RDP should reduce point count");
    assert_eq!(ts_out.len(), val_out.len());
    assert_eq!(ts_out.len(), stats.n_kept);

    // Every dropped point must be within epsilon of the linear interpolant of its kept neighbors.
    check_all_dropped_within_epsilon(&ts, &val, &ts_out, &val_out, cfg.epsilon);
}

#[test]
fn roundtrip_vw_returns_exact_n_out() {
    let (ts, val) = sine_ts(500);
    let cfg = Config { algo: Algo::Vw, n_out: 40, ..Default::default() };
    let (data, stats) = compress_with_stats(&ts, &val, &cfg).unwrap();
    let (ts_out, val_out) = decompress(&data).unwrap();

    assert_eq!(ts_out.len(), 40, "VW must return exactly n_out points");
    assert_eq!(val_out.len(), 40);
    assert_eq!(stats.n_kept, 40);
}

#[test]
fn roundtrip_rdp_n_at_most_n_out() {
    let (ts, val) = sine_ts(1000);
    let cfg = Config { algo: Algo::RdpN, n_out: 60, ..Default::default() };
    let (data, stats) = compress_with_stats(&ts, &val, &cfg).unwrap();
    let (ts_out, _) = decompress(&data).unwrap();

    assert!(ts_out.len() <= 60, "RDP-n must return at most n_out; got {}", ts_out.len());
    assert!(ts_out.len() >= 2, "must keep at least endpoints");
    assert_eq!(stats.n_kept, ts_out.len());
}

// ─── Fracture-curve test (primary use case) ──────────────────────────────────

#[test]
fn fracture_curve_peak_and_first_post_drop_kept() {
    let (ts, val) = fracture_ts();
    let cfg = Config {
        algo: Algo::Rdp,
        epsilon: 1.0,
        ..Default::default()
    };
    let data = compress(&ts, &val, &cfg).unwrap();
    let (ts_out, _val_out) = decompress(&data).unwrap();

    let peak_ts = ts[300];
    let post_drop_ts = ts[301];

    assert!(
        ts_out.contains(&peak_ts),
        "Peak point (index 300, ts={peak_ts}) must be kept"
    );
    assert!(
        ts_out.contains(&post_drop_ts),
        "First post-drop point (index 301, ts={post_drop_ts}) must be kept"
    );
}

// ─── Constant series ─────────────────────────────────────────────────────────

#[test]
fn constant_series_keeps_only_endpoints() {
    let ts: Vec<i64> = (0..100i64).map(|i| i * 1_000_000).collect();
    let val = vec![42.0f64; 100];
    let cfg = Config { algo: Algo::Rdp, epsilon: 0.1, ..Default::default() };
    let (data, stats) = compress_with_stats(&ts, &val, &cfg).unwrap();
    let (ts_out, val_out) = decompress(&data).unwrap();

    assert_eq!(ts_out.len(), 2, "Constant series: only 2 endpoints should be kept");
    assert!((val_out[0] - 42.0).abs() < 1e-9);
    assert!((val_out[1] - 42.0).abs() < 1e-9);
    assert_eq!(stats.quant_bits, 1);
    assert!(stats.max_error < 1e-12, "max_error for constant series must be ~0");
}

// ─── Regular timestamp mode ───────────────────────────────────────────────────

#[test]
fn regular_ts_mode_reconstruction_exact() {
    let n = 1000usize;
    let t0 = 1_000_000_000i64;
    let interval = 500_000i64; // 500 µs
    let ts: Vec<i64> = (0..n as i64).map(|i| t0 + i * interval).collect();
    let val: Vec<f64> = (0..n).map(|i| (i as f64 * 0.01).cos() * 50.0).collect();

    let cfg = Config {
        algo: Algo::Rdp,
        ts_mode: TsMode::Regular,
        epsilon: 0.5,
        ..Default::default()
    };
    let data = compress(&ts, &val, &cfg).unwrap();
    let (ts_out, _) = decompress(&data).unwrap();

    // Every decoded timestamp must be a valid sample time (t0 + k * interval).
    for &t in &ts_out {
        assert_eq!((t - t0) % interval, 0, "Decoded ts {t} is not on the regular grid");
    }
    assert!(ts_out[0] == t0 || ts_out[0] >= t0);
    assert!(*ts_out.last().unwrap() <= ts[n - 1]);
}

// ─── Axis normalisation ───────────────────────────────────────────────────────

#[test]
fn normalize_axes_produces_different_result() {
    // ns timestamps + values 0..500 — without normalization the time axis
    // (nanoseconds) overwhelms the value axis, keeping the wrong points.
    let n = 200usize;
    let ts: Vec<i64> = (0..n as i64).map(|i| i * 1_000_000_000).collect(); // 1s apart
    let val: Vec<f64> = (0..n).map(|i| (i as f64 * 0.15).sin() * 250.0 + 250.0).collect();

    let cfg_no_norm = Config {
        algo: Algo::Rdp, epsilon: 10.0, normalize_axes: false, value_range: 500.0,
        ..Default::default()
    };
    let cfg_norm = Config {
        algo: Algo::Rdp, epsilon: 10.0, normalize_axes: true, value_range: 500.0,
        ..Default::default()
    };

    let data_no = compress(&ts, &val, &cfg_no_norm).unwrap();
    let data_yes = compress(&ts, &val, &cfg_norm).unwrap();
    let (ts_no, _) = decompress(&data_no).unwrap();
    let (ts_yes, _) = decompress(&data_yes).unwrap();

    // The two should NOT be identical (axis normalization changes which points survive).
    assert_ne!(ts_no, ts_yes, "normalize_axes should produce different kept points");
}

// ─── Edge cases ───────────────────────────────────────────────────────────────

#[test]
fn edge_case_single_point() {
    let ts = vec![0i64];
    let val = vec![1.0f64];
    let cfg = Config::default();
    let data = compress(&ts, &val, &cfg).unwrap();
    let (ts_out, val_out) = decompress(&data).unwrap();
    assert_eq!(ts_out, vec![0i64]);
    assert!((val_out[0] - 1.0).abs() < 1.0); // within epsilon
}

#[test]
fn edge_case_two_points() {
    let ts = vec![0i64, 1_000_000];
    let val = vec![0.0f64, 1.0];
    let cfg = Config::default();
    let data = compress(&ts, &val, &cfg).unwrap();
    let (ts_out, val_out) = decompress(&data).unwrap();
    assert_eq!(ts_out.len(), 2);
    assert!((val_out[0] - 0.0).abs() < 2.0);
    assert!((val_out[1] - 1.0).abs() < 2.0);
}

#[test]
fn nan_value_returns_bad_input() {
    let ts = vec![0i64, 1_000_000, 2_000_000];
    let val = vec![0.0f64, f64::NAN, 2.0];
    let cfg = Config::default();
    assert!(matches!(compress(&ts, &val, &cfg), Err(curvepress::CpError::BadInput(_))));
}

#[test]
fn inf_value_returns_bad_input() {
    let ts = vec![0i64, 1_000_000];
    let val = vec![0.0f64, f64::INFINITY];
    let cfg = Config::default();
    assert!(matches!(compress(&ts, &val, &cfg), Err(curvepress::CpError::BadInput(_))));
}

#[test]
fn non_monotonic_ts_returns_bad_input() {
    let ts = vec![0i64, 2_000_000, 1_000_000];
    let val = vec![0.0f64, 1.0, 2.0];
    let cfg = Config::default();
    assert!(matches!(compress(&ts, &val, &cfg), Err(curvepress::CpError::BadInput(_))));
}

#[test]
fn duplicate_ts_returns_bad_input() {
    let ts = vec![0i64, 1_000_000, 1_000_000];
    let val = vec![0.0f64, 1.0, 2.0];
    let cfg = Config::default();
    assert!(matches!(compress(&ts, &val, &cfg), Err(curvepress::CpError::BadInput(_))));
}

// ─── Stats checks ─────────────────────────────────────────────────────────────

#[test]
fn stats_max_error_within_1_5_epsilon_no_normalize() {
    let (ts, val) = sine_ts(2000);
    let epsilon = 2.0;
    let cfg = Config {
        algo: Algo::Rdp,
        epsilon,
        normalize_axes: false,
        ..Default::default()
    };
    let (_, stats) = compress_with_stats(&ts, &val, &cfg).unwrap();
    assert!(
        stats.max_error <= epsilon * 1.5 + 1e-9,
        "max_error {} > 1.5 * epsilon {}",
        stats.max_error,
        epsilon * 1.5
    );
}

#[test]
fn stats_quant_bits_epsilon_range_ratio() {
    // epsilon = range / 1000  →  n_steps ≈ 1000  →  n_bits = ceil(log2(1001)) = 10
    let n = 500usize;
    let ts: Vec<i64> = (0..n as i64).map(|i| i * 1_000_000).collect();
    let val: Vec<f64> = (0..n).map(|i| i as f64).collect(); // range = 499
    let range = 499.0f64;
    let epsilon = range / 1000.0;
    let cfg = Config { algo: Algo::Rdp, epsilon, ..Default::default() };
    let (_, stats) = compress_with_stats(&ts, &val, &cfg).unwrap();
    // n_steps = ceil(499 / 0.499) = ceil(1000) = 1000 → n_bits = ceil(log2(1001)) = 10
    assert_eq!(stats.quant_bits, 10, "Expected 10 quant bits, got {}", stats.quant_bits);
}

// ─── Interpolate ─────────────────────────────────────────────────────────────

#[test]
fn interpolate_regular_grid() {
    let ts = vec![0i64, 10_000, 20_000, 30_000];
    let val = vec![0.0f64, 10.0, 20.0, 30.0];
    let out = interpolate(&ts, &val, 0, 30_000, 5_000).unwrap();
    assert_eq!(out.len(), 7); // floor(30000/5000)+1
    for (i, &v) in out.iter().enumerate() {
        let expected = i as f64 * 5.0;
        assert!((v - expected).abs() < 1e-9, "out[{i}]={v}, expected {expected}");
    }
}

#[test]
fn interpolate_clamps_outside_range() {
    let ts = vec![10_000i64, 20_000];
    let val = vec![5.0f64, 10.0];
    // Grid starts before and ends after data range.
    let out = interpolate(&ts, &val, 0, 30_000, 10_000).unwrap();
    assert_eq!(out.len(), 4);
    // t=0 clamps to val[0]=5.0
    assert!((out[0] - 5.0).abs() < 1e-9);
    // t=30000 clamps to val[last]=10.0
    assert!((out[3] - 10.0).abs() < 1e-9);
}

// ─── helpers ─────────────────────────────────────────────────────────────────

/// For every dropped point, the reconstruction error (value deviation from the
/// linear interpolant between the two kept neighbors) must be <= epsilon.
///
/// This uses the ORIGINAL values at kept timestamps (not the decoded/quantized
/// ones), because RDP guarantees the epsilon bound against the original values.
/// Quantization adds at most epsilon/2 on top (~1.5*epsilon total, tested
/// separately via Stats.max_error).
fn check_all_dropped_within_epsilon(
    orig_ts: &[i64],
    orig_val: &[f64],
    kept_ts: &[i64],
    _kept_val_decoded: &[f64], // decoded (quantized) — not used here; see note above
    epsilon: f64,
) {
    // Build a map of kept timestamp → original value.
    let kept_map: std::collections::HashMap<i64, f64> =
        kept_ts.iter().cloned()
            .zip(
                // Extract original values at kept timestamps.
                kept_ts.iter().map(|kt| {
                    let pos = orig_ts.iter().position(|t| t == kt).expect("kept ts in orig");
                    orig_val[pos]
                })
            )
            .collect();

    let n_kept = kept_ts.len();
    let mut j = 0usize;

    // Original values at kept indices (in order).
    let kept_orig_val: Vec<f64> = kept_ts.iter()
        .map(|kt| *kept_map.get(kt).unwrap())
        .collect();

    for (&t, &v) in orig_ts.iter().zip(orig_val.iter()) {
        if kept_map.contains_key(&t) {
            continue; // kept point — skip
        }
        // Linear interpolation between kept neighbours using ORIGINAL values.
        while j + 1 < n_kept - 1 && kept_ts[j + 1] <= t {
            j += 1;
        }
        let span = (kept_ts[j + 1] - kept_ts[j]) as f64;
        let frac = (t - kept_ts[j]) as f64 / span;
        let recon = kept_orig_val[j] + frac * (kept_orig_val[j + 1] - kept_orig_val[j]);
        assert!(
            (v - recon).abs() <= epsilon + 1e-9,
            "Dropped point at t={t}: orig={v}, recon={recon}, diff={}, epsilon={epsilon}",
            (v - recon).abs()
        );
    }
}
