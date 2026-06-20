// Node.js WASM test harness for curvepress.
// Run: node tests/wasm/test_wasm.mjs
// Requires: wasm-pack build --target nodejs --features wasm
//           (output in pkg/)

import { compress_rdp, compress_vw, compress_rdpn, decompress, interpolate, version } from '../../pkg/curvepress.js';

let passed = 0;
let failed = 0;

function assert(condition, message) {
    if (condition) {
        console.log(`  ✓ ${message}`);
        passed++;
    } else {
        console.error(`  ✗ FAIL: ${message}`);
        failed++;
    }
}

function assertClose(a, b, tol, message) {
    assert(Math.abs(a - b) <= tol, `${message} (${a} ≈ ${b}, tol=${tol})`);
}

// ─── helpers ──────────────────────────────────────────────────────────────────

function makeSine(n) {
    const ts  = new BigInt64Array(n);
    const val = new Float64Array(n);
    for (let i = 0; i < n; i++) {
        ts[i]  = BigInt(i) * 1_000_000n;
        val[i] = Math.sin(i * 0.05) * 100.0;
    }
    return [ts, val];
}

function makeLine(n) {
    const ts  = new BigInt64Array(n);
    const val = new Float64Array(n);
    for (let i = 0; i < n; i++) {
        ts[i]  = BigInt(i) * 1_000_000n;
        val[i] = i;
    }
    return [ts, val];
}

// ─── tests ────────────────────────────────────────────────────────────────────

console.log('curvepress WASM test suite');
console.log(`  version: ${version()}`);

// Round-trip RDP
{
    const [ts, val] = makeSine(500);
    const data = compress_rdp(ts, val, 1.0);
    const dec  = decompress(data);
    assert(dec.len < 500, `RDP reduces point count: ${dec.len} < 500`);
    assert(dec.timestamps.length === dec.values.length, 'ts and val same length');
    assert(version().startsWith('0.'), 'version starts with 0.');
}

// Round-trip VW — exact n_out
{
    const [ts, val] = makeSine(300);
    const data = compress_vw(ts, val, 30);
    const dec  = decompress(data);
    assert(dec.len === 30, `VW returns exactly n_out=30: got ${dec.len}`);
}

// Round-trip RDP-N — at most n_out
{
    const [ts, val] = makeSine(500);
    const data = compress_rdpn(ts, val, 50, 100.0);
    const dec  = decompress(data);
    assert(dec.len <= 50, `RDP-N returns at most n_out=50: got ${dec.len}`);
}

// Decompressed values within epsilon of original at kept timestamps
{
    const [ts, val] = makeLine(100);
    const epsilon = 1.0;
    const data = compress_rdp(ts, val, epsilon);
    const dec  = decompress(data);
    const kts  = dec.timestamps;
    const kval = dec.values;
    const map  = new Map();
    for (let i = 0; i < kts.length; i++) map.set(kts[i].toString(), kval[i]);
    let maxErr = 0;
    for (let i = 0; i < ts.length; i++) {
        const tsKey = ts[i].toString();
        if (map.has(tsKey)) continue;
        let j = 0;
        while (j + 1 < kts.length - 1 && kts[j + 1] <= ts[i]) j++;
        const frac = Number(ts[i] - kts[j]) / Number(kts[j + 1] - kts[j]);
        const recon = kval[j] + frac * (kval[j + 1] - kval[j]);
        maxErr = Math.max(maxErr, Math.abs(val[i] - recon));
    }
    assert(maxErr <= epsilon * 1.5 + 1e-9, `max reconstruction error ${maxErr.toFixed(4)} <= 1.5*epsilon`);
}

// Interpolate — single point
{
    const ts  = new BigInt64Array([0n, 10000n, 20000n, 30000n]);
    const val = new Float64Array([0.0, 10.0, 20.0, 30.0]);
    assertClose(interpolate(ts, val, 5000n),  5.0, 1e-9, 'interpolate midpoint');
    assertClose(interpolate(ts, val, 0n),     0.0, 1e-9, 'interpolate at start');
    assertClose(interpolate(ts, val, 30000n), 30.0, 1e-9, 'interpolate at end');
}

// Interpolate — clamping
{
    const ts  = new BigInt64Array([10000n, 20000n]);
    const val = new Float64Array([5.0, 10.0]);
    assertClose(interpolate(ts, val, 0n),     5.0,  1e-9, 'clamp before start');
    assertClose(interpolate(ts, val, 99999n), 10.0, 1e-9, 'clamp after end');
}

// Error on bad input (non-monotonic timestamps)
{
    const ts  = new BigInt64Array([0n, 2_000_000n, 1_000_000n]);
    const val = new Float64Array([0.0, 1.0, 2.0]);
    let threw = false;
    try { compress_rdp(ts, val, 1.0); }
    catch (_) { threw = true; }
    assert(threw, 'non-monotonic ts throws');
}

