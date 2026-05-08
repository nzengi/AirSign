/**
 * SendPage — Online machine (Step 1)
 *
 * 1. Read sender pubkey from the connected Phantom wallet (header Connect button).
 * 2. User enters recipient + amount. We fetch a fresh devnet blockhash and
 *    serialize a SystemProgram::Transfer message.
 * 3. WASM WasmSendSession encrypts (Argon2id + ChaCha20-Poly1305) and
 *    LT-fountain-codes the message into QR frames.
 * 4. Frames loop on the canvas — hold the screen up to the camera of the
 *    air-gapped Sign tab (or open it in a second tab via the header).
 */

import { useCallback, useEffect, useRef, useState } from "react";
import QRCode from "qrcode";
import { createSendSession, encodeFrameForQr } from "../lib/wasm-api.js";
import { PasswordStrengthMeter } from "../components/PasswordStrengthMeter.js";
import {
  base58Decode,
  buildTransferMessage,
  bytesToBase64,
  DEVNET_RPC_URL,
  fetchLatestBlockhash,
  getBalanceLamports,
} from "../lib/solana-tx.js";
import { useWallet } from "../lib/wallet-ctx.js";

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

interface BuiltTx {
  from: string;
  to: string;
  lamports: bigint;
  blockhash: string;
  loadedAt: number;
}

/* Default devnet recipient (Solana System "incinerator"-style burn target).
 * Users can override in the input. Used only as a placeholder so judges
 * don't have to paste an address before trying the flow.
 */
const DEFAULT_RECIPIENT_B58 = "1nc1nerator11111111111111111111111111111111";

