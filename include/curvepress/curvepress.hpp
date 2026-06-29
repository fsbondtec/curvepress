#pragma once
/// curvepress – lossy time-series compression (C++20 header-only library)
///
/// Algorithms: RDP, Visvalingam-Whyatt, RDP-N (binary-searched epsilon)
/// + quantization + LEB-128/zigzag varint encoding.
///
/// CMake target: curvepress::curvepress
/// Include path: #include <curvepress/curvepress.hpp>
///
/// All functions throw on error (std::invalid_argument / std::runtime_error).
/// No global state. Thread-safe (each call is independent).

#include <cstdint>
#include <span>
#include <vector>

#include "detail/codec.hpp"
#include "detail/radial.hpp"

namespace curvepress {

// ── Public types ──────────────────────────────────────────────────────────────

/// Compression statistics returned by compress_*_stats() overloads.
struct Stats {
    std::size_t n_input{};
    std::size_t n_kept{};
    std::size_t bytes_raw{};
    std::size_t bytes_compressed{};
    double      ratio{};
    double      max_error{};
    int         quant_bits{};
};

/// Decompressed output: parallel arrays of timestamps and values.
struct Decoded {
    std::vector<int64_t> timestamps_ns;
    std::vector<double>  values;
};

// ── compress_rdp ─────────────────────────────────────────────────────────────

/// Compress with Ramer-Douglas-Peucker.
/// `epsilon` is the maximum absolute error in the value domain.
///
/// @throws std::invalid_argument  on bad input (NaN, non-monotonic ts, …)
/// @throws std::runtime_error     on internal error
inline std::vector<uint8_t> compress_rdp(
    std::span<const int64_t> timestamps_ns,
    std::span<const double>  values,
    double                   epsilon)
{
    return detail::compress_impl(
        { timestamps_ns.begin(), timestamps_ns.end() },
        { values.begin(),        values.end()        },
        detail::Algo::Rdp, epsilon, 0, nullptr);
}

/// compress_rdp with statistics output.
inline std::vector<uint8_t> compress_rdp(
    std::span<const int64_t> timestamps_ns,
    std::span<const double>  values,
    double                   epsilon,
    Stats&                   stats)
{
    detail::CompressStats cs{};
    auto out = detail::compress_impl(
        { timestamps_ns.begin(), timestamps_ns.end() },
        { values.begin(),        values.end()        },
        detail::Algo::Rdp, epsilon, 0, &cs);
    stats = { cs.n_input, cs.n_kept, cs.bytes_raw, cs.bytes_compressed,
              cs.ratio, cs.max_error, static_cast<int>(cs.quant_bits) };
    return out;
}

// ── compress_vw ──────────────────────────────────────────────────────────────

/// Compress with Visvalingam-Whyatt.
/// `n_out` is the exact number of kept points (clamped to [2, n]).
inline std::vector<uint8_t> compress_vw(
    std::span<const int64_t> timestamps_ns,
    std::span<const double>  values,
    std::size_t              n_out)
{
    return detail::compress_impl(
        { timestamps_ns.begin(), timestamps_ns.end() },
        { values.begin(),        values.end()        },
        detail::Algo::Vw, 0.0, n_out, nullptr);
}

/// compress_vw with statistics output.
inline std::vector<uint8_t> compress_vw(
    std::span<const int64_t> timestamps_ns,
    std::span<const double>  values,
    std::size_t              n_out,
    Stats&                   stats)
{
    detail::CompressStats cs{};
    auto out = detail::compress_impl(
        { timestamps_ns.begin(), timestamps_ns.end() },
        { values.begin(),        values.end()        },
        detail::Algo::Vw, 0.0, n_out, &cs);
    stats = { cs.n_input, cs.n_kept, cs.bytes_raw, cs.bytes_compressed,
              cs.ratio, cs.max_error, static_cast<int>(cs.quant_bits) };
    return out;
}

// ── compress_rdpn ────────────────────────────────────────────────────────────

/// Compress with RDP-N: binary-searches epsilon ∈ [0, epsilon] to yield
/// at most `n_out` kept points.
/// `epsilon` is the upper bound of the binary search range.
inline std::vector<uint8_t> compress_rdpn(
    std::span<const int64_t> timestamps_ns,
    std::span<const double>  values,
    std::size_t              n_out,
    double                   epsilon)
{
    return detail::compress_impl(
        { timestamps_ns.begin(), timestamps_ns.end() },
        { values.begin(),        values.end()        },
        detail::Algo::RdpN, epsilon, n_out, nullptr);
}

/// compress_rdpn with statistics output.
inline std::vector<uint8_t> compress_rdpn(
    std::span<const int64_t> timestamps_ns,
    std::span<const double>  values,
    std::size_t              n_out,
    double                   epsilon,
    Stats&                   stats)
{
    detail::CompressStats cs{};
    auto out = detail::compress_impl(
        { timestamps_ns.begin(), timestamps_ns.end() },
        { values.begin(),        values.end()        },
        detail::Algo::RdpN, epsilon, n_out, &cs);
    stats = { cs.n_input, cs.n_kept, cs.bytes_raw, cs.bytes_compressed,
              cs.ratio, cs.max_error, static_cast<int>(cs.quant_bits) };
    return out;
}

// ── decompress ───────────────────────────────────────────────────────────────

/// Decompress a byte stream produced by any compress_*() function.
///
/// @throws std::runtime_error on corrupt or unsupported data.
inline Decoded decompress(std::span<const uint8_t> data)
{
    auto [ts, val] = detail::decompress_impl(data.data(), data.size());
    return { std::move(ts), std::move(val) };
}

// ── interpolate ──────────────────────────────────────────────────────────────

/// Linear interpolation at query timestamp `t` from support points.
/// Points outside [ts[0], ts[last]] are clamped (flat extrapolation).
///
/// @throws std::invalid_argument on empty input.
inline double interpolate(
    std::span<const int64_t> ts,
    std::span<const double>  val,
    int64_t                  t)
{
    return detail::interpolate_impl(ts.data(), val.data(), ts.size(), t);
}

// ── radial_filter ────────────────────────────────────────────────────────────

/// O(n) radial-distance pre-filter: drops any point whose value deviation
/// from the last kept point is less than `radius`.
/// Returns a boolean mask (true = keep). First and last points always kept.
inline std::vector<bool> radial_filter(
    std::span<const int64_t> timestamps_ns,
    std::span<const double>  values,
    double                   radius)
{
    return detail::radial_filter(
        { timestamps_ns.begin(), timestamps_ns.end() },
        { values.begin(),        values.end()        },
        radius);
}

// ── version ───────────────────────────────────────────────────────────────────

inline const char* version() noexcept { return "0.2.0"; }

} // namespace curvepress
