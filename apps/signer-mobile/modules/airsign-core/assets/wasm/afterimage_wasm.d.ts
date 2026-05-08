/* tslint:disable */
/* eslint-disable */

/**
 * Decodes and decrypts received QR frame payloads.
 *
 * Feed each QR-decoded `Uint8Array` into [`ingest_frame`](Self::ingest_frame).
 * Once [`is_complete`](Self::is_complete) returns `true`, call
 * [`get_data`](Self::get_data) to retrieve the plaintext.
 */
export class WasmRecvSession {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Returns the filename embedded in the metadata frame, or `undefined` if
     * no metadata frame has been received yet.
     */
    filename(): string | undefined;
    /**
     * Retrieve the decrypted plaintext data.
     *
     * Returns an empty `Uint8Array` if the session is not yet complete.
     *
     * # Errors
     * Throws a JS exception if decryption fails (wrong password / tampered data).
     */
    get_data(): Uint8Array;
    /**
     * Feed a raw QR-decoded payload into the session.
     *
     * Returns `true` if the transfer is complete after this frame.
     * Silently ignores frames that cannot be parsed (corrupted / duplicate).
     */
    ingest_frame(frame: Uint8Array): boolean;
    /**
     * Returns `true` if all data has been received and decrypted successfully.
     */
    is_complete(): boolean;
    /**
     * Create a new receive session with the shared password.
     */
    constructor(password: string);
    /**
     * Returns the original plaintext size in bytes, or `undefined` if no
     * metadata frame has been received yet.
     */
    original_size(): number | undefined;
    /**
     * Fraction of required droplets received (0.0 – 1.0).
     */
    progress(): number;
    /**
     * Returns the protocol version byte from the metadata frame (`1`, `2`, or
     * `3`), or `undefined` if no metadata frame has been received yet.
     */
    protocol_version(): number | undefined;
    /**
     * Number of frames received so far.
     */
    received_count(): bigint;
}

/**
 * Encrypts and fountain-encodes data for transmission as QR frames.
 *
 * Call [`next_frame`](Self::next_frame) in a loop and display each returned
 * `Uint8Array` as a QR code.  Use [`progress`](Self::progress) to drive a
 * progress bar or to decide when enough frames have been sent.
 */
export class WasmSendSession {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Total number of source droplets (before redundancy).
     */
    droplet_count(): number;
    /**
     * Number of frames emitted so far.
     */
    frame_index(): number;
    /**
     * Returns `true` if there are more frames to send.
     *
     * When no [`set_limit`](Self::set_limit) has been set this always returns
     * `true` — use [`progress`](Self::progress) or [`total_frames`](Self::total_frames)
     * to decide when to stop.
     */
    has_next(): boolean;
    /**
     * Create a new send session.
     *
     * - `data`     — raw plaintext bytes to transmit
     * - `filename` — cleartext filename embedded in the metadata frame
     * - `password` — shared Argon2id encryption password
     *
     * # Errors
     * Throws a JS exception if key derivation or encryption fails.
     */
    constructor(data: Uint8Array, filename: string, password: string);
    /**
     * Returns the next encoded frame as a `Uint8Array`, or `null` if done.
     *
     * Each returned slice should be encoded as a QR code and displayed for
     * the configured frame interval.
     */
    next_frame(): Uint8Array | undefined;
    /**
     * Fraction of recommended frames emitted (0.0 – 1.0).
     *
     * Values above 1.0 are possible when sending beyond the recommended count
     * for extra redundancy.
     */
    progress(): number;
    /**
     * Recommended total droplets to transmit (source + redundancy, no metadata).
     */
    recommended_droplet_count(): number;
    /**
     * Set a hard upper limit on frames generated.
     *
     * After `limit` frames [`has_next`](Self::has_next) returns `false` and
     * [`next_frame`](Self::next_frame) returns `null`.  Set to `0` to remove
     * any previously set limit.
     */
    set_limit(limit: number): void;
    /**
     * Recommended total frames to transmit (source blocks + redundancy + metadata).
     *
     * This is a *suggestion* — transmitting more frames increases reliability
     * on noisy optical channels.
     */
    total_frames(): number;
}

/**
 * Decode a Base-64 string back to bytes.
 *
 * # Errors
 * Throws a JS exception if the input is not valid Base-64.
 */
export function decode_base64(s: string): Uint8Array;

/**
 * Decode a hex string to bytes.
 *
 * # Errors
 * Throws a JS exception if the input is not valid hex.
 */
export function decode_hex(s: string): Uint8Array;

/**
 * Encode a byte slice as a standard Base-64 string (RFC 4648, no padding stripped).
 *
 * Useful for passing `Uint8Array` data through JSON / localStorage.
 */
export function encode_base64(data: Uint8Array): string;

/**
 * Hex-encode a byte slice (lowercase).
 */
export function encode_hex(data: Uint8Array): string;

/**
 * Compute the recommended frame count for a payload of `size` bytes.
 */
export function recommended_frames(size: number): number;

export function start(): void;

/**
 * Returns the AfterImage library version string.
 */
export function version(): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_wasmrecvsession_free: (a: number, b: number) => void;
    readonly __wbg_wasmsendsession_free: (a: number, b: number) => void;
    readonly decode_base64: (a: number, b: number, c: number) => void;
    readonly decode_hex: (a: number, b: number, c: number) => void;
    readonly encode_base64: (a: number, b: number, c: number) => void;
    readonly encode_hex: (a: number, b: number, c: number) => void;
    readonly recommended_frames: (a: number) => number;
    readonly start: () => void;
    readonly version: (a: number) => void;
    readonly wasmrecvsession_filename: (a: number, b: number) => void;
    readonly wasmrecvsession_get_data: (a: number, b: number) => void;
    readonly wasmrecvsession_ingest_frame: (a: number, b: number, c: number) => number;
    readonly wasmrecvsession_is_complete: (a: number) => number;
    readonly wasmrecvsession_new: (a: number, b: number) => number;
    readonly wasmrecvsession_original_size: (a: number) => number;
    readonly wasmrecvsession_progress: (a: number) => number;
    readonly wasmrecvsession_protocol_version: (a: number) => number;
    readonly wasmrecvsession_received_count: (a: number) => bigint;
    readonly wasmsendsession_droplet_count: (a: number) => number;
    readonly wasmsendsession_frame_index: (a: number) => number;
    readonly wasmsendsession_has_next: (a: number) => number;
    readonly wasmsendsession_new: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly wasmsendsession_next_frame: (a: number, b: number) => void;
    readonly wasmsendsession_progress: (a: number) => number;
    readonly wasmsendsession_recommended_droplet_count: (a: number) => number;
    readonly wasmsendsession_set_limit: (a: number, b: number) => void;
    readonly wasmsendsession_total_frames: (a: number) => number;
    readonly __wbindgen_export: (a: number) => void;
    readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
    readonly __wbindgen_export2: (a: number, b: number) => number;
    readonly __wbindgen_export3: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_export4: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
