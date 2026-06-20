/// C ABI surface for the C++ binding.
///
/// These `extern "C"` functions are the ONLY C-ABI surface in curvepress.
/// `cbindgen` generates `include/curvepress.h` from them at build time.
/// The C++ consumer never calls this directly — it uses `cpp/include/curvepress/curvepress.hpp`.
///
/// Every function catches panics (via `std::panic::catch_unwind`) and translates
/// them to error codes, because panics crossing the FFI boundary are UB.
use crate::{Algo, Config, TsMode};
use std::ffi::CStr;
use std::os::raw::c_char;
use std::panic::catch_unwind;
use std::slice;

// ─── Error codes ─────────────────────────────────────────────────────────────

pub const CP_OK: i32 = 0;
pub const CP_ERR_BAD_INPUT: i32 = -1;
pub const CP_ERR_BUFFER_TOO_SMALL: i32 = -2;
pub const CP_ERR_CORRUPT: i32 = -3;
pub const CP_ERR_PANIC: i32 = -99;
pub const CP_ERR_NULL: i32 = -100;

fn to_code(r: &crate::CpError) -> i32 {
    match r {
        crate::CpError::BadInput(_) => CP_ERR_BAD_INPUT,
        crate::CpError::BufferTooSmall => CP_ERR_BUFFER_TOO_SMALL,
        crate::CpError::Corrupt => CP_ERR_CORRUPT,
    }
}

// ─── CpConfig ────────────────────────────────────────────────────────────────

/// C representation of `Config`.
/// `algo`: 0=RDP, 1=VW, 2=RDP_N
/// `ts_mode`: 0=Irregular, 1=Regular
/// `use_radial_prefilter`: 0=disabled, 1=enabled
#[repr(C)]
pub struct CpConfig {
    pub algo: u32,
    pub ts_mode: u32,
    pub epsilon: f64,
    pub n_out: usize,
    pub use_radial_prefilter: i32,
    pub radial_epsilon: f64,
    pub normalize_axes: i32,
    pub value_range: f64,
}

/// C representation of `Stats`.
#[repr(C)]
pub struct CpStats {
    pub n_input: usize,
    pub n_kept: usize,
    pub bytes_raw: usize,
    pub bytes_compressed: usize,
    pub ratio: f64,
    pub max_error: f64,
    pub quant_bits: u32,
}

fn from_c_config(c: &CpConfig) -> Config {
    Config {
        algo: match c.algo { 1 => Algo::Vw, 2 => Algo::RdpN, _ => Algo::Rdp },
        ts_mode: if c.ts_mode == 1 { TsMode::Regular } else { TsMode::Irregular },
        epsilon: c.epsilon,
        n_out: c.n_out,
        radial_prefilter: if c.use_radial_prefilter != 0 { Some(c.radial_epsilon) } else { None },
        normalize_axes: c.normalize_axes != 0,
        value_range: c.value_range,
    }
}

// ─── Public C functions ──────────────────────────────────────────────────────

/// Fill `*cfg` with default values.
///
/// # Safety
/// `cfg` must be a valid, aligned, non-null pointer to a `CpConfig`.
#[no_mangle]
pub unsafe extern "C" fn cp_config_default(cfg: *mut CpConfig) {
    if cfg.is_null() { return; }
    let def = Config::default();
    (*cfg) = CpConfig {
        algo: match def.algo { Algo::Rdp => 0, Algo::Vw => 1, Algo::RdpN => 2 },
        ts_mode: if def.ts_mode == TsMode::Regular { 1 } else { 0 },
        epsilon: def.epsilon,
        n_out: def.n_out,
        use_radial_prefilter: 0,
        radial_epsilon: 0.0,
        normalize_axes: 0,
        value_range: def.value_range,
    };
}

