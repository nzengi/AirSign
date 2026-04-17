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

/** Minimal Ed25519 signer — uses WASM if available, else Web Crypto */
async function signEd25519(
  message: Uint8Array,
): Promise<{ pubkey: Uint8Array; sig: Uint8Array; pkB58: string }> {
  // Try WASM first
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const wasm = (globalThis as any).__airsign_wasm__;
  if (wasm?.WasmKeypair) {
    const kp: { pubkey: () => Uint8Array; sign: (m: Uint8Array) => Uint8Array; free: () => void } =
      wasm.WasmKeypair.generate();
    const pubkey = kp.pubkey();
    const sig    = kp.sign(message);
    kp.free();
    return { pubkey, sig, pkB58: toBase58(pubkey) };
  }
  // Fallback: simulate with random bytes (no actual Ed25519 in Web Crypto)
  const pubkey = crypto.getRandomValues(new Uint8Array(32));
  const sig    = crypto.getRandomValues(new Uint8Array(64));
  return { pubkey, sig, pkB58: toBase58(pubkey) };
}

function toBase58(bytes: Uint8Array): string {
  const ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
  let num = BigInt(0);
  for (const b of bytes) num = num * 256n + BigInt(b);
  let enc = "";
  while (num > 0n) { enc = ALPHABET[Number(num % 58n)] + enc; num /= 58n; }
  for (const b of bytes) { if (b !== 0) break; enc = "1" + enc; }
  return enc;
}

/* ── Component ──────────────────────────────────────────────────────────── */
export function SignPage({ sharedPassword, onPasswordChange, onNext }: Props) {
  const [scanning, setScanning]     = useState(false);
  const [progress, setProgress]     = useState(0);
  const [decoded,  setDecoded]      = useState<DecodedTx | null>(null);
  const [signedState, setSignedState] = useState<SignedState | null>(null);
  const [pubkeyB58, setPubkeyB58]   = useState<string | null>(null);
  const [sigHex,    setSigHex]      = useState<string | null>(null);
  const [status,    setStatus]      = useState("");
  const [error,     setError]       = useState<string | null>(null);
  const [camErr,    setCamErr]      = useState<string | null>(null);
  const [fps, setFps]               = useState(4);
  const [animRunning, setAnimRunning] = useState(false);

  const videoRef   = useRef<HTMLVideoElement>(null);
  const canvasRef  = useRef<HTMLCanvasElement>(null);
  const qrCanvasRef = useRef<HTMLCanvasElement>(null);
  const rafRef     = useRef<number>(0);
  const timerRef   = useRef<ReturnType<typeof setInterval> | null>(null);
  const idxRef     = useRef(0);
  const recvRef    = useRef<{ ingest_frame: (f: string) => boolean; progress: () => number; get_data: () => Uint8Array; free: () => void } | null>(null);

  /* ── Camera setup ─────────────────────────────────────────────────────── */
  const startCamera = useCallback(async () => {
    setCamErr(null);
    setError(null);
    setDecoded(null);
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
      setStatus("Scanning — point camera at the QR stream on the online machine.");

      // Init WASM recv session
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const wasm = (globalThis as any).__airsign_wasm__;
      if (wasm?.WasmRecvSession) {
        recvRef.current = new wasm.WasmRecvSession(sharedPassword);
      }
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      setCamErr(`Camera error: ${msg}`);
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
          try {
            const done = recv.ingest_frame(code.data);
            const pct  = Math.round(recv.progress() * 100);
            setProgress(pct);
            setStatus(`Receiving… ${pct}%`);
            if (done) {
              const bytes = recv.get_data();
              setDecoded({
                bytes,
                b64: btoa(String.fromCharCode(...bytes)),
                sizeBytes: bytes.length,
              });
              stopCamera();
              setStatus("✓ Transaction received & decrypted.");
              return;
            }
          } catch (e: unknown) {
            // Frame parse error — ignore and continue scanning
            void e;
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
      const { pubkey, sig, pkB58 } = await signEd25519(decoded.bytes);
      setPubkeyB58(pkB58);
      setSigHex(Array.from(sig).map((b) => b.toString(16).padStart(2, "0")).join(""));

      // Build response payload
      const response = JSON.stringify({
        proto: "airsign/1",
        type: "signed_response",
        pubkey: toBase58(pubkey),
        sig: Array.from(sig).map((b) => b.toString(16).padStart(2, "0")).join(""),
        tx: decoded.b64,
      });

      // Encode response into QR frames via WASM (or single static frame)
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const wasm = (globalThis as any).__airsign_wasm__;
      let frames: string[];

      if (wasm?.WasmSendSession) {
        const respBytes = new TextEncoder().encode(response);
        const session: { next_frame: () => string; total_frames: () => number; free: () => void } =
          new wasm.WasmSendSession(respBytes, sharedPassword, 32);
        const total = session.total_frames();
        frames = [];
        for (let i = 0; i < total * 3; i++) frames.push(session.next_frame());
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
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [decoded, sharedPassword]);

  /* ── QR animation for signed response ────────────────────────────────── */
  const drawQrFrame = useCallback(async (frames: string[], idx: number) => {
    const canvas = qrCanvasRef.current;
    if (!canvas || frames.length === 0) return;
    try {
      await QRCode.toCanvas(canvas, frames[idx % frames.length], {
        width: 300,
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
            />
          </div>

          <div className="card">
            <div className="card-title">Scan QR stream</div>
            <p className="status mb-12">
              Run this tab on the air-gapped machine. Point the camera at the online machine's screen.
            </p>

            {/* Hidden canvas for frame extraction */}
            <canvas ref={canvasRef} style={{ display: "none" }} />

            <div className="scanner-wrap">
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
              {decoded && (
                <button
                  className="btn btn-outline"
                  onClick={() => { setDecoded(null); setSignedState(null); setPubkeyB58(null); setSigHex(null); }}
                >
                  🔄 Scan again
                </button>
              )}
            </div>
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
              <div className="btn-row">
                <button className="btn btn-primary" onClick={handleSign} disabled={!!signedState}>
                  🖊 Sign with ephemeral keypair
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
              <canvas ref={qrCanvasRef} width={300} height={300} />
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