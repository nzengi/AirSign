/**
 * useRecvSession — React hook for the AirSign QR receive pipeline.
 *
 * Manages a `WasmRecvSession` lifecycle.  Call `ingest(frame)` for every
 * QR-decoded `Uint8Array` received from the camera.  The hook tracks progress
 * and fires `onComplete` once the fountain code has been fully decoded and
 * decrypted.
 *
 * @example
 * ```tsx
 * const { ingest, progress, isComplete, data, filename } =
 *   useRecvSession({ password: "s3cr3t", onComplete: (buf, name) => save(buf, name) });
 * ```
 */

import { useCallback, useEffect, useRef, useState } from "react";
import type { RecvSessionState, WasmRecvSession } from "../types.js";

// ─── Options ─────────────────────────────────────────────────────────────────

export interface UseRecvSessionOptions {
  /** Shared Argon2id password. */
  password: string;
  /** Called once when the full payload has been decoded and decrypted. */
  onComplete?: (data: Uint8Array, filename?: string) => void;
  /** Called on each ingested frame with the current progress (0.0 – 1.0). */
  onProgress?: (progress: number) => void;
  /** Called if decryption fails or an internal error occurs. */
  onError?: (error: string) => void;
}

// ─── Hook ─────────────────────────────────────────────────────────────────────

export function useRecvSession(
  options: UseRecvSessionOptions,
): RecvSessionState {
  const { password, onComplete, onProgress, onError } = options;

  // ── State ──────────────────────────────────────────────────────────────────
  const [progress, setProgress] = useState(0);
  const [receivedCount, setReceivedCount] = useState(0);
  const [isComplete, setIsComplete] = useState(false);
  const [data, setData] = useState<Uint8Array | null>(null);
  const [filename, setFilename] = useState<string | undefined>(undefined);
  const [originalSize, setOriginalSize] = useState<number | undefined>(
    undefined,
  );
  const [error, setError] = useState<string | null>(null);

  // ── Refs ───────────────────────────────────────────────────────────────────
  const sessionRef = useRef<WasmRecvSession | null>(null);
  const isCompleteRef = useRef(false);

  // ── Session factory ────────────────────────────────────────────────────────
  const createSession = useCallback((): WasmRecvSession => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const wasm = (globalThis as any).__airsign_wasm__;
    if (!wasm) {
      throw new Error(
        "AirSign WASM module not initialised. Call initAirSign() before rendering.",
      );
    }
    return new wasm.WasmRecvSession(password) as WasmRecvSession;
  }, [password]);

  // ── Ensure a session exists ────────────────────────────────────────────────
  const ensureSession = useCallback((): WasmRecvSession => {
    if (!sessionRef.current) {
      sessionRef.current = createSession();
    }
    return sessionRef.current;
  }, [createSession]);

  // ── ingest ─────────────────────────────────────────────────────────────────
  const ingest = useCallback(
    (frame: Uint8Array): void => {
      if (isCompleteRef.current) return; // already done — ignore extra frames

      try {
        const session = ensureSession();
        const done = session.ingest_frame(frame);

        const p = session.progress();
        const count = Number(session.received_count());
        const fname = session.filename();
        const fsize = session.original_size();

        setProgress(p);
        setReceivedCount(count);
        if (fname !== undefined) setFilename(fname);
        if (fsize !== undefined) setOriginalSize(fsize);
        onProgress?.(p);

        if (done && !isCompleteRef.current) {
          isCompleteRef.current = true;
          try {
            const payload = session.get_data();
            setData(payload);
            setIsComplete(true);
            setError(null);
            onComplete?.(payload, fname);
          } catch (decryptErr: unknown) {
            const msg =
              decryptErr instanceof Error
                ? decryptErr.message
                : String(decryptErr);
            setError(`Decryption failed: ${msg}`);
            onError?.(`Decryption failed: ${msg}`);
          }
        }
      } catch (e: unknown) {
        const msg = e instanceof Error ? e.message : String(e);
        setError(msg);
        onError?.(msg);
      }
    },
    [ensureSession, onComplete, onError, onProgress],
  );

  // ── Reset ──────────────────────────────────────────────────────────────────
  const reset = useCallback(() => {
    sessionRef.current?.free();
    sessionRef.current = null;
    isCompleteRef.current = false;
    setProgress(0);
    setReceivedCount(0);
    setIsComplete(false);
    setData(null);
    setFilename(undefined);
    setOriginalSize(undefined);
    setError(null);
  }, []);

  // ── Re-create session when password changes ────────────────────────────────
  useEffect(() => {
    reset();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [password]);

  // ── Cleanup on unmount ─────────────────────────────────────────────────────
  useEffect(() => {
    return () => {
      sessionRef.current?.free();
      sessionRef.current = null;
    };
  }, []);

  return {
    progress,
    receivedCount,
    isComplete,
    data,
    filename,
    originalSize,
    ingest,
    reset,
    error,
  };
}