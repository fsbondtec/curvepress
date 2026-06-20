/// PyO3 Python binding — exposes the full `Config` surface.
///
/// Built with maturin. No C ABI involved — PyO3 talks to CPython directly.
/// See `pyproject.toml` for the build configuration.
use pyo3::prelude::*;
use numpy::{PyArray1, PyReadonlyArray1, IntoPyArray};
use crate::{Algo, Config, TsMode};

fn algo_from_u32(a: u32) -> Algo {
    match a { 1 => Algo::Vw, 2 => Algo::RdpN, _ => Algo::Rdp }
}
fn ts_mode_from_u32(m: u32) -> TsMode {
    if m == 1 { TsMode::Regular } else { TsMode::Irregular }
}

/// Compress (timestamps_ns: int64 array, values: float64 array) → bytes.
///
/// Parameters
/// ----------
/// timestamps : np.ndarray[int64]   Strictly increasing nanosecond timestamps.
/// values     : np.ndarray[float64] Finite (no NaN / Inf).
/// epsilon    : float               Max abs error (RDP / RDP-N). Default 1.0.
/// algo       : int                 0=RDP, 1=VW, 2=RDP-N. Default 0.
/// n_out      : int                 Target point count (VW / RDP-N). Default 100.
/// normalize_axes : bool            Scale time axis before distances. Default False.
/// value_range    : float           Override for normalization/RDP-N bound (0=auto).
/// ts_mode    : int                 0=Irregular, 1=Regular. Default 0.
/// radial_prefilter : float | None  Radial distance pre-filter radius. Default None.
#[pyfunction]
#[pyo3(signature = (
    timestamps, values,
    epsilon=1.0, algo=0, n_out=100,
    normalize_axes=false, value_range=0.0,
    ts_mode=0, radial_prefilter=None
))]
fn compress<'py>(
    py: Python<'py>,
    timestamps: PyReadonlyArray1<i64>,
    values: PyReadonlyArray1<f64>,
    epsilon: f64,
    algo: u32,
    n_out: usize,
    normalize_axes: bool,
    value_range: f64,
    ts_mode: u32,
    radial_prefilter: Option<f64>,
) -> PyResult<Py<pyo3::types::PyBytes>> {
    let cfg = Config {
        algo: algo_from_u32(algo),
        ts_mode: ts_mode_from_u32(ts_mode),
        epsilon,
        n_out,
        radial_prefilter,
        normalize_axes,
        value_range,
    };
    let out = crate::compress(timestamps.as_slice()?, values.as_slice()?, &cfg)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    Ok(pyo3::types::PyBytes::new(py, &out).into())
}

/// Compress and return (bytes, stats_dict).
#[pyfunction]
#[pyo3(signature = (
    timestamps, values,
    epsilon=1.0, algo=0, n_out=100,
    normalize_axes=false, value_range=0.0,
    ts_mode=0, radial_prefilter=None
))]
fn compress_stats<'py>(
    py: Python<'py>,
    timestamps: PyReadonlyArray1<i64>,
    values: PyReadonlyArray1<f64>,
    epsilon: f64,
    algo: u32,
    n_out: usize,
    normalize_axes: bool,
    value_range: f64,
    ts_mode: u32,
    radial_prefilter: Option<f64>,
) -> PyResult<(Py<pyo3::types::PyBytes>, pyo3::types::PyObject)> {
    let cfg = Config {
        algo: algo_from_u32(algo),
        ts_mode: ts_mode_from_u32(ts_mode),
        epsilon,
        n_out,
        radial_prefilter,
        normalize_axes,
        value_range,
    };
    let (out, stats) = crate::compress_with_stats(timestamps.as_slice()?, values.as_slice()?, &cfg)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

    let bytes = pyo3::types::PyBytes::new(py, &out).into();
    let dict = pyo3::types::PyDict::new(py);
    dict.set_item("n_input", stats.n_input)?;
    dict.set_item("n_kept", stats.n_kept)?;
    dict.set_item("bytes_raw", stats.bytes_raw)?;
    dict.set_item("bytes_compressed", stats.bytes_compressed)?;
    dict.set_item("ratio", stats.ratio)?;
    dict.set_item("max_error", stats.max_error)?;
    dict.set_item("quant_bits", stats.quant_bits)?;
    Ok((bytes, dict.into()))
}

/// Decompress bytes → (timestamps: int64 array, values: float64 array).
#[pyfunction]
fn decompress<'py>(
    py: Python<'py>,
    data: &[u8],
) -> PyResult<(Py<PyArray1<i64>>, Py<PyArray1<f64>>)> {
    let (ts, val) = crate::decompress(data)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    Ok((ts.into_pyarray(py).into(), val.into_pyarray(py).into()))
}

/// Interpolate kept points onto a regular grid.
///
/// Returns a float64 array of length floor((t_end - t_start) / interval_ns) + 1.
/// Points outside the data range are clamped to the nearest endpoint value.
#[pyfunction]
fn interpolate<'py>(
    py: Python<'py>,
    timestamps: PyReadonlyArray1<i64>,
    values: PyReadonlyArray1<f64>,
    t_start: i64,
    t_end: i64,
    interval_ns: i64,
) -> PyResult<Py<PyArray1<f64>>> {
    let out = crate::interpolate(
        timestamps.as_slice()?,
        values.as_slice()?,
        t_start, t_end, interval_ns,
    ).map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    Ok(out.into_pyarray(py).into())
}

/// Return the library version string.
#[pyfunction]
fn version() -> &'static str {
    crate::version()
}

#[pymodule]
fn _curvepress(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(compress, m)?)?;
    m.add_function(wrap_pyfunction!(compress_stats, m)?)?;
    m.add_function(wrap_pyfunction!(decompress, m)?)?;
    m.add_function(wrap_pyfunction!(interpolate, m)?)?;
    m.add_function(wrap_pyfunction!(version, m)?)?;
    Ok(())
}