// ─── summary ──────────────────────────────────────────────────────────────────
console.log(`\n${passed + failed} tests: ${passed} passed, ${failed} failed`);
if (failed > 0) process.exit(1);

let passed = 0;
let failed = 0;

function assert(condition, message) {
    if (condition) {
        console.log(`  ✓ ${message}`);
        passed++;
    } else {
        console.error(`  ✗ FAIL: ${message}`);
        failed++;
    }
}

function assertClose(a, b, tol, message) {
    assert(Math.abs(a - b) <= tol, `${message} (${a} ≈ ${b}, tol=${tol})`);
}

// ─── helpers ──────────────────────────────────────────────────────────────────

function makeSine(n) {
    const ts  = new BigInt64Array(n);
    const val = new Float64Array(n);
    for (let i = 0; i < n; i++) {
        ts[i]  = BigInt(i) * 1_000_000n;
        val[i] = Math.sin(i * 0.05) * 100.0;
    }
    return [ts, val];
}

function makeLine(n) {
    const ts  = new BigInt64Array(n);
    const val = new Float64Array(n);
    for (let i = 0; i < n; i++) {
        ts[i]  = BigInt(i) * 1_000_000n;
        val[i] = i;
    }
    return [ts, val];
}

// ─── tests ────────────────────────────────────────────────────────────────────

console.log('curvepress WASM test suite');
console.log(`  version: ${version()}`);

// Round-trip RDP
{
    const [ts, val] = makeSine(500);
    const data = compress(ts, val, 1.0, 0, 100, false, 0.0, 0, null);
    const dec  = decompress(data);
    assert(dec.len < 500, `RDP reduces point count: ${dec.len} < 500`);
    assert(dec.timestamps.length === dec.values.length, 'ts and val same length');
    assert(version().startsWith('0.'), 'version starts with 0.');
}

// Round-trip VW — exact n_out
{
    const [ts, val] = makeSine(300);
    const data = compress(ts, val, 1.0, 1 /*VW*/, 30, false, 0.0, 0, null);
    const dec  = decompress(data);
    assert(dec.len === 30, `VW returns exactly n_out=30: got ${dec.len}`);
}

// Decompressed values within epsilon of original at kept timestamps
{
    const [ts, val] = makeLine(100);
    const epsilon = 1.0;
    const data = compress(ts, val, epsilon, 0, 100, false, 0.0, 0, null);
    const dec  = decompress(data);
    const kts  = dec.timestamps;
    const kval = dec.values;
    // Build a map from kept ts (as string) → kept val
    const map = new Map();
    for (let i = 0; i < kts.length; i++) map.set(kts[i].toString(), kval[i]);
    // Interpolate and check all original points within ~1.5*epsilon
    let maxErr = 0;
    for (let i = 0; i < ts.length; i++) {
        const tsKey = ts[i].toString();
        if (map.has(tsKey)) continue;
        // Find neighbors
        let j = 0;
        while (j + 1 < kts.length - 1 && kts[j + 1] <= ts[i]) j++;
        const frac = Number(ts[i] - kts[j]) / Number(kts[j + 1] - kts[j]);
        const recon = kval[j] + frac * (kval[j + 1] - kval[j]);
        maxErr = Math.max(maxErr, Math.abs(val[i] - recon));
    }
    assert(maxErr <= epsilon * 1.5 + 1e-9, `max reconstruction error ${maxErr.toFixed(4)} <= 1.5*epsilon`);
}

// Interpolate
{
    const ts  = new BigInt64Array([0n, 10000n, 20000n, 30000n]);
    const val = new Float64Array([0.0, 10.0, 20.0, 30.0]);
    const out = interpolate(ts, val, 0n, 30000n, 5000n);
    assert(out.length === 7, `interpolate output length 7: got ${out.length}`);
    for (let i = 0; i < out.length; i++) {
        assertClose(out[i], i * 5.0, 1e-9, `interpolate[${i}]`);
    }
}

// Error on bad input (non-monotonic timestamps)
{
    const ts  = new BigInt64Array([0n, 2_000_000n, 1_000_000n]);
    const val = new Float64Array([0.0, 1.0, 2.0]);
    let threw = false;
    try { compress(ts, val, 1.0, 0, 100, false, 0.0, 0, null); }
    catch (_) { threw = true; }
    assert(threw, 'non-monotonic ts throws');
}

// ─── summary ──────────────────────────────────────────────────────────────────
console.log(`\n${passed + failed} tests: ${passed} passed, ${failed} failed`);
if (failed > 0) process.exit(1);
