/**
 * ReceivePage — Online machine (Step 3)
 *
 * 1. Camera scans the signed-response QR stream from the air-gapped machine
 * 2. WASM WasmRecvSession reassembles & decrypts the signed response
 * 3. User selects a cluster and broadcasts the signed transaction
 * 4. Shows balance + Explorer link after broadcast
 */

import { useCallback, useEffect, useRef, useState } from "react";
import jsQR from "jsqr";
import { createRecvSession, getWasm, decodeFrameFromQr } from "../lib/wasm-api.js";
import { PasswordStrengthMeter } from "../components/PasswordStrengthMeter.js";

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

/* ── Cluster config ─────────────────────────────────────────────────────── */
type ClusterId = "devnet" | "testnet" | "mainnet" | "custom";

interface ClusterConfig {
  id: ClusterId;
  label: string;
  rpc: string;
  explorerParam: string;   // ?cluster=… or ""
  solscanParam: string;    // ?cluster=… or ""
  airdropEnabled: boolean;
}

/** Default devnet RPC. The cluster selector starts on devnet so this is also
 *  the global default when the user lands on the Receive tab. */
const DEFAULT_DEVNET_RPC = "https://api.devnet.solana.com";

const CLUSTERS: ClusterConfig[] = [
  {
    id: "devnet",
    label: "Devnet",
    rpc: DEFAULT_DEVNET_RPC,
    explorerParam: "?cluster=devnet",
    solscanParam: "?cluster=devnet",
    airdropEnabled: true,
  },
  {
    id: "testnet",
    label: "Testnet",
    rpc: "https://api.testnet.solana.com",
    explorerParam: "?cluster=testnet",
    solscanParam: "?cluster=testnet",
    airdropEnabled: true,
  },
  {
    id: "mainnet",
    label: "Mainnet-beta",
    rpc: "https://api.mainnet-beta.solana.com",
    explorerParam: "",
    solscanParam: "",
    airdropEnabled: false,
  },
  {
    id: "custom",
    label: "Custom RPC",
    rpc: "",
    explorerParam: "?cluster=custom",
    solscanParam: "",
    airdropEnabled: false,
  },
];

function getCluster(id: ClusterId, customUrl?: string): ClusterConfig {
  const c = CLUSTERS.find((x) => x.id === id) ?? CLUSTERS[0];
  if (id === "custom") {
    // If the user picked custom but never typed an RPC, fall back to devnet
    // so the broadcast button isn't dead-on-arrival.
    if (customUrl?.trim()) return { ...c, rpc: customUrl.trim() };
    return { ...c, rpc: DEFAULT_DEVNET_RPC };
  }
  return c;
}

/* ── RPC helpers ─────────────────────────────────────────────────────────── */
async function broadcastToCluster(
  txB64: string,
  rpcUrl: string
): Promise<BroadcastResult> {
  const body = JSON.stringify({
    jsonrpc: "2.0",
    id: 1,
    method: "sendTransaction",
    params: [txB64, { encoding: "base64", preflightCommitment: "processed" }],
  });

  const res = await fetch(rpcUrl, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body,
  });

  if (!res.ok) throw new Error(`RPC HTTP ${res.status}`);

  const json = (await res.json()) as {
    result?: string;
    error?: { message: string; data?: unknown };
  };

  if (json.error) return { signature: "", error: json.error.message };
  return { signature: json.result ?? "" };
}

async function getBalance(pubkey: string, rpcUrl: string): Promise<number | null> {
  try {
    const body = JSON.stringify({
      jsonrpc: "2.0",
      id: 1,
      method: "getBalance",
      params: [pubkey, { commitment: "confirmed" }],
    });
    const res = await fetch(rpcUrl, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body,
    });
    const json = (await res.json()) as { result?: { value: number } };
    return typeof json.result?.value === "number" ? json.result.value / 1e9 : null;
  } catch {
    return null;
  }
}

