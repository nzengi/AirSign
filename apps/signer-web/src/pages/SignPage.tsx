/**
 * SignPage — Air-gapped machine (Step 2)
 *
 * 1. Camera scans the fountain-coded QR stream from the online machine
 * 2. WASM WasmRecvSession reassembles & decrypts the transaction
 * 3. User reviews the transaction and signs with an ephemeral Ed25519 keypair
 * 4. The signed response is re-encoded as a new QR stream for the online machine
 */

import { useCallback, useEffect, useRef, useState } from "react";
import jsQR from "jsqr";
import QRCode from "qrcode";
import { Message, Transaction } from "@solana/web3.js";
import {
  createSendSession,
  createRecvSession,
  getWasm,
  encodeFrameForQr,
  decodeFrameFromQr,
} from "../lib/wasm-api.js";
import {
  base58Encode,
  buildSignedTransaction,
  bytesToBase64,
} from "../lib/solana-tx.js";
import { walletFromBase58Secret } from "../lib/demo-wallet.js";
import { useWallet } from "../lib/wallet-ctx.js";
import { PasswordStrengthMeter } from "../components/PasswordStrengthMeter.js";

type KeyMode = "wallet" | "import";

/* ── Types ──────────────────────────────────────────────────────────────── */
interface Props {
  sharedPassword: string;
  onPasswordChange: (p: string) => void;
  onNext: () => void;
}

interface DecodedTx {
  bytes: Uint8Array;
  b64: string;
  sizeBytes: number;
}

interface SignedState {
  frames: string[];
  current: number;
}

interface SignResult {
  pubkey: Uint8Array;
  sig: Uint8Array;
  pkB58: string;
  /** Full wire-format transaction when signed via wallet's signTransaction.
   *  When set, broadcast THIS instead of rebuilding from the original
   *  message bytes — web3.js may re-serialize the message and Phantom signs
   *  the re-serialized form. */
  fullTx?: Uint8Array;
}

function toBase58(bytes: Uint8Array): string { return base58Encode(bytes); }

