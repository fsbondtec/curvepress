/// Error type for curvepress operations.
#[derive(Debug, thiserror::Error)]
pub enum CpError {
    /// Input validation failure (empty arrays, non-monotonic timestamps, NaN/Inf values).
    #[error("bad input: {0}")]
    BadInput(String),

    /// Output buffer too small (C ABI only).
    #[error("buffer too small")]
    BufferTooSmall,

    /// Byte stream is corrupted or truncated.
    #[error("corrupt stream")]
    Corrupt,
}
