/**
 * SendPage — Online machine (Step 1)
 *
 * 1. User pastes / generates a demo Solana transaction (base64)
 * 2. Sets a shared password
 * 3. The WASM WasmSendSession encodes it into fountain-coded, encrypted QR frames
 * 4. QrAnimator loops the frames — hold phone up on the air-gapped device
 */

import { useCallback, useEffect, useRef, useState } from "react";
import QRCode from "qrcode";

/* ── Demo transaction (base64-encoded minimal Solana tx stub) ──────────── */
const DEMO_TX_B64 =
  "AQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA" +
  "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA" +
  "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAB" +
  "AQACBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA" +
  "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAB" +
  "AgAAAAAAAAA=";

/* ── Helpers ────────────────────────────────────────────────────────────── */
function shortenB64(s: string, maxLen = 64): string {
  return s.length > maxLen ? s.slice(0, maxLen) + "…" : s;
}

function parseRiskLevel(
  flags: string[],
): "high" | "medium" | "low" | "none" {
  const up = flags.map((f) => f.toUpperCase());
  if (up.some((f) => f.includes("UNKNOWN") || f.includes("DRAIN")))
    return "high";
  if (up.some((f) => f.includes("LARGE"))) return "medium";
  if (flags.length > 0) return "low";
  return "none";
}

/* ── Types ──────────────────────────────────────────────────────────────── */
interface Props {
  sharedPassword: string;
  onPasswordChange: (p: string) => void;
  onNext: () => void;
}

interface FrameState {
  frames: string[];
  current: number;
  total: number;
}

