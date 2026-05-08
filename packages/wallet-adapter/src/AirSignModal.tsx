/**
 * AirSignModal / AirSignProvider
 *
 * React portal that renders the air-gapped signing UI on top of any dApp.
 * Listens for "airsign:request" DOM events posted by AirSignWalletAdapter
 * and runs the two-phase QR exchange flow:
 *
 *   Phase 1 – SEND:  fountain-encodes the payload → QrAnimator streams QR frames
 *   Phase 2 – RECV:  QrScanner captures response QR frames → resolves request
 *
 * A random 6-digit PIN is generated per request and shown to the user; they
 * type it into the AirSign mobile app so both sides share the same Argon2id
 * session password.
 *
 * Consumer usage (add once near the root of your app):
 *
 *   import { AirSignProvider } from "@airsign/wallet-adapter";
 *   <AirSignProvider wasmUrl="/wasm/afterimage_wasm_bg.wasm">
 *     <App />
 *   </AirSignProvider>
 */

import React, {
  useCallback,
  useEffect,
  useMemo,
  useState,
} from "react";
import { createPortal } from "react-dom";
import { initAirSign, QrAnimator, QrScanner } from "@airsign/react";
import {
  type AirSignRequest,
  type AirSignResponse,
  resolveAirSignRequest,
} from "./adapter.js";

// ─── Types ────────────────────────────────────────────────────────────────────

export interface AirSignProviderProps {
  children: React.ReactNode;
  /** Path/URL to afterimage_wasm_bg.wasm — forwarded to initAirSign(). */
  wasmUrl?: string;
}

type Phase = "send" | "recv" | "error";

// ─── Helpers ──────────────────────────────────────────────────────────────────

/** Generate a cryptographically random 6-digit PIN. */
function genPin(): string {
  const arr = new Uint32Array(1);
  crypto.getRandomValues(arr);
  return String(arr[0]! % 1_000_000).padStart(6, "0");
}

/** Encode a signing request as UTF-8 JSON bytes for QrAnimator. */
function encodeRequest(req: AirSignRequest): Uint8Array {
  let obj: Record<string, unknown>;
  switch (req.kind) {
    case "connect":
      obj = {
        airsign: 1,
        action: "connect",
        origin:
          typeof window !== "undefined" ? window.location.origin : "",
      };
      break;
    case "signTransaction":
      obj = {
        airsign: 1,
        action: "signTransaction",
        tx: Array.from(req.tx),
      };
      break;
    case "signAllTransactions":
      obj = {
        airsign: 1,
        action: "signAllTransactions",
        txs: req.txs.map((t) => Array.from(t)),
      };
      break;
    case "signMessage":
      obj = {
        airsign: 1,
        action: "signMessage",
        message: Array.from(req.message),
      };
      break;
  }
  return new TextEncoder().encode(JSON.stringify(obj));
}

/** Parse the signed response received from the mobile app. */
function parseResponse(data: Uint8Array): AirSignResponse {
  try {
    const text = new TextDecoder().decode(data);
    const obj = JSON.parse(text) as Record<string, unknown>;

    if (
      obj["action"] === "connected" &&
      typeof obj["pubkey"] === "string"
    ) {
      return { kind: "connected", pubkeyBase58: obj["pubkey"] };
    }
    if (
      obj["action"] === "signedTransaction" &&
      Array.isArray(obj["signed"])
    ) {
      return {
        kind: "signedTransaction",
        signed: new Uint8Array(obj["signed"] as number[]),
      };
    }
    if (
      obj["action"] === "signedAllTransactions" &&
      Array.isArray(obj["signed"])
    ) {
      return {
        kind: "signedAllTransactions",
        signed: (obj["signed"] as number[][]).map(
          (s) => new Uint8Array(s)
        ),
      };
    }
    if (
      obj["action"] === "signedMessage" &&
      Array.isArray(obj["signature"])
    ) {
      return {
        kind: "signedMessage",
        signature: new Uint8Array(obj["signature"] as number[]),
      };
    }
    if (obj["action"] === "cancelled") {
      return { kind: "cancelled" };
    }
    return {
      kind: "error",
      message: `Unknown action: ${String(obj["action"])}`,
    };
  } catch (e: unknown) {
    return {
      kind: "error",
      message: `Parse error: ${e instanceof Error ? e.message : String(e)}`,
    };
  }
}

