/**
 * TypeScript type declarations for curvepress WASM.
 * wasm-pack also emits types; this file extends/refines them.
 */

/**
 * Compress time series data into a self-describing byte stream.
 *
 * @param timestamps   BigInt64Array — strictly increasing nanosecond timestamps.
 * @param values       Float64Array  — finite values (no NaN / Inf).
 * @param epsilon      Max absolute error for RDP / RDP-N. Default 1.0.
 * @param algo         0=RDP, 1=VW, 2=RDP-N. Default 0.
 * @param n_out        Target point count for VW / RDP-N. Default 100.
 * @param normalize_axes  Scale time axis before distance computation. Default false.
 * @param value_range  Override for normalization / RDP-N bound; 0 = auto.
 * @param ts_mode      0=Irregular, 1=Regular. Default 0.
 * @param radial_prefilter  Radial distance pre-filter radius; null = disabled.
 * @returns Uint8Array byte stream.
 */
export function compress(
    timestamps: BigInt64Array,
    values: Float64Array,
    epsilon?: number,
    algo?: number,
    n_out?: number,
    normalize_axes?: boolean,
    value_range?: number,
    ts_mode?: number,
    radial_prefilter?: number | null,
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
 * Decompress a byte stream produced by `compress`.
 *
 * @param data  Uint8Array produced by compress().
 * @returns Decoded object.
 */
export function decompress(data: Uint8Array): Decoded;

/**
 * Interpolate kept support points onto a regular time grid.
 *
 * Output length = Math.floor(Number(t_end - t_start) / Number(interval_ns)) + 1.
 *
 * @returns Float64Array of interpolated values.
 */
export function interpolate(
    timestamps: BigInt64Array,
    values: Float64Array,
    t_start: bigint,
    t_end: bigint,
    interval_ns: bigint,
): Float64Array;

/** Return the library version string. */
export function version(): string;