/* ── Component ──────────────────────────────────────────────────────────── */
export function SendPage({ sharedPassword, onPasswordChange, onNext }: Props) {
  const [txB64, setTxB64] = useState(DEMO_TX_B64);
  const [status, setStatus] = useState<string>("");
  const [error, setError] = useState<string | null>(null);
  const [frameState, setFrameState] = useState<FrameState | null>(null);
  const [running, setRunning] = useState(false);
  const [fps, setFps] = useState(4);

  const canvasRef = useRef<HTMLCanvasElement>(null);
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const idxRef = useRef(0);

  /* Draw current QR frame onto canvas */
  const drawFrame = useCallback(
    async (frames: string[], idx: number) => {
      const canvas = canvasRef.current;
      if (!canvas || frames.length === 0) return;
      try {
        await QRCode.toCanvas(canvas, frames[idx % frames.length], {
          width: 320,
          margin: 1,
          color: { dark: "#ffffff", light: "#000000" },
        });
      } catch {
        /* ignore */
      }
    },
    [],
  );

  /* Build fountain-coded QR frames via WASM */
  const buildFrames = useCallback(async () => {
    setError(null);
    setFrameState(null);
    setRunning(false);
    if (timerRef.current) clearInterval(timerRef.current);

    if (!txB64.trim()) { setError("Transaction is empty."); return; }
    if (!sharedPassword.trim()) { setError("Password is empty."); return; }

    setStatus("Encoding…");

    try {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const wasm = (globalThis as any).__airsign_wasm__;
      if (!wasm) throw new Error("WASM not loaded");

      const txBytes = Uint8Array.from(atob(txB64), (c) => c.charCodeAt(0));
      const session: {
        next_frame: () => string;
        total_frames: () => number;
        free: () => void;
      } = new wasm.WasmSendSession(txBytes, sharedPassword, 32);

      const total = session.total_frames();
      // Pre-generate 3× the total frames so the animation loops smoothly
      const frames: string[] = [];
      for (let i = 0; i < total * 3; i++) frames.push(session.next_frame());
      session.free();

      setFrameState({ frames, current: 0, total });
      setStatus(`Ready — ${total} unique frames (showing ${frames.length} in loop)`);
      idxRef.current = 0;
      await drawFrame(frames, 0);
    } catch (e: unknown) {
      // Fallback: simulate with a single static QR
      const msg = e instanceof Error ? e.message : String(e);
      setError(`WASM error: ${msg}. Showing static demo QR.`);

      const demoPayload = JSON.stringify({
        proto: "airsign/1",
        seq: 0,
        data: shortenB64(txB64, 80),
      });
      const frames = [demoPayload];
      setFrameState({ frames, current: 0, total: 1 });
      idxRef.current = 0;
      await drawFrame(frames, 0);
    }
  }, [txB64, sharedPassword, drawFrame]);

  /* Start/stop animation */
  const startAnimation = useCallback(
    (frames: string[]) => {
      if (timerRef.current) clearInterval(timerRef.current);
      setRunning(true);
      timerRef.current = setInterval(async () => {
        idxRef.current = (idxRef.current + 1) % frames.length;
        setFrameState((prev) =>
          prev ? { ...prev, current: idxRef.current } : prev,
        );
        await drawFrame(frames, idxRef.current);
      }, Math.round(1000 / fps));
    },
    [fps, drawFrame],
  );

  const stopAnimation = useCallback(() => {
    if (timerRef.current) { clearInterval(timerRef.current); timerRef.current = null; }
    setRunning(false);
  }, []);

  /* Re-start animation when fps changes while running */
  useEffect(() => {
    if (running && frameState) startAnimation(frameState.frames);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [fps]);

  useEffect(() => () => { if (timerRef.current) clearInterval(timerRef.current); }, []);

  const progress = frameState
    ? Math.min(100, Math.round((frameState.current / Math.max(frameState.total - 1, 1)) * 100))
    : 0;

  /* ── Render ─────────────────────────────────────────────────────────── */
  return (
    <main className="page">
      {/* Step indicator */}
      <div className="steps">
        <div className="step-item active">
          <div className="step-dot">1</div>
          <span>Prepare & Send</span>
        </div>
        <div className="step-line" />
        <div className="step-item">
          <div className="step-dot">2</div>
          <span>Air-gap Sign</span>
        </div>
        <div className="step-line" />
        <div className="step-item">
          <div className="step-dot">3</div>
          <span>Receive & Broadcast</span>
        </div>
      </div>

      <div className="two-col">
        {/* Left — inputs */}
        <div>
          <div className="card">
            <div className="card-title">Transaction (base64)</div>
            <textarea
              rows={5}
              value={txB64}
              onChange={(e) => setTxB64(e.target.value)}
              placeholder="Paste a base64-encoded Solana transaction…"
            />
            <div className="btn-row">
              <button
                className="btn btn-outline"
                onClick={() => setTxB64(DEMO_TX_B64)}
              >
                Load demo tx
              </button>
            </div>
          </div>

          <div className="card">
            <div className="card-title">Shared password</div>
            <label htmlFor="pw-send">
              Both machines must use the same password (Argon2id KDF).
            </label>
            <input
              id="pw-send"
              type="text"
              value={sharedPassword}
              onChange={(e) => onPasswordChange(e.target.value)}
            />
          </div>

          <div className="card">
            <div className="card-title">Animation speed</div>
            <label>Frames per second: <strong>{fps}</strong></label>
            <input
              type="range"
              min={1}
              max={12}
              value={fps}
              onChange={(e) => setFps(Number(e.target.value))}
              style={{ width: "100%", accentColor: "var(--accent)" }}
            />
          </div>

          {/* Risk analysis stub */}
          <div className="card">
            <div className="card-title">Transaction analysis</div>
            {(() => {
              let flags: string[] = [];
              try {
                const bytes = atob(txB64);
                if (bytes.length < 100) flags = ["SMALL_TX"];
                else if (bytes.length > 1000) flags = ["LARGE_TX"];
                else flags = [];
              } catch { flags = ["INVALID_B64"]; }
              const level = parseRiskLevel(flags);
              return (
                <>
                  <div className="mb-8">
                    {flags.length === 0 ? (
                      <span className="badge badge-low">✓ No risk flags</span>
                    ) : flags.map((f) => (
                      <span
                        key={f}
                        className={`badge badge-${
                          level === "high" ? "high" : level === "medium" ? "medium" : "low"
                        }`}
                        style={{ marginRight: 4 }}
                      >
                        {f}
                      </span>
                    ))}
                  </div>
                  <p className="status">
                    Tx size: {(() => { try { return atob(txB64).length; } catch { return "?"; } })()} bytes
                  </p>
                </>
              );
            })()}
          </div>
        </div>

        {/* Right — QR display */}
        <div>
          <div className="card">
            <div className="card-title">QR stream</div>

            <div className="qr-wrap">
              <canvas ref={canvasRef} width={320} height={320} />
              {!frameState && (
                <p className="status">Press "Start QR stream" to begin</p>
              )}
            </div>

            {frameState && (
              <>
                <div className="progress-wrap">
                  <div
                    className={`progress-bar${progress >= 100 ? " done" : ""}`}
                    style={{ width: `${progress}%` }}
                  />
                </div>
                <p className="status">
                  Frame {frameState.current + 1} / {frameState.frames.length}
                  &nbsp;·&nbsp;{frameState.total} unique
                </p>
              </>
            )}

            {status && !error && (
              <p className="status active mt-16">{status}</p>
            )}
            {error && <div className="alert alert-err mt-16">{error}</div>}

            <div className="btn-row">
              <button
                className="btn btn-primary"
                onClick={async () => {
                  await buildFrames();
                  if (frameState || true) {
                    // frameState set asynchronously; re-read from closure after tick
                    setTimeout(() => {
                      setFrameState((prev) => {
                        if (prev) startAnimation(prev.frames);
                        return prev;
                      });
                    }, 50);
                  }
                }}
              >
                ▶ Start QR stream
              </button>

              {running && (
                <button className="btn btn-outline" onClick={stopAnimation}>
                  ⏸ Pause
                </button>
              )}
              {!running && frameState && (
                <button
                  className="btn btn-outline"
                  onClick={() => startAnimation(frameState.frames)}
                >
                  ▶ Resume
                </button>
              )}
            </div>
          </div>

          <div className="card">
            <div className="card-title">Instructions</div>
            <ol style={{ paddingLeft: 20, fontSize: 13, color: "var(--muted)", lineHeight: 2 }}>
              <li>Paste the unsigned Solana transaction (base64) above.</li>
              <li>Set a shared password — the same value on both machines.</li>
              <li>Click <strong style={{ color: "var(--text)" }}>Start QR stream</strong>.</li>
              <li>Hold this screen up to the air-gapped device's camera.</li>
              <li>Once the air-gapped machine finishes scanning, proceed to tab 2.</li>
            </ol>
            <div className="btn-row">
              <button className="btn btn-success" onClick={onNext}>
                Continue to Sign tab →
              </button>
            </div>
          </div>
        </div>
      </div>
    </main>
  );
}