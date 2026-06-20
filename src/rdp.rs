/// Perpendicular distance from point P to the LINE SEGMENT AB (clamped
/// projection — NOT the infinite line). This is the correct formula for RDP:
/// using the infinite-line distance is the classic bug near endpoints.
fn point_to_segment_dist(
    px: f64, py: f64,
    ax: f64, ay: f64,
    bx: f64, by: f64,
) -> f64 {
    let dx = bx - ax;
    let dy = by - ay;
    let len_sq = dx * dx + dy * dy;
    if len_sq == 0.0 {
        // degenerate segment: A == B
        let ex = px - ax;
        let ey = py - ay;
        return (ex * ex + ey * ey).sqrt();
    }
    // t = clamp(dot(AP, AB) / |AB|^2, 0, 1)
    let t = ((px - ax) * dx + (py - ay) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);
    let qx = ax + t * dx;
    let qy = ay + t * dy;
    let ex = px - qx;
    let ey = py - qy;
    (ex * ex + ey * ey).sqrt()
}

/// Build the (x, y) working arrays from raw timestamps + values.
///
/// When `normalize_axes` is true, the time axis is scaled so that the
/// full time span maps onto `value_range` before distance computations.
/// This prevents the time axis (in nanoseconds!) from dominating.
fn make_xy(
    ts: &[i64],
    val: &[f64],
    normalize: bool,
    value_range: f64,
) -> (Vec<f64>, Vec<f64>) {
    let n = ts.len();
    let mut x: Vec<f64> = (0..n).map(|i| ts[i] as f64).collect();
    let y: Vec<f64> = val.to_vec();

    if normalize && n >= 2 {
        let t_min = x[0];
        let t_max = x[n - 1];
        let t_span = t_max - t_min;
        if t_span > 0.0 && value_range > 0.0 {
            let scale = value_range / t_span;
            for xi in &mut x {
                *xi = (*xi - t_min) * scale;
            }
        }
    }
    (x, y)
}

/// Iterative Ramer-Douglas-Peucker simplification.
///
/// Returns a boolean mask of length `n` where `true` means the point is kept.
/// The first and last points are always kept.
///
/// Uses an explicit stack instead of recursion to avoid stack overflow on
/// large inputs (e.g. 10^6-point fracture-curve data).
pub fn rdp_simplify(
    ts: &[i64],
    val: &[f64],
    epsilon: f64,
    normalize: bool,
    value_range: f64,
) -> Vec<bool> {
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
    if n == 2 {
        return kept;
    }

    let (x, y) = make_xy(ts, val, normalize, value_range);

    // Stack holds (start_idx, end_idx) ranges to process.
    let mut stack: Vec<(usize, usize)> = vec![(0, n - 1)];

    while let Some((start, end)) = stack.pop() {
        if end <= start + 1 {
            continue;
        }
        // Find the point with the maximum distance to segment [start, end].
        let mut max_dist = 0.0_f64;
        let mut max_idx = start + 1;
        for i in (start + 1)..end {
            let d = point_to_segment_dist(
                x[i], y[i],
                x[start], y[start],
                x[end], y[end],
            );
            if d > max_dist {
                max_dist = d;
                max_idx = i;
            }
        }
        if max_dist > epsilon {
            kept[max_idx] = true;
            stack.push((start, max_idx));
            stack.push((max_idx, end));
        }
    }
    kept
}

/// RDP-n: binary-search for an epsilon in `[0, search_max]` such that
/// `rdp_simplify` returns approximately `n_out` kept points.
///
/// Returns the kept mask with the fewest points that still satisfies
/// kept_count <= n_out (or all points if no such epsilon exists in range).
pub fn rdp_n_simplify(
    ts: &[i64],
    val: &[f64],
    n_out: usize,
    search_max: f64,
    normalize: bool,
    value_range: f64,
) -> Vec<bool> {
    let n = ts.len();
    // Clamp: always keep at least 2 points (endpoints).
    let n_out = n_out.max(2).min(n);

    // If we already have few enough points, keep all.
    if n <= n_out {
        return vec![true; n];
    }

    let mut lo = 0.0_f64;
    let mut hi = search_max.max(1.0); // guard against 0
    let mut best_mask = rdp_simplify(ts, val, hi, normalize, value_range);

    for _ in 0..50 {
        let mid = (lo + hi) / 2.0;
        let mask = rdp_simplify(ts, val, mid, normalize, value_range);
        let cnt = mask.iter().filter(|&&k| k).count();
        if cnt <= n_out {
            best_mask = mask;
            hi = mid;
        } else {
            lo = mid;
        }
        if (hi - lo) / hi.max(1e-15) < 1e-9 {
            break;
        }
    }
    best_mask
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_line(n: usize) -> (Vec<i64>, Vec<f64>) {
        let ts: Vec<i64> = (0..n as i64).map(|i| i * 1_000_000).collect();
        let val: Vec<f64> = (0..n).map(|i| i as f64).collect();
        (ts, val)
    }

    #[test]
    fn collinear_keeps_endpoints_only() {
        let (ts, val) = make_line(100);
        let kept = rdp_simplify(&ts, &val, 0.5, false, 1.0);
        let cnt = kept.iter().filter(|&&k| k).count();
        assert_eq!(cnt, 2);
        assert!(kept[0]);
        assert!(kept[99]);
    }

    #[test]
    fn rdp_n_returns_at_most_n_out() {
        let ts: Vec<i64> = (0..1000i64).map(|i| i * 1_000_000).collect();
        let val: Vec<f64> = ts.iter().map(|&t| (t as f64 * 0.001).sin()).collect();
        let kept = rdp_n_simplify(&ts, &val, 50, 500.0, false, 1.0);
        let cnt = kept.iter().filter(|&&k| k).count();
        assert!(cnt <= 50, "got {cnt} points, expected <= 50");
        assert!(kept[0]);
        assert!(kept[999]);
    }
}
