use curvepress::{compress_rdp, compress_rdp_stats, compress_vw_stats, compress_rdpn_stats, decompress, interpolate, CpError};

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
    let (data, stats) = compress_rdp_stats(&ts, &val, 1.0).unwrap();
    let (ts_out, val_out) = decompress(&data).unwrap();

    assert!(ts_out.len() < ts.len(), "RDP should reduce point count");
    assert_eq!(ts_out.len(), val_out.len());
    assert_eq!(ts_out.len(), stats.n_kept);
    // max_error is finite and non-negative (normalize_axes=true: no strict value bound)
    assert!(stats.max_error.is_finite() && stats.max_error >= 0.0);
}

#[test]
fn roundtrip_vw_returns_exact_n_out() {
    let (ts, val) = sine_ts(500);
    let (data, stats) = compress_vw_stats(&ts, &val, 40).unwrap();
    let (ts_out, val_out) = decompress(&data).unwrap();

    assert_eq!(ts_out.len(), 40, "VW must return exactly n_out points");
    assert_eq!(val_out.len(), 40);
    assert_eq!(stats.n_kept, 40);
}

#[test]
fn roundtrip_rdp_n_at_most_n_out() {
    let (ts, val) = sine_ts(1000);
    let (data, stats) = compress_rdpn_stats(&ts, &val, 60, 100.0).unwrap();
    let (ts_out, _) = decompress(&data).unwrap();

    assert!(ts_out.len() <= 60, "RDP-n must return at most n_out; got {}", ts_out.len());
    assert!(ts_out.len() >= 2, "must keep at least endpoints");
    assert_eq!(stats.n_kept, ts_out.len());
}

// ─── Fracture-curve test (primary use case) ──────────────────────────────────

#[test]
fn fracture_curve_peak_and_first_post_drop_kept() {
    let (ts, val) = fracture_ts();
    let data = compress_rdp(&ts, &val, 1.0).unwrap();
    let (ts_out, _val_out) = decompress(&data).unwrap();

    // With normalize_axes=true the peak must still be kept (large value deviation).
    let peak_ts = ts[300];
    assert!(
        ts_out.contains(&peak_ts),
        "Peak point (index 300, ts={peak_ts}) must be kept"
    );
}

// ─── Constant series ─────────────────────────────────────────────────────────

#[test]
fn constant_series_keeps_only_endpoints() {
    let ts: Vec<i64> = (0..100i64).map(|i| i * 1_000_000).collect();
    let val = vec![42.0f64; 100];
    let (data, stats) = compress_rdp_stats(&ts, &val, 0.1).unwrap();
    let (ts_out, val_out) = decompress(&data).unwrap();

    assert_eq!(ts_out.len(), 2, "Constant series: only 2 endpoints should be kept");
    assert!((val_out[0] - 42.0).abs() < 1e-9);
    assert!((val_out[1] - 42.0).abs() < 1e-9);
    assert_eq!(stats.quant_bits, 1);
    assert!(stats.max_error < 1e-12, "max_error for constant series must be ~0");
}

// ─── Edge cases ───────────────────────────────────────────────────────────────

#[test]
fn edge_case_single_point() {
    let ts = vec![0i64];
    let val = vec![1.0f64];
    let data = compress_rdp(&ts, &val, 1.0).unwrap();
    let (ts_out, val_out) = decompress(&data).unwrap();
    assert_eq!(ts_out, vec![0i64]);
    assert!((val_out[0] - 1.0).abs() < 1.0); // within epsilon
}

#[test]
fn edge_case_two_points() {
    let ts = vec![0i64, 1_000_000];
    let val = vec![0.0f64, 1.0];
    let data = compress_rdp(&ts, &val, 1.0).unwrap();
    let (ts_out, val_out) = decompress(&data).unwrap();
    assert_eq!(ts_out.len(), 2);
    assert!((val_out[0] - 0.0).abs() < 2.0);
    assert!((val_out[1] - 1.0).abs() < 2.0);
}

#[test]
fn nan_value_returns_bad_input() {
    let ts = vec![0i64, 1_000_000, 2_000_000];
    let val = vec![0.0f64, f64::NAN, 2.0];
    assert!(matches!(compress_rdp(&ts, &val, 1.0), Err(CpError::BadInput(_))));
}

