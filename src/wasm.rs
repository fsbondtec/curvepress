/// wasm-bindgen WASM binding for curvepress.
///
/// Build: `wasm-pack build --target bundler --features wasm`
/// The output `pkg/` is an npm-ready package. Refine TS types in
/// `wasm/curvepress.d.ts`. No C ABI involved — wasm-bindgen is Rust-native.
use wasm_bindgen::prelude::*;

/// Compress with Ramer-Douglas-Peucker.
///
/// @param timestamps  BigInt64Array of strictly increasing nanosecond timestamps.
/// @param values      Float64Array of finite values (no NaN / Inf).
/// @param epsilon     Max absolute error in the value domain.
/// @returns Uint8Array byte stream.
#[wasm_bindgen]
pub fn compress_rdp(
    timestamps: &[i64],
    values: &[f64],
    epsilon: f64,
) -> Result<Vec<u8>, JsError> {
    crate::compress_rdp(timestamps, values, epsilon)
        .map_err(|e| JsError::new(&e.to_string()))
}

/// Compress with Visvalingam-Whyatt.
///
/// @param timestamps  BigInt64Array of strictly increasing nanosecond timestamps.
/// @param values      Float64Array of finite values.
/// @param n_out       Exact number of kept points.
/// @returns Uint8Array byte stream.
#[wasm_bindgen]
pub fn compress_vw(
    timestamps: &[i64],
    values: &[f64],
    n_out: usize,
) -> Result<Vec<u8>, JsError> {
    crate::compress_vw(timestamps, values, n_out)
        .map_err(|e| JsError::new(&e.to_string()))
}

/// Compress with RDP-N (binary-searched epsilon to hit `n_out` points).
///
/// @param timestamps  BigInt64Array.
/// @param values      Float64Array.
/// @param n_out       Target point count.
/// @param epsilon     Upper bound for the RDP search.
/// @returns Uint8Array byte stream.
#[wasm_bindgen]
pub fn compress_rdpn(
    timestamps: &[i64],
    values: &[f64],
    n_out: usize,
    epsilon: f64,
) -> Result<Vec<u8>, JsError> {
    crate::compress_rdpn(timestamps, values, n_out, epsilon)
        .map_err(|e| JsError::new(&e.to_string()))
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

/// Decompress a byte stream produced by any `compress_*` function.
///
/// @param data  Uint8Array produced by a compress function.
/// @returns Decoded object with .timestamps (BigInt64Array) and .values (Float64Array).
#[wasm_bindgen]
pub fn decompress(data: &[u8]) -> Result<Decoded, JsError> {
    let (timestamps, values) = crate::decompress(data)
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(Decoded { timestamps, values })
}

/// Reconstruct the value at a single timestamp `t` from the support points.
///
/// @param timestamps  BigInt64Array of kept timestamps (from decompress).
/// @param values      Float64Array of kept values (from decompress).
/// @param t           Query timestamp (nanoseconds, bigint).
/// @returns Interpolated value (number). Clamped at data boundaries.
#[wasm_bindgen]
pub fn interpolate(
    timestamps: &[i64],
    values: &[f64],
    t: i64,
) -> Result<f64, JsError> {
    crate::interpolate(timestamps, values, t)
        .map_err(|e| JsError::new(&e.to_string()))
}

/// Return the library version string.
#[wasm_bindgen]
pub fn version() -> String {
    crate::version().to_string()
}
