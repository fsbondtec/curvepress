#pragma once
#include <algorithm>
#include <cassert>
#include <cmath>
#include <cstdint>
#include <limits>
#include <vector>

namespace curvepress::detail {

struct Quantized {
    double         val_min{};
    double         val_range{}; ///< val_max - val_min (0 for constant series)
    uint32_t       n_bits{1};   ///< [1, 32]
    std::vector<uint32_t> codes;
};

/// Quantize `values` using at most `epsilon` absolute error per step.
///
/// Resolution:
///   n_steps = ceil(range / epsilon)
///   n_bits  = clamp(ceil(log2(n_steps + 1)), 1, 32)
///   scale   = ((1 << n_bits) - 1) / range
///   q[i]    = round((val[i] - val_min) * scale)
///
/// Constant series (range <= 0): n_bits=1, all codes=0, reconstructs as val_min.
inline Quantized quantize(const std::vector<double>& values, double epsilon) {
    double val_min =  std::numeric_limits<double>::infinity();
    double val_max = -std::numeric_limits<double>::infinity();
    for (double v : values) {
        if (v < val_min) val_min = v;
        if (v > val_max) val_max = v;
    }
    double val_range = val_max - val_min;

    if (val_range <= 0.0) {
        Quantized q;
        q.val_min   = val_min;
        q.val_range = 0.0;
        q.n_bits    = 1;
        q.codes.assign(values.size(), 0u);
        return q;
    }

    double   n_steps  = std::ceil(val_range / epsilon);
    uint32_t n_bits   = static_cast<uint32_t>(std::clamp(
                            static_cast<int>(std::ceil(std::log2(n_steps + 1.0))), 1, 32));
    double   max_code = static_cast<double>((uint64_t{1} << n_bits) - 1u);
    double   scale    = max_code / val_range;

    Quantized q;
    q.val_min   = val_min;
    q.val_range = val_range;
    q.n_bits    = n_bits;
    q.codes.reserve(values.size());
    for (double v : values)
        q.codes.push_back(static_cast<uint32_t>(std::round((v - val_min) * scale)));

    return q;
}

/// Reconstruct f64 values from quantized codes.
/// Returns val_min for all points when val_range <= 0 (constant series).
inline std::vector<double> dequantize(
    const std::vector<uint32_t>& codes,
    double val_min, double val_range, uint32_t n_bits)
{
    if (val_range <= 0.0 || n_bits == 0) {
        return std::vector<double>(codes.size(), val_min);
    }
    double max_code = static_cast<double>((uint64_t{1} << n_bits) - 1u);
    double scale    = max_code / val_range;
    std::vector<double> out;
    out.reserve(codes.size());
    for (uint32_t q : codes)
        out.push_back(val_min + static_cast<double>(q) / scale);
    return out;
}

} // namespace curvepress::detail
