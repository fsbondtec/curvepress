#pragma once
#include <algorithm>
#include <cmath>
#include <cstdint>
#include <stack>
#include <vector>

namespace curvepress::detail {

/// Perpendicular distance from point P to the LINE SEGMENT AB (clamped
/// projection – NOT the infinite line). Correct formula for RDP.
inline double point_to_segment_dist(
    double px, double py,
    double ax, double ay,
    double bx, double by) noexcept
{
    double dx = bx - ax;
    double dy = by - ay;
    double len_sq = dx * dx + dy * dy;
    if (len_sq == 0.0) {
        double ex = px - ax;
        double ey = py - ay;
        return std::sqrt(ex * ex + ey * ey);
    }
    double t = ((px - ax) * dx + (py - ay) * dy) / len_sq;
    t = std::clamp(t, 0.0, 1.0);
    double qx = ax + t * dx;
    double qy = ay + t * dy;
    double ex = px - qx;
    double ey = py - qy;
    return std::sqrt(ex * ex + ey * ey);
}

/// Build normalised x/y working arrays.
/// When normalize=true the time axis is scaled so that the full span maps
/// to value_range, preventing nanosecond timestamps from dominating.
inline void make_xy(
    const std::vector<int64_t>& ts,
    const std::vector<double>&  val,
    bool                        normalize,
    double                      value_range,
    std::vector<double>&        x,
    std::vector<double>&        y)
{
    std::size_t n = ts.size();
    x.resize(n);
    y.assign(val.begin(), val.end());
    for (std::size_t i = 0; i < n; ++i)
        x[i] = static_cast<double>(ts[i]);

    if (normalize && n >= 2) {
        double t_min  = x[0];
        double t_max  = x[n - 1];
        double t_span = t_max - t_min;
        if (t_span > 0.0 && value_range > 0.0) {
            double scale = value_range / t_span;
            for (double& xi : x) xi = (xi - t_min) * scale;
        }
    }
}

/// Iterative Ramer-Douglas-Peucker simplification.
/// Returns a boolean mask of length n (true = keep). Endpoints always kept.
/// Uses an explicit stack to avoid stack overflow on large inputs.
inline std::vector<bool> rdp_simplify(
    const std::vector<int64_t>& ts,
    const std::vector<double>&  val,
    double                      epsilon,
    bool                        normalize,
    double                      value_range)
{
    std::size_t n = ts.size();
    std::vector<bool> kept(n, false);
    if (n == 0) return kept;
    kept[0] = true;
    if (n == 1) return kept;
    kept[n - 1] = true;
    if (n == 2) return kept;

    std::vector<double> x, y;
    make_xy(ts, val, normalize, value_range, x, y);

    std::stack<std::pair<std::size_t, std::size_t>> stk;
    stk.emplace(0, n - 1);

    while (!stk.empty()) {
        auto [start, end] = stk.top(); stk.pop();
        if (end <= start + 1) continue;

        double      max_dist = 0.0;
        std::size_t max_idx  = start + 1;
        for (std::size_t i = start + 1; i < end; ++i) {
            double d = point_to_segment_dist(
                x[i], y[i],
                x[start], y[start],
                x[end],   y[end]);
            if (d > max_dist) { max_dist = d; max_idx = i; }
        }
        if (max_dist > epsilon) {
            kept[max_idx] = true;
            stk.emplace(start, max_idx);
            stk.emplace(max_idx, end);
        }
    }
    return kept;
}

/// RDP-N: binary-search for an epsilon in [0, search_max] so that
/// rdp_simplify returns at most n_out kept points.
inline std::vector<bool> rdp_n_simplify(
    const std::vector<int64_t>& ts,
    const std::vector<double>&  val,
    std::size_t                 n_out,
    double                      search_max,
    bool                        normalize,
    double                      value_range)
{
    std::size_t n = ts.size();
    n_out = std::clamp(n_out, std::size_t{2}, n);
    if (n <= n_out) return std::vector<bool>(n, true);

    double lo = 0.0;
    double hi = std::max(search_max, 1.0);
    auto best_mask = rdp_simplify(ts, val, hi, normalize, value_range);

    for (int iter = 0; iter < 50; ++iter) {
        double mid  = (lo + hi) / 2.0;
        auto   mask = rdp_simplify(ts, val, mid, normalize, value_range);
        std::size_t cnt = 0;
        for (bool k : mask) cnt += k ? 1u : 0u;
        if (cnt <= n_out) {
            best_mask = mask;
            hi = mid;
        } else {
            lo = mid;
        }
        if (hi > 0.0 && (hi - lo) / hi < 1e-9) break;
    }
    return best_mask;
}

} // namespace curvepress::detail
