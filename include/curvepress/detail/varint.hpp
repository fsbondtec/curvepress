#pragma once
#include <cstdint>
#include <vector>
#include <optional>

namespace curvepress::detail {

/// Unsigned LEB-128 encoding. Each byte carries 7 payload bits; the high bit
/// signals that more bytes follow.
inline void write_varint(std::vector<uint8_t>& buf, uint64_t value) {
    do {
        uint8_t byte = static_cast<uint8_t>(value & 0x7Fu);
        value >>= 7;
        if (value != 0) byte |= 0x80u;
        buf.push_back(byte);
    } while (value != 0);
}

/// Decode one unsigned LEB-128 varint from data[pos..].
/// Advances pos past the consumed bytes. Returns nullopt on truncation or
/// an overlong encoding (> 10 bytes for a uint64).
inline std::optional<uint64_t> read_varint(const uint8_t* data, std::size_t data_len, std::size_t& pos) {
    uint64_t result = 0;
    unsigned shift = 0;
    while (true) {
        if (pos >= data_len) return std::nullopt;
        uint64_t byte = data[pos++];
        result |= (byte & 0x7Fu) << shift;
        if ((byte & 0x80u) == 0) return result;
        shift += 7;
        if (shift >= 64) return std::nullopt; // malformed / overlong
    }
}

/// Zigzag-encode a signed integer to an unsigned one so that small magnitudes
/// produce small varints.
///   zigzag(n) = (n << 1) ^ (n >> 63)
inline uint64_t zigzag_encode(int64_t n) noexcept {
    return static_cast<uint64_t>((n << 1) ^ (n >> 63));
}

/// Inverse of zigzag_encode.
inline int64_t zigzag_decode(uint64_t n) noexcept {
    return static_cast<int64_t>((n >> 1)) ^ -static_cast<int64_t>(n & 1u);
}

} // namespace curvepress::detail
