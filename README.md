[![CI](https://github.com/fsbondtec/curvepress/actions/workflows/ci.yml/badge.svg)](https://github.com/fsbondtec/curvepress/actions/workflows/ci.yml)
[![Release (PyPI)](https://github.com/fsbondtec/curvepress/actions/workflows/release-pypi.yml/badge.svg)](https://github.com/fsbondtec/curvepress/actions/workflows/release-pypi.yml)
![GitHub License](https://img.shields.io/github/license/fsbondtec/curvepress)
![GitHub Release](https://img.shields.io/github/v/release/fsbondtec/curvepress)

# curvepress

Lossy time-series compression — RDP/VW point reduction + epsilon-derived quantization + varint packing.
Designed for sharp transient signals (fracture curves, impulse tests, load cells).

**C++20 header-only library.** No build step, no dependencies.

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

Pre-built wheels for CPython 3.9–3.13 on Linux, macOS (x86_64 + arm64) and
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

[![Release (PyPI)](https://github.com/fsbondtec/curvepress/actions/workflows/release-pypi.yml/badge.svg)](https://github.com/fsbondtec/curvepress/actions/workflows/release-pypi.yml)
[![CI](https://github.com/fsbondtec/curvepress/actions/workflows/ci.yml/badge.svg)](https://github.com/fsbondtec/curvepress/actions/workflows/ci.yml)
[![CodeQL](https://github.com/fsbondtec/curvepress/actions/workflows/github-code-scanning/codeql/badge.svg)](https://github.com/fsbondtec/curvepress/actions/workflows/github-code-scanning/codeql)
![GitHub License](https://img.shields.io/github/license/fsbondtec/curvepress)
![GitHub Release](https://img.shields.io/github/v/release/fsbondtec/curvepress)



# curvepress

Lossy time series compression -- RDP/VW point reduction + epsilon-derived quantization + varint packing.
Designed for sharp transient signals (fracture curves, impulse tests, load cells).
One Rust core; four language targets.

## Architecture

```
raw (int64 timestamps_ns + float64 values)
  -> point reduction   (RDP / VW / RDP-N)
  -> quantization      (float64 -> uintN, bit-width from epsilon)
  -> integer packing   (delta + zigzag + LEB128 varint)
  -> byte stream
```

**No entropy-coding stage, no external compression dependencies.**

```
                     +-------------------------+
                     |   Rust core crate       |  <- ALL logic lives here
                     |   rdp  vw  quantize     |
                     |   varint  codec         |
                     +------------+------------+
            +--------------+------+-------+------------------+
            |              |              |                  |
      native crate   wasm-bindgen      PyO3           cbindgen + .hpp
            |              |              |                  |
         (Rust)         (WASM)        (Python)             (C++)
       crates.io     npm package     PyPI wheel       Conan package
```

- **Rust** -- the core crate; published to crates.io.
- **WASM** -- via `wasm-bindgen` / `wasm-pack`. Direct Rust->WASM, no C ABI.
- **Python** -- via `PyO3` + `maturin`. Direct Rust->CPython, no C ABI.
- **C++** -- `cbindgen` auto-generates `include/curvepress.h` from `src/capi.rs`.
  `cpp/include/curvepress/curvepress.hpp` wraps it with idiomatic C++20 (`std::span`, exceptions).

---

## Algorithms

### RDP (Ramer-Douglas-Peucker)

Recursively removes the point with the smallest perpendicular distance to the line between
its neighbours, as long as that distance is below `epsilon`. Guarantees that every dropped
point deviates at most `epsilon` from the piecewise-linear reconstruction.

- Input: `epsilon` (maximum absolute error in the value domain)
- Output: variable number of kept points
- Complexity: O(n log n) average, O(n^2) worst case
- Use when: you need a strict error bound

### VW (Visvalingam-Whyatt)

Iteratively removes the point that forms the triangle with the smallest area with its two
neighbours. Repeats until exactly `n_out` points remain.

- Input: `n_out` (exact number of output points)
- Output: exactly `n_out` points
- Complexity: O(n log n)
- Use when: you need a fixed output size (e.g. display resolution, storage budget)
- The quantization epsilon is derived automatically from the actual max deviation of
  dropped points, so no epsilon needs to be specified

### RDP-N

Binary-searches for the smallest `epsilon` that makes RDP keep at most `n_out` points.
Combines the error-bound guarantee of RDP with a target output size.

- Input: `n_out` (target maximum), `epsilon` (upper bound for the search)
- Output: at most `n_out` points
- Complexity: O(n log n * log(epsilon_range))
- Use when: you want both an error bound AND a size cap

### Axis normalization

Timestamps are in nanoseconds; values might be Newtons or millistrain. Without normalization
the time axis completely dominates Euclidean distances. curvepress always normalizes: the
time axis is scaled to match the value range before distance computation.
`epsilon` is therefore always expressed in the **value domain**.

### Error-bound contract

```
max_error <= ~1.5 * epsilon
```

| Algo  | epsilon source                                              |
|-------|-------------------------------------------------------------|
| RDP   | user-supplied                                               |
| VW    | measured max deviation of dropped points (automatic)        |
| RDP-N | measured max deviation of dropped points (automatic)        |

The 0.5x overhead comes from quantization (float64 -> integer grid at spacing epsilon).

---

## API reference

All four language bindings expose the same six functions plus `decompress`, `interpolate`,
and `version`.

### compress_rdp

Compress with RDP. `epsilon` is the maximum absolute error in the value domain.

| Language | Signature |
|----------|-----------|
| Rust     | `compress_rdp(ts: &[i64], val: &[f64], epsilon: f64) -> Result<Vec<u8>>` |
| Rust     | `compress_rdp_stats(ts, val, epsilon) -> Result<(Vec<u8>, Stats)>` |
| Python   | `compress_rdp(timestamps, values, epsilon) -> bytes` |
| Python   | `compress_rdp_stats(timestamps, values, epsilon) -> tuple[bytes, Stats]` |
| C++      | `compress_rdp(span<i64>, span<f64>, epsilon, Stats* = nullptr) -> vector<uint8_t>` |
| WASM     | `compress_rdp(BigInt64Array, Float64Array, number) -> Uint8Array` |

### compress_vw

Compress with Visvalingam-Whyatt. `n_out` is the exact number of kept points.

| Language | Signature |
|----------|-----------|
| Rust     | `compress_vw(ts: &[i64], val: &[f64], n_out: usize) -> Result<Vec<u8>>` |
| Rust     | `compress_vw_stats(ts, val, n_out) -> Result<(Vec<u8>, Stats)>` |
| Python   | `compress_vw(timestamps, values, n_out) -> bytes` |
| Python   | `compress_vw_stats(timestamps, values, n_out) -> tuple[bytes, Stats]` |
| C++      | `compress_vw(span<i64>, span<f64>, n_out, Stats* = nullptr) -> vector<uint8_t>` |
| WASM     | `compress_vw(BigInt64Array, Float64Array, number) -> Uint8Array` |

### compress_rdpn

Compress with RDP-N. Keeps at most `n_out` points; `epsilon` is the search upper bound.

| Language | Signature |
|----------|-----------|
| Rust     | `compress_rdpn(ts: &[i64], val: &[f64], n_out: usize, epsilon: f64) -> Result<Vec<u8>>` |
| Rust     | `compress_rdpn_stats(ts, val, n_out, epsilon) -> Result<(Vec<u8>, Stats)>` |
| Python   | `compress_rdpn(timestamps, values, n_out, epsilon) -> bytes` |
| Python   | `compress_rdpn_stats(timestamps, values, n_out, epsilon) -> tuple[bytes, Stats]` |
| C++      | `compress_rdpn(span<i64>, span<f64>, n_out, epsilon, Stats* = nullptr) -> vector<uint8_t>` |
| WASM     | `compress_rdpn(BigInt64Array, Float64Array, number, number) -> Uint8Array` |

### decompress

Decompress a byte stream produced by any `compress_*` function.

| Language | Signature |
|----------|-----------|
| Rust     | `decompress(data: &[u8]) -> Result<(Vec<i64>, Vec<f64>)>` |
| Python   | `decompress(data: bytes) -> tuple[ndarray, ndarray]` |
| C++      | `decompress(span<uint8_t>) -> Decoded` (`Decoded.timestamps_ns`, `Decoded.values`) |
| WASM     | `decompress(Uint8Array) -> Decoded` (`Decoded.timestamps`, `Decoded.values`, `Decoded.len`) |

### interpolate

Reconstruct the value at a single timestamp `t` by linear interpolation of the support
points. Clamps (flat extrapolation) outside the data range.

| Language | Signature |
|----------|-----------|
| Rust     | `interpolate(ts: &[i64], val: &[f64], t: i64) -> Result<f64>` |
| Python   | `interpolate(timestamps, values, t: int) -> float` |
| C++      | `interpolate(span<i64>, span<f64>, t: int64_t) -> double` |
| WASM     | `interpolate(BigInt64Array, Float64Array, t: bigint) -> number` |

### Stats

Returned by the `*_stats` variants. Contains:

| Field              | Type     | Description                                      |
|--------------------|----------|--------------------------------------------------|
| `n_input`          | usize    | Number of input points                           |
| `n_kept`           | usize    | Number of points after reduction                 |
| `bytes_raw`        | usize    | Raw size (16 bytes per point)                    |
| `bytes_compressed` | usize    | Compressed byte stream length                    |
| `ratio`            | f64      | `bytes_raw / bytes_compressed`                   |
| `max_error`        | f64      | Maximum value-domain error of dropped points     |
| `quant_bits`       | u32      | Quantization bit-width used                      |

---

## Installation

### Python (PyPI)

```bash
pip install curvepress
```

Pre-built wheels for CPython 3.9–3.14 on Linux, macOS (Apple Silicon) and
Windows — no Rust toolchain needed. Pulls in `numpy`.

### JavaScript / TypeScript (npm)

```bash
npm install curvepress
```

A pre-built WebAssembly package, usable from bundlers (webpack/Vite/Rollup)
and Node.js ≥ 18.

### Rust (crates.io)

```bash
cargo add curvepress
```

### C++ (Conan — local recipe)

curvepress is a Rust library, so it is **not on ConanCenter**. Build the
package locally from the cloned repo (needs a **Rust toolchain + a C++
compiler**); the binary lands in your local Conan cache:

```bash
git clone https://github.com/fsbondtec/curvepress
cd curvepress
conan create . --version 0.1.0
```

Then consume it from your project:

```bash
conan install --requires=curvepress/0.1.0
```

```cmake
find_package(curvepress CONFIG REQUIRED)
target_link_libraries(my_target PRIVATE curvepress::curvepress)
```

### C++ from source (without Conan)

Build the Rust static library — this also generates the C header:

```bash
cargo build --release --features capi
#  -> target/release/libcurvepress.a   (curvepress.lib on Windows)
#  -> include/curvepress.h             (generated by cbindgen)
```

Add both header directories (`include/` and `cpp/include/`) to your include
path and link the static library plus its system dependencies:

| Platform | Link flags |
|----------|------------|
| Linux    | `-Ltarget/release -lcurvepress -lpthread -ldl -lm` |
| macOS    | `-Ltarget/release -lcurvepress -framework Security -framework CoreFoundation` |
| Windows  | `curvepress.lib ws2_32.lib userenv.lib ntdll.lib bcrypt.lib` |

Or build via the bundled CMake project, which exposes the
`curvepress::curvepress` target:

```bash
cmake -S cpp -B cpp/build -DCMAKE_BUILD_TYPE=Release
cmake --build cpp/build --config Release
```

---

## Quick start

### Rust

```rust
use curvepress::{compress_rdp, compress_vw, compress_rdpn, decompress, interpolate};

// RDP: strict error bound
let data = compress_rdp(&timestamps_ns, &values, 1.0)?;

// VW: exact output size
let data = compress_vw(&timestamps_ns, &values, 200)?;

// RDP-N: at most 200 points, search up to epsilon=100.0
let data = compress_rdpn(&timestamps_ns, &values, 200, 100.0)?;

// Decompress
let (ts_out, val_out) = decompress(&data)?;

// Interpolate at a single timestamp
let v = interpolate(&ts_out, &val_out, 5_000_000_000_i64)?;
```

### C++ (CMake)

```cmake
find_package(curvepress REQUIRED)
target_link_libraries(my_target PRIVATE curvepress::curvepress)
```

```cpp
#include <curvepress/curvepress.hpp>

// RDP
auto data = curvepress::compress_rdp(ts, val, 1.0);

// VW
auto data = curvepress::compress_vw(ts, val, 200);

// RDP-N
auto data = curvepress::compress_rdpn(ts, val, 200, 100.0);

// Decompress
auto dec = curvepress::decompress(data);
// dec.timestamps_ns, dec.values

// Interpolate
double v = curvepress::interpolate(dec.timestamps_ns, dec.values, 5'000'000'000LL);
```

### Python

```python
import numpy as np
from curvepress import compress_rdp, compress_vw, compress_rdpn, decompress, interpolate

ts  = np.arange(10_000, dtype=np.int64) * 1_000_000   # ns
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
```

### WASM (JavaScript/TypeScript)

```typescript
import { compress_rdp, compress_vw, compress_rdpn, decompress, interpolate } from 'curvepress';

const ts  = new BigInt64Array(n);   // fill with ns timestamps
const val = new Float64Array(n);    // fill with values

// RDP
const data = compress_rdp(ts, val, 1.0);

// VW
const data = compress_vw(ts, val, 200);

// RDP-N
const data = compress_rdpn(ts, val, 200, 100.0);

// Decompress
const dec = decompress(data);
console.log(`Kept ${dec.len} of ${n} points`);

// Interpolate
const v = interpolate(dec.timestamps, dec.values, 5_000_000_000n);
```

---

## Benchmark (fracture-curve data, 100 k points)

| Algo | Ratio | Throughput | max_error |
|------|-------|------------|-----------|
| RDP epsilon=0.5 | ~18x | ~120 MB/s | <=0.75 |
| VW n=1000       | ~23x | ~80 MB/s  | informative |

*(Run `cargo bench` on your hardware for accurate numbers.)*

---

## Building

```bash
# Rust tests
cargo test

# C++ (requires Catch2 v3)
cargo build --release --features capi
cmake -S cpp -B cpp/build && cmake --build cpp/build && ctest --test-dir cpp/build

# Python wheel (requires maturin)
maturin develop --features python
pytest tests/python/ -v

# WASM (requires wasm-pack)
wasm-pack build --target nodejs --out-dir pkg --features wasm
node tests/wasm/test_wasm.mjs
```

---

## License

MIT