// ─── Title helper ─────────────────────────────────────────────────────────────

function requestTitle(req: AirSignRequest): string {
  return {
    connect: "Connect to AirSign",
    signTransaction: "Sign Transaction",
    signAllTransactions: "Sign Transactions",
    signMessage: "Sign Message",
  }[req.kind];
}

// ─── Modal content ────────────────────────────────────────────────────────────

function ModalContent({
  request,
  onDone,
}: {
  request: AirSignRequest;
  onDone: (res: AirSignResponse) => void;
}) {
  // New PIN per request so each session is isolated.
  const pin = useMemo(() => genPin(), [request]);
  const password = pin; // password === pin for simplicity

  const [phase, setPhase] = useState<Phase>("send");
  const [errorMsg, setErrorMsg] = useState<string>("");

  const payload = useMemo(() => encodeRequest(request), [request]);

  const handleScanComplete = useCallback(
    (data: Uint8Array) => {
      onDone(parseResponse(data));
    },
    [onDone]
  );

  const handleScanError = useCallback((error: string) => {
    setErrorMsg(error);
    setPhase("error");
  }, []);

  const handleCancel = useCallback(
    () => onDone({ kind: "cancelled" }),
    [onDone]
  );

  return (
    <div style={styles.overlay}>
      <div style={styles.card}>
        {/* Header */}
        <div style={styles.header}>
          <span style={styles.logo}>✈</span>
          <span style={styles.title}>{requestTitle(request)}</span>
          <button
            style={styles.closeBtn}
            onClick={handleCancel}
            aria-label="Cancel"
          >
            ✕
          </button>
        </div>

        {/* PIN */}
        <div style={styles.pinBox}>
          <span style={styles.pinLabel}>Session PIN</span>
          <span style={styles.pinCode}>{pin}</span>
          <span style={styles.pinHint}>
            Enter this PIN in the AirSign mobile app
          </span>
        </div>

        {/* Phase: SEND */}
        {phase === "send" && (
          <>
            <p style={styles.instruction}>
              Open <strong>AirSign</strong> on your phone, enter the PIN, then
              scan this QR code.
            </p>
            <div style={styles.qrWrapper}>
              <QrAnimator
                data={payload}
                filename="airsign_request.bin"
                password={password}
                fps={8}
                qrScale={4}
              />
            </div>
            <button
              style={styles.primaryBtn}
              onClick={() => setPhase("recv")}
            >
              Done scanning → show response scanner
            </button>
          </>
        )}

        {/* Phase: RECV */}
        {phase === "recv" && (
          <>
            <p style={styles.instruction}>
              After signing on your phone, point the phone's screen at
              your webcam to transmit the signed response.
            </p>
            <div style={styles.qrWrapper}>
              <QrScanner
                password={password}
                onComplete={handleScanComplete}
                onError={handleScanError}
                className="airsign-scanner"
              />
            </div>
            <button
              style={styles.secondaryBtn}
              onClick={() => setPhase("send")}
            >
              ← Back to send QR
            </button>
          </>
        )}

        {/* Phase: ERROR */}
        {phase === "error" && (
          <>
            <p style={{ ...styles.instruction, color: "#ef4444" }}>
              {errorMsg}
            </p>
            <button
              style={styles.primaryBtn}
              onClick={() => setPhase("send")}
            >
              Try again
            </button>
          </>
        )}

        <button style={styles.cancelLink} onClick={handleCancel}>
          Cancel signing
        </button>
      </div>
    </div>
  );
}

