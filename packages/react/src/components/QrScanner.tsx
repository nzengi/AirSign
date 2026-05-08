/**
 * QrScanner — captures camera frames, decodes QR codes, and feeds the
 * AirSign receive pipeline.
 *
 * Opens the device camera via `getUserMedia`, draws frames onto an offscreen
 * canvas, uses `jsQR` for QR decoding, and passes each decoded binary payload
 * to `useRecvSession.ingest()`.  Fires `onComplete` once the full fountain
 * code has been assembled and decrypted.
 *
 * @example
 * ```tsx
 * <QrScanner
 *   password="shared-secret"
 *   onComplete={(data, filename) => {
 *     // data is the decrypted Uint8Array
 *     broadcastTransaction(data);
 *   }}
 *   onProgress={(p) => setProgress(p)}
 * />
 * ```
 */

import React, { useEffect, useRef, useState } from "react";
import { useRecvSession } from "../hooks/useRecvSession.js";
import type { QrScannerProps } from "../types.js";

// ─── Component ────────────────────────────────────────────────────────────────

export function QrScanner({
  password,
  deviceId,
  onComplete,
  onProgress,
  onError,
  className,
}: QrScannerProps) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const rafRef = useRef<number | null>(null);
  const streamRef = useRef<MediaStream | null>(null);

  const [cameraError, setCameraError] = useState<string | null>(null);
  const [scanning, setScanning] = useState(false);

  // ── Receive session ────────────────────────────────────────────────────────
  const { ingest, progress, isComplete, reset, error: sessionError } =
    useRecvSession({ password, onComplete, onProgress, onError });

  // ── Start camera ───────────────────────────────────────────────────────────
  useEffect(() => {
    let cancelled = false;

    async function startCamera() {
      try {
        const constraints: MediaStreamConstraints = {
          video: deviceId
            ? { deviceId: { exact: deviceId } }
            : { facingMode: "environment" },
          audio: false,
        };

        const stream = await navigator.mediaDevices.getUserMedia(constraints);
        if (cancelled) {
          stream.getTracks().forEach((t) => t.stop());
          return;
        }

        streamRef.current = stream;
        const video = videoRef.current;
        if (!video) return;
        video.srcObject = stream;
        await video.play();
        setCameraError(null);
        setScanning(true);
      } catch (e: unknown) {
        const msg = e instanceof Error ? e.message : String(e);
        setCameraError(`Camera error: ${msg}`);
        onError?.(`Camera error: ${msg}`);
      }
    }

    void startCamera();

    return () => {
      cancelled = true;
      streamRef.current?.getTracks().forEach((t) => t.stop());
      streamRef.current = null;
      setScanning(false);
    };
  }, [deviceId, onError]);

  // ── Scan loop ──────────────────────────────────────────────────────────────
  useEffect(() => {
    if (!scanning || isComplete) return;

    let jsQR: ((data: Uint8ClampedArray, width: number, height: number) => { data: string } | null) | null = null;

    async function loadJsQR() {
      try {
        // Use a string variable so tsc does not try to resolve the module
        // at type-check time (jsqr is an optional peer dep installed by the app).
        const jsqrPkg = "jsqr";
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const mod: any = await import(/* @vite-ignore */ jsqrPkg);
        // jsqr exports a default function
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        jsQR = (mod.default ?? mod) as (data: Uint8ClampedArray, width: number, height: number) => { data: string } | null;
      } catch {
        const msg = "jsqr library not found. Add 'jsqr' to your dependencies.";
        setCameraError(msg);
        onError?.(msg);
      }
    }

    void loadJsQR().then(() => {
      function tick() {
        const video = videoRef.current;
        const canvas = canvasRef.current;
        if (!video || !canvas || !jsQR || isComplete) return;

        if (video.readyState !== video.HAVE_ENOUGH_DATA) {
          rafRef.current = requestAnimationFrame(tick);
          return;
        }

        const { videoWidth: w, videoHeight: h } = video;
        if (w === 0 || h === 0) {
          rafRef.current = requestAnimationFrame(tick);
          return;
        }

        canvas.width = w;
        canvas.height = h;
        const ctx = canvas.getContext("2d", { willReadFrequently: true });
        if (!ctx) return;

        ctx.drawImage(video, 0, 0, w, h);
        const imageData = ctx.getImageData(0, 0, w, h);

        const code = jsQR(imageData.data, w, h);
        if (code) {
          // Convert latin-1 string back to binary Uint8Array.
          const bytes = new Uint8Array(code.data.length);
          for (let i = 0; i < code.data.length; i++) {
            bytes[i] = code.data.charCodeAt(i) & 0xff;
          }
          ingest(bytes);
        }

        rafRef.current = requestAnimationFrame(tick);
      }

      rafRef.current = requestAnimationFrame(tick);
    });

    return () => {
      if (rafRef.current !== null) {
        cancelAnimationFrame(rafRef.current);
        rafRef.current = null;
      }
    };
  }, [scanning, isComplete, ingest, onError]);

  // ── Derived ────────────────────────────────────────────────────────────────
  const pct = Math.min(100, Math.round(progress * 100));
  const displayError = cameraError ?? sessionError;

  // ── Render ─────────────────────────────────────────────────────────────────
  return (
    <div
      className={className}
      style={{
        display: "inline-flex",
        flexDirection: "column",
        alignItems: "center",
        gap: "0.5rem",
      }}
    >
      {/* Live video feed */}
      <div style={{ position: "relative", display: "inline-block" }}>
        <video
          ref={videoRef}
          muted
          playsInline
          style={{ display: "block", maxWidth: "100%", borderRadius: "4px" }}
          aria-label="Camera feed for QR scanning"
        />
        {/* Scanning overlay */}
        {scanning && !isComplete && (
          <div
            style={{
              position: "absolute",
              inset: 0,
              border: "3px solid #6366f1",
              borderRadius: "4px",
              pointerEvents: "none",
            }}
            aria-hidden="true"
          />
        )}
      </div>

      {/* Hidden canvas for frame capture */}
      <canvas ref={canvasRef} style={{ display: "none" }} aria-hidden="true" />

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
            background: isComplete ? "#22c55e" : "#6366f1",
            transition: "width 0.1s linear",
            borderRadius: "3px",
          }}
        />
      </div>

      {/* Status line */}
      <p
        style={{
          margin: 0,
          fontSize: "0.75rem",
          color: "#64748b",
          fontVariantNumeric: "tabular-nums",
        }}
      >
        {displayError ? (
          <span style={{ color: "#ef4444" }}>⚠ {displayError}</span>
        ) : isComplete ? (
          "✓ Transfer complete"
        ) : scanning ? (
          `Receiving… ${pct}%`
        ) : (
          "Waiting for camera…"
        )}
      </p>

      {/* Reset button (shown after completion or error) */}
      {(isComplete || displayError) && (
        <button
          type="button"
          onClick={reset}
          style={{
            padding: "0.25rem 0.75rem",
            fontSize: "0.75rem",
            borderRadius: "4px",
            border: "1px solid #e2e8f0",
            background: "#f8fafc",
            cursor: "pointer",
            color: "#334155",
          }}
        >
          Scan again
        </button>
      )}
    </div>
  );
}

QrScanner.displayName = "QrScanner";