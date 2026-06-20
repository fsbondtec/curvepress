/// Radial-distance pre-filter: O(n) pass that drops any point whose distance
/// in the VALUE domain from the last kept point is less than `radius`.
///
/// This is a cheap noise remover before RDP. The first and last points are
/// always kept.
#[allow(dead_code)]
pub fn radial_filter(ts: &[i64], val: &[f64], radius: f64) -> Vec<bool> {
    let n = ts.len();
    let mut kept = vec![false; n];
    if n == 0 {
        return kept;
    }
    kept[0] = true;
    if n == 1 {
        return kept;
    }
    kept[n - 1] = true;

    let mut last_kept_val = val[0];
    for i in 1..(n - 1) {
        if (val[i] - last_kept_val).abs() >= radius {
            kept[i] = true;
            last_kept_val = val[i];
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_noise_below_radius() {
        // All values within 0.1 of 0.0 except one spike at index 5.
        let ts: Vec<i64> = (0..10i64).map(|i| i * 1_000_000).collect();
        let mut val = vec![0.0f64; 10];
        val[5] = 1.0;
        let kept = radial_filter(&ts, &val, 0.5);
        assert!(kept[0]);
        assert!(kept[5]);
        assert!(kept[9]);
        // Points before the spike that are within 0.5 of 0.0 should be dropped.
        assert!(!kept[1]);
        assert!(!kept[2]);
    }
}
