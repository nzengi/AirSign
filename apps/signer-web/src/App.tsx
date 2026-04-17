import { useEffect, useState } from "react";
import { initWasm } from "./wasm.js";
import { SendPage } from "./pages/SendPage.js";
import { SignPage } from "./pages/SignPage.js";
import { ReceivePage } from "./pages/ReceivePage.js";

type Tab = "send" | "sign" | "receive";

const TABS: { id: Tab; label: string; emoji: string }[] = [
  { id: "send",    label: "1 · Prepare & Send",  emoji: "📡" },
  { id: "sign",    label: "2 · Air-gap Sign",     emoji: "🔐" },
  { id: "receive", label: "3 · Receive & Broadcast", emoji: "📨" },
];

export function App() {
  const [tab, setTab] = useState<Tab>("send");
  const [wasmReady, setWasmReady] = useState(false);
  const [wasmError, setWasmError] = useState<string | null>(null);

  // Shared state that flows across the three stages
  const [sharedPassword, setSharedPassword] = useState("demo-password-123");

  useEffect(() => {
    initWasm()
      .then(() => setWasmReady(true))
      .catch((e: unknown) => {
        setWasmError(e instanceof Error ? e.message : String(e));
        // Still mark as "ready" with a degraded mode — demo pages show the error
        setWasmReady(true);
      });
  }, []);

  if (!wasmReady) {
    return (
      <div className="app">
        <header className="header">
          <span className="header-logo">🔐</span>
          <span className="header-title">AirSign</span>
        </header>
        <div className="page" style={{ paddingTop: 80, textAlign: "center" }}>
          <p className="status active">Loading WASM module…</p>
        </div>
      </div>
    );
  }

  return (
    <div className="app">
      <header className="header">
        <span className="header-logo">🔐</span>
        <span className="header-title">AirSign</span>
        <span className="header-subtitle">
          Air-gapped Solana signing · encrypted fountain-coded QR stream
        </span>
      </header>

      <nav className="tab-bar">
        {TABS.map((t) => (
          <button
            key={t.id}
            className={`tab${tab === t.id ? " active" : ""}`}
            onClick={() => setTab(t.id)}
          >
            {t.emoji} {t.label}
          </button>
        ))}
      </nav>

      {wasmError && (
        <div className="page" style={{ paddingBottom: 0 }}>
          <div className="alert alert-err">
            ⚠️ WASM failed to load: {wasmError}. The demo will run in simulation
            mode — all crypto operations are mocked.
          </div>
        </div>
      )}

      {tab === "send" && (
        <SendPage
          sharedPassword={sharedPassword}
          onPasswordChange={setSharedPassword}
          onNext={() => setTab("sign")}
        />
      )}
      {tab === "sign" && (
        <SignPage
          sharedPassword={sharedPassword}
          onPasswordChange={setSharedPassword}
          onNext={() => setTab("receive")}
        />
      )}
      {tab === "receive" && (
        <ReceivePage
          sharedPassword={sharedPassword}
          onPasswordChange={setSharedPassword}
        />
      )}
    </div>
  );
}