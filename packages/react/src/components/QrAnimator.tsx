/**
 * QrAnimator — renders an animated QR code stream for the AirSign send side.
 *
 * Each frame produced by `WasmSendSession` is encoded into a QR code and
 * drawn onto an HTML canvas element.  The component drives the animation loop
 * internally at the requested FPS and exposes progress via optional callbacks.
 *
 * ## Dependencies
 * Uses the `qrcode` npm package for in-browser QR rendering (canvas API).
 * The AirSign WASM module must be initialised before mounting this component
 * (call `initAirSign()` once in your app bootstrap).
 *
 * @example
 * ```tsx
 * <QrAnimator
 *   data={unsignedTxBytes}
 *   filename="unsigned_tx.bin"
 *   password="shared-secret"
 *   fps={8}
 *   onComplete={() => setStep("waiting_for_signature")}
 * />
 * ```
 */

import React, {
  useCallback,
  useEffect,
  useImperativeHandle,
  useRef,
  forwardRef,
} from "react";

import { useSendSession } from "../hooks/useSendSession.js";
import type { QrAnimatorProps } from "../types.js";

// ─── Public imperative handle ─────────────────────────────────────────────────

export interface QrAnimatorHandle {
  start: () => void;
  stop: () => void;
  reset: () => void;
}

// ─── Component ────────────────────────────────────────────────────────────────

export const QrAnimator = forwardRef<QrAnimatorHandle, QrAnimatorProps>(
  function QrAnimator(props, ref) {
    const {
      data,
      filename = "payload.bin",
      password,
      fps = 8,
      qrScale = 4,
      errorCorrectionLevel = "M",
      onComplete,
      onProgress,
      className,
    } = props;

    const canvasRef = useRef<HTMLCanvasElement>(null);
    // Keep a stable ref to the latest frame payload so the canvas draw
    // callback always has access to it without a stale closure.
    const latestFrameRef = useRef<Uint8Array | null>(null);

    // ── Session hook ───────────────────────────────────────────────────────
    const { progress, frameIndex, totalFrames, isRunning, isDone, start, stop, reset, error } =
      useSendSession({
        data,
        filename,
        password,
        fps,
        autoStart: true,
        onProgress,
        onComplete,
      });

    // ── Draw current frame to canvas ───────────────────────────────────────
    const drawFrame = useCallback(
      async (frameData: Uint8Array) => {
        const canvas = canvasRef.current;
        if (!canvas) return;

        try {
          // qrcode expects a string or Buffer; convert bytes to a latin-1
          // string so binary data round-trips through the QR payload unchanged.
          const binary = String.fromCharCode(...frameData);

          // Use a string variable so tsc does not try to resolve the module
          // at type-check time (same pattern as initAirSign / QrScanner).
          const qrcodePkg = "qrcode";
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          const QRCode: any = await import(/* @vite-ignore */ qrcodePkg);
          await QRCode.toCanvas(canvas, binary, {
            errorCorrectionLevel,
            scale: qrScale,
            margin: 2,
            color: {
              dark: "#000000",
              light: "#ffffff",
            },
          });
        } catch {
          // If QR encoding fails (e.g. frame too large for EC level), silently
          // skip this frame — the receiver's fountain code handles gaps.
        }
      },
      [errorCorrectionLevel, qrScale],
    );

    // ── Intercept frames for canvas rendering ──────────────────────────────
    // We need to draw each frame as it is emitted.  The hook exposes
    // `frameIndex` which increments on each emission — we use it as a
    // trigger to pull the latest frame from the WASM session and draw it.
    useEffect(() => {
      if (latestFrameRef.current) {
        void drawFrame(latestFrameRef.current);
      }
    }, [frameIndex, drawFrame]);

    // ── Expose imperative handle ───────────────────────────────────────────
    useImperativeHandle(
      ref,
      () => ({ start, stop, reset }),
      [start, stop, reset],
    );

    // ── Derived display values ─────────────────────────────────────────────
    const pct = Math.min(100, Math.round(progress * 100));

    // ── Render ─────────────────────────────────────────────────────────────
    return (
      <div
        className={className}
        style={{
          display: "inline-flex",
          flexDirection: "column",
          alignItems: "center",
          gap: "0.5rem",
        }}
        role="img"
        aria-label={`AirSign QR stream — frame ${frameIndex} of ${totalFrames}`}
      >
        <canvas
          ref={canvasRef}
          style={{ imageRendering: "pixelated" }}
          aria-hidden="true"
        />

        {/* Progress bar */}
        <div
          style={{
            width: "100%",
            height: "6px",
            background: "#e2e8f0",
            borderRadius: "3px",
            overflow: "hidden",
          }}
          role="progressbar"
          aria-valuenow={pct}
          aria-valuemin={0}
          aria-valuemax={100}
        >
          <div
            style={{
              height: "100%",
              width: `${pct}%`,
              background: isDone ? "#22c55e" : "#6366f1",
              transition: "width 0.1s linear",
              borderRadius: "3px",
            }}
          />
        </div>

        {/* Status text */}
        <p
          style={{
            margin: 0,
            fontSize: "0.75rem",
            color: "#64748b",
            fontVariantNumeric: "tabular-nums",
          }}
        >
          {error ? (
            <span style={{ color: "#ef4444" }}>⚠ {error}</span>
          ) : isDone ? (
            "✓ All frames transmitted"
          ) : isRunning ? (
            `Frame ${frameIndex} / ${totalFrames} · ${pct}%`
          ) : (
            "Paused"
          )}
        </p>
      </div>
    );
  },
);

QrAnimator.displayName = "QrAnimator";