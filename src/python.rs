/// PyO3 Python binding for curvepress.
///
/// Built with maturin. No C ABI involved — PyO3 talks to CPython directly.
/// See `pyproject.toml` for the build configuration.
use pyo3::prelude::*;
use numpy::{PyArray1, PyReadonlyArray1, IntoPyArray};

fn stats_to_dict(py: Python<'_>, s: &crate::Stats) -> PyResult<pyo3::types::PyObject> {
    let dict = pyo3::types::PyDict::new(py);
    dict.set_item("n_input", s.n_input)?;
    dict.set_item("n_kept", s.n_kept)?;
    dict.set_item("bytes_raw", s.bytes_raw)?;
    dict.set_item("bytes_compressed", s.bytes_compressed)?;
    dict.set_item("ratio", s.ratio)?;
    dict.set_item("max_error", s.max_error)?;
    dict.set_item("quant_bits", s.quant_bits)?;
    Ok(dict.into())
}

/// Compress with Ramer-Douglas-Peucker.
///
/// Parameters
/// ----------
/// timestamps : np.ndarray[int64]   Strictly increasing nanosecond timestamps.
/// values     : np.ndarray[float64] Finite (no NaN / Inf).
/// epsilon    : float               Max abs error in the value domain.
#[pyfunction]
#[pyo3(signature = (timestamps, values, epsilon))]
fn compress_rdp<'py>(
    py: Python<'py>,
    timestamps: PyReadonlyArray1<i64>,
    values: PyReadonlyArray1<f64>,
    epsilon: f64,
) -> PyResult<Py<pyo3::types::PyBytes>> {
    let out = crate::compress_rdp(timestamps.as_slice()?, values.as_slice()?, epsilon)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    Ok(pyo3::types::PyBytes::new(py, &out).into())
}

/// Compress with RDP and return (bytes, stats_dict).
#[pyfunction]
#[pyo3(signature = (timestamps, values, epsilon))]
fn compress_rdp_stats<'py>(
    py: Python<'py>,
    timestamps: PyReadonlyArray1<i64>,
    values: PyReadonlyArray1<f64>,
    epsilon: f64,
) -> PyResult<(Py<pyo3::types::PyBytes>, pyo3::types::PyObject)> {
    let (out, stats) = crate::compress_rdp_stats(timestamps.as_slice()?, values.as_slice()?, epsilon)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    Ok((pyo3::types::PyBytes::new(py, &out).into(), stats_to_dict(py, &stats)?))
}

/// Compress with Visvalingam-Whyatt.
///
/// Parameters
/// ----------
/// timestamps : np.ndarray[int64]   Strictly increasing nanosecond timestamps.
/// values     : np.ndarray[float64] Finite (no NaN / Inf).
/// n_out      : int                 Exact number of kept points.
#[pyfunction]
#[pyo3(signature = (timestamps, values, n_out))]
fn compress_vw<'py>(
    py: Python<'py>,
    timestamps: PyReadonlyArray1<i64>,
    values: PyReadonlyArray1<f64>,
    n_out: usize,
) -> PyResult<Py<pyo3::types::PyBytes>> {
    let out = crate::compress_vw(timestamps.as_slice()?, values.as_slice()?, n_out)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    Ok(pyo3::types::PyBytes::new(py, &out).into())
}

/// Compress with VW and return (bytes, stats_dict).
#[pyfunction]
#[pyo3(signature = (timestamps, values, n_out))]
fn compress_vw_stats<'py>(
    py: Python<'py>,
    timestamps: PyReadonlyArray1<i64>,
    values: PyReadonlyArray1<f64>,
    n_out: usize,
) -> PyResult<(Py<pyo3::types::PyBytes>, pyo3::types::PyObject)> {
    let (out, stats) = crate::compress_vw_stats(timestamps.as_slice()?, values.as_slice()?, n_out)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    Ok((pyo3::types::PyBytes::new(py, &out).into(), stats_to_dict(py, &stats)?))
}

/// Compress with RDP-N (binary-searched epsilon to hit `n_out` points).
///
/// Parameters
/// ----------
/// timestamps : np.ndarray[int64]
/// values     : np.ndarray[float64]
/// n_out      : int                 Target point count.
/// epsilon    : float               Upper bound for the RDP search.
#[pyfunction]
#[pyo3(signature = (timestamps, values, n_out, epsilon))]
fn compress_rdpn<'py>(
    py: Python<'py>,
    timestamps: PyReadonlyArray1<i64>,
    values: PyReadonlyArray1<f64>,
    n_out: usize,
    epsilon: f64,
) -> PyResult<Py<pyo3::types::PyBytes>> {
    let out = crate::compress_rdpn(timestamps.as_slice()?, values.as_slice()?, n_out, epsilon)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    Ok(pyo3::types::PyBytes::new(py, &out).into())
}

/// Compress with RDP-N and return (bytes, stats_dict).
#[pyfunction]
#[pyo3(signature = (timestamps, values, n_out, epsilon))]
fn compress_rdpn_stats<'py>(
    py: Python<'py>,
    timestamps: PyReadonlyArray1<i64>,
    values: PyReadonlyArray1<f64>,
    n_out: usize,
    epsilon: f64,
) -> PyResult<(Py<pyo3::types::PyBytes>, pyo3::types::PyObject)> {
    let (out, stats) = crate::compress_rdpn_stats(timestamps.as_slice()?, values.as_slice()?, n_out, epsilon)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    Ok((pyo3::types::PyBytes::new(py, &out).into(), stats_to_dict(py, &stats)?))
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

/// Reconstruct the value at a single timestamp `t` from the support points.
///
/// Parameters
/// ----------
/// timestamps : np.ndarray[int64]   Kept timestamps (from decompress).
/// values     : np.ndarray[float64] Kept values (from decompress).
/// t          : int                 Query timestamp (nanoseconds).
///
/// Returns
/// -------
/// float  Linearly interpolated value. Clamped at data boundaries.
#[pyfunction]
fn interpolate(
    timestamps: PyReadonlyArray1<i64>,
    values: PyReadonlyArray1<f64>,
    t: i64,
) -> PyResult<f64> {
    crate::interpolate(timestamps.as_slice()?, values.as_slice()?, t)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Return the library version string.
#[pyfunction]
fn version() -> &'static str {
    crate::version()
}

#[pymodule]
fn _curvepress(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(compress_rdp, m)?)?;
    m.add_function(wrap_pyfunction!(compress_rdp_stats, m)?)?;
    m.add_function(wrap_pyfunction!(compress_vw, m)?)?;
    m.add_function(wrap_pyfunction!(compress_vw_stats, m)?)?;
    m.add_function(wrap_pyfunction!(compress_rdpn, m)?)?;
    m.add_function(wrap_pyfunction!(compress_rdpn_stats, m)?)?;
    m.add_function(wrap_pyfunction!(decompress, m)?)?;
    m.add_function(wrap_pyfunction!(interpolate, m)?)?;
    m.add_function(wrap_pyfunction!(version, m)?)?;
    Ok(())
}