// ─── AirSignProvider ──────────────────────────────────────────────────────────

export function AirSignProvider({
  children,
  wasmUrl,
}: AirSignProviderProps) {
  const [activeRequest, setActiveRequest] =
    useState<AirSignRequest | null>(null);
  const [mounted, setMounted] = useState(false);

  // Initialise WASM once.
  useEffect(() => {
    setMounted(true);
    initAirSign(wasmUrl).catch(console.error);
  }, [wasmUrl]);

  // Listen for requests from the adapter.
  useEffect(() => {
    const handler = (e: Event) => {
      const req = (e as CustomEvent<AirSignRequest>).detail;
      setActiveRequest(req);
    };
    window.addEventListener("airsign:request", handler);
    return () => window.removeEventListener("airsign:request", handler);
  }, []);

  const handleDone = useCallback((res: AirSignResponse) => {
    setActiveRequest(null);
    resolveAirSignRequest(res);
  }, []);

  return (
    <>
      {children}
      {mounted &&
        activeRequest &&
        createPortal(
          <ModalContent request={activeRequest} onDone={handleDone} />,
          document.body
        )}
    </>
  );
}

// ─── Inline styles ────────────────────────────────────────────────────────────

const styles: Record<string, React.CSSProperties> = {
  overlay: {
    position: "fixed",
    inset: 0,
    backgroundColor: "rgba(0,0,0,0.75)",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    zIndex: 99999,
    fontFamily: "system-ui, -apple-system, sans-serif",
  },
  card: {
    backgroundColor: "#0f0f0f",
    border: "1px solid #2a2a2a",
    borderRadius: 16,
    padding: "28px 32px",
    width: 380,
    maxWidth: "calc(100vw - 32px)",
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    gap: 16,
    color: "#f5f5f5",
  },
  header: {
    width: "100%",
    display: "flex",
    alignItems: "center",
    gap: 10,
  },
  logo: { fontSize: 22 },
  title: { flex: 1, fontWeight: 700, fontSize: 16 },
  closeBtn: {
    background: "none",
    border: "none",
    color: "#9ca3af",
    fontSize: 18,
    cursor: "pointer",
    lineHeight: 1,
    padding: 4,
  },
  pinBox: {
    width: "100%",
    background: "#1a1a1a",
    borderRadius: 10,
    padding: "12px 16px",
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    gap: 4,
  },
  pinLabel: { fontSize: 11, color: "#6b7280", textTransform: "uppercase", letterSpacing: 1 },
  pinCode: {
    fontSize: 32,
    fontWeight: 700,
    letterSpacing: 8,
    color: "#9945ff",
    fontFamily: "monospace",
  },
  pinHint: { fontSize: 11, color: "#6b7280" },
  instruction: {
    margin: 0,
    fontSize: 14,
    color: "#d1d5db",
    textAlign: "center",
    lineHeight: 1.5,
  },
  qrWrapper: {
    borderRadius: 12,
    overflow: "hidden",
    background: "#fff",
    padding: 8,
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    minHeight: 200,
    minWidth: "100%",
  },
  primaryBtn: {
    width: "100%",
    padding: "12px 16px",
    borderRadius: 10,
    border: "none",
    backgroundColor: "#9945ff",
    color: "#fff",
    fontWeight: 600,
    fontSize: 14,
    cursor: "pointer",
  },
  secondaryBtn: {
    width: "100%",
    padding: "12px 16px",
    borderRadius: 10,
    border: "1px solid #374151",
    backgroundColor: "transparent",
    color: "#d1d5db",
    fontWeight: 600,
    fontSize: 14,
    cursor: "pointer",
  },
  cancelLink: {
    background: "none",
    border: "none",
    color: "#6b7280",
    fontSize: 13,
    cursor: "pointer",
    textDecoration: "underline",
    padding: 0,
  },
};