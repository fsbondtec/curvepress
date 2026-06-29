#pragma once
/// Compression pipeline: ties together point-reduction, quantization, and
/// varint encoding.
///
/// Binary layout (little-endian):
///   [4]  magic "CPRS"
///   [1]  version = 1
///   [1]  flags: bits[0-1]=algo, bit[2]=ts_mode, bits[3-7]=reserved
///   [1]  quant_bits (uint8, [1,32])
///   [1]  reserved = 0
///   [8]  val_min   (f64 LE)
///   [8]  val_range (f64 LE)
///   [4]  n_kept    (uint32 LE)
///   [4]  n_input   (uint32 LE)
///   [.]  payload: value varint stream, then timestamp stream
///
/// This layout is bitwise-identical to the original Rust implementation.
#include <algorithm>
#include <cassert>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <limits>
#include <stdexcept>
#include <string>
#include <unordered_set>
#include <vector>

#include "quantize.hpp"
#include "rdp.hpp"
#include "varint.hpp"
#include "vw.hpp"

namespace curvepress::detail {

// ── constants ─────────────────────────────────────────────────────────────────

static constexpr uint8_t  MAGIC[4] = {'C','P','R','S'};
static constexpr uint8_t  FORMAT_VERSION = 1;
static constexpr std::size_t HEADER_LEN  = 4 + 1 + 1 + 1 + 1 + 8 + 8 + 4 + 4; // = 32

enum class Algo : uint8_t { Rdp = 0, Vw = 1, RdpN = 2 };

// ── little-endian helpers ──────────────────────────────────────────────────────

inline void write_f64_le(std::vector<uint8_t>& buf, double v) {
    uint64_t bits;
    std::memcpy(&bits, &v, 8);
    for (int i = 0; i < 8; ++i) buf.push_back(static_cast<uint8_t>(bits >> (i * 8)));
}
inline void write_u32_le(std::vector<uint8_t>& buf, uint32_t v) {
    for (int i = 0; i < 4; ++i) buf.push_back(static_cast<uint8_t>(v >> (i * 8)));
}
inline double read_f64_le(const uint8_t* data, std::size_t off) {
    uint64_t bits = 0;
    for (int i = 0; i < 8; ++i) bits |= static_cast<uint64_t>(data[off + i]) << (i * 8);
    double v; std::memcpy(&v, &bits, 8); return v;
}
inline uint32_t read_u32_le(const uint8_t* data, std::size_t off) {
    uint32_t v = 0;
    for (int i = 0; i < 4; ++i) v |= static_cast<uint32_t>(data[off + i]) << (i * 8);
    return v;
}

// ── interpolation helper ───────────────────────────────────────────────────────

inline double interp_linear(
    const std::vector<int64_t>& kept_ts,
    const std::vector<double>&  kept_val,
    int64_t                     t)
{
    std::size_t n = kept_ts.size();
    if (n == 0) return 0.0;
    if (t <= kept_ts[0])     return kept_val[0];
    if (t >= kept_ts[n - 1]) return kept_val[n - 1];

    // Binary search: find j where kept_ts[j] <= t < kept_ts[j+1].
    std::size_t lo = 0, hi = n - 2;
    while (lo < hi) {
        std::size_t mid = (lo + hi + 1) / 2;
        if (kept_ts[mid] <= t) lo = mid; else hi = mid - 1;
    }
    double span = static_cast<double>(kept_ts[lo + 1] - kept_ts[lo]);
    double frac = static_cast<double>(t - kept_ts[lo]) / span;
    return kept_val[lo] + frac * (kept_val[lo + 1] - kept_val[lo]);
}

// ── error metrics ─────────────────────────────────────────────────────────────

/// Max vertical deviation of dropped points from the piecewise-linear
/// interpolation of the kept points (using original, pre-quantization values).
/// Used as quantization epsilon for VW and RDP-N.
inline double max_reconstruction_error_of_dropped(
    const std::vector<int64_t>& orig_ts,
    const std::vector<double>&  orig_val,
    const std::vector<int64_t>& kept_ts,
    const std::vector<double>&  kept_val)
{
    std::size_t n_kept = kept_ts.size();
    if (n_kept == 0 || n_kept == orig_ts.size()) return 0.0;

    std::unordered_set<int64_t> kept_set(kept_ts.begin(), kept_ts.end());
    double      max_err = 0.0;
    std::size_t j       = 0;

    for (std::size_t idx = 0; idx < orig_ts.size(); ++idx) {
        int64_t t = orig_ts[idx];
        double  v = orig_val[idx];
        if (kept_set.count(t)) continue;
        while (j + 1 < n_kept - 1 && kept_ts[j + 1] <= t) ++j;
        double span  = static_cast<double>(kept_ts[j + 1] - kept_ts[j]);
        double frac  = static_cast<double>(t - kept_ts[j]) / span;
        double recon = kept_val[j] + frac * (kept_val[j + 1] - kept_val[j]);
        max_err = std::max(max_err, std::abs(v - recon));
    }
    return max_err;
}

/// Max absolute error of the full lossy pipeline over ALL original points
/// (kept points interpolated using quantized values).
inline double compute_max_error(
    const std::vector<int64_t>& orig_ts,
    const std::vector<double>&  orig_val,
    const std::vector<int64_t>& kept_ts,
    const std::vector<double>&  kept_recon)
{
    if (kept_ts.empty()) return 0.0;
    std::size_t n_kept  = kept_ts.size();
    double      max_err = 0.0;
    std::size_t j       = 0;

    for (std::size_t idx = 0; idx < orig_ts.size(); ++idx) {
        int64_t t    = orig_ts[idx];
        double  orig = orig_val[idx];
        double  recon;
        if (t <= kept_ts[0]) {
            recon = kept_recon[0];
        } else if (t >= kept_ts[n_kept - 1]) {
            recon = kept_recon[n_kept - 1];
        } else {
            while (j + 1 < n_kept - 1 && kept_ts[j + 1] <= t) ++j;
            double span = static_cast<double>(kept_ts[j + 1] - kept_ts[j]);
            double frac = static_cast<double>(t - kept_ts[j]) / span;
            recon = kept_recon[j] + frac * (kept_recon[j + 1] - kept_recon[j]);
        }
        max_err = std::max(max_err, std::abs(orig - recon));
    }
    return max_err;
}

// ── public result types ────────────────────────────────────────────────────────

struct CompressStats {
    std::size_t n_input{};
    std::size_t n_kept{};
    std::size_t bytes_raw{};
    std::size_t bytes_compressed{};
    double      ratio{};
    double      max_error{};
    uint32_t    quant_bits{};
};

// ── compress ──────────────────────────────────────────────────────────────────

inline std::vector<uint8_t> compress_impl(
    const std::vector<int64_t>& timestamps_ns,
    const std::vector<double>&  values,
    Algo                        algo,
    double                      epsilon,   // used by RDP / RDP-N
    std::size_t                 n_out,     // used by VW / RDP-N
    CompressStats*              stats_out)
{
    std::size_t n = timestamps_ns.size();
    if (n != values.size())
        throw std::invalid_argument("curvepress: timestamps and values have different lengths");
    if (n == 0)
        throw std::invalid_argument("curvepress: empty input");

    for (std::size_t i = 1; i < n; ++i)
        if (timestamps_ns[i] <= timestamps_ns[i - 1])
            throw std::invalid_argument(
                "curvepress: timestamps not strictly increasing at index " + std::to_string(i));

    for (std::size_t i = 0; i < n; ++i)
        if (!std::isfinite(values[i]))
            throw std::invalid_argument(
                "curvepress: non-finite value at index " + std::to_string(i));

    // Effective value range (auto-detected).
    double v_min_data =  std::numeric_limits<double>::infinity();
    double v_max_data = -std::numeric_limits<double>::infinity();
    for (double v : values) { v_min_data = std::min(v_min_data, v); v_max_data = std::max(v_max_data, v); }
    double eff_vrange = std::max(v_max_data - v_min_data, 1.0);

    // 1. Point reduction (normalize_axes always true).
    std::vector<bool> kept_mask;
    switch (algo) {
        case Algo::Rdp:
            kept_mask = rdp_simplify(timestamps_ns, values, epsilon, true, eff_vrange);
            break;
        case Algo::Vw:
            kept_mask = vw_simplify(timestamps_ns, values, n_out, true, eff_vrange);
            break;
        case Algo::RdpN:
            kept_mask = rdp_n_simplify(timestamps_ns, values, n_out, eff_vrange, true, eff_vrange);
            break;
    }

    // Extract kept points.
    std::vector<int64_t> kept_ts;
    std::vector<double>  kept_val;
    for (std::size_t i = 0; i < n; ++i) {
        if (kept_mask[i]) {
            kept_ts.push_back(timestamps_ns[i]);
            kept_val.push_back(values[i]);
        }
    }
    std::size_t n_kept = kept_ts.size();

    // 2. Determine quantization epsilon.
    double quant_epsilon;
    switch (algo) {
        case Algo::Rdp:
            quant_epsilon = epsilon;
            break;
        case Algo::Vw:
        case Algo::RdpN: {
            double measured = max_reconstruction_error_of_dropped(
                timestamps_ns, values, kept_ts, kept_val);
            if (measured > 0.0) {
                quant_epsilon = measured;
            } else if (epsilon > 0.0) {
                quant_epsilon = epsilon;
            } else {
                quant_epsilon = std::max(eff_vrange / 1'000'000.0,
                                         std::numeric_limits<double>::epsilon());
            }
            break;
        }
    }

    // 3. Quantize kept values.
    Quantized q = quantize(kept_val, quant_epsilon);

    // 4. Compute max_error over ALL original input points.
    auto recon_kept = dequantize(q.codes, q.val_min, q.val_range, q.n_bits);
    double max_error = compute_max_error(timestamps_ns, values, kept_ts, recon_kept);

    // 5. Encode payload.
    std::vector<uint8_t> payload;
    // Value stream: first code as plain varint, then zigzag deltas.
    write_varint(payload, static_cast<uint64_t>(q.codes[0]));
    for (std::size_t i = 1; i < n_kept; ++i) {
        int64_t delta = static_cast<int64_t>(q.codes[i]) - static_cast<int64_t>(q.codes[i - 1]);
        write_varint(payload, zigzag_encode(delta));
    }
    // Timestamp stream: t0 as plain varint, then plain varint deltas.
    write_varint(payload, static_cast<uint64_t>(static_cast<uint64_t>(kept_ts[0])));
    for (std::size_t i = 1; i < n_kept; ++i) {
        uint64_t delta = static_cast<uint64_t>(kept_ts[i] - kept_ts[i - 1]);
        write_varint(payload, delta);
    }

    // 6. Assemble header.
    std::vector<uint8_t> out;
    out.reserve(HEADER_LEN + payload.size());
    out.insert(out.end(), MAGIC, MAGIC + 4);
    out.push_back(FORMAT_VERSION);
    out.push_back(static_cast<uint8_t>(algo)); // ts_mode bit = 0 (always Irregular)
    out.push_back(static_cast<uint8_t>(q.n_bits));
    out.push_back(0u); // reserved
    write_f64_le(out, q.val_min);
    write_f64_le(out, q.val_range);
    write_u32_le(out, static_cast<uint32_t>(n_kept));
    write_u32_le(out, static_cast<uint32_t>(n));
    out.insert(out.end(), payload.begin(), payload.end());

    if (stats_out) {
        stats_out->n_input          = n;
        stats_out->n_kept           = n_kept;
        stats_out->bytes_raw        = n * 16u;
        stats_out->bytes_compressed = out.size();
        stats_out->ratio            = static_cast<double>(n * 16u) / static_cast<double>(out.size());
        stats_out->max_error        = max_error;
        stats_out->quant_bits       = q.n_bits;
    }

    return out;
}

// ── decompress ────────────────────────────────────────────────────────────────

inline std::pair<std::vector<int64_t>, std::vector<double>>
decompress_impl(const uint8_t* data, std::size_t data_len)
{
    if (data_len < HEADER_LEN)
        throw std::runtime_error("curvepress: corrupt stream (too short)");
    if (std::memcmp(data, MAGIC, 4) != 0)
        throw std::runtime_error("curvepress: corrupt stream (bad magic)");
    if (data[4] != FORMAT_VERSION)
        throw std::runtime_error("curvepress: corrupt stream (unsupported version)");

    uint32_t quant_bits = data[6];
    double   val_min    = read_f64_le(data, 8);
    double   val_range  = read_f64_le(data, 16);
    uint32_t n_kept     = read_u32_le(data, 24);
    // n_input at offset 28 is informational; not needed for decode.

    if (n_kept == 0)
        throw std::runtime_error("curvepress: corrupt stream (n_kept=0)");

    std::size_t pos = HEADER_LEN;

    // Decode value stream.
    auto first_code_opt = read_varint(data, data_len, pos);
    if (!first_code_opt) throw std::runtime_error("curvepress: corrupt stream");
    std::vector<uint32_t> codes;
    codes.reserve(n_kept);
    codes.push_back(static_cast<uint32_t>(*first_code_opt));

    for (uint32_t i = 1; i < n_kept; ++i) {
        auto raw = read_varint(data, data_len, pos);
        if (!raw) throw std::runtime_error("curvepress: corrupt stream");
        int64_t  delta = zigzag_decode(*raw);
        int64_t  prev  = static_cast<int64_t>(codes.back());
        codes.push_back(static_cast<uint32_t>(prev + delta));
    }

    auto values = dequantize(codes, val_min, val_range, quant_bits);

    // Decode timestamp stream (Irregular: t0 plain varint, then plain deltas).
    auto t0_opt = read_varint(data, data_len, pos);
    if (!t0_opt) throw std::runtime_error("curvepress: corrupt stream");
    std::vector<int64_t> timestamps;
    timestamps.reserve(n_kept);
    timestamps.push_back(static_cast<int64_t>(*t0_opt));

    for (uint32_t i = 1; i < n_kept; ++i) {
        auto delta_opt = read_varint(data, data_len, pos);
        if (!delta_opt) throw std::runtime_error("curvepress: corrupt stream");
        timestamps.push_back(timestamps.back() + static_cast<int64_t>(*delta_opt));
    }

    return { std::move(timestamps), std::move(values) };
}

// ── interpolate ───────────────────────────────────────────────────────────────

inline double interpolate_impl(
    const int64_t* ts, const double* val, std::size_t n, int64_t t)
{
    if (n == 0) throw std::invalid_argument("curvepress: empty ts");
    if (t <= ts[0])     return val[0];
    if (t >= ts[n - 1]) return val[n - 1];
    // Binary search.
    std::size_t lo = 0, hi = n - 2;
    while (lo < hi) {
        std::size_t mid = (lo + hi + 1) / 2;
        if (ts[mid] <= t) lo = mid; else hi = mid - 1;
    }
    double span = static_cast<double>(ts[lo + 1] - ts[lo]);
    double frac = static_cast<double>(t - ts[lo]) / span;
    return val[lo] + frac * (val[lo + 1] - val[lo]);
}

} // namespace curvepress::detail
