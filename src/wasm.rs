/// wasm-bindgen WASM binding — exposes the full `Config` surface.
///
/// Build: `wasm-pack build --target bundler --features wasm`
/// The output `pkg/` is an npm-ready package. Refine TS types in
/// `wasm/curvepress.d.ts`. No C ABI involved — wasm-bindgen is Rust-native.
use wasm_bindgen::prelude::*;
use crate::{Algo, Config, TsMode};

fn algo_from_u32(a: u32) -> Algo {
    match a { 1 => Algo::Vw, 2 => Algo::RdpN, _ => Algo::Rdp }
}
fn ts_mode_from_u32(m: u32) -> TsMode {
    if m == 1 { TsMode::Regular } else { TsMode::Irregular }
}

/// Compress time series data into a byte stream.
///
/// @param timestamps  BigInt64Array of strictly increasing nanosecond timestamps.
/// @param values      Float64Array of finite values (no NaN / Inf).
/// @param epsilon     Max absolute error for RDP / RDP-N (default 1.0).
/// @param algo        0=RDP, 1=VW, 2=RDP-N (default 0).
/// @param n_out       Target point count for VW / RDP-N (default 100).
/// @param normalize_axes  Scale time axis before distance computation (default false).
/// @param value_range Override for normalization / RDP-N bound; 0 = auto (default 0).
/// @param ts_mode     0=Irregular, 1=Regular (default 0).
/// @param radial_prefilter  Radial distance pre-filter radius; null = disabled (default null).
/// @returns Uint8Array byte stream.
#[wasm_bindgen]
pub fn compress(
    timestamps: &[i64],
    values: &[f64],
    epsilon: f64,
    algo: u32,
    n_out: usize,
    normalize_axes: bool,
    value_range: f64,
    ts_mode: u32,
    radial_prefilter: Option<f64>,
) -> Result<Vec<u8>, JsError> {
    let cfg = Config {
        algo: algo_from_u32(algo),
        ts_mode: ts_mode_from_u32(ts_mode),
        epsilon,
        n_out,
        radial_prefilter,
        normalize_axes,
        value_range,
    };
    crate::compress(timestamps, values, &cfg).map_err(|e| JsError::new(&e.to_string()))
}

/// Decompressed time series data.
#[wasm_bindgen]
pub struct Decoded {
    timestamps: Vec<i64>,
    values: Vec<f64>,
}

#[wasm_bindgen]
impl Decoded {
    /// BigInt64Array of kept nanosecond timestamps.
    #[wasm_bindgen(getter)]
    pub fn timestamps(&self) -> Vec<i64> { self.timestamps.clone() }
    /// Float64Array of kept values.
    #[wasm_bindgen(getter)]
    pub fn values(&self) -> Vec<f64> { self.values.clone() }
    /// Number of kept points.
    #[wasm_bindgen(getter)]
    pub fn len(&self) -> usize { self.timestamps.len() }
}

/// Decompress a byte stream produced by `compress`.
///
/// @param data  Uint8Array produced by compress().
/// @returns Decoded object with .timestamps (BigInt64Array) and .values (Float64Array).
#[wasm_bindgen]
pub fn decompress(data: &[u8]) -> Result<Decoded, JsError> {
    let (timestamps, values) = crate::decompress(data)
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(Decoded { timestamps, values })
}

/// Interpolate kept support points onto a regular time grid.
///
/// @param timestamps  BigInt64Array of kept timestamps (from decompress).
/// @param values      Float64Array of kept values (from decompress).
/// @param t_start     Grid start (nanoseconds).
/// @param t_end       Grid end (nanoseconds, inclusive).
/// @param interval_ns Grid step size (nanoseconds).
/// @returns Float64Array of interpolated values (length = floor((t_end-t_start)/interval)+1).
#[wasm_bindgen]
pub fn interpolate(
    timestamps: &[i64],
    values: &[f64],
    t_start: i64,
    t_end: i64,
    interval_ns: i64,
) -> Result<Vec<f64>, JsError> {
    crate::interpolate(timestamps, values, t_start, t_end, interval_ns)
        .map_err(|e| JsError::new(&e.to_string()))
}

/// Return the library version string.
#[wasm_bindgen]
pub fn version() -> String {
    crate::version().to_string()
}
