"""Python test suite for curvepress.

Run with: pytest tests/python/test_python.py -v
Requires the wheel to be installed: maturin develop --features python
"""
import time

import numpy as np
import pytest

from curvepress import compress, compress_stats, decompress, interpolate, version


# ─── helpers ─────────────────────────────────────────────────────────────────

def sine_series(n: int = 1000):
    ts = np.arange(n, dtype=np.int64) * 1_000_000
    val = np.sin(np.arange(n, dtype=np.float64) * 0.05) * 100.0
    return ts, val


def fracture_series(n: int = 500):
    ts = np.arange(n, dtype=np.int64) * 1_000_000
    val = np.zeros(n, dtype=np.float64)
    val[:300] = np.arange(300) / 3.0
    val[300] = 150.0
    for i in range(301, min(310, n)):
        val[i] = max(0.0, 100.0 - (i - 300) * 11.0)
    for i in range(310, n):
        val[i] = 0.5 * abs(np.sin(i * 0.3))
    return ts, val


# ─── basic round-trip ────────────────────────────────────────────────────────

def test_roundtrip_rdp():
    ts, val = sine_series(2000)
    data = compress(ts, val, epsilon=0.5)
    ts_out, val_out = decompress(data)
    assert len(ts_out) < len(ts), "RDP should reduce point count"
    assert len(ts_out) == len(val_out)


def test_roundtrip_vw():
    ts, val = sine_series(500)
    data = compress(ts, val, algo=1, n_out=40)
    ts_out, val_out = decompress(data)
    assert len(ts_out) == 40, f"VW must return exactly n_out; got {len(ts_out)}"


def test_roundtrip_rdp_n():
    ts, val = sine_series(1000)
    data = compress(ts, val, algo=2, n_out=60)
    ts_out, _ = decompress(data)
    assert len(ts_out) <= 60, f"RDP-n must return at most n_out; got {len(ts_out)}"
    assert len(ts_out) >= 2


# ─── fracture curve ──────────────────────────────────────────────────────────

def test_fracture_curve():
    ts, val = fracture_series()
    data = compress(ts, val, epsilon=1.0)
    ts_out, _ = decompress(data)
    assert ts[300] in ts_out, "Peak point must be kept"
    assert ts[301] in ts_out, "First post-drop point must be kept"


# ─── axis normalisation ──────────────────────────────────────────────────────

def test_axis_normalization():
    n = 200
    ts = np.arange(n, dtype=np.int64) * 1_000_000_000  # 1 s apart in ns
    val = (np.sin(np.arange(n) * 0.15) * 250.0 + 250.0).astype(np.float64)

    data_no  = compress(ts, val, epsilon=10.0, normalize_axes=False, value_range=500.0)
    data_yes = compress(ts, val, epsilon=10.0, normalize_axes=True,  value_range=500.0)
    ts_no,  _ = decompress(data_no)
    ts_yes, _ = decompress(data_yes)
    assert not np.array_equal(ts_no, ts_yes), "Normalization should produce different kept points"


# ─── stats ───────────────────────────────────────────────────────────────────

def test_stats_max_error_bounded():
    ts, val = sine_series(2000)
    _, stats = compress_stats(ts, val, epsilon=2.0)
    assert stats["max_error"] <= 2.0 * 1.5 + 1e-9


def test_stats_quant_bits():
    n = 500
    ts = np.arange(n, dtype=np.int64) * 1_000_000
    val = np.arange(n, dtype=np.float64)  # range = 499
    epsilon = 499.0 / 1000.0
    _, stats = compress_stats(ts, val, epsilon=epsilon)
    assert stats["quant_bits"] == 10


# ─── error handling ──────────────────────────────────────────────────────────

def test_nan_raises():
    ts = np.array([0, 1_000_000, 2_000_000], dtype=np.int64)
    val = np.array([0.0, float("nan"), 2.0])
    with pytest.raises(ValueError):
        compress(ts, val)


def test_non_monotonic_ts_raises():
    ts = np.array([0, 2_000_000, 1_000_000], dtype=np.int64)
    val = np.array([0.0, 1.0, 2.0])
    with pytest.raises(ValueError):
        compress(ts, val)


# ─── interpolate ─────────────────────────────────────────────────────────────

def test_interpolate():
    ts = np.array([0, 10_000, 20_000, 30_000], dtype=np.int64)
    val = np.array([0.0, 10.0, 20.0, 30.0])
    out = interpolate(ts, val, 0, 30_000, 5_000)
    assert len(out) == 7
    expected = np.arange(7) * 5.0
    np.testing.assert_allclose(out, expected, atol=1e-9)


# ─── large series performance ─────────────────────────────────────────────────

def test_large_series_under_500ms():
    n = 1_000_000
    ts = np.arange(n, dtype=np.int64) * 100_000
    val = np.sin(np.arange(n) * 0.001).astype(np.float64) * 100.0
    t0 = time.perf_counter()
    compress(ts, val, epsilon=0.5)
    elapsed = time.perf_counter() - t0
    assert elapsed < 0.5, f"1M-point compression took {elapsed:.3f}s (limit 0.5s)"


# ─── version ─────────────────────────────────────────────────────────────────

def test_version():
    v = version()
    assert isinstance(v, str)
    assert v.startswith("0.")