/* ── Camera error mapping ───────────────────────────────────────────────── */
function formatCameraError(e: unknown): string {
  if (e instanceof DOMException || (e instanceof Error && "name" in e)) {
    const name = (e as { name?: string }).name ?? "";
    switch (name) {
      case "NotAllowedError":
      case "PermissionDeniedError":
        return "Camera permission denied. Click the camera icon in your browser's address bar " +
               "to enable, then click Start scanning again. Or use the manual paste fallback below.";
      case "NotFoundError":
      case "DevicesNotFoundError":
        return "No camera detected. Connect a camera or use the manual paste fallback below.";
      case "NotReadableError":
      case "TrackStartError":
        return "Camera is in use by another application. Close any app or browser tab using the camera and retry.";
      case "OverconstrainedError":
        return "No camera matches the requested settings (rear-facing, 640×480). Try a different device or use manual paste.";
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

/* ── Component ──────────────────────────────────────────────────────────── */
export function SignPage({ sharedPassword, onPasswordChange, onNext }: Props) {
  const { wallet, connect, connecting, phantomInstalled } = useWallet();
  const [scanning, setScanning]     = useState(false);
  const [progress, setProgress]     = useState(0);
  const [decoded,  setDecoded]      = useState<DecodedTx | null>(null);
  const [signedState, setSignedState] = useState<SignedState | null>(null);
  const [pubkeyB58, setPubkeyB58]   = useState<string | null>(null);
  const [sigHex,    setSigHex]      = useState<string | null>(null);
  const [status,    setStatus]      = useState("");
  const [error,     setError]       = useState<string | null>(null);
  const [camErr,    setCamErr]      = useState<string | null>(null);
  const [fps, setFps]               = useState(6);
  const [animRunning, setAnimRunning] = useState(false);
  const [keyMode, setKeyMode]       = useState<KeyMode>("wallet");
  const [importSecret, setImportSecret] = useState("");
  const [manualPaste, setManualPaste] = useState("");
  const [manualOpen, setManualOpen]  = useState(false);

  /** Sign with the selected key source.
   *
   * For the wallet path we use Phantom's `signTransaction` rather than
   * `signMessage`. Phantom's signMessage refuses payloads that look like a
   * Solana transaction (anti-phishing heuristic), so the only correct way to
   * get Phantom to sign a real transfer is to hand it a Transaction object.
   */
  async function signWith(message: Uint8Array): Promise<SignResult> {
    if (keyMode === "wallet") {
      if (!wallet) throw new Error("connect a wallet first");

      // Try to parse as a Solana legacy message. If it parses, sign via
      // signTransaction. If not, fall back to signMessage for arbitrary bytes.
      let tx: Transaction | null = null;
      try {
        tx = Transaction.populate(Message.from(message));
      } catch { /* not a Solana message — fall through */ }

      if (tx) {
        const { signature, serialized } = await wallet.signTransaction(tx);
        return {
          pubkey: wallet.pubkey,
          sig: signature,
          pkB58: wallet.pubkeyB58,
          fullTx: serialized,
        };
      }
      const sig = await wallet.signMessageBytes(message);
      return { pubkey: wallet.pubkey, sig, pkB58: wallet.pubkeyB58 };
    }
    // import
    const w = walletFromBase58Secret(importSecret);
    const sig = w.sign(message);
    const result = { pubkey: w.pubkey, sig, pkB58: w.pubkeyB58 };
    w.destroy();
    return result;
  }

  const videoRef   = useRef<HTMLVideoElement>(null);
  const canvasRef  = useRef<HTMLCanvasElement>(null);
  const qrCanvasRef = useRef<HTMLCanvasElement>(null);
  const rafRef     = useRef<number>(0);
  const timerRef   = useRef<ReturnType<typeof setInterval> | null>(null);
  const idxRef     = useRef(0);
  const recvRef    = useRef<ReturnType<typeof createRecvSession> | null>(null);

  /* ── Camera setup ─────────────────────────────────────────────────────── */
  const startCamera = useCallback(async () => {
    setCamErr(null);
    setError(null);
    setDecoded(null);
    setProgress(0);
    setStatus("Starting camera…");

    if (!navigator.mediaDevices?.getUserMedia) {
      setCamErr(
        "This browser does not expose a camera API (mediaDevices.getUserMedia is undefined). " +
        "Use the manual paste fallback below — or open in Chrome/Firefox/Safari over HTTPS.",
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
      setStatus("Scanning — point camera at the QR stream on the online machine.");

      // Init WASM recv session
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
      (videoRef.current.srcObject as MediaStream)
        .getTracks()
        .forEach((t) => t.stop());
      videoRef.current.srcObject = null;
    }
    if (recvRef.current) { recvRef.current.free(); recvRef.current = null; }
    setScanning(false);
    setStatus("");
  }, []);

  /* ── Manual paste fallback (no camera) ───────────────────────────────── */
  const submitManualPaste = useCallback(() => {
    setError(null);
    setCamErr(null);
    if (!manualPaste.trim()) {
      setError("Paste at least one frame's payload.");
      return;
    }
    if (!getWasm()) {
      setError("WASM not loaded — manual paste decryption unavailable.");
      return;
    }
    const recv = createRecvSession(sharedPassword);
    let done = false;
    let frameCount = 0;
    // Accept newline-separated frames so the operator can paste several at once.
    for (const line of manualPaste.split(/\r?\n/)) {
      const t = line.trim();
      if (!t) continue;
      try {
        const frameBytes = decodeFrameFromQr(t);
        done = recv.ingest_frame(frameBytes);
        frameCount += 1;
        if (done) break;
      } catch { /* ignore malformed line */ }
    }
    if (!done) {
      const pct = Math.round(recv.progress() * 100);
      recv.free();
      setError(
        `Reassembly incomplete after ${frameCount} pasted frame(s) (${pct}% reconstructed). ` +
        `Paste more frames separated by newlines and try again.`,
      );
      return;
    }
    try {
      const bytes = recv.get_data();
      setDecoded({
        bytes,
        b64: btoa(String.fromCharCode(...bytes)),
        sizeBytes: bytes.length,
      });
      setStatus(`✓ Reassembled ${bytes.length} bytes from ${frameCount} pasted frame(s).`);
      setManualOpen(false);
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(
        `Decryption failed (likely wrong password): ${msg}. ` +
        `Verify the shared password matches the sender and try again.`,
      );
    } finally {
      recv.free();
    }
  }, [manualPaste, sharedPassword]);

  /* ── QR decode loop ───────────────────────────────────────────────────── */
  useEffect(() => {
    if (!scanning) return;

    const tick = () => {
      const video  = videoRef.current;
      const canvas = canvasRef.current;
      if (!video || !canvas || video.readyState < 2) {
        rafRef.current = requestAnimationFrame(tick);
        return;
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
          // Phase 1 — frame ingest (parse errors are ignored; the fountain
          // code is loss-tolerant so we silently retry on the next frame).
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

          // Phase 2 — AEAD decryption (a failure here is FATAL; it almost
          // always means the operator typed the wrong password). We stop
          // the camera and surface a clear error so they can fix it.
          if (done) {
            try {
              const bytes = recv.get_data();
              setDecoded({
                bytes,
                b64: btoa(String.fromCharCode(...bytes)),
                sizeBytes: bytes.length,
              });
              stopCamera();
              setStatus("✓ Transaction received & decrypted.");
              return;
            } catch (e: unknown) {
              const msg = e instanceof Error ? e.message : String(e);
              setError(
                `Decryption failed — most likely a password mismatch with the online machine. ` +
                  `Verify both sides typed the same shared password and scan the stream again. ` +
                  `(Underlying error: ${msg})`,
              );
              stopCamera();
              setStatus("✗ Decryption failed.");
              return;
            }
          }
        } else {
          // No WASM — simulation mode: treat any QR data as the "transaction"
          const simBytes = new TextEncoder().encode(code.data);
          setDecoded({
            bytes: simBytes,
            b64: btoa(code.data.slice(0, 200)),
            sizeBytes: simBytes.length,
          });
          stopCamera();
          setStatus("✓ QR data received (simulation mode).");
          return;
        }
      }

      rafRef.current = requestAnimationFrame(tick);
    };

    rafRef.current = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafRef.current);
  }, [scanning, stopCamera]);

  /* ── Sign ─────────────────────────────────────────────────────────────── */
  const handleSign = useCallback(async () => {
    if (!decoded) return;
    setError(null);
    setStatus("Signing…");

    try {
      const { pubkey, sig, pkB58, fullTx } = await signWith(decoded.bytes);
      setPubkeyB58(pkB58);
      setSigHex(Array.from(sig).map((b) => b.toString(16).padStart(2, "0")).join(""));

      // Prefer the wire-format tx returned by Phantom — web3.js's Message
      // round-trip can produce bytes that differ from our hand-rolled
      // builder, and Phantom signs the *re-serialized* form. Rebuilding the
      // tx from `decoded.bytes` (original) + Phantom's sig would fail
      // signature verification on broadcast.
      //
      // Otherwise (imported-secret path or non-Solana payload) we know the
      // signature is over `decoded.bytes` exactly, so [shortvec(1)][sig][msg]
      // produces a valid wire tx.
      const fullTxB64 = fullTx
        ? bytesToBase64(fullTx)
        : sig.length === 64
        ? bytesToBase64(buildSignedTransaction(sig, decoded.bytes))
        : decoded.b64;

      // Build response payload
      const response = JSON.stringify({
        proto: "airsign/1",
        type: "signed_response",
        pubkey: toBase58(pubkey),
        sig: Array.from(sig).map((b) => b.toString(16).padStart(2, "0")).join(""),
        tx: fullTxB64,
      });

      // Encode response into QR frames via WASM (or single static frame)
      let frames: string[];

      if (getWasm()) {
        const respBytes = new TextEncoder().encode(response);
        const session = createSendSession(respBytes, sharedPassword, "signed-response.json");
        const total = session.total_frames();
        frames = [];
        for (let i = 0; i < total * 3; i++) {
          const f = session.next_frame();
          if (!f || typeof f === "string") break;
          frames.push(encodeFrameForQr(f));
        }
        session.free();
      } else {
        frames = [response];
      }

      idxRef.current = 0;
      setSignedState({ frames, current: 0 });
      setStatus(`✓ Signed with pubkey ${pkB58.slice(0, 8)}… — QR stream ready.`);

      // Draw first frame
      await drawQrFrame(frames, 0);
    } catch (e: unknown) {
      const raw = e instanceof Error ? e.message : String(e);
      setError(raw);
    }
  }, [decoded, sharedPassword, keyMode, importSecret, wallet]);

  /* ── QR animation for signed response ────────────────────────────────── */
  const drawQrFrame = useCallback(async (frames: string[], idx: number) => {
    const canvas = qrCanvasRef.current;
    if (!canvas || frames.length === 0) return;
    try {
      await QRCode.toCanvas(canvas, frames[idx % frames.length], {
        width: 360,
        margin: 1,
        color: { dark: "#ffffff", light: "#000000" },
      });
    } catch { /* ignore */ }
  }, []);

  const startAnim = useCallback((frames: string[]) => {
    if (timerRef.current) clearInterval(timerRef.current);
    setAnimRunning(true);
    timerRef.current = setInterval(async () => {
      idxRef.current = (idxRef.current + 1) % frames.length;
      setSignedState((prev) => prev ? { ...prev, current: idxRef.current } : prev);
      await drawQrFrame(frames, idxRef.current);
    }, Math.round(1000 / fps));
  }, [fps, drawQrFrame]);

  const stopAnim = useCallback(() => {
    if (timerRef.current) { clearInterval(timerRef.current); timerRef.current = null; }
    setAnimRunning(false);
  }, []);

  useEffect(() => () => {
    cancelAnimationFrame(rafRef.current);
    if (timerRef.current) clearInterval(timerRef.current);
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
        <div className="step-item active">
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
        {/* Left — camera + controls */}
        <div>
          <div className="card">
            <div className="card-title">Shared password</div>
            <label htmlFor="pw-sign">Must match the password on the online machine.</label>
            <input
              id="pw-sign"
              type="text"
              value={sharedPassword}
              onChange={(e) => onPasswordChange(e.target.value)}
              disabled={scanning}
              autoComplete="off"
              spellCheck={false}
            />
            <PasswordStrengthMeter password={sharedPassword} alwaysVisible />
          </div>

          <div className="card">
            <div className="card-title">Scan QR stream</div>
            <p className="status mb-12">
              Run this tab on the air-gapped machine. Point the camera at the online machine's screen.
            </p>

            {/* Hidden canvas for frame extraction */}
            <canvas ref={canvasRef} style={{ display: "none" }} />

            <div className={`scanner-wrap${scanning ? " scanning" : ""}`}>
              <video ref={videoRef} muted playsInline style={{ width: "100%", borderRadius: 8, background: "#000" }} />
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
              {!scanning && !decoded && (
                <button className="btn btn-primary" onClick={startCamera}>
                  📷 Start scanning
                </button>
              )}
              {scanning && (
                <button className="btn btn-outline" onClick={stopCamera}>
                  ⏹ Stop camera
                </button>
              )}
              {!decoded && (
                <button
                  className="btn btn-outline btn-sm"
                  onClick={() => setManualOpen((v) => !v)}
                  title="Paste QR payloads from another tool if camera access isn't possible"
                >
                  ⌨ {manualOpen ? "Hide" : "Manual paste fallback"}
                </button>
              )}
              {decoded && (
                <button
                  className="btn btn-outline"
                  onClick={() => { setDecoded(null); setSignedState(null); setPubkeyB58(null); setSigHex(null); }}
                >
                  🔄 Scan again
                </button>
              )}
            </div>

            {manualOpen && !decoded && (
              <div style={{ marginTop: 12, padding: 10, border: "1px dashed var(--border)", borderRadius: 6 }}>
                <p style={{ fontSize: 12, color: "var(--muted)", marginBottom: 6 }}>
                  Paste base64-encoded frames from the sender (one per line). The fountain code only
                  needs ~K frames to reassemble — paste them all and submit.
                </p>
                <textarea
                  rows={4}
                  value={manualPaste}
                  onChange={(e) => setManualPaste(e.target.value)}
                  placeholder="frame1-base64&#10;frame2-base64&#10;…"
                  style={{ fontFamily: "monospace", fontSize: 11, width: "100%" }}
                />
                <div className="btn-row" style={{ marginTop: 6 }}>
                  <button className="btn btn-primary btn-sm" onClick={submitManualPaste}>
                    Submit pasted frames
                  </button>
                  <button className="btn btn-outline btn-sm" onClick={() => setManualPaste("")}>
                    Clear
                  </button>
                </div>
              </div>
            )}
          </div>

          {decoded && (
            <div className="card">
              <div className="card-title">Received transaction</div>
              <table className="ix-table">
                <tbody>
                  <tr><th>Size</th><td>{decoded.sizeBytes} bytes</td></tr>
                  <tr>
                    <th>Data (truncated)</th>
                    <td className="mono">{decoded.b64.slice(0, 64)}…</td>
                  </tr>
                </tbody>
              </table>

              <div style={{ marginTop: 12 }}>
                <label style={{ display: "block", marginBottom: 6, fontWeight: 600 }}>
                  Signing key
                </label>
                <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginBottom: 8 }}>
                  <button
                    className={`btn btn-sm ${keyMode === "wallet" ? "btn-primary" : "btn-outline"}`}
                    onClick={() => setKeyMode("wallet")}
                    disabled={!!signedState}
                  >
                    👻 Connected wallet
                  </button>
                  <button
                    className={`btn btn-sm ${keyMode === "import" ? "btn-primary" : "btn-outline"}`}
                    onClick={() => setKeyMode("import")}
                    disabled={!!signedState}
                  >
                    🔑 Imported secret key
                  </button>
                </div>
                {keyMode === "wallet" && (
                  wallet ? (
                    <p style={{ fontSize: 12, color: "var(--muted)", margin: "4px 0 8px" }}>
                      Will sign via Phantom <code>signTransaction</code> · pubkey
                      {" "}<span className="mono">{wallet.pubkeyB58}</span>.
                      Phantom will show the parsed transfer and prompt you to approve.
                    </p>
                  ) : (
                    <div className="alert alert-info" style={{ marginBottom: 8 }}>
                      No wallet connected.
                      {phantomInstalled ? (
                        <button
                          className="btn btn-primary btn-sm"
                          onClick={connect}
                          disabled={connecting}
                          style={{ marginLeft: 8 }}
                        >
                          {connecting ? "Connecting…" : "Connect Wallet"}
                        </button>
                      ) : (
                        <a
                          href="https://phantom.app"
                          target="_blank"
                          rel="noopener noreferrer"
                          className="btn btn-outline btn-sm"
                          style={{ marginLeft: 8 }}
                        >
                          Install Phantom ↗
                        </a>
                      )}
                    </div>
                  )
                )}
                {keyMode === "import" && (
                  <>
                    <div className="alert alert-err" style={{ marginBottom: 8 }}>
                      <strong>⚠ Pasting a secret key here is dangerous.</strong>
                      {" "}Anything you paste is in this browser's memory and exposed to any
                      script on the page. Only use this path on an air-gapped device, with a
                      throwaway keypair, or for testing — <strong>never</strong> with a key
                      that holds real funds. Prefer the connected-wallet path (Phantom signs
                      via <code>signTransaction</code> and never exposes the secret).
                    </div>
                    <p style={{ fontSize: 12, color: "var(--muted)", margin: "4px 0 6px" }}>
                      Paste a base58-encoded ed25519 secret key (32-byte seed or 64-byte expanded).
                    </p>
                    <textarea
                      rows={2}
                      value={importSecret}
                      onChange={(e) => setImportSecret(e.target.value)}
                      placeholder="base58 secret key…"
                      style={{ fontFamily: "monospace", fontSize: 12 }}
                    />
                  </>
                )}
              </div>

              <div className="btn-row">
                <button
                  className="btn btn-primary"
                  onClick={handleSign}
                  disabled={
                    !!signedState
                    || (keyMode === "wallet" && !wallet)
                    || (keyMode === "import" && !importSecret.trim())
                  }
                >
                  🖊 Sign transaction
                </button>
              </div>
            </div>
          )}

          {pubkeyB58 && (
            <div className="card">
              <div className="card-title">Signature</div>
              <table className="ix-table">
                <tbody>
                  <tr><th>Pubkey</th><td className="mono">{pubkeyB58}</td></tr>
                  <tr><th>Sig (first 32 bytes)</th><td className="mono">{sigHex?.slice(0, 64)}…</td></tr>
                </tbody>
              </table>
            </div>
          )}
        </div>

        {/* Right — signed QR stream */}
        <div>
          <div className="card">
            <div className="card-title">Signed response QR stream</div>

            <div className="qr-wrap">
              <canvas ref={qrCanvasRef} width={360} height={360} />
              {!signedState && (
                <p className="status">Awaiting signature…</p>
              )}
            </div>

            {signedState && (
              <>
                <p className="status mt-16 active">{status}</p>
                <div className="btn-row">
                  <button
                    className="btn btn-primary"
                    onClick={() => startAnim(signedState.frames)}
                    disabled={animRunning}
                  >
                    ▶ Animate
                  </button>
                  {animRunning && (
                    <button className="btn btn-outline" onClick={stopAnim}>⏸ Pause</button>
                  )}
                </div>
                <div style={{ marginTop: 12 }}>
                  <label>FPS: <strong>{fps}</strong></label>
                  <input
                    type="range" min={1} max={12} value={fps}
                    onChange={(e) => setFps(Number(e.target.value))}
                    style={{ width: "100%", accentColor: "var(--accent)" }}
                  />
                </div>
              </>
            )}

            {error && <div className="alert alert-err" style={{ marginTop: 12 }}>{error}</div>}
          </div>

          <div className="card">
            <div className="card-title">Instructions</div>
            <ol style={{ paddingLeft: 20, fontSize: 13, color: "var(--muted)", lineHeight: 2 }}>
              <li>Enter the same shared password as the online machine.</li>
              <li>Click <strong style={{ color: "var(--text)" }}>Start scanning</strong> and point the camera at the QR stream.</li>
              <li>When scan completes, review the decoded transaction.</li>
              <li>Click <strong style={{ color: "var(--text)" }}>Sign</strong> to sign with an ephemeral keypair.</li>
              <li>Hold this screen up to the online machine's camera (tab 3).</li>
            </ol>
            <div className="btn-row">
              <button className="btn btn-success" onClick={onNext} disabled={!signedState}>
                Continue to Receive tab →
              </button>
            </div>
          </div>
        </div>
      </div>
    </main>
  );
}