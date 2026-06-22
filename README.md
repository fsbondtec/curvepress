[![Release (PyPI)](https://github.com/fsbondtec/curvepress/actions/workflows/release.yml/badge.svg)](https://github.com/fsbondtec/curvepress/actions/workflows/release.yml)

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
