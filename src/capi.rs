/// C ABI surface for the C++ binding.
///
/// These `extern "C"` functions are the ONLY C-ABI surface in curvepress.
/// `cbindgen` generates `include/curvepress.h` from them at build time.
/// The C++ consumer never calls this directly â€” it uses `cpp/include/curvepress/curvepress.hpp`.
///
/// Every function catches panics (via `std::panic::catch_unwind`) and translates
/// them to error codes, because panics crossing the FFI boundary are UB.
use std::os::raw::c_char;
use std::panic::catch_unwind;
use std::slice;

// â”€â”€â”€ Error codes â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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

// â”€â”€â”€ CpStats â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Compression statistics (optional output from compress functions).
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

fn write_stats(stats: *mut CpStats, s: &crate::Stats) {
    if stats.is_null() { return; }
    unsafe {
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
}

fn compress_to_buf(
    result: Result<(Vec<u8>, crate::Stats), crate::CpError>,
    out_buf: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
    stats: *mut CpStats,
) -> i32 {
    match result {
        Err(e) => to_code(&e),
        Ok((data, s)) => {
            unsafe { *out_len = data.len(); }
            write_stats(stats, &s);
            if out_buf.is_null() {
                return CP_OK; // dry run
            }
            if out_cap < data.len() {
                return CP_ERR_BUFFER_TOO_SMALL;
            }
            unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), out_buf, data.len()); }
            CP_OK
        }
    }
}

// â”€â”€â”€ Public C functions â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Compress with RDP. `epsilon` is the maximum absolute error in the value domain.
///
/// Pass `out_buf = NULL` for a dry run (writes required length to `*out_len`).
/// Returns 0 on success, negative error code otherwise.
///
/// # Safety
/// All non-null pointers must be valid and properly aligned for their types.
#[no_mangle]
pub unsafe extern "C" fn cp_compress_rdp(
    timestamps_ns: *const i64,
    values: *const f64,
    n: usize,
    epsilon: f64,
    out_buf: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
    stats: *mut CpStats,
) -> i32 {
    if timestamps_ns.is_null() || values.is_null() || out_len.is_null() {
        return CP_ERR_NULL;
    }
    let result = catch_unwind(|| {
        let ts = slice::from_raw_parts(timestamps_ns, n);
        let val = slice::from_raw_parts(values, n);
        crate::compress_rdp_stats(ts, val, epsilon)
    });
    match result {
        Err(_) => CP_ERR_PANIC,
        Ok(r) => compress_to_buf(r, out_buf, out_cap, out_len, stats),
    }
}

/// Compress with Visvalingam-Whyatt. `n_out` is the target number of kept points.
///
/// # Safety
/// All non-null pointers must be valid and properly aligned for their types.
#[no_mangle]
pub unsafe extern "C" fn cp_compress_vw(
    timestamps_ns: *const i64,
    values: *const f64,
    n: usize,
    n_out: usize,
    out_buf: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
    stats: *mut CpStats,
) -> i32 {
    if timestamps_ns.is_null() || values.is_null() || out_len.is_null() {
        return CP_ERR_NULL;
    }
    let result = catch_unwind(|| {
        let ts = slice::from_raw_parts(timestamps_ns, n);
        let val = slice::from_raw_parts(values, n);
        crate::compress_vw_stats(ts, val, n_out)
    });
    match result {
        Err(_) => CP_ERR_PANIC,
        Ok(r) => compress_to_buf(r, out_buf, out_cap, out_len, stats),
    }
}

/// Compress with RDP-N (binary-searched epsilon to hit `n_out` points).
/// `epsilon` is the upper bound for the RDP search.
///
/// # Safety
/// All non-null pointers must be valid and properly aligned for their types.
#[no_mangle]
pub unsafe extern "C" fn cp_compress_rdpn(
    timestamps_ns: *const i64,
    values: *const f64,
    n: usize,
    n_out: usize,
    epsilon: f64,
    out_buf: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
    stats: *mut CpStats,
) -> i32 {
    if timestamps_ns.is_null() || values.is_null() || out_len.is_null() {
        return CP_ERR_NULL;
    }
    let result = catch_unwind(|| {
        let ts = slice::from_raw_parts(timestamps_ns, n);
        let val = slice::from_raw_parts(values, n);
        crate::compress_rdpn_stats(ts, val, n_out, epsilon)
    });
    match result {
        Err(_) => CP_ERR_PANIC,
        Ok(r) => compress_to_buf(r, out_buf, out_cap, out_len, stats),
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

/// Interpolate a single value at timestamp `t` from the support points.
///
/// Writes the interpolated value to `*val_out`. Returns 0 on success.
///
/// # Safety
/// All pointers must be valid.
#[no_mangle]
pub unsafe extern "C" fn cp_interpolate(
    ts_in: *const i64,
    val_in: *const f64,
    n_in: usize,
    t: i64,
    val_out: *mut f64,
) -> i32 {
    if ts_in.is_null() || val_in.is_null() || val_out.is_null() {
        return CP_ERR_NULL;
    }
    let result = catch_unwind(|| {
        let ts = slice::from_raw_parts(ts_in, n_in);
        let val = slice::from_raw_parts(val_in, n_in);
        crate::interpolate(ts, val, t)
    });
    match result {
        Err(_) => CP_ERR_PANIC,
        Ok(Err(e)) => to_code(&e),
        Ok(Ok(v)) => {
            *val_out = v;
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
    static VERSION: std::sync::OnceLock<std::ffi::CString> = std::sync::OnceLock::new();
    VERSION.get_or_init(|| {
        std::ffi::CString::new(crate::version()).unwrap()
    }).as_ptr()
}