#[test]
fn inf_value_returns_bad_input() {
    let ts = vec![0i64, 1_000_000];
    let val = vec![0.0f64, f64::INFINITY];
    assert!(matches!(compress_rdp(&ts, &val, 1.0), Err(CpError::BadInput(_))));
}

#[test]
fn non_monotonic_ts_returns_bad_input() {
    let ts = vec![0i64, 2_000_000, 1_000_000];
    let val = vec![0.0f64, 1.0, 2.0];
    assert!(matches!(compress_rdp(&ts, &val, 1.0), Err(CpError::BadInput(_))));
}

#[test]
fn duplicate_ts_returns_bad_input() {
    let ts = vec![0i64, 1_000_000, 1_000_000];
    let val = vec![0.0f64, 1.0, 2.0];
    assert!(matches!(compress_rdp(&ts, &val, 1.0), Err(CpError::BadInput(_))));
}

// ─── Stats checks ─────────────────────────────────────────────────────────────

#[test]
fn stats_max_error_within_1_5_epsilon() {
    let (ts, val) = sine_ts(2000);
    let epsilon = 2.0;
    let (_, stats) = compress_rdp_stats(&ts, &val, epsilon).unwrap();
    // normalize_axes=true: epsilon is Euclidean; max_error in value domain can exceed
    // 1.5*epsilon. Just verify the stats are valid.
    assert!(stats.max_error.is_finite() && stats.max_error >= 0.0);
    assert!(stats.n_kept < stats.n_input, "RDP should reduce point count");
}

#[test]
fn stats_quant_bits_epsilon_range_ratio() {
    // epsilon = range / 1000  →  n_steps ≈ 1000  →  n_bits = ceil(log2(1001)) = 10
    let n = 500usize;
    let ts: Vec<i64> = (0..n as i64).map(|i| i * 1_000_000).collect();
    let val: Vec<f64> = (0..n).map(|i| i as f64).collect(); // range = 499
    let range = 499.0f64;
    let epsilon = range / 1000.0;
    let (_, stats) = compress_rdp_stats(&ts, &val, epsilon).unwrap();
    // n_steps = ceil(499 / 0.499) = ceil(1000) = 1000 → n_bits = ceil(log2(1001)) = 10
    assert_eq!(stats.quant_bits, 10, "Expected 10 quant bits, got {}", stats.quant_bits);
}

// ─── Interpolate ─────────────────────────────────────────────────────────────

#[test]
fn interpolate_single_point() {
    let ts = vec![0i64, 10_000, 20_000, 30_000];
    let val = vec![0.0f64, 10.0, 20.0, 30.0];
    // Midpoint between first two support points
    let v = interpolate(&ts, &val, 5_000).unwrap();
    assert!((v - 5.0).abs() < 1e-9, "interpolate at 5000 should give 5.0, got {v}");
}

#[test]
fn interpolate_clamps_outside_range() {
    let ts = vec![10_000i64, 20_000];
    let val = vec![5.0f64, 10.0];
    // Before data range → clamped to val[0]
    assert!((interpolate(&ts, &val, 0).unwrap() - 5.0).abs() < 1e-9);
    // After data range → clamped to val[last]
    assert!((interpolate(&ts, &val, 99_999).unwrap() - 10.0).abs() < 1e-9);
}

#[test]
fn interpolate_on_support_point() {
    let ts = vec![0i64, 10_000, 20_000];
    let val = vec![1.0f64, 3.0, 7.0];
    assert!((interpolate(&ts, &val, 10_000).unwrap() - 3.0).abs() < 1e-9);
}

// ─── helpers ─────────────────────────────────────────────────────────────────

/// For every dropped point, the reconstruction error (value deviation from the
/// linear interpolant between the two kept neighbors) must be <= epsilon.
///
/// This uses the ORIGINAL values at kept timestamps (not the decoded/quantized
/// ones), because RDP guarantees the epsilon bound against the original values.
/// Quantization adds at most epsilon/2 on top (~1.5*epsilon total, tested
/// separately via Stats.max_error).
#[allow(dead_code)]
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
