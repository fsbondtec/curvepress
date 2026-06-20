/**
 * TypeScript type declarations for curvepress WASM.
 */

/**
 * Compress with Ramer-Douglas-Peucker.
 *
 * @param timestamps  BigInt64Array — strictly increasing nanosecond timestamps.
 * @param values      Float64Array  — finite values (no NaN / Inf).
 * @param epsilon     Max absolute error in the value domain.
 * @returns Uint8Array byte stream.
 */
export function compress_rdp(
    timestamps: BigInt64Array,
    values: Float64Array,
    epsilon: number,
): Uint8Array;

/**
 * Compress with Visvalingam-Whyatt.
 *
 * @param timestamps  BigInt64Array.
 * @param values      Float64Array.
 * @param n_out       Exact number of kept points.
 * @returns Uint8Array byte stream.
 */
export function compress_vw(
    timestamps: BigInt64Array,
    values: Float64Array,
    n_out: number,
): Uint8Array;

/**
 * Compress with RDP-N (binary-searched epsilon to hit n_out points).
 *
 * @param timestamps  BigInt64Array.
 * @param values      Float64Array.
 * @param n_out       Target point count.
 * @param epsilon     Upper bound for the RDP search.
 * @returns Uint8Array byte stream.
 */
export function compress_rdpn(
    timestamps: BigInt64Array,
    values: Float64Array,
    n_out: number,
    epsilon: number,
): Uint8Array;

/** Decompressed time series data. */
export class Decoded {
    /** BigInt64Array of kept nanosecond timestamps. */
    readonly timestamps: BigInt64Array;
    /** Float64Array of kept values. */
    readonly values: Float64Array;
    /** Number of kept points. */
    readonly len: number;
    free(): void;
}

/**
 * Decompress a byte stream produced by any compress function.
 *
 * @param data  Uint8Array produced by a compress function.
 * @returns Decoded object.
 */
export function decompress(data: Uint8Array): Decoded;

/**
 * Reconstruct the value at a single timestamp from the support points.
 *
 * @param timestamps  BigInt64Array of kept timestamps (from decompress).
 * @param values      Float64Array of kept values (from decompress).
 * @param t           Query timestamp (nanoseconds, bigint).
 * @returns Interpolated value (number). Clamped at data boundaries.
 */
export function interpolate(
    timestamps: BigInt64Array,
    values: Float64Array,
    t: bigint,
): number;

/** Return the library version string. */
export function version(): string;
