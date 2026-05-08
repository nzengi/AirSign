/**
 * useSendSession — React hook for the AirSign QR send pipeline.
 *
 * Manages a `WasmSendSession` lifecycle, drives the frame-emission loop at
 * the requested FPS, and exposes progress state to the component tree.
 *
 * @example
 * ```tsx
 * const { progress, frameIndex, totalFrames, isRunning, start, stop, reset } =
 *   useSendSession({ data: txBytes, filename: "unsigned_tx.bin", password: "s3cr3t" });
 * ```
 */

import { useCallback, useEffect, useRef, useState } from "react";
import type { SendSessionState, WasmSendSession } from "../types.js";

// ─── Options ─────────────────────────────────────────────────────────────────

export interface UseSendSessionOptions {
  /** Raw plaintext bytes to transmit. */
  data: Uint8Array;
  /** Logical filename embedded in the METADATA frame. @default "payload.bin" */
  filename?: string;
  /** Shared Argon2id password. */
  password: string;
  /** Frames per second. @default 8 */
  fps?: number;
  /**
   * If true the loop starts automatically when the hook mounts (or when
   * `data` / `password` changes).  @default false
   */
  autoStart?: boolean;
  /** Called when the recommended frame count has been fully transmitted. */
  onComplete?: () => void;
  /** Called on each frame with the current progress value (0.0 – 1.0). */
  onProgress?: (progress: number) => void;
}

// ─── Hook ─────────────────────────────────────────────────────────────────────

export function useSendSession(options: UseSendSessionOptions): SendSessionState {
  const {
    data,
    filename = "payload.bin",
    password,
    fps = 8,
    autoStart = false,
    onComplete,
    onProgress,
  } = options;

  const intervalMs = Math.max(1, Math.round(1000 / fps));

  // ── State ──────────────────────────────────────────────────────────────────
  const [progress, setProgress] = useState(0);
  const [frameIndex, setFrameIndex] = useState(0);
  const [totalFrames, setTotalFrames] = useState(0);
  const [isRunning, setIsRunning] = useState(false);
  const [isDone, setIsDone] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // ── Refs ───────────────────────────────────────────────────────────────────
  // We keep the WASM session in a ref so it doesn't trigger re-renders.
  const sessionRef = useRef<WasmSendSession | null>(null);
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const isRunningRef = useRef(false);

  // ── Session factory ────────────────────────────────────────────────────────
  const createSession = useCallback((): WasmSendSession | null => {
    try {
      // Dynamic import so the hook works in SSR environments — WASM is only
      // loaded in the browser.
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const wasm = (globalThis as any).__airsign_wasm__;
      if (!wasm) {
        setError(
          "AirSign WASM module not initialised. Call initAirSign() before rendering.",
        );
        return null;
      }
      const session: WasmSendSession = new wasm.WasmSendSession(
        data,
        filename,
        password,
      );
      setTotalFrames(session.total_frames());
      setError(null);
      return session;
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(`Failed to create send session: ${msg}`);
      return null;
    }
  }, [data, filename, password]);

  // ── Stop helper ────────────────────────────────────────────────────────────
  const stopLoop = useCallback(() => {
    if (timerRef.current !== null) {
      clearInterval(timerRef.current);
      timerRef.current = null;
    }
    isRunningRef.current = false;
    setIsRunning(false);
  }, []);

  // ── Start ──────────────────────────────────────────────────────────────────
  const start = useCallback(() => {
    if (isRunningRef.current) return; // already running

    // Lazily create the session if it doesn't exist yet.
    if (!sessionRef.current) {
      const s = createSession();
      if (!s) return;
      sessionRef.current = s;
    }

    isRunningRef.current = true;
    setIsRunning(true);

    timerRef.current = setInterval(() => {
      const session = sessionRef.current;
      if (!session) return;

      const frame = session.next_frame();
      if (frame === null) {
        // Limit reached (set_limit was called).
        stopLoop();
        return;
      }

      const p = session.progress();
      const idx = session.frame_index();

      setProgress(p);
      setFrameIndex(idx);
      onProgress?.(p);

      if (p >= 1.0 && !isDone) {
        setIsDone(true);
        onComplete?.();
      }
    }, intervalMs);
  }, [createSession, intervalMs, isDone, onComplete, onProgress, stopLoop]);

  // ── Stop ───────────────────────────────────────────────────────────────────
  const stop = useCallback(() => {
    stopLoop();
  }, [stopLoop]);

  // ── Reset ──────────────────────────────────────────────────────────────────
  const reset = useCallback(() => {
    stopLoop();
    sessionRef.current?.free();
    sessionRef.current = null;
    setProgress(0);
    setFrameIndex(0);
    setTotalFrames(0);
    setIsDone(false);
    setError(null);
  }, [stopLoop]);

  // ── Auto-start effect ──────────────────────────────────────────────────────
  useEffect(() => {
    if (autoStart) {
      start();
    }
    return () => {
      stopLoop();
    };
    // We intentionally only run this on mount / when the key inputs change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [data, password, autoStart]);

  // ── Re-create session when inputs change ───────────────────────────────────
  useEffect(() => {
    // If currently running, stop, discard and restart.
    const wasRunning = isRunningRef.current;
    stopLoop();
    sessionRef.current?.free();
    sessionRef.current = null;
    setProgress(0);
    setFrameIndex(0);
    setIsDone(false);

    if (wasRunning || autoStart) {
      // Small defer so React finishes rendering first.
      setTimeout(start, 0);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [data, filename, password]);

  // ── Cleanup on unmount ─────────────────────────────────────────────────────
  useEffect(() => {
    return () => {
      stopLoop();
      sessionRef.current?.free();
      sessionRef.current = null;
    };
  }, [stopLoop]);

  return {
    progress,
    frameIndex,
    totalFrames,
    isRunning,
    isDone,
    start,
    stop,
    reset,
    error,
  };
}