use std::collections::BinaryHeap;
use std::cmp::Ordering;

/// Compute triangle area for three consecutive kept points a, i, b.
fn triangle_area(tx: &[f64], vy: &[f64], a: usize, i: usize, b: usize) -> f64 {
    0.5 * ((tx[i] - tx[a]) * (vy[b] - vy[a]) - (tx[b] - tx[a]) * (vy[i] - vy[a])).abs()
}

/// Visvalingam-Whyatt simplification with the effective-area variant
/// (Visvalingam 2016): when removing a point, clamp each remaining neighbor's
/// recomputed area up to the area of the removed point to prevent out-of-order
/// removal artifacts.
///
/// Keeps exactly `n_out` points (clamped to `[2, n]`).
pub fn vw_simplify(
    ts: &[i64],
    val: &[f64],
    n_out: usize,
    normalize: bool,
    value_range: f64,
) -> Vec<bool> {
    let n = ts.len();
    let n_out = n_out.max(2).min(n);

    if n <= n_out {
        return vec![true; n];
    }

    // Build working arrays. Subtract ts[0] before casting to avoid precision loss.
    let t0 = ts[0] as f64;
    let mut tx: Vec<f64> = ts.iter().map(|&t| t as f64 - t0).collect();
    let vy: Vec<f64> = val.to_vec();

    if normalize && n >= 2 {
        let t_span = tx[n - 1];
        let vr = if value_range > 0.0 { value_range } else { 1.0 };
        if t_span > 0.0 {
            let scale = vr / t_span;
            for xi in &mut tx {
                *xi *= scale;
            }
        }
    }

    // Doubly-linked list over active indices.
    let mut prev: Vec<usize> = (0..n).map(|i| if i == 0 { 0 } else { i - 1 }).collect();
    let mut next: Vec<usize> = (0..n).map(|i| if i == n - 1 { n - 1 } else { i + 1 }).collect();
    // Generation counter per point: bump when area changes so stale heap entries are skipped.
    let mut gens: Vec<u64> = vec![0; n];
    let mut areas: Vec<f64> = vec![f64::INFINITY; n];

    // Use ordered_float for heap keys — requires the crate; avoid it by storing
    // bits instead. We'll use a manual workaround without the crate.
    // Re-implement Entry as an f64 wrapper using total_cmp.

    // Initialize areas for interior points.
    for i in 1..(n - 1) {
        areas[i] = triangle_area(&tx, &vy, prev[i], i, next[i]);
    }

    // Min-heap via a Vec + manual sort — but that's O(n²). Use std BinaryHeap
    // with negated bits instead of ordered_float.
    struct MinHeapEntry {
        area_bits: u64, // f64::to_bits of the NEGATED area so that smaller area floats first
        idx: usize,
        gen: u64,
    }
    impl PartialEq for MinHeapEntry {
        fn eq(&self, other: &Self) -> bool { self.area_bits == other.area_bits }
    }
    impl Eq for MinHeapEntry {}
    impl PartialOrd for MinHeapEntry {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
    }
    impl Ord for MinHeapEntry {
        // We store negated area as bits using f64's IEEE bit ordering:
        // larger bits == more-negative == smaller original area → max-heap gives min.
        fn cmp(&self, other: &Self) -> Ordering {
            self.area_bits.cmp(&other.area_bits)
        }
    }

    fn neg_bits(area: f64) -> u64 {
        // For positive finite floats, IEEE bit patterns are totally ordered.
        // Smaller area → we want HIGHER priority in the max-heap.
        // Invert by subtracting from u64::MAX.
        // INFINITY (endpoints, never removed) → lowest priority (comes last).
        u64::MAX - area.to_bits()
    }

    let mut heap: BinaryHeap<MinHeapEntry> = BinaryHeap::with_capacity(n);
    for (i, &area) in areas.iter().enumerate().take(n - 1).skip(1) {
        heap.push(MinHeapEntry { area_bits: neg_bits(area), idx: i, gen: 0 });
    }

    let mut remaining = n;
    let mut last_removed_area = 0.0_f64;

    while remaining > n_out {
        // Pop the point with the smallest effective area.
        let entry = loop {
            match heap.pop() {
                None => break None,
                Some(e) if e.gen != gens[e.idx] => continue, // stale
                Some(e) => break Some(e),
            }
        };
        let entry = match entry { Some(e) => e, None => break };

        let i = entry.idx;
        if i == 0 || i == n - 1 {
            continue; // endpoints are protected
        }

        // Track effective area: cannot be less than the last removed area.
        last_removed_area = last_removed_area.max(areas[i]);
        areas[i] = f64::INFINITY; // mark as removed

        // Splice out of linked list.
        let p = prev[i];
        let nx = next[i];
        next[p] = nx;
        prev[nx] = p;
        remaining -= 1;

        // Recompute neighbors' effective areas.
        if p != 0 {
            let pp = prev[p];
            let new_area = triangle_area(&tx, &vy, pp, p, nx).max(last_removed_area);
            areas[p] = new_area;
            gens[p] += 1;
            heap.push(MinHeapEntry { area_bits: neg_bits(new_area), idx: p, gen: gens[p] });
        }
        if nx != n - 1 {
            let nnx = next[nx];
            let new_area = triangle_area(&tx, &vy, p, nx, nnx).max(last_removed_area);
            areas[nx] = new_area;
            gens[nx] += 1;
            heap.push(MinHeapEntry { area_bits: neg_bits(new_area), idx: nx, gen: gens[nx] });
        }
    }

    // Build mask: endpoints are always kept; interior points are kept if their
    // area is STILL FINITE (never removed). Removed points have areas[i] == INFINITY.
    let mut kept = vec![false; n];
    kept[0] = true;
    kept[n - 1] = true;
    for i in 1..(n - 1) {
        if areas[i] != f64::INFINITY {
            kept[i] = true;
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_exact_n_out() {
        let ts: Vec<i64> = (0..200i64).map(|i| i * 1_000_000).collect();
        let val: Vec<f64> = ts.iter().map(|&t| (t as f64 * 5e-9).sin() * 100.0).collect();
        let kept = vw_simplify(&ts, &val, 30, false, 1.0);
        let cnt = kept.iter().filter(|&&k| k).count();
        assert_eq!(cnt, 30, "VW should return exactly n_out={}, got {}", 30, cnt);
    }

    #[test]
    fn keeps_all_when_n_out_ge_n() {
        let ts: Vec<i64> = (0..10i64).map(|i| i * 1_000_000).collect();
        let val: Vec<f64> = (0..10).map(|i| i as f64).collect();
        let kept = vw_simplify(&ts, &val, 100, false, 1.0);
        assert!(kept.iter().all(|&k| k));
    }
}
