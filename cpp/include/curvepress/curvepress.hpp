#pragma once
/// curvepress C++20 wrapper — idiomatic RAII/exception interface over the
/// auto-generated `curvepress.h` (cbindgen output).
///
/// The consumer sees ONLY this header. The raw C API is an implementation
/// detail; never call `cp_*` functions directly.
///
/// Link: add the Rust static library built by `cpp/CMakeLists.txt`.
/// CMake target: `curvepress::curvepress`

#include <curvepress.h>   // cbindgen-generated, from include/curvepress.h
#include <cstdint>
#include <optional>
#include <span>
#include <stdexcept>
#include <string>
#include <vector>

namespace curvepress {

// ─── Enumerations ────────────────────────────────────────────────────────────

enum class Algo  : uint32_t { Rdp = 0, Vw = 1, RdpN = 2 };
enum class TsMode : uint32_t { Irregular = 0, Regular = 1 };

// ─── Config ──────────────────────────────────────────────────────────────────

/// Compression configuration. Mirrors `crate::Config` in Rust.
struct Config {
    Algo        algo              = Algo::Rdp;
    TsMode      ts_mode           = TsMode::Irregular;
    double      epsilon           = 1.0;
    std::size_t n_out             = 100;
    /// Set to a positive value to enable the radial-distance pre-filter.
    std::optional<double> radial_prefilter = std::nullopt;
    bool        normalize_axes    = false;
    /// 0 = auto (measured from data). Override with a positive value.
    double      value_range       = 0.0;
};

// ─── Stats ───────────────────────────────────────────────────────────────────

struct Stats {
    std::size_t n_input{};
    std::size_t n_kept{};
    std::size_t bytes_raw{};
    std::size_t bytes_compressed{};
    double      ratio{};
    double      max_error{};
    int         quant_bits{};
};

// ─── Decoded ─────────────────────────────────────────────────────────────────

struct Decoded {
    std::vector<int64_t> timestamps_ns;
    std::vector<double>  values;
};

// ─── helpers (internal) ──────────────────────────────────────────────────────

namespace detail {

inline void check(int code) {
    if (code == 0) return;
    const char* msg = cp_strerror(code);
    switch (code) {
        case CP_ERR_BAD_INPUT:     throw std::invalid_argument(msg);
        case CP_ERR_BUFFER_TOO_SMALL: throw std::length_error(msg);
        case CP_ERR_CORRUPT:       throw std::runtime_error(msg);
        default:                   throw std::runtime_error(std::string("curvepress: ") + msg);
    }
}

inline CpConfig to_c(const Config& cfg) {
    CpConfig c{};
    cp_config_default(&c);
    c.algo                 = static_cast<uint32_t>(cfg.algo);
    c.ts_mode              = static_cast<uint32_t>(cfg.ts_mode);
    c.epsilon              = cfg.epsilon;
    c.n_out                = cfg.n_out;
    c.use_radial_prefilter = cfg.radial_prefilter.has_value() ? 1 : 0;
    c.radial_epsilon       = cfg.radial_prefilter.value_or(0.0);
    c.normalize_axes       = cfg.normalize_axes ? 1 : 0;
    c.value_range          = cfg.value_range;
    return c;
}

} // namespace detail

// ─── compress ────────────────────────────────────────────────────────────────

/// Compress `(timestamps_ns, values)` into a byte stream.
///
/// @throws std::invalid_argument  on bad input (empty, non-monotonic, NaN/Inf).
/// @throws std::runtime_error     on internal error.
inline std::vector<uint8_t> compress(
    std::span<const int64_t> timestamps_ns,
    std::span<const double>  values,
    const Config&            cfg   = {},
    Stats*                   stats = nullptr)
{
    CpConfig c = detail::to_c(cfg);

    // Dry run: get required output size.
    std::size_t out_len = 0;
    detail::check(cp_compress(
        &c,
        timestamps_ns.data(), values.data(), timestamps_ns.size(),
        nullptr, 0, &out_len,
        nullptr));

    // Real run.
    std::vector<uint8_t> out(out_len);
    CpStats cs{};
    detail::check(cp_compress(
        &c,
        timestamps_ns.data(), values.data(), timestamps_ns.size(),
        out.data(), out.size(), &out_len,
        stats ? &cs : nullptr));

    if (stats) {
        stats->n_input          = cs.n_input;
        stats->n_kept           = cs.n_kept;
        stats->bytes_raw        = cs.bytes_raw;
        stats->bytes_compressed = cs.bytes_compressed;
        stats->ratio            = cs.ratio;
        stats->max_error        = cs.max_error;
        stats->quant_bits       = static_cast<int>(cs.quant_bits);
    }
    return out;
}

// ─── decompress ──────────────────────────────────────────────────────────────

/// Decompress a byte stream produced by `compress`.
///
/// @throws std::runtime_error on corrupt data.
inline Decoded decompress(std::span<const uint8_t> data) {
    // First pass: get n_kept without allocating target buffers.
    // We allocate conservatively: use n_input from the header (first 32 bytes).
    // The header stores n_input at offset 28 and n_kept at offset 24.
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

// ─── interpolate ─────────────────────────────────────────────────────────────

/// Reconstruct values on a regular grid via linear interpolation.
///
/// Output length = floor((t_end - t_start) / interval_ns) + 1.
/// Points outside the data range are clamped (flat extrapolation).
///
/// @throws std::invalid_argument on bad parameters.
inline std::vector<double> interpolate(
    std::span<const int64_t> ts,
    std::span<const double>  val,
    int64_t t_start,
    int64_t t_end,
    int64_t interval_ns)
{
    if (interval_ns <= 0) throw std::invalid_argument("curvepress: interval_ns must be > 0");
    if (t_end < t_start)  throw std::invalid_argument("curvepress: t_end < t_start");

    const std::size_t n_out =
        static_cast<std::size_t>((t_end - t_start) / interval_ns) + 1u;
    std::vector<double> out(n_out);
    detail::check(cp_interpolate(
        ts.data(), val.data(), ts.size(),
        t_start, t_end, interval_ns,
        out.data(), n_out));
    return out;
}

// ─── version ─────────────────────────────────────────────────────────────────

inline const char* version() { return cp_version(); }

} // namespace curvepress
