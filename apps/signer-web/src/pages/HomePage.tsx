interface Props {
  onJump: (tab: string) => void;
}

export function HomePage({ onJump }: Props) {
  return (
    <div className="home">
      {/* ─── Hero ─────────────────────────────────────────────────────── */}
      <section className="home-hero">
        <div>
          <h1>
            Air-gapped Solana signing,<br />
            <span className="grad">no hardware required.</span>
          </h1>

          <p className="lead">
            AirSign moves your Solana private key onto any device that has{" "}
            <strong style={{ color: "var(--text)" }}>never touched the internet</strong>{" "}
            — an old laptop, a phone in airplane mode, a Raspberry Pi — and signs
            transactions over an encrypted, fountain-coded QR stream. No USB. No
            Bluetooth. No network. Just light.
          </p>

          <div className="ctas">
            <button className="btn btn-primary btn-lg" onClick={() => onJump("send")}>
              Start sending →
            </button>
            <button
              className="btn btn-outline btn-lg"
              onClick={() => onJump("frost")}
            >
              FROST threshold signing
            </button>
          </div>
        </div>

        {/* Air-gap visual */}
        <div className="hero-visual">
          <div className="hero-visual-inner">
            <div className="hero-visual-half">
              <div className="hero-visual-icon">
                <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M5 12.55a11 11 0 0 1 14.08 0"/><path d="M1.42 9a16 16 0 0 1 21.16 0"/><path d="M8.53 16.11a6 6 0 0 1 6.95 0"/><circle cx="12" cy="20" r="1"/>
                </svg>
              </div>
              Online
            </div>
            <div className="hero-visual-half">
              <div className="hero-visual-icon">
                <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
                  <rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/>
                </svg>
              </div>
              Air-gapped
            </div>
            <div className="hero-visual-arrow">⇄</div>
          </div>
        </div>
      </section>

      {/* ─── Comparison table ──────────────────────────────────────────── */}
      <section className="card mb-16" style={{ marginBottom: 32 }}>
        <div className="card-title">How AirSign compares</div>
        <div style={{ overflowX: "auto" }}>
          <table className="ix-table compare-table">
            <thead>
              <tr>
                <th style={{ minWidth: 220 }}>Capability</th>
                <th>Hot wallet<br /><span style={{ fontWeight: 400, color: "var(--muted)" }}>Phantom</span></th>
                <th>Hardware wallet<br /><span style={{ fontWeight: 400, color: "var(--muted)" }}>Ledger</span></th>
                <th>MPC sharding<br /><span style={{ fontWeight: 400, color: "var(--muted)" }}>Unruggable</span></th>
                <th>HW air-gap<br /><span style={{ fontWeight: 400, color: "var(--muted)" }}>Aurora</span></th>
                <th style={{ color: "var(--accent2)" }}>AirSign</th>
              </tr>
            </thead>
            <tbody>
              <tr>
                <th>Private key fully offline</th>
                <td>✗</td><td>✓</td><td>shards online</td><td>✓</td>
                <td className="ok">✓</td>
              </tr>
              <tr>
                <th>No USB / Bluetooth surface</th>
                <td>—</td><td>✗</td><td>—</td><td>✓</td>
                <td className="ok">✓ optical only</td>
              </tr>
              <tr>
                <th>Zero hardware cost</th>
                <td>✓</td><td>✗ ($79+)</td><td>✓</td><td>✗ (custom HW)</td>
                <td className="ok">✓ any device</td>
              </tr>
              <tr>
                <th>Threshold signatures (FROST)</th>
                <td>✗</td><td>✗</td><td>✗</td><td>✗</td>
                <td className="ok">✓ RFC 9591</td>
              </tr>
              <tr>
                <th>Survives QR frame loss</th>
                <td>—</td><td>—</td><td>—</td><td>retransmits</td>
                <td className="ok">✓ LT fountain code</td>
              </tr>
              <tr>
                <th>Open-source crypto stack</th>
                <td>UI only</td><td>partial</td><td>partial</td><td>partial</td>
                <td className="ok">✓ Rust + WASM</td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>

      {/* ─── 3-step flow ──────────────────────────────────────────────── */}
      <div className="flow">
        <div className="flow-item">
          <div className="num">1</div>
          <h4>Prepare & Send</h4>
          <p>Online machine encrypts the unsigned transaction and streams it as a fountain-coded QR sequence.</p>
        </div>
        <div className="flow-arrow">→</div>
        <div className="flow-item">
          <div className="num">2</div>
          <h4>Air-gap Sign</h4>
          <p>Offline device scans, decrypts, and signs with Ed25519. The private key never leaves the device.</p>
        </div>
        <div className="flow-arrow">→</div>
        <div className="flow-item">
          <div className="num">3</div>
          <h4>Receive & Broadcast</h4>
          <p>Online machine reassembles the signed response and submits to any Solana cluster via standard RPC.</p>
        </div>
      </div>

      {/* ─── Feature cards ────────────────────────────────────────────── */}
      <div className="feat-grid">
        <div className="feat">
          <div className="feat-icon">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
              <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
            </svg>
          </div>
          <h3>Truly air-gapped</h3>
          <p>
            Private keys never touch a networked device. The only channel between
            devices is a camera and a screen — no USB, no Bluetooth, no NFC.
          </p>
        </div>
        <div className="feat">
          <div className="feat-icon">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
              <rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/>
            </svg>
          </div>
          <h3>Fountain-coded QR</h3>
          <p>
            LT fountain coding (Luby 2002) lets the receiver reconstruct the full payload
            from any sufficient subset of frames — survives up to 30% frame loss, no retransmits.
          </p>
        </div>
        <div className="feat">
          <div className="feat-icon">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
              <circle cx="12" cy="12" r="10"/><path d="M12 8v4l3 3"/>
            </svg>
          </div>
          <h3>Threshold-ready</h3>
          <p>
            FROST (RFC 9591) threshold signatures built in. Sign with t-of-n offline
            devices and produce a single standard Ed25519 signature.
          </p>
        </div>
      </div>

      {/* ─── Capabilities ───────────────────────────────────────────── */}
      <p className="tracks-title">Capabilities</p>
      <div className="tracks">
        <div className="track" onClick={() => onJump("multisig")}>
          <div className="name">M-of-N Multisig</div>
          <div className="desc">Coordinate partial signatures from multiple air-gapped signers.</div>
        </div>
        <div className="track" onClick={() => onJump("frost")}>
          <div className="name">FROST Threshold</div>
          <div className="desc">RFC 9591 t-of-n Ed25519 threshold signatures, in-browser.</div>
        </div>
        <div className="track" onClick={() => onJump("dkg")}>
          <div className="name">Trustless DKG</div>
          <div className="desc">Dealer-free distributed key generation for FROST shares.</div>
        </div>
        <div className="track" onClick={() => onJump("squads")}>
          <div className="name">Squads v4</div>
          <div className="desc">Air-gap signer for Squads Protocol multisig vaults.</div>
        </div>
      </div>

      {/* ─── Closing CTA ──────────────────────────────────────────────── */}
      <div className="home-cta-final">
        <h2>Run it on devnet</h2>
        <p>
          Connect your Phantom wallet (set to <strong>Devnet</strong> in Phantom's network
          settings, with some devnet SOL — get it from
          {" "}<a href="https://faucet.solana.com" target="_blank" rel="noopener noreferrer"
            style={{ color: "var(--accent2)" }}>faucet.solana.com</a>),
          then walk through Prepare → Sign → Broadcast. Every transaction lands on devnet
          with a real Explorer link. Open a second tab to run the air-gapped QR + camera path.
        </p>
        <div style={{ display: "flex", gap: 12, flexWrap: "wrap", justifyContent: "center" }}>
          <button className="btn btn-primary btn-lg" onClick={() => onJump("send")}>
            Start: Prepare & Send →
          </button>
          <button
            className="btn btn-outline btn-lg"
            onClick={() => {
              const url = new URL(window.location.href);
              url.searchParams.set("role", "signer");
              url.hash = "#sign";
              window.open(url.toString(), "_blank", "noopener,noreferrer");
            }}
          >
            🪟 Open Sign Tab →
          </button>
        </div>
      </div>
    </div>
  );
}