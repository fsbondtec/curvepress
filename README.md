[![CI](https://github.com/fsbondtec/curvepress/actions/workflows/ci.yml/badge.svg)](https://github.com/fsbondtec/curvepress/actions/workflows/ci.yml)
[![Release (PyPI)](https://github.com/fsbondtec/curvepress/actions/workflows/release-pypi.yml/badge.svg)](https://github.com/fsbondtec/curvepress/actions/workflows/release-pypi.yml)
![GitHub License](https://img.shields.io/github/license/fsbondtec/curvepress)
![GitHub Release](https://img.shields.io/github/v/release/fsbondtec/curvepress)

# curvepress

Lossy time-series compression - RDP/VW point reduction + epsilon-derived quantization + varint packing.
Designed for sharp transient signals (fracture curves, impulse tests, load cells).

**C++20 header-only library.** No build step, no dependencies.

> **Rust / WASM** targets live in the companion repository
> [fsbondtec/curvepress-rust](https://github.com/fsbondtec/curvepress-rust).

## Architecture

```
raw (int64 timestamps_ns + float64 values)
  -> point reduction   (RDP / VW / RDP-N)
  -> quantization      (float64 -> uintN, bit-width from epsilon)
  -> integer packing   (delta + zigzag + LEB128 varint)
  -> byte stream
```

**No entropy-coding stage, no external dependencies.**

```
include/curvepress/
    curvepress.hpp          <- public API (C++20, header-only)
    detail/
        rdp.hpp             <- Ramer-Douglas-Peucker
        vw.hpp              <- Visvalingam-Whyatt
        radial.hpp          <- radial pre-filter
        quantize.hpp        <- float64 -> uintN
        varint.hpp          <- LEB-128 + zigzag
        codec.hpp           <- compress / decompress pipeline
```

---

## Algorithms

### RDP (Ramer-Douglas-Peucker)

Iteratively removes the point with the smallest perpendicular distance to the
line segment between its neighbours, as long as that distance is below `epsilon`.
Every dropped point deviates at most `epsilon` from the piecewise-linear reconstruction.

- Input: `epsilon` (maximum absolute error in the value domain)
- Output: variable number of kept points
- Complexity: O(n log n) average, O(n²) worst case
- Use when: you need a strict error bound

### VW (Visvalingam-Whyatt)

Iteratively removes the point that forms the triangle with the smallest effective area
with its two neighbours (Visvalingam 2016 effective-area variant). Repeats until
exactly `n_out` points remain.

- Input: `n_out` (exact number of output points)
- Output: exactly `n_out` points
- Complexity: O(n log n)
- Use when: you need a fixed output size (display resolution, storage budget)

### RDP-N

Binary-searches for the smallest `epsilon` that makes RDP keep at most `n_out` points.
Combines the error bound of RDP with a target output size.

- Input: `n_out` (maximum), `epsilon` (upper bound for the binary search)
- Output: at most `n_out` points
- Complexity: O(n log n · log(epsilon_range))
- Use when: you want both an error bound and a size cap

### Axis normalization

Timestamps are in nanoseconds; values may be Newtons, millistrain, etc.
Without normalization the time axis completely dominates Euclidean distances.
curvepress always normalizes: the time axis is scaled to match the value range
before distance computation. `epsilon` is therefore always expressed in the
**value domain**.

### Error-bound contract

```
max_error ≤ ~1.5 × epsilon
```

| Algo  | epsilon source                                              |
|-------|-------------------------------------------------------------|
| RDP   | user-supplied                                               |
| VW    | measured max deviation of dropped points (automatic)        |
| RDP-N | measured max deviation of dropped points (automatic)        |

The 0.5× overhead comes from quantization (float64 → integer grid at spacing epsilon).

---

## API reference

### compress_rdp

```cpp
// Without stats
std::vector<uint8_t> compress_rdp(
    std::span<const int64_t> timestamps_ns,
    std::span<const double>  values,
    double                   epsilon);

// With stats
std::vector<uint8_t> compress_rdp(
    std::span<const int64_t> timestamps_ns,
    std::span<const double>  values,
    double                   epsilon,
    Stats&                   stats);
```

### compress_vw

```cpp
std::vector<uint8_t> compress_vw(
    std::span<const int64_t> timestamps_ns,
    std::span<const double>  values,
    std::size_t              n_out,
    Stats&                   stats = ...); // optional overload
```

### compress_rdpn

```cpp
std::vector<uint8_t> compress_rdpn(
    std::span<const int64_t> timestamps_ns,
    std::span<const double>  values,
    std::size_t              n_out,
    double                   epsilon,
    Stats&                   stats = ...); // optional overload
```

### decompress

```cpp
Decoded decompress(std::span<const uint8_t> data);
// Decoded.timestamps_ns  std::vector<int64_t>
// Decoded.values         std::vector<double>
```

### interpolate

Linear interpolation at a single query timestamp. Clamps outside the data range.

```cpp
double interpolate(
    std::span<const int64_t> timestamps_ns,
    std::span<const double>  values,
    int64_t                  t);
```

### radial_filter

O(n) pre-filter: drops any point whose value deviation from the last kept point
is less than `radius`. Useful as a cheap noise remover before RDP.

```cpp
std::vector<bool> radial_filter(
    std::span<const int64_t> timestamps_ns,
    std::span<const double>  values,
    double                   radius);
```

### Stats

Returned by the `stats` overloads.

| Field              | Type         | Description                                  |
|--------------------|--------------|----------------------------------------------|
| `n_input`          | `size_t`     | Number of input points                       |
| `n_kept`           | `size_t`     | Points after reduction                       |
| `bytes_raw`        | `size_t`     | Raw size (16 bytes per point)                |
| `bytes_compressed` | `size_t`     | Compressed byte stream length                |
| `ratio`            | `double`     | `bytes_raw / bytes_compressed`               |
| `max_error`        | `double`     | Max value-domain error over all input points |
| `quant_bits`       | `int`        | Quantization bit-width used                  |

### Error handling

All functions throw on invalid input or corrupt data:

| Exception                | Cause                                           |
|--------------------------|-------------------------------------------------|
| `std::invalid_argument`  | NaN/Inf value, non-monotonic or duplicate timestamps, empty input |
| `std::runtime_error`     | Corrupt or unsupported compressed stream        |

---

## Installation

### C++ via Conan (recommended)

```bash
conan install --requires="curvepress/0.2.0"
```

```cmake
find_package(curvepress CONFIG REQUIRED)
target_link_libraries(my_target PRIVATE curvepress::curvepress)
```

### C++ from source (without Conan)

The library is header-only — copy or clone the `include/` directory and add it
to your include path:

```bash
git clone https://github.com/fsbondtec/curvepress
```

```cmake
# CMake
add_subdirectory(curvepress)          # exposes curvepress::curvepress
# or
target_include_directories(my_target PRIVATE curvepress/include)
```

Requires: C++20 compiler (MSVC 19.29+, GCC 11+, Clang 14+).

### Python (PyPI)

```bash
pip install curvepress
```

Pre-built wheels for CPython 3.9–3.13 on Linux (x86_64, aarch64), macOS (arm64) and
Windows x64. Pulls in `numpy`.

---

## Quick start

### C++

```cpp
#include <curvepress/curvepress.hpp>
#include <cstdint>
#include <vector>

std::vector<int64_t> ts  = { 0, 1'000'000, 2'000'000, /* ... */ };
std::vector<double>  val = { 0.0, 12.5, 11.9, /* ... */ };

// RDP — strict error bound
auto data = curvepress::compress_rdp(ts, val, /*epsilon=*/0.5);

// VW — exact output size
auto data = curvepress::compress_vw(ts, val, /*n_out=*/200);

// RDP-N — at most 200 points, search up to epsilon=100
auto data = curvepress::compress_rdpn(ts, val, /*n_out=*/200, /*epsilon=*/100.0);

// Decompress
auto dec = curvepress::decompress(data);
// dec.timestamps_ns  std::vector<int64_t>
// dec.values         std::vector<double>

// Interpolate at a single timestamp
double v = curvepress::interpolate(dec.timestamps_ns, dec.values, 1'500'000LL);

// With stats
curvepress::Stats stats;
auto data = curvepress::compress_rdp(ts, val, 0.5, stats);
// stats.n_kept, stats.ratio, stats.max_error, stats.quant_bits
```

### Python

```python
import numpy as np
from curvepress import compress_rdp, compress_vw, compress_rdpn, decompress, interpolate

ts  = np.arange(10_000, dtype=np.int64) * 1_000_000   # nanoseconds
val = np.sin(np.arange(10_000) * 0.01) * 100.0

# RDP
data = compress_rdp(ts, val, epsilon=0.5)

# VW
data = compress_vw(ts, val, n_out=200)

# RDP-N
data = compress_rdpn(ts, val, n_out=200, epsilon=100.0)

# Decompress
ts_out, val_out = decompress(data)
print(f"Kept {len(ts_out)} of {len(ts)} points")

# Interpolate
v = interpolate(ts_out, val_out, t=5_000_000)

# With stats
data, stats = compress_rdp_stats(ts, val, epsilon=0.5)
print(stats)  # dict: n_input, n_kept, bytes_raw, bytes_compressed, ratio, max_error, quant_bits
```

---

## Building & testing

### C++ tests (GTest via Conan)

```powershell
# 1. Get GTest into the local Conan cache
conan install --requires="gtest/1.16.0" --build=missing `
    -g CMakeDeps -g CMakeToolchain --output-folder C:\path\to\deps

# 2. Configure
cmake -S . -B build -DCURVEPRESS_BUILD_TESTS=ON `
    -DCMAKE_PREFIX_PATH="C:\path\to\deps" -DCMAKE_BUILD_TYPE=Release

# 3. Build
cmake --build build --config Release

# 4. Run
ctest --test-dir build -C Release --output-on-failure
```

### Python wheel (development install)

```bash
pip install scikit-build-core pybind11 numpy
pip install --no-build-isolation -e .
pytest tests/python/ -v
```

### Conan package + test_package

```bash
conan create . --build=missing
```

---

## Benchmark (fracture-curve data, 100 k points)

| Algo            | Ratio | max_error |
|-----------------|-------|-----------|
| RDP epsilon=0.5 | ~18×  | ≤ 0.75    |
| VW n=1000       | ~23×  | informative |

---

## License

MIT
