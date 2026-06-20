#pragma once
/// curvepress C++20 wrapper - idiomatic RAII/exception interface over the
/// auto-generated `curvepress.h` (cbindgen output).
///
/// The consumer sees ONLY this header. The raw C API is an implementation
/// detail; never call `cp_*` functions directly.
///
/// Link: add the Rust static library built by `cpp/CMakeLists.txt`.
/// CMake target: `curvepress::curvepress`

#include <curvepress.h>   // cbindgen-generated, from include/curvepress.h
#include <cstdint>
#include <span>
#include <stdexcept>
#include <string>
#include <vector>

namespace curvepress {

// --- Stats -------------------------------------------------------------------

struct Stats {
    std::size_t n_input{};
    std::size_t n_kept{};
    std::size_t bytes_raw{};
    std::size_t bytes_compressed{};
    double      ratio{};
    double      max_error{};
    int         quant_bits{};
};

// --- Decoded -----------------------------------------------------------------

struct Decoded {
    std::vector<int64_t> timestamps_ns;
    std::vector<double>  values;
};

// --- helpers (internal) ------------------------------------------------------

namespace detail {

inline void check(int code) {
    if (code == 0) return;
    const char* msg = cp_strerror(code);
    switch (code) {
        case CP_ERR_BAD_INPUT:        throw std::invalid_argument(msg);
        case CP_ERR_BUFFER_TOO_SMALL: throw std::length_error(msg);
        case CP_ERR_CORRUPT:          throw std::runtime_error(msg);
        default:                      throw std::runtime_error(std::string("curvepress: ") + msg);
    }
}

inline std::vector<uint8_t> run_compress(
    std::span<const int64_t> timestamps_ns,
    std::span<const double>  values,
    CpStats* cs,
    auto compress_fn)
{
    std::size_t out_len = 0;
    // Dry run to get required size.
    detail::check(compress_fn(
        timestamps_ns.data(), values.data(), timestamps_ns.size(),
        nullptr, 0, &out_len, nullptr));

    std::vector<uint8_t> out(out_len);
    detail::check(compress_fn(
        timestamps_ns.data(), values.data(), timestamps_ns.size(),
        out.data(), out.size(), &out_len, cs));

    return out;
}

inline void fill_stats(Stats* dst, const CpStats& src) {
    if (!dst) return;
    dst->n_input          = src.n_input;
    dst->n_kept           = src.n_kept;
    dst->bytes_raw        = src.bytes_raw;
    dst->bytes_compressed = src.bytes_compressed;
    dst->ratio            = src.ratio;
    dst->max_error        = src.max_error;
    dst->quant_bits       = static_cast<int>(src.quant_bits);
}

} // namespace detail

// --- compress_rdp ------------------------------------------------------------

/// Compress with RDP. `epsilon` is the maximum absolute error in the value domain.
///
/// @throws std::invalid_argument  on bad input.
/// @throws std::runtime_error     on internal error.
inline std::vector<uint8_t> compress_rdp(
    std::span<const int64_t> timestamps_ns,
    std::span<const double>  values,
    double                   epsilon,
    Stats*                   stats = nullptr)
{
    CpStats cs{};
    auto fn = [epsilon](auto ts, auto val, auto n, auto ob, auto oc, auto ol, auto st) {
        return cp_compress_rdp(ts, val, n, epsilon, ob, oc, ol, st);
    };
    auto out = detail::run_compress(timestamps_ns, values, stats ? &cs : nullptr, fn);
    detail::fill_stats(stats, cs);
    return out;
}

// --- compress_vw -------------------------------------------------------------

/// Compress with Visvalingam-Whyatt. `n_out` is the exact number of kept points.
///
/// @throws std::invalid_argument  on bad input.
inline std::vector<uint8_t> compress_vw(
    std::span<const int64_t> timestamps_ns,
    std::span<const double>  values,
    std::size_t              n_out,
    Stats*                   stats = nullptr)
{
    CpStats cs{};
    auto fn = [n_out](auto ts, auto val, auto n, auto ob, auto oc, auto ol, auto st) {
        return cp_compress_vw(ts, val, n, n_out, ob, oc, ol, st);
    };
    auto out = detail::run_compress(timestamps_ns, values, stats ? &cs : nullptr, fn);
    detail::fill_stats(stats, cs);
    return out;
}

// --- compress_rdpn -----------------------------------------------------------

/// Compress with RDP-N (binary-searched epsilon to hit `n_out` points).
/// `epsilon` is the upper bound for the RDP search.
///
/// @throws std::invalid_argument  on bad input.
inline std::vector<uint8_t> compress_rdpn(
    std::span<const int64_t> timestamps_ns,
    std::span<const double>  values,
    std::size_t              n_out,
    double                   epsilon,
    Stats*                   stats = nullptr)
{
    CpStats cs{};
    auto fn = [n_out, epsilon](auto ts, auto val, auto n, auto ob, auto oc, auto ol, auto st) {
        return cp_compress_rdpn(ts, val, n, n_out, epsilon, ob, oc, ol, st);
    };
    auto out = detail::run_compress(timestamps_ns, values, stats ? &cs : nullptr, fn);
    detail::fill_stats(stats, cs);
    return out;
}

// --- decompress --------------------------------------------------------------

/// Decompress a byte stream produced by any `compress_*` function.
///
/// @throws std::runtime_error on corrupt data.
inline Decoded decompress(std::span<const uint8_t> data) {
    if (data.size() < 32) throw std::runtime_error("curvepress: corrupt stream");
    uint32_t n_kept_hdr{};
    std::memcpy(&n_kept_hdr, data.data() + 24, 4);

    Decoded out;
    out.timestamps_ns.resize(n_kept_hdr);
    out.values.resize(n_kept_hdr);

    std::size_t n_out = 0;
    detail::check(cp_decompress(
        data.data(), data.size(),
        out.timestamps_ns.data(), out.values.data(),
        n_kept_hdr, &n_out));

    out.timestamps_ns.resize(n_out);
    out.values.resize(n_out);
    return out;
}

// --- interpolate -------------------------------------------------------------

/// Reconstruct the value at a single timestamp `t` from the support points
/// via linear interpolation.
///
/// Points outside the data range are clamped (flat extrapolation).
///
/// @throws std::invalid_argument on bad parameters.
inline double interpolate(
    std::span<const int64_t> ts,
    std::span<const double>  val,
    int64_t                  t)
{
    double result{};
    detail::check(cp_interpolate(
        ts.data(), val.data(), ts.size(),
        t, &result));
    return result;
}

// --- version -----------------------------------------------------------------

inline const char* version() { return cp_version(); }

} // namespace curvepress
