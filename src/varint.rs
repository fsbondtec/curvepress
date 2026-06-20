/// Unsigned LEB-128 encoding. Each byte carries 7 payload bits; the high bit
/// signals that more bytes follow.
pub fn write_varint(buf: &mut Vec<u8>, mut value: u64) {
    loop {
        let byte = (value & 0x7F) as u8;
        value >>= 7;
        if value == 0 {
            buf.push(byte);
            break;
        } else {
            buf.push(byte | 0x80);
        }
    }
}

/// Decode one unsigned LEB-128 varint from `data[pos..]`.
/// Advances `pos` past the consumed bytes. Returns `None` on truncation or
/// an overlong encoding (> 10 bytes for a u64).
pub fn read_varint(data: &[u8], pos: &mut usize) -> Option<u64> {
    let mut result: u64 = 0;
    let mut shift = 0u32;
    loop {
        if *pos >= data.len() {
            return None;
        }
        let byte = data[*pos] as u64;
        *pos += 1;
        result |= (byte & 0x7F) << shift;
        if byte & 0x80 == 0 {
            return Some(result);
        }
        shift += 7;
        if shift >= 64 {
            return None; // malformed / overlong
        }
    }
}

/// Zigzag-encode a signed integer to an unsigned one so that small magnitudes
/// (positive or negative) produce small varints.
///
/// zigzag(n) = (n << 1) ^ (n >> 63)
#[inline]
pub fn zigzag_encode(n: i64) -> u64 {
    ((n << 1) ^ (n >> 63)) as u64
}

/// Inverse of `zigzag_encode`.
#[inline]
pub fn zigzag_decode(n: u64) -> i64 {
    ((n >> 1) as i64) ^ (-((n & 1) as i64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zigzag_roundtrip() {
        for v in [0i64, 1, -1, i64::MAX, i64::MIN, 127, -128, 1_000_000, -1_000_000] {
            assert_eq!(zigzag_decode(zigzag_encode(v)), v);
        }
    }

    #[test]
    fn varint_roundtrip() {
        let values = [0u64, 1, 127, 128, 255, 16383, 16384, u64::MAX];
        for &v in &values {
            let mut buf = Vec::new();
            write_varint(&mut buf, v);
            let mut pos = 0;
            assert_eq!(read_varint(&buf, &mut pos), Some(v));
            assert_eq!(pos, buf.len());
        }
    }
}