/// Compress `n` `(timestamp_ns, value)` pairs.
///
/// If `out_buf` is null, writes the required byte length to `*out_len` (dry run).
/// Returns 0 on success, or a negative error code.
///
/// # Safety
/// All non-null pointers must be valid and properly aligned for their types.
#[no_mangle]
pub unsafe extern "C" fn cp_compress(
    cfg: *const CpConfig,
    timestamps_ns: *const i64,
    values: *const f64,
    n: usize,
    out_buf: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
    stats: *mut CpStats,
) -> i32 {
    if cfg.is_null() || timestamps_ns.is_null() || values.is_null() || out_len.is_null() {
        return CP_ERR_NULL;
    }
    let result = catch_unwind(|| {
        let ts = slice::from_raw_parts(timestamps_ns, n);
        let val = slice::from_raw_parts(values, n);
        let rust_cfg = from_c_config(&*cfg);
        crate::compress_with_stats(ts, val, &rust_cfg)
    });
    match result {
        Err(_) => CP_ERR_PANIC,
        Ok(Err(e)) => to_code(&e),
        Ok(Ok((data, s))) => {
            *out_len = data.len();
            if !stats.is_null() {
                (*stats) = CpStats {
                    n_input: s.n_input,
                    n_kept: s.n_kept,
                    bytes_raw: s.bytes_raw,
                    bytes_compressed: s.bytes_compressed,
                    ratio: s.ratio,
                    max_error: s.max_error,
                    quant_bits: s.quant_bits,
                };
            }
            if out_buf.is_null() {
                return CP_OK; // dry run: length written, done
            }
            if out_cap < data.len() {
                return CP_ERR_BUFFER_TOO_SMALL;
            }
            std::ptr::copy_nonoverlapping(data.as_ptr(), out_buf, data.len());
            CP_OK
        }
    }
}

/// Decompress a byte stream into `(ts_out, val_out)` arrays of capacity `n_cap`.
///
/// Writes the number of decoded points to `*n_out`. Returns 0 on success.
///
/// # Safety
/// All pointers must be valid. `ts_out` and `val_out` must have capacity `n_cap`.
#[no_mangle]
pub unsafe extern "C" fn cp_decompress(
    buf: *const u8,
    buf_len: usize,
    ts_out: *mut i64,
    val_out: *mut f64,
    n_cap: usize,
    n_out: *mut usize,
) -> i32 {
    if buf.is_null() || ts_out.is_null() || val_out.is_null() || n_out.is_null() {
        return CP_ERR_NULL;
    }
    let result = catch_unwind(|| {
        let data = slice::from_raw_parts(buf, buf_len);
        crate::decompress(data)
    });
    match result {
        Err(_) => CP_ERR_PANIC,
        Ok(Err(e)) => to_code(&e),
        Ok(Ok((ts, val))) => {
            *n_out = ts.len();
            if n_cap < ts.len() {
                return CP_ERR_BUFFER_TOO_SMALL;
            }
            std::ptr::copy_nonoverlapping(ts.as_ptr(), ts_out, ts.len());
            std::ptr::copy_nonoverlapping(val.as_ptr(), val_out, val.len());
            CP_OK
        }
    }
}

/// Interpolate onto a regular grid.
///
/// Output count = floor((t_end - t_start) / interval_ns) + 1. `val_out` must
/// have at least that many elements.
///
/// # Safety
/// All pointers must be valid. `val_out` must have sufficient capacity.
#[no_mangle]
pub unsafe extern "C" fn cp_interpolate(
    ts_in: *const i64,
    val_in: *const f64,
    n_in: usize,
    t_start: i64,
    t_end: i64,
    interval_ns: i64,
    val_out: *mut f64,
    n_out: usize,
) -> i32 {
    if ts_in.is_null() || val_in.is_null() || val_out.is_null() {
        return CP_ERR_NULL;
    }
    let result = catch_unwind(|| {
        let ts = slice::from_raw_parts(ts_in, n_in);
        let val = slice::from_raw_parts(val_in, n_in);
        crate::interpolate(ts, val, t_start, t_end, interval_ns)
    });
    match result {
        Err(_) => CP_ERR_PANIC,
        Ok(Err(e)) => to_code(&e),
        Ok(Ok(out)) => {
            if n_out < out.len() {
                return CP_ERR_BUFFER_TOO_SMALL;
            }
            std::ptr::copy_nonoverlapping(out.as_ptr(), val_out, out.len());
            CP_OK
        }
    }
}

/// Return a static C string describing the error code.
#[no_mangle]
pub extern "C" fn cp_strerror(err: i32) -> *const c_char {
    let s = match err {
        CP_OK => "ok\0",
        CP_ERR_BAD_INPUT => "bad input\0",
        CP_ERR_BUFFER_TOO_SMALL => "buffer too small\0",
        CP_ERR_CORRUPT => "corrupt stream\0",
        CP_ERR_PANIC => "internal panic\0",
        CP_ERR_NULL => "null pointer\0",
        _ => "unknown error\0",
    };
    s.as_ptr() as *const c_char
}

/// Return the library version string (null-terminated).
#[no_mangle]
pub extern "C" fn cp_version() -> *const c_char {
    // SAFETY: CARGO_PKG_VERSION is always ASCII + null-terminated after the concat.
    static VERSION: std::sync::OnceLock<std::ffi::CString> = std::sync::OnceLock::new();
    VERSION.get_or_init(|| {
        std::ffi::CString::new(crate::version()).unwrap()
    }).as_ptr()
}