/* ── Camera error mapping ───────────────────────────────────────────────── */
function formatCameraError(e: unknown): string {
  if (e instanceof DOMException || (e instanceof Error && "name" in e)) {
    const name = (e as { name?: string }).name ?? "";
    switch (name) {
      case "NotAllowedError":
      case "PermissionDeniedError":
        return "Camera permission denied. Click the camera icon in your browser's address bar to enable, then click Start scanning again.";
      case "NotFoundError":
      case "DevicesNotFoundError":
        return "No camera detected. Connect a camera or use a different device.";
      case "NotReadableError":
      case "TrackStartError":
        return "Camera is in use by another application. Close other apps using the camera and retry.";
      case "OverconstrainedError":
        return "No camera matches the requested settings. Try a different device.";
      case "SecurityError":
        return "Camera blocked by browser security policy. Camera access requires HTTPS (or localhost).";
      case "AbortError":
        return "Camera start was aborted before a stream was returned. Try again.";
      default: {
        const msg = e instanceof Error ? e.message : String(e);
        return `Camera error (${name || "unknown"}): ${msg}`;
      }
    }
  }
  return `Camera error: ${e instanceof Error ? e.message : String(e)}`;
}

/* ── Helpers ─────────────────────────────────────────────────────────────── */
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

function explorerUrl(sig: string, cluster: ClusterConfig): string {
  return `https://explorer.solana.com/tx/${sig}${cluster.explorerParam}`;
}

