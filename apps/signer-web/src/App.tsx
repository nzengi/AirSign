import { useEffect, useState } from "react";
import { initWasm } from "./wasm.js";
import { HomePage } from "./pages/HomePage.js";
import { SendPage } from "./pages/SendPage.js";
import { SignPage } from "./pages/SignPage.js";
import { ReceivePage } from "./pages/ReceivePage.js";
import { MultisigPage } from "./pages/MultisigPage.js";
import { FrostPage } from "./pages/FrostPage.js";
import DkgPage from "./pages/DkgPage.js";
import SquadsPage from "./pages/SquadsPage.js";
import { useWallet } from "./lib/wallet-ctx.js";

type Tab =
  | "home"
  | "send"
  | "sign"
  | "receive"
  | "multisig"
  | "frost"
  | "dkg"
  | "squads";

const TABS: { id: Tab; label: string }[] = [
  { id: "home",     label: "Home" },
  { id: "send",     label: "1 · Prepare & Send" },
  { id: "sign",     label: "2 · Air-gap Sign" },
  { id: "receive",  label: "3 · Receive & Broadcast" },
  { id: "multisig", label: "Multisig" },
  { id: "frost",    label: "FROST" },
  { id: "dkg",      label: "DKG" },
  { id: "squads",   label: "Squads v4" },
];

function initialTabFromUrl(): Tab {
  if (typeof window === "undefined") return "home";
  const params = new URLSearchParams(window.location.search);
  const role = params.get("role");
  if (role === "signer") return "sign";
  if (role === "sender") return "send";
  if (role === "receiver") return "receive";
  const hash = window.location.hash.replace(/^#/, "");
  const valid: Tab[] = ["home","send","sign","receive","multisig","frost","dkg","squads"];
  if ((valid as string[]).includes(hash)) return hash as Tab;
  return "home";
}

function HeaderWalletButton() {
  const { wallet, connect, disconnect, connecting, error, phantomInstalled } = useWallet();
  if (wallet) {
    return (
      <button
        className="btn btn-outline btn-sm header-wallet"
        onClick={disconnect}
        title="Click to disconnect"
      >
        👻 {wallet.pubkeyB58.slice(0, 4)}…{wallet.pubkeyB58.slice(-4)}
      </button>
    );
  }
  if (!phantomInstalled) {
    return (
      <a
        href="https://phantom.app"
        target="_blank"
        rel="noopener noreferrer"
        className="btn btn-outline btn-sm header-wallet"
      >
        Install Phantom ↗
      </a>
    );
  }
  return (
    <button
      className="btn btn-primary btn-sm header-wallet"
      onClick={connect}
      disabled={connecting}
      title={error ?? "Connect a Phantom wallet (set to Devnet in Phantom settings)"}
    >
      {connecting ? "Connecting…" : "👻 Connect Wallet"}
    </button>
  );
}

export function App() {
  const [tab, setTab] = useState<Tab>(() => initialTabFromUrl());
  const [wasmReady, setWasmReady] = useState(false);
  const [wasmError, setWasmError] = useState<string | null>(null);

  const [sharedPassword, setSharedPassword] = useState("demo-password-123");

  useEffect(() => {
    initWasm()
      .then(() => setWasmReady(true))
      .catch((e: unknown) => {
        setWasmError(e instanceof Error ? e.message : String(e));
        setWasmReady(true);
      });
  }, []);

  if (!wasmReady) {
    return (
      <div className="app">
        <header className="header">
          <span className="header-title">AirSign</span>
        </header>
        <div className="page" style={{ paddingTop: 80, textAlign: "center" }}>
          <p className="status active">Loading…</p>
        </div>
      </div>
    );
  }

  return (
    <div className="app">
      <header className="header">
        <span className="header-title">AirSign</span>
        <span className="header-subtitle">
          Air-gapped Solana signing · devnet
        </span>
        <span className="header-spacer" />
        <HeaderWalletButton />
      </header>

      <nav className="tab-bar">
        {TABS.map((t) => (
          <button
            key={t.id}
            className={`tab${tab === t.id ? " active" : ""}`}
            onClick={() => setTab(t.id)}
          >
            {t.label}
          </button>
        ))}
      </nav>

      {wasmError && tab !== "home" && (
        <div className="page" style={{ paddingBottom: 0 }}>
          <div className="alert alert-err">
            WASM failed to load: {wasmError}
          </div>
        </div>
      )}

      {tab === "home" && (
        <HomePage onJump={(t) => setTab(t as Tab)} />
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
      {tab === "multisig" && <MultisigPage />}
      {tab === "frost" && <FrostPage />}
      {tab === "dkg" && <DkgPage />}
      {tab === "squads" && <SquadsPage />}
    </div>
  );
}
