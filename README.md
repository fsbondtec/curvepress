# curvepress

Lossy time series compression — RDP/VW point reduction + ε-derived quantization + varint packing.
Designed for sharp transient signals (fracture curves, impulse tests, load cells).
One Rust core; four language targets.

## Architecture

```
raw (int64 timestamps + float64 values)
  → point reduction   (RDP / VW / RDP-n)
  → quantization      (float64 → uintN, bit-width from ε)
  → integer packing   (delta + zigzag + LEB128 varint)
  → byte stream
```

**No entropy-coding stage, no Zstd, no external compression dependencies.**
Point reduction + quantization carry the bulk of the ratio for transient signals;
a general entropy coder would add ≤10–20% while dragging in a large dependency
that blocks Embedded / WASM / strict-supply-chain use.

```
                     ┌─────────────────────────┐
                     │   Rust core crate        │  ← ALL logic lives here
                     │   rdp  vw  quantize      │
                     │   varint  codec          │
                     └────────────┬────────────┘
            ┌──────────────┬──────┴───────┬──────────────┐
            │              │              │              │
      native crate   wasm-bindgen      PyO3        cbindgen + .hpp
            │              │              │              │
         (Rust)         (WASM)        (Python)         (C++)
       crates.io     npm package     PyPI wheel    Conan package
```

- **Rust** — the core crate; published to crates.io. Source of truth for all algorithms.
- **WASM** — via `wasm-bindgen` / `wasm-pack`. Direct Rust→WASM, no C ABI.
- **Python** — via `PyO3` + `maturin`. Direct Rust→CPython, no C ABI.
- **C++** — `cbindgen` auto-generates `include/curvepress.h` from the thin `extern "C"` surface
  in `src/capi.rs`. A hand-written `cpp/include/curvepress/curvepress.hpp` sits on top to give
  idiomatic C++20 (RAII, `std::span`, exceptions). The C++ consumer never sees raw pointers.

**Single source of truth:** every algorithm is implemented exactly once, in Rust. A bug fixed
in `rdp.rs` is fixed for all four targets simultaneously.

---

## Gap analysis

| Library | Algo | ε→quant | Axis norm | Pkged |
|---|---|---|---|---|
| **curvepress** | RDP + VW + RDP-n | ✓ | ✓ | Rust / C++ / Python / WASM |
| `simplification` (Rust) | RDP | ✗ | ✗ | crates.io |
| `pybind11-rdp` / `fastrdp` | RDP | ✗ | ✗ | PyPI only |
| `psimpl` (C++) | many | ✗ | ✗ | unmaintained, no pkg |
| `geo` (Rust, GIS) | RDP | ✗ | ✗ | crates.io |

None of these target time series with automatic quantization, axis normalization,
or a four-language distribution from one core.

---

## Algorithm guide

| Need | Use |
|---|---|
| Specific absolute error bound ε | **RDP** (`algo=Rdp`, set `epsilon`) |
| Specific output point count | **VW** (`algo=Vw`, set `n_out`) — O(n log n), exact count |
| Error bound AND point count | **RDP-n** (`algo=RdpN`, set both) — binary-searches epsilon |
| Very noisy input, RDP wasting budget | Set `radial_prefilter` to drop sub-noise points first |

---

## Axis normalization

Timestamps are in nanoseconds; values might be Newtons or millistrain. Without normalization,
the ns-scale time axis completely dominates Euclidean distances and RDP/VW remove the wrong
points. Set `normalize_axes = true` and provide `value_range` (or leave it `0.0` to auto-detect)
so the time axis is scaled to match the value range before distance computation.

**Note:** when `normalize_axes = true`, `epsilon` is a Euclidean (time+value) tolerance;
the value-domain `max_error ≤ 1.5 × epsilon` contract is no longer guaranteed.

---

## Error-bound contract

When `normalize_axes = false`:

```
max_error ≤ ~1.5 × epsilon
```

- RDP guarantees: each dropped point deviates ≤ `epsilon` from the linear interpolant of
  its two kept neighbours.
- Quantization adds ≤ `epsilon/2` on top.

For a strict `epsilon` bound: set `epsilon/2` in the config and accept the halved
compression ratio.

---

## Quick start

### Rust

```rust
use curvepress::{compress, decompress, Algo, Config};

let cfg = Config { algo: Algo::Rdp, epsilon: 1.0, ..Default::default() };
let data = compress(&timestamps_ns, &values, &cfg)?;
let (ts_out, val_out) = decompress(&data)?;
```

### C++ (CMake)

```cmake
find_package(curvepress REQUIRED)
target_link_libraries(my_target PRIVATE curvepress::curvepress)
```

```cpp
#include <curvepress/curvepress.hpp>

curvepress::Config cfg;
cfg.epsilon = 1.0;
auto data = curvepress::compress(ts, val, cfg);
auto dec  = curvepress::decompress(data);
```

### Python

```python
import numpy as np
from curvepress import compress, decompress

ts  = np.arange(10_000, dtype=np.int64) * 1_000_000   # ns
val = np.sin(np.arange(10_000) * 0.01) * 100.0

data = compress(ts, val, epsilon=0.5)
ts_out, val_out = decompress(data)
print(f"Kept {len(ts_out)} of {len(ts)} points")
```

### WASM (JavaScript/TypeScript)

```typescript
import { compress, decompress } from 'curvepress';

const ts  = new BigInt64Array(n);   // fill with ns timestamps
const val = new Float64Array(n);    // fill with values

const data = compress(ts, val, 1.0);
const dec  = decompress(data);
console.log(`Kept ${dec.len} of ${n} points`);
```

---

## Benchmark (fracture-curve data, 100 k points)

| Algo | Ratio | Throughput | max_error |
|------|-------|------------|-----------|
| RDP ε=0.5 | ~18× | ~120 MB/s | ≤0.75 |
| VW n=1000 | ~23× | ~80 MB/s | informative |

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
