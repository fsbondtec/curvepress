//! curvepress — lossy time series compression (RDP/VW + quantization + varint).
//!
//! All algorithm logic lives in this crate. C++/Python/WASM are thin bindings
//! over the public API exposed here.
//!
//! # Usage
//!
//! Choose a compression algorithm:
//! - [`compress_rdp`]  — Ramer-Douglas-Peucker (epsilon-based, ~1.5× error bound)
//! - [`compress_vw`]   — Visvalingam-Whyatt (target point count)
//! - [`compress_rdpn`] — RDP with binary search to hit a target point count
//!
//! Decompress with [`decompress`], then query values with [`interpolate`].
//!
//! The time axis is always normalized to the value range before distance
//! computation, so `epsilon` is always a value-domain tolerance.

mod rdp;
mod vw;
mod radial;
mod quantize;
mod varint;
mod codec;
mod error;

pub use error::CpError;

#[cfg(feature = "capi")]   mod capi;
#[cfg(feature = "python")] mod python;
#[cfg(feature = "wasm")]   mod wasm;

// ─── Internal types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Algo { Rdp, Vw, RdpN }

pub(crate) struct Config {
    pub(crate) algo: Algo,
    /// RDP / RDP-N: maximum absolute error in the value domain.
    pub(crate) epsilon: f64,
    /// VW / RDP-N: target number of output points. Clamped to `[2, n]`.
    pub(crate) n_out: usize,
}

// ─── Public types ────────────────────────────────────────────────────────────

/// Compression statistics returned by `*_stats` variants.
#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub n_input: usize,
    pub n_kept: usize,
    /// `n_input * 16` (raw i64 timestamps + f64 values).
    pub bytes_raw: usize,
    pub bytes_compressed: usize,
    /// `bytes_raw / bytes_compressed`.
    pub ratio: f64,
    /// Maximum absolute reconstruction error over all original input points
    /// (full lossy pipeline: point-drop + quantization).
    /// Bounded by ~`1.5 * epsilon` for RDP.
    pub max_error: f64,
    pub quant_bits: u32,
}

// ─── Public API ─────────────────────────────────────────────────────────────

/// Compress with Ramer-Douglas-Peucker. `epsilon` is the maximum absolute
/// reconstruction error in the value domain.
pub fn compress_rdp(ts: &[i64], val: &[f64], epsilon: f64) -> Result<Vec<u8>, CpError> {
    codec::compress_inner(ts, val, &Config { algo: Algo::Rdp, epsilon, n_out: 0 })
        .map(|(d, _)| d)
}

/// Like [`compress_rdp`] but also returns [`Stats`].
pub fn compress_rdp_stats(ts: &[i64], val: &[f64], epsilon: f64) -> Result<(Vec<u8>, Stats), CpError> {
    codec::compress_inner(ts, val, &Config { algo: Algo::Rdp, epsilon, n_out: 0 })
}

/// Compress with Visvalingam-Whyatt. `n_out` is the exact number of kept points.
pub fn compress_vw(ts: &[i64], val: &[f64], n_out: usize) -> Result<Vec<u8>, CpError> {
    codec::compress_inner(ts, val, &Config { algo: Algo::Vw, epsilon: 0.0, n_out })
        .map(|(d, _)| d)
}

/// Like [`compress_vw`] but also returns [`Stats`].
pub fn compress_vw_stats(ts: &[i64], val: &[f64], n_out: usize) -> Result<(Vec<u8>, Stats), CpError> {
    codec::compress_inner(ts, val, &Config { algo: Algo::Vw, epsilon: 0.0, n_out })
}

/// Compress with RDP binary-searched to hit `n_out` points. `epsilon` is used
/// as the search bound (upper limit for the RDP epsilon).
pub fn compress_rdpn(ts: &[i64], val: &[f64], n_out: usize, epsilon: f64) -> Result<Vec<u8>, CpError> {
    codec::compress_inner(ts, val, &Config { algo: Algo::RdpN, epsilon, n_out })
        .map(|(d, _)| d)
}

/// Like [`compress_rdpn`] but also returns [`Stats`].
pub fn compress_rdpn_stats(ts: &[i64], val: &[f64], n_out: usize, epsilon: f64) -> Result<(Vec<u8>, Stats), CpError> {
    codec::compress_inner(ts, val, &Config { algo: Algo::RdpN, epsilon, n_out })
}

/// Decompress a byte stream produced by any `compress_*` function.
///
/// Returns the kept support points `(timestamps_ns, values)`.
pub fn decompress(data: &[u8]) -> Result<(Vec<i64>, Vec<f64>), CpError> {
    codec::decompress_inner(data)
}

/// Reconstruct the value at a single timestamp `t` from the support points
/// via linear interpolation.
///
/// - `t` before `ts[0]` → clamped to `val[0]` (flat extrapolation).
/// - `t` after `ts[last]` → clamped to `val[last]`.
pub fn interpolate(ts: &[i64], val: &[f64], t: i64) -> Result<f64, CpError> {
    codec::interpolate_point(ts, val, t)
}

/// Semver string of this build.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