function solscanUrl(sig: string, cluster: ClusterConfig): string {
  return `https://solscan.io/tx/${sig}${cluster.solscanParam}`;
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
  const [copied, setCopied] = useState(false);

  // Cluster selector
  const [clusterId,   setClusterId]   = useState<ClusterId>("devnet");
  const [customRpc,   setCustomRpc]   = useState("");
  const [balance,     setBalance]     = useState<number | null>(null);

  const videoRef  = useRef<HTMLVideoElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const rafRef    = useRef<number>(0);
  const recvRef   = useRef<ReturnType<typeof createRecvSession> | null>(null);

  const cluster = getCluster(clusterId, customRpc);

  /* ── Camera ─────────────────────────────────────────────────────────── */
  const startCamera = useCallback(async () => {
    setCamErr(null);
    setResponse(null);
    setBroadcastResult(null);
    setBalance(null);
    setProgress(0);
    setStatus("Starting camera…");

    if (!navigator.mediaDevices?.getUserMedia) {
      setCamErr(
        "This browser does not expose a camera API (mediaDevices.getUserMedia is undefined). " +
        "Open in Chrome/Firefox/Safari over HTTPS.",
      );
      return;
    }

    try {
      const stream = await navigator.mediaDevices.getUserMedia({
        video: { facingMode: "environment", width: { ideal: 640 }, height: { ideal: 480 } },
      });
      if (!videoRef.current) return;
      videoRef.current.srcObject = stream;
      await videoRef.current.play();
      setScanning(true);
      setStatus("Scanning — point camera at the air-gapped machine's screen.");

      if (getWasm()) {
        recvRef.current = createRecvSession(sharedPassword);
      }
    } catch (e: unknown) {
      setCamErr(formatCameraError(e));
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
          // Phase 1 — frame ingest (parse errors silently retry)
          let done = false;
          try {
            const frameBytes = decodeFrameFromQr(code.data);
            done = recv.ingest_frame(frameBytes);
            const pct  = Math.round(recv.progress() * 100);
            setProgress(pct);
            setStatus(`Receiving… ${pct}%`);
          } catch {
            /* malformed/duplicate frame — keep scanning */
          }

          // Phase 2 — AEAD decryption (a failure = wrong password almost certainly)
          if (done) {
            try {
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
            } catch (e: unknown) {
              const msg = e instanceof Error ? e.message : String(e);
              setCamErr(
                `Decryption failed — most likely a password mismatch with the air-gapped machine. ` +
                  `Verify both sides typed the same shared password and scan the stream again. ` +
                  `(Underlying error: ${msg})`,
              );
              stopCamera();
              setStatus("✗ Decryption failed.");
              return;
            }
          }
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

  /* ── Balance fetch (after response) ─────────────────────────────────── */
  useEffect(() => {
    if (!response || !response.pubkey || response.pubkey === "?" || !cluster.rpc) return;
    let cancelled = false;
    getBalance(response.pubkey, cluster.rpc).then((b) => {
      if (!cancelled) setBalance(b);
    });
    return () => { cancelled = true; };
  }, [response, cluster.rpc]);

  /* ── Broadcast ──────────────────────────────────────────────────────── */
  const handleBroadcast = useCallback(async () => {
    if (!response?.tx || !cluster.rpc) return;
    setBroadcasting(true);
    setBroadcastResult(null);
    setStatus(`Broadcasting to ${cluster.label}…`);
    try {
      const result = await broadcastToCluster(response.tx, cluster.rpc);
      setBroadcastResult(result);
      if (result.error) {
        setStatus(`✗ Broadcast failed: ${result.error}`);
      } else {
        setStatus(`✓ Transaction submitted to ${cluster.label}!`);
      }
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      setBroadcastResult({ signature: "", error: msg });
      setStatus(`✗ Broadcast error: ${msg}`);
    } finally {
      setBroadcasting(false);
    }
  }, [response, cluster]);

  /* ── Copy signature ──────────────────────────────────────────────────── */
  const handleCopy = useCallback((text: string) => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1800);
    });
  }, []);

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
        {/* Left — camera + cluster */}
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
              autoComplete="off"
              spellCheck={false}
            />
            <PasswordStrengthMeter password={sharedPassword} alwaysVisible />
          </div>

          {/* Cluster selector */}
          <div className="card">
            <div className="card-title">Target cluster</div>
            <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginBottom: 8 }}>
              {CLUSTERS.map((c) => (
                <button
                  key={c.id}
                  className={`btn btn-sm ${clusterId === c.id ? "btn-primary" : "btn-outline"}`}
                  onClick={() => setClusterId(c.id)}
                  style={{ fontSize: 13, padding: "4px 12px" }}
                >
                  {c.label}
                </button>
              ))}
            </div>
            {clusterId === "custom" && (
              <input
                type="text"
                placeholder="https://my-rpc.example.com"
                value={customRpc}
                onChange={(e) => setCustomRpc(e.target.value)}
                style={{ marginTop: 4 }}
              />
            )}
            <p style={{ fontSize: 12, color: "var(--muted)", marginTop: 6 }}>
              {cluster.rpc || "Enter custom RPC URL above"}
            </p>
          </div>

          <div className="card">
            <div className="card-title">Scan signed response QR stream</div>
            <p className="status mb-12">
              Point the camera at the air-gapped machine's QR stream (tab 2, right panel).
            </p>

            <canvas ref={canvasRef} style={{ display: "none" }} />

            <div className={`scanner-wrap${scanning ? " scanning" : ""}`}>
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
                  onClick={() => {
                    setResponse(null);
                    setBroadcastResult(null);
                    setBalance(null);
                  }}
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
              <li>Select the target <strong style={{ color: "var(--text)" }}>cluster</strong> (devnet for testing).</li>
              <li>Click <strong style={{ color: "var(--text)" }}>Start scanning</strong> and point the camera at the air-gapped machine's QR stream.</li>
              <li>Once scanning completes, review the signature details on the right.</li>
              <li>Click <strong style={{ color: "var(--text)" }}>Broadcast</strong> to submit the transaction.</li>
              <li>Copy the signature and verify on Solana Explorer or Solscan.</li>
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
                      <td className="mono" style={{ wordBreak: "break-all" }}>{response.pubkey}</td>
                    </tr>
                    {balance !== null && (
                      <tr>
                        <th>Balance</th>
                        <td className="mono">{balance.toFixed(6)} SOL</td>
                      </tr>
                    )}
                    <tr>
                      <th>Signature (first 32B)</th>
                      <td className="mono">{response.sig.slice(0, 64)}{response.sig.length > 64 ? "…" : ""}</td>
                    </tr>
                    <tr>
                      <th>Tx (base64, truncated)</th>
                      <td className="mono">{response.tx.slice(0, 64)}{response.tx.length > 64 ? "…" : ""}</td>
                    </tr>
                    <tr>
                      <th>Cluster</th>
                      <td className="mono">{cluster.label}</td>
                    </tr>
                  </tbody>
                </table>
              </div>

              {/* Broadcast card */}
              <div className="card">
                <div className="card-title">🚀 Broadcast to {cluster.label}</div>
                {clusterId === "custom" && !customRpc.trim() && (
                  <div className="alert alert-info" style={{ marginBottom: 12 }}>
                    Custom RPC empty — falling back to <code>{DEFAULT_DEVNET_RPC}</code>.
                    Enter a URL in the cluster selector to override.
                  </div>
                )}
                {!cluster.rpc && (
                  <div className="alert alert-err" style={{ marginBottom: 12 }}>
                    No RPC URL configured for this cluster. Switch to Devnet or enter a custom URL.
                  </div>
                )}
                {cluster.id === "devnet" && (
                  <div className="alert alert-info" style={{ marginBottom: 12, fontSize: 13 }}>
                    Using <strong>devnet</strong> · the SystemProgram::Transfer message produced by
                    "Load demo tx" is broadcastable end-to-end. If the demo wallet runs out of SOL,
                    use the <strong>Request Airdrop</strong> card above before broadcasting.
                  </div>
                )}

                {broadcastResult && (
                  <div className={`alert ${broadcastResult.error ? "alert-err" : "alert-ok"}`} style={{ marginBottom: 12 }}>
                    {broadcastResult.error ? (
                      <>✗ RPC error: {broadcastResult.error}</>
                    ) : (
                      <div>
                        <div style={{ marginBottom: 6 }}>✓ Transaction submitted to <strong>{cluster.label}</strong>!</div>
                        <div
                          className="mono"
                          style={{
                            fontSize: 12,
                            wordBreak: "break-all",
                            background: "rgba(0,43,91,0.06)",
                            borderRadius: 4,
                            padding: "4px 6px",
                            marginBottom: 8,
                          }}
                        >
                          {broadcastResult.signature}
                        </div>
                        <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
                          <button
                            className="btn btn-sm btn-outline"
                            onClick={() => handleCopy(broadcastResult.signature)}
                            style={{ fontSize: 12 }}
                          >
                            {copied ? "✓ Copied!" : "📋 Copy sig"}
                          </button>
                          <a
                            href={explorerUrl(broadcastResult.signature, cluster)}
                            target="_blank"
                            rel="noopener noreferrer"
                            className="btn btn-sm btn-outline"
                            style={{ fontSize: 12, textDecoration: "none" }}
                          >
                            🔍 Explorer ↗
                          </a>
                          {cluster.solscanParam !== undefined && (
                            <a
                              href={solscanUrl(broadcastResult.signature, cluster)}
                              target="_blank"
                              rel="noopener noreferrer"
                              className="btn btn-sm btn-outline"
                              style={{ fontSize: 12, textDecoration: "none" }}
                            >
                              📊 Solscan ↗
                            </a>
                          )}
                        </div>
                      </div>
                    )}
                  </div>
                )}

                <div className="btn-row">
                  <button
                    className="btn btn-success"
                    onClick={handleBroadcast}
                    disabled={broadcasting || !response.tx || !!broadcastResult || !cluster.rpc}
                  >
                    {broadcasting ? "Broadcasting…" : `🚀 Broadcast to ${cluster.label}`}
                  </button>
                  {broadcastResult && (
                    <button
                      className="btn btn-outline"
                      onClick={() => setBroadcastResult(null)}
                    >
                      ↺ Retry
                    </button>
                  )}
                </div>
              </div>

              {/* CLI equivalent */}
              <div className="card">
                <div className="card-title">CLI equivalent</div>
                <p style={{ fontSize: 12, color: "var(--muted)", marginBottom: 8 }}>
                  Reproduce this broadcast with the AirSign CLI:
                </p>
                <div
                  className="mono"
                  style={{
                    background: "rgba(0,43,91,0.06)",
                    borderRadius: 6,
                    padding: "10px 12px",
                    fontSize: 12,
                    whiteSpace: "pre-wrap",
                    wordBreak: "break-all",
                    color: "var(--accent2)",
                    marginBottom: 8,
                  }}
                >
                  {`airsign broadcast sign_response.json --cluster ${clusterId}`}
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
                <p style={{ fontSize: 13, color: "var(--muted)", marginTop: 12 }}>
                  Selected cluster: <strong style={{ color: "var(--text)" }}>{cluster.label}</strong>
                </p>
              </div>
            </div>
          )}
        </div>
      </div>
    </main>
  );
}