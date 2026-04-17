/**
 * ReceivePage — Online machine (Step 3)
 *
 * 1. Camera scans the signed-response QR stream from the air-gapped machine
 * 2. WASM WasmRecvSession reassembles & decrypts the signed response
 * 3. User can broadcast the signed transaction to Solana devnet
 */

import { useCallback, useEffect, useRef, useState } from "react";
import jsQR from "jsqr";

/* ── Types ──────────────────────────────────────────────────────────────── */
interface Props {
  sharedPassword: string;
  onPasswordChange: (p: string) => void;
}

interface SignedResponse {
  pubkey: string;
  sig: string;
  tx: string;        // base64 signed tx
  raw: string;       // raw JSON payload
}

interface BroadcastResult {
  signature: string;
  slot?: number;
  error?: string;
}

/* ── Helpers ────────────────────────────────────────────────────────────── */
function parseSignedResponse(raw: string): SignedResponse | null {
  try {
    const obj = JSON.parse(raw) as Record<string, unknown>;
    if (
      typeof obj.pubkey === "string" &&
      typeof obj.sig    === "string" &&
      typeof obj.tx     === "string"
    ) {
      return { pubkey: obj.pubkey, sig: obj.sig, tx: obj.tx, raw };
    }
  } catch { /* ignore */ }
  return null;
}

const DEVNET_RPC = "https://api.devnet.solana.com";

async function broadcastToDevnet(txB64: string): Promise<BroadcastResult> {
  const body = JSON.stringify({
    jsonrpc: "2.0",
    id: 1,
    method: "sendTransaction",
    params: [
      txB64,
      { encoding: "base64", preflightCommitment: "processed" },
    ],
  });

  const res = await fetch(DEVNET_RPC, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body,
  });

  if (!res.ok) throw new Error(`RPC HTTP ${res.status}`);

  const json = (await res.json()) as {
    result?: string;
    error?: { message: string };
  };

  if (json.error) return { signature: "", error: json.error.message };
  return { signature: json.result ?? "" };
}

/* ── Component ──────────────────────────────────────────────────────────── */
export function ReceivePage({ sharedPassword, onPasswordChange }: Props) {
  const [scanning,  setScanning]  = useState(false);
  const [progress,  setProgress]  = useState(0);
  const [response,  setResponse]  = useState<SignedResponse | null>(null);
  const [status,    setStatus]    = useState("");
  const [camErr,    setCamErr]    = useState<string | null>(null);
  const [broadcastResult, setBroadcastResult] = useState<BroadcastResult | null>(null);
  const [broadcasting, setBroadcasting] = useState(false);

  const videoRef  = useRef<HTMLVideoElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const rafRef    = useRef<number>(0);
  const recvRef   = useRef<{
    ingest_frame: (f: string) => boolean;
    progress: () => number;
    get_data: () => Uint8Array;
    free: () => void;
  } | null>(null);

  /* ── Camera ─────────────────────────────────────────────────────────── */
  const startCamera = useCallback(async () => {
    setCamErr(null);
    setResponse(null);
    setBroadcastResult(null);
    setProgress(0);
    setStatus("Starting camera…");

    try {
      const stream = await navigator.mediaDevices.getUserMedia({
        video: { facingMode: "environment", width: { ideal: 640 }, height: { ideal: 480 } },
      });
      if (!videoRef.current) return;
      videoRef.current.srcObject = stream;
      await videoRef.current.play();
      setScanning(true);
      setStatus("Scanning — point camera at the air-gapped machine's screen.");

      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const wasm = (globalThis as any).__airsign_wasm__;
      if (wasm?.WasmRecvSession) {
        recvRef.current = new wasm.WasmRecvSession(sharedPassword);
      }
    } catch (e: unknown) {
      setCamErr(`Camera error: ${e instanceof Error ? e.message : String(e)}`);
      setScanning(false);
    }
  }, [sharedPassword]);

  const stopCamera = useCallback(() => {
    cancelAnimationFrame(rafRef.current);
    if (videoRef.current?.srcObject) {
      (videoRef.current.srcObject as MediaStream).getTracks().forEach((t) => t.stop());
      videoRef.current.srcObject = null;
    }
    if (recvRef.current) { recvRef.current.free(); recvRef.current = null; }
    setScanning(false);
  }, []);

  /* ── Decode loop ────────────────────────────────────────────────────── */
  useEffect(() => {
    if (!scanning) return;

    const tick = () => {
      const video  = videoRef.current;
      const canvas = canvasRef.current;
      if (!video || !canvas || video.readyState < 2) {
        rafRef.current = requestAnimationFrame(tick); return;
      }
      const ctx = canvas.getContext("2d");
      if (!ctx) { rafRef.current = requestAnimationFrame(tick); return; }

      canvas.width  = video.videoWidth;
      canvas.height = video.videoHeight;
      ctx.drawImage(video, 0, 0);

      const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);
      const code = jsQR(imageData.data, imageData.width, imageData.height);

      if (code?.data) {
        const recv = recvRef.current;
        if (recv) {
          try {
            const done = recv.ingest_frame(code.data);
            const pct  = Math.round(recv.progress() * 100);
            setProgress(pct);
            setStatus(`Receiving… ${pct}%`);
            if (done) {
              const bytes = recv.get_data();
              const text  = new TextDecoder().decode(bytes);
              const parsed = parseSignedResponse(text);
              if (parsed) {
                setResponse(parsed);
                stopCamera();
                setStatus("✓ Signed response received & decrypted.");
              } else {
                setStatus("✓ Data received — could not parse as signed response (raw shown below).");
                setResponse({ pubkey: "?", sig: "?", tx: "", raw: text });
                stopCamera();
              }
              return;
            }
          } catch { /* ignore frame errors */ }
        } else {
          // Simulation: treat QR payload directly as the response JSON
          const parsed = parseSignedResponse(code.data);
          if (parsed) {
            setResponse(parsed);
          } else {
            setResponse({ pubkey: "simulation", sig: "simulation", tx: "", raw: code.data });
          }
          stopCamera();
          setStatus("✓ QR received (simulation mode).");
          return;
        }
      }

      rafRef.current = requestAnimationFrame(tick);
    };

    rafRef.current = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafRef.current);
  }, [scanning, stopCamera]);

  /* ── Broadcast ──────────────────────────────────────────────────────── */
  const handleBroadcast = useCallback(async () => {
    if (!response?.tx) return;
    setBroadcasting(true);
    setBroadcastResult(null);
    setStatus("Broadcasting to Solana devnet…");
    try {
      const result = await broadcastToDevnet(response.tx);
      setBroadcastResult(result);
      setStatus(result.error ? `✗ Broadcast failed: ${result.error}` : `✓ Broadcast accepted — sig: ${result.signature.slice(0, 16)}…`);
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      setBroadcastResult({ signature: "", error: msg });
      setStatus(`✗ Broadcast error: ${msg}`);
    } finally {
      setBroadcasting(false);
    }
  }, [response]);

  useEffect(() => () => {
    cancelAnimationFrame(rafRef.current);
    if (recvRef.current) { recvRef.current.free(); recvRef.current = null; }
  }, []);

  /* ── Render ─────────────────────────────────────────────────────────── */
  return (
    <main className="page">
      {/* Step indicator */}
      <div className="steps">
        <div className="step-item done">
          <div className="step-dot">✓</div>
          <span>Prepare & Send</span>
        </div>
        <div className="step-line" />
        <div className="step-item done">
          <div className="step-dot">✓</div>
          <span>Air-gap Sign</span>
        </div>
        <div className="step-line" />
        <div className="step-item active">
          <div className="step-dot">3</div>
          <span>Receive & Broadcast</span>
        </div>
      </div>

      <div className="two-col">
        {/* Left — camera */}
        <div>
          <div className="card">
            <div className="card-title">Shared password</div>
            <label htmlFor="pw-recv">Must match the password used on both machines.</label>
            <input
              id="pw-recv"
              type="text"
              value={sharedPassword}
              onChange={(e) => onPasswordChange(e.target.value)}
              disabled={scanning}
            />
          </div>

          <div className="card">
            <div className="card-title">Scan signed response QR stream</div>
            <p className="status mb-12">
              Point the camera at the air-gapped machine's QR stream (tab 2, right panel).
            </p>

            <canvas ref={canvasRef} style={{ display: "none" }} />

            <div className="scanner-wrap">
              <video
                ref={videoRef}
                muted
                playsInline
                style={{ width: "100%", borderRadius: 8, background: "#000" }}
              />
            </div>

            {scanning && (
              <div style={{ marginTop: 12 }}>
                <div className="progress-wrap">
                  <div className="progress-bar" style={{ width: `${progress}%` }} />
                </div>
                <p className="status active">{status}</p>
              </div>
            )}

            {camErr && <div className="alert alert-err" style={{ marginTop: 12 }}>{camErr}</div>}

            <div className="btn-row">
              {!scanning && !response && (
                <button className="btn btn-primary" onClick={startCamera}>
                  📷 Start scanning
                </button>
              )}
              {scanning && (
                <button className="btn btn-outline" onClick={stopCamera}>
                  ⏹ Stop camera
                </button>
              )}
              {response && (
                <button
                  className="btn btn-outline"
                  onClick={() => { setResponse(null); setBroadcastResult(null); }}
                >
                  🔄 Scan again
                </button>
              )}
            </div>
          </div>

          {/* Instructions */}
          <div className="card">
            <div className="card-title">Instructions</div>
            <ol style={{ paddingLeft: 20, fontSize: 13, color: "var(--muted)", lineHeight: 2 }}>
              <li>Switch to this tab on the <strong style={{ color: "var(--text)" }}>online machine</strong>.</li>
              <li>Click <strong style={{ color: "var(--text)" }}>Start scanning</strong> and point the camera at the air-gapped machine's QR stream.</li>
              <li>Once scanning completes, review the signature details on the right.</li>
              <li>Click <strong style={{ color: "var(--text)" }}>Broadcast to Devnet</strong> to submit the transaction.</li>
              <li>Copy the transaction signature and verify on Solana Explorer.</li>
            </ol>
          </div>
        </div>

        {/* Right — signed response + broadcast */}
        <div>
          {response ? (
            <>
              <div className="card">
                <div className="card-title">Signed response</div>
                {status && (
                  <div className={`alert ${status.startsWith("✓") ? "alert-ok" : "alert-info"}`}>
                    {status}
                  </div>
                )}
                <table className="ix-table">
                  <tbody>
                    <tr>
                      <th>Signer pubkey</th>
                      <td className="mono">{response.pubkey}</td>
                    </tr>
                    <tr>
                      <th>Signature (first 32B)</th>
                      <td className="mono">{response.sig.slice(0, 64)}{response.sig.length > 64 ? "…" : ""}</td>
                    </tr>
                    <tr>
                      <th>Tx (base64, truncated)</th>
                      <td className="mono">{response.tx.slice(0, 64)}{response.tx.length > 64 ? "…" : ""}</td>
                    </tr>
                  </tbody>
                </table>
              </div>

              <div className="card">
                <div className="card-title">Broadcast</div>
                <div className="alert alert-info" style={{ marginBottom: 16 }}>
                  This demo uses Solana <strong>devnet</strong>. The demo transaction stub will likely fail
                  signature verification — this is expected. In production, a real unsigned transaction
                  would be constructed with AirSign CLI and signed with a real keypair.
                </div>

                {broadcastResult && (
                  <div className={`alert ${broadcastResult.error ? "alert-err" : "alert-ok"}`}>
                    {broadcastResult.error ? (
                      <>✗ RPC error: {broadcastResult.error}</>
                    ) : (
                      <>
                        ✓ Transaction submitted!
                        <br />
                        <span className="mono">{broadcastResult.signature}</span>
                        <br />
                        <a
                          href={`https://explorer.solana.com/tx/${broadcastResult.signature}?cluster=devnet`}
                          target="_blank"
                          rel="noopener noreferrer"
                          style={{ color: "var(--accent2)" }}
                        >
                          View on Solana Explorer ↗
                        </a>
                      </>
                    )}
                  </div>
                )}

                <div className="btn-row">
                  <button
                    className="btn btn-success"
                    onClick={handleBroadcast}
                    disabled={broadcasting || !response.tx || !!broadcastResult}
                  >
                    {broadcasting ? "Broadcasting…" : "🚀 Broadcast to Devnet"}
                  </button>
                </div>
              </div>

              {/* Raw payload */}
              <div className="card">
                <div className="card-title">Raw payload</div>
                <textarea
                  readOnly
                  rows={6}
                  value={response.raw}
                  style={{ cursor: "text" }}
                />
              </div>
            </>
          ) : (
            <div className="card">
              <div className="card-title">Waiting for signed response…</div>
              <div className="hero" style={{ padding: "40px 16px" }}>
                <p className="status" style={{ fontSize: 48, marginBottom: 12 }}>📨</p>
                <p className="status">
                  Scan the QR stream from the air-gapped machine to receive the signed transaction.
                </p>
              </div>
            </div>
          )}
        </div>
      </div>
    </main>
  );
}