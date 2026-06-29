#pragma once
#include <cmath>
#include <cstdint>
#include <vector>

namespace curvepress::detail {

/// Radial-distance pre-filter: O(n) pass that drops any point whose distance
/// in the VALUE domain from the last kept point is less than radius.
///
/// First and last points are always kept.
inline std::vector<bool> radial_filter(
    const std::vector<int64_t>& /*ts*/,
    const std::vector<double>&  val,
    double                      radius)
{
    std::size_t n = val.size();
    std::vector<bool> kept(n, false);
    if (n == 0) return kept;
    kept[0] = true;
    if (n == 1) return kept;
    kept[n - 1] = true;

    double last_kept_val = val[0];
    for (std::size_t i = 1; i < n - 1; ++i) {
        if (std::abs(val[i] - last_kept_val) >= radius) {
            kept[i]       = true;
            last_kept_val = val[i];
        }
    }
    return kept;
}

} // namespace curvepress::detail
