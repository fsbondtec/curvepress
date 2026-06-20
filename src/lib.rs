//! curvepress — lossy time series compression (RDP/VW + quantization + varint).
//!
//! All algorithm logic lives in this crate. C++/Python/WASM are thin bindings
//! over the public API exposed here.
//!
//! # Error-bound contract
//!
//! When `normalize_axes = false` the maximum absolute reconstruction error is
//! approximately `1.5 * epsilon`: RDP introduces at most `epsilon` (dropped
//! points deviate at most `epsilon` from the linear interpolant of kept
//! neighbours), and quantization adds at most `epsilon / 2`. To achieve a
//! strict `epsilon` bound, set `epsilon / 2` in the config and accept the
//! halved compression ratio.
//!
//! When `normalize_axes = true`, `epsilon` is a Euclidean (time + value)
//! tolerance; the value-domain error is no longer bounded by `epsilon` and
//! `Stats::max_error` is reported informatively rather than as a guarantee.

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

// ─── Public types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Algo {
    /// Ramer-Douglas-Peucker (epsilon-based). Default.
    Rdp,
    /// Visvalingam-Whyatt (target-count-based, O(n log n)).
    Vw,
    /// RDP with binary-searched epsilon to hit a target point count.
    RdpN,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TsMode {
    /// Timestamps are uniformly spaced; encode t0 + interval + index stream.
    Regular,
    /// Timestamps are irregularly spaced; encode delta stream. Default.
    Irregular,
}

/// Compression configuration.
#[derive(Debug, Clone)]
pub struct Config {
    pub algo: Algo,
    pub ts_mode: TsMode,
    /// RDP / RDP-N: maximum absolute error in the value domain.
    pub epsilon: f64,
    /// VW / RDP-N: target number of output points. Clamped to `[2, n]`.
    pub n_out: usize,
    /// Optional radial-distance pre-filter radius (value domain).
    /// Drops any point whose distance from the last kept point is below this.
    /// `None` (default) disables the pre-filter.
    pub radial_prefilter: Option<f64>,
    /// Scale the time axis so the full time span maps onto `value_range`
    /// before computing distances / areas. Prevents the time axis from
    /// dominating when timestamps are in nanoseconds.
    ///
    /// Note: when `true`, `epsilon` becomes a Euclidean tolerance and the
    /// `max_error <= 1.5 * epsilon` contract is no longer guaranteed.
    pub normalize_axes: bool,
    /// Expected max–min of values. Used as the scale target when
    /// `normalize_axes = true`, and as the search bound for RDP-N.
    ///
    /// Set to `0.0` (or leave at default `1.0` which will be overridden
    /// at runtime) to let curvepress measure the actual range from the data.
    /// Any value `<= 0` causes a fall-back to the measured range.
    pub value_range: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            algo: Algo::Rdp,
            ts_mode: TsMode::Irregular,
            epsilon: 1.0,
            n_out: 100,
            radial_prefilter: None,
            normalize_axes: false,
            // 0.0 = auto: fall back to measured val_max-val_min.
            // Set a positive value to override (e.g. when you know the
            // expected signal range beforehand).
            value_range: 0.0,
        }
    }
}

/// Compression statistics.
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
    /// Bounded by ~`1.5 * epsilon` when `normalize_axes = false`.
    pub max_error: f64,
    pub quant_bits: u32,
}

// ─── Public API ─────────────────────────────────────────────────────────────

/// Compress `(timestamp_ns, value)` pairs into a self-describing byte stream.
///
/// # Preconditions
/// - `timestamps_ns` must be strictly increasing.
/// - `values` must be finite (no NaN, no Inf).
/// - Both slices must have the same length and be non-empty.
pub fn compress(
    timestamps_ns: &[i64],
    values: &[f64],
    cfg: &Config,
) -> Result<Vec<u8>, CpError> {
    codec::compress_inner(timestamps_ns, values, cfg).map(|(data, _)| data)
}

/// Same as [`compress`] but also returns [`Stats`].
pub fn compress_with_stats(
    timestamps_ns: &[i64],
    values: &[f64],
    cfg: &Config,
) -> Result<(Vec<u8>, Stats), CpError> {
    codec::compress_inner(timestamps_ns, values, cfg)
}

/// Decompress a byte stream produced by [`compress`] to the kept support points.
///
/// The returned vectors have length `Stats::n_kept` (not `n_input`).
pub fn decompress(data: &[u8]) -> Result<(Vec<i64>, Vec<f64>), CpError> {
    codec::decompress_inner(data)
}

/// Reconstruct values on a regular grid from kept support points via linear
/// interpolation.
///
/// - Grid points outside `[ts[0], ts[n-1]]` are clamped (flat extrapolation).
/// - Output length = `floor((t_end - t_start) / interval_ns) + 1`.
pub fn interpolate(
    ts: &[i64],
    val: &[f64],
    t_start: i64,
    t_end: i64,
    interval_ns: i64,
) -> Result<Vec<f64>, CpError> {
    codec::interpolate_inner(ts, val, t_start, t_end, interval_ns)
}

/// Semver string of this build.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
