#pragma once
#include <algorithm>
#include <cassert>
#include <cmath>
#include <cstdint>
#include <bit>
#include <limits>
#include <queue>
#include <vector>

namespace curvepress::detail {

namespace vw_impl {

inline double triangle_area(
    const std::vector<double>& tx,
    const std::vector<double>& vy,
    std::size_t a, std::size_t i, std::size_t b) noexcept
{
    return 0.5 * std::abs(
        (tx[i] - tx[a]) * (vy[b] - vy[a]) -
        (tx[b] - tx[a]) * (vy[i] - vy[a]));
}

// Min-heap entry stored in a max-heap via negated IEEE bit pattern.
struct Entry {
    uint64_t    area_bits{}; // ~area.to_bits() → larger = smaller area → max-heap == min-heap
    std::size_t idx{};
    uint64_t    gen{};

    bool operator<(const Entry& o) const noexcept { return area_bits < o.area_bits; }
};

// For positive-finite f64, IEEE bit patterns are monotone.
// Invert them so that a max-heap gives min-area first.
// Infinity (endpoints) maps to lowest priority.
inline uint64_t neg_bits(double area) noexcept {
    return std::numeric_limits<uint64_t>::max() - std::bit_cast<uint64_t>(area);
}

} // namespace vw_impl

/// Visvalingam-Whyatt simplification (effective-area variant, Visvalingam 2016).
/// Keeps exactly n_out points (clamped to [2, n]).
inline std::vector<bool> vw_simplify(
    const std::vector<int64_t>& ts,
    const std::vector<double>&  val,
    std::size_t                 n_out,
    bool                        normalize,
    double                      value_range)
{
    std::size_t n = ts.size();
    n_out = std::clamp(n_out, std::size_t{2}, n);
    if (n <= n_out) return std::vector<bool>(n, true);

    // Build working arrays. Subtract ts[0] to avoid float precision loss.
    double t0 = static_cast<double>(ts[0]);
    std::vector<double> tx(n), vy(val.begin(), val.end());
    for (std::size_t i = 0; i < n; ++i) tx[i] = static_cast<double>(ts[i]) - t0;

    if (normalize && n >= 2) {
        double t_span = tx[n - 1];
        double vr     = value_range > 0.0 ? value_range : 1.0;
        if (t_span > 0.0) {
            double scale = vr / t_span;
            for (double& xi : tx) xi *= scale;
        }
    }

    // Doubly-linked list over active indices.
    std::vector<std::size_t> prev(n), next(n);
    for (std::size_t i = 0; i < n; ++i) { prev[i] = i == 0 ? 0 : i - 1; next[i] = i == n - 1 ? n - 1 : i + 1; }

    std::vector<uint64_t> gens(n, 0);
    std::vector<double>   areas(n, std::numeric_limits<double>::infinity());

    using namespace vw_impl;

    // Initialise areas for interior points.
    for (std::size_t i = 1; i < n - 1; ++i)
        areas[i] = triangle_area(tx, vy, prev[i], i, next[i]);

    std::priority_queue<Entry> heap;
    for (std::size_t i = 1; i < n - 1; ++i)
        heap.push({ neg_bits(areas[i]), i, 0u });

    std::size_t remaining        = n;
    double      last_removed_area = 0.0;

    while (remaining > n_out) {
        // Pop smallest effective area, skipping stale entries.
        Entry entry{};
        bool found = false;
        while (!heap.empty()) {
            entry = heap.top(); heap.pop();
            if (entry.gen == gens[entry.idx]) { found = true; break; }
        }
        if (!found) break;

        std::size_t i = entry.idx;
        if (i == 0 || i == n - 1) continue; // endpoints protected

        last_removed_area = std::max(last_removed_area, areas[i]);
        areas[i] = std::numeric_limits<double>::infinity(); // mark removed

        // Splice out of linked list.
        std::size_t p  = prev[i];
        std::size_t nx = next[i];
        next[p]  = nx;
        prev[nx] = p;
        --remaining;

        // Recompute effective areas of neighbours.
        if (p != 0) {
            double a = std::max(triangle_area(tx, vy, prev[p], p, nx), last_removed_area);
            areas[p] = a;
            ++gens[p];
            heap.push({ neg_bits(a), p, gens[p] });
        }
        if (nx != n - 1) {
            double a = std::max(triangle_area(tx, vy, p, nx, next[nx]), last_removed_area);
            areas[nx] = a;
            ++gens[nx];
            heap.push({ neg_bits(a), nx, gens[nx] });
        }
    }

    // Build mask: endpoints always kept; interior kept if area is finite (not removed).
    std::vector<bool> kept(n, false);
    kept[0] = kept[n - 1] = true;
    for (std::size_t i = 1; i < n - 1; ++i)
        if (std::isfinite(areas[i])) kept[i] = true;
    return kept;
}

} // namespace curvepress::detail