/* ── Component ──────────────────────────────────────────────────────────── */
export function SendPage({ sharedPassword, onPasswordChange, onNext }: Props) {
  const { wallet, connect, connecting, phantomInstalled } = useWallet();

  const [recipient, setRecipient] = useState(DEFAULT_RECIPIENT_B58);
  const [amountSol, setAmountSol] = useState("0.001");
  const [txB64, setTxB64] = useState("");
  const [builtTx, setBuiltTx] = useState<BuiltTx | null>(null);
  const [building, setBuilding] = useState(false);
  const [balanceSol, setBalanceSol] = useState<number | null>(null);

  const [status, setStatus] = useState<string>("");
  const [error, setError] = useState<string | null>(null);
  const [frameState, setFrameState] = useState<FrameState | null>(null);
  const [running, setRunning] = useState(false);
  const [fps, setFps] = useState(6);

  const canvasRef = useRef<HTMLCanvasElement>(null);
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const idxRef = useRef(0);

  /* Refresh balance whenever the wallet changes. */
  useEffect(() => {
    if (!wallet) { setBalanceSol(null); return; }
    let cancelled = false;
    getBalanceLamports(wallet.pubkeyB58, DEVNET_RPC_URL).then((lamports) => {
      if (!cancelled) setBalanceSol(lamports == null ? null : lamports / 1e9);
    });
    return () => { cancelled = true; };
  }, [wallet, builtTx]);

  /* ── Build the real on-chain transfer message ───────────────────────── */
  const buildTx = useCallback(async () => {
    if (!wallet) {
      setError("Connect your Phantom wallet first (button at top right).");
      return;
    }
    setError(null);
    setBuilding(true);
    setStatus("Validating inputs…");
    try {
      const toBytes = base58Decode(recipient.trim());
      if (toBytes.length !== 32) {
        throw new Error(`recipient does not decode to 32 bytes (got ${toBytes.length})`);
      }
      const sol = parseFloat(amountSol);
      if (!isFinite(sol) || sol <= 0) {
        throw new Error("amount must be a positive number");
      }
      const lamports = BigInt(Math.floor(sol * 1e9));

      setStatus("Fetching latest devnet blockhash…");
      const blockhash = await fetchLatestBlockhash(DEVNET_RPC_URL);

      const msg = buildTransferMessage(wallet.pubkey, toBytes, lamports, blockhash);
      setTxB64(bytesToBase64(msg.messageBytes));
      setBuiltTx({
        from: msg.summary.from,
        to: msg.summary.to,
        lamports: msg.summary.lamports,
        blockhash: msg.summary.blockhash,
        loadedAt: Date.now(),
      });
      setStatus(
        `✓ Transfer message ready · blockhash ${blockhash.slice(0, 8)}… (expires in ~90s — refresh before signing if you wait)`,
      );
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
      setStatus("");
    } finally {
      setBuilding(false);
    }
  }, [wallet, recipient, amountSol]);

  /* Draw current QR frame onto canvas */
  const drawFrame = useCallback(
    async (frames: string[], idx: number) => {
      const canvas = canvasRef.current;
      if (!canvas || frames.length === 0) return;
      try {
        await QRCode.toCanvas(canvas, frames[idx % frames.length], {
          width: 380,
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

    if (!txB64.trim()) { setError("Build the transfer message first."); return; }
    if (!sharedPassword.trim()) { setError("Shared password is empty."); return; }

    setStatus("Encoding…");

    try {
      const txBytes = Uint8Array.from(atob(txB64), (c) => c.charCodeAt(0));
      const session = createSendSession(txBytes, sharedPassword, "tx.bin");

      const total = session.total_frames();
      const frames: string[] = [];
      for (let i = 0; i < total * 3; i++) {
        const f = session.next_frame();
        if (!f || typeof f === "string") break;
        frames.push(encodeFrameForQr(f));
      }
      session.free();

      setFrameState({ frames, current: 0, total });
      setStatus(`Ready — ${total} unique frames (showing ${frames.length} in loop)`);
      idxRef.current = 0;
      await drawFrame(frames, 0);
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(`WASM error: ${msg}`);
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

      {!wallet && (
        <div className="alert alert-info" style={{ marginBottom: 16 }}>
          <strong>Connect a Phantom wallet</strong> (set to <em>Devnet</em> in Phantom's
          network settings, with at least ~0.01 SOL) to build a real on-chain transfer.
          {phantomInstalled ? (
            <button
              className="btn btn-primary btn-sm"
              onClick={connect}
              disabled={connecting}
              style={{ marginLeft: 12 }}
            >
              {connecting ? "Connecting…" : "Connect Wallet"}
            </button>
          ) : (
            <a
              href="https://phantom.app"
              target="_blank"
              rel="noopener noreferrer"
              className="btn btn-outline btn-sm"
              style={{ marginLeft: 12 }}
            >
              Install Phantom ↗
            </a>
          )}
        </div>
      )}

      <div className="two-col">
        {/* Left — inputs */}
        <div>
          <div className="card">
            <div className="card-title">Build devnet transfer</div>
            <p style={{ fontSize: 13, color: "var(--muted)", marginBottom: 8 }}>
              SystemProgram::Transfer message — sender is your connected wallet, signature
              comes in step 2 (air-gap signer).
            </p>
            <table className="ix-table" style={{ marginBottom: 10 }}>
              <tbody>
                <tr>
                  <th>From</th>
                  <td className="mono">
                    {wallet ? wallet.pubkeyB58 : <span className="status">— connect wallet —</span>}
                  </td>
                </tr>
                {wallet && (
                  <tr>
                    <th>Balance</th>
                    <td className="mono">
                      {balanceSol == null
                        ? <span style={{ color: "var(--muted)" }}>fetching…</span>
                        : `${balanceSol.toFixed(6)} SOL · devnet`}
                      {balanceSol != null && balanceSol < 0.005 && (
                        <span style={{ color: "var(--warn,#b45309)", marginLeft: 8 }}>
                          ⚠ low — fund at <a href="https://faucet.solana.com" target="_blank"
                            rel="noopener noreferrer">faucet.solana.com</a>
                        </span>
                      )}
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
            <label htmlFor="recipient" style={{ display: "block", fontSize: 13, marginTop: 8 }}>
              Recipient (base58)
            </label>
            <input
              id="recipient"
              type="text"
              value={recipient}
              onChange={(e) => { setRecipient(e.target.value); setBuiltTx(null); }}
              placeholder="recipient pubkey…"
              spellCheck={false}
              autoComplete="off"
              style={{ fontFamily: "monospace", fontSize: 13 }}
            />
            <label htmlFor="amount" style={{ display: "block", fontSize: 13, marginTop: 8 }}>
              Amount (SOL)
            </label>
            <input
              id="amount"
              type="number"
              min="0"
              step="0.001"
              value={amountSol}
              onChange={(e) => { setAmountSol(e.target.value); setBuiltTx(null); }}
            />
            <div className="btn-row" style={{ marginTop: 10 }}>
              <button
                className="btn btn-primary"
                onClick={buildTx}
                disabled={building || !wallet}
              >
                {building ? "Building…" : builtTx ? "🔄 Refresh blockhash" : "🛠 Build transfer"}
              </button>
            </div>
            {builtTx && (
              <table className="ix-table" style={{ marginTop: 12 }}>
                <tbody>
                  <tr><th>From</th><td className="mono">{builtTx.from}</td></tr>
                  <tr><th>To</th><td className="mono">{builtTx.to}</td></tr>
                  <tr><th>Amount</th><td>{(Number(builtTx.lamports) / 1e9).toFixed(6)} SOL</td></tr>
                  <tr><th>Blockhash</th><td className="mono">{builtTx.blockhash.slice(0,16)}…</td></tr>
                  <tr><th>Built at</th><td>{new Date(builtTx.loadedAt).toLocaleTimeString()}</td></tr>
                </tbody>
              </table>
            )}
            {error && <div className="alert alert-err mt-16">{error}</div>}
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
              autoComplete="off"
              spellCheck={false}
            />
            <PasswordStrengthMeter password={sharedPassword} alwaysVisible />
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

          <details className="card" style={{ paddingBottom: 6 }}>
            <summary style={{ cursor: "pointer", fontWeight: 600 }}>Advanced — paste raw message bytes</summary>
            <p style={{ fontSize: 12, color: "var(--muted)", marginTop: 8 }}>
              Bypass the on-chain builder and paste any base64 payload. The QR pipeline does
              not care what bytes it streams; only the broadcast step does.
            </p>
            <textarea
              rows={4}
              value={txB64}
              onChange={(e) => { setTxB64(e.target.value); setBuiltTx(null); }}
              placeholder="base64 message bytes…"
              style={{ fontFamily: "monospace", fontSize: 11 }}
            />
          </details>
        </div>

        {/* Right — QR display */}
        <div>
          <div className="card">
            <div className="card-title">QR stream</div>

            <div className="qr-wrap">
              <canvas ref={canvasRef} width={380} height={380} />
              {!frameState && (
                <p className="status">
                  {builtTx || txB64 ? "Press \"Start QR stream\" to begin" : "Build a transfer first."}
                </p>
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

            <div className="btn-row">
              <button
                className="btn btn-primary"
                disabled={!txB64.trim()}
                onClick={async () => {
                  await buildFrames();
                  setTimeout(() => {
                    setFrameState((prev) => {
                      if (prev) startAnimation(prev.frames);
                      return prev;
                    });
                  }, 50);
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
              <li>Connect your Phantom wallet (top-right · set network to <strong style={{ color: "var(--text)" }}>Devnet</strong>).</li>
              <li>Pick a recipient + amount, click <strong style={{ color: "var(--text)" }}>Build transfer</strong>.</li>
              <li>Click <strong style={{ color: "var(--text)" }}>Start QR stream</strong>.</li>
              <li>Open the Sign tab on a second device (or in a second browser tab) and scan.</li>
              <li>After signing, return here / open the Receive tab to broadcast.</li>
            </ol>
            <div className="btn-row">
              <button
                className="btn btn-outline btn-sm"
                onClick={() => {
                  const url = new URL(window.location.href);
                  url.searchParams.set("role", "signer");
                  url.hash = "#sign";
                  window.open(url.toString(), "_blank", "noopener,noreferrer");
                }}
              >
                🪟 Open Sign Tab →
              </button>
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
