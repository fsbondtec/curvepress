/// Output of the quantization stage.
pub struct Quantized {
    pub val_min: f64,
    pub val_range: f64, // val_max - val_min (0.0 for constant series)
    pub n_bits: u32,    // [1, 32]
    pub codes: Vec<u32>,
}

/// Quantize `values` (the kept subset, already filtered by the simplification
/// stage) using at most `epsilon` absolute error per step.
///
/// # Resolution
/// ```text
/// n_steps = (range / epsilon).ceil()
/// n_bits  = max(1, min(32, ceil(log2(n_steps + 1))))
/// scale   = ((1 << n_bits) - 1) as f64 / range   [0 when range == 0]
/// q[i]    = round((val[i] - val_min) * scale) as u32
/// ```
///
/// Constant series (range <= 0): quant_bits = 1, all codes = 0, reconstruct
/// as val_min. No division by range.
pub fn quantize(values: &[f64], epsilon: f64) -> Quantized {
    let val_min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let val_max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let val_range = val_max - val_min;

    if val_range <= 0.0 {
        return Quantized {
            val_min,
            val_range: 0.0,
            n_bits: 1,
            codes: vec![0u32; values.len()],
        };
    }

    let n_steps = (val_range / epsilon).ceil();
    let n_bits = ((n_steps + 1.0).log2().ceil() as u32).clamp(1, 32);
    let max_code = ((1u64 << n_bits) - 1) as f64;
    let scale = max_code / val_range;

    let codes: Vec<u32> = values
        .iter()
        .map(|&v| ((v - val_min) * scale).round() as u32)
        .collect();

    Quantized { val_min, val_range, n_bits, codes }
}

/// Reconstruct f64 values from quantized codes.
///
/// Returns `val_min` for all points when `scale == 0` (constant series).
pub fn dequantize(codes: &[u32], val_min: f64, val_range: f64, n_bits: u32) -> Vec<f64> {
    if val_range <= 0.0 || n_bits == 0 {
        return vec![val_min; codes.len()];
    }
    let max_code = ((1u64 << n_bits) - 1) as f64;
    let scale = max_code / val_range;
    codes.iter().map(|&q| val_min + q as f64 / scale).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_roundtrip() {
        let vals = vec![0.0, 25.0, 50.0, 75.0, 100.0];
        let eps = 0.5;
        let q = quantize(&vals, eps);
        let recon = dequantize(&q.codes, q.val_min, q.val_range, q.n_bits);
        for (orig, rec) in vals.iter().zip(recon.iter()) {
            assert!((orig - rec).abs() <= eps, "{orig} vs {rec}");
        }
    }

    #[test]
    fn constant_series() {
        let vals = vec![42.0; 10];
        let q = quantize(&vals, 1.0);
        assert_eq!(q.n_bits, 1);
        assert!(q.codes.iter().all(|&c| c == 0));
        let recon = dequantize(&q.codes, q.val_min, q.val_range, q.n_bits);
        assert!(recon.iter().all(|&v| (v - 42.0).abs() < 1e-12));
    }

    #[test]
    fn n_bits_clamped_to_32() {
        // huge range, tiny epsilon → would overflow without clamp
        let vals = vec![0.0, 1e15];
        let q = quantize(&vals, 1e-10);
        assert!(q.n_bits <= 32);
    }
}
