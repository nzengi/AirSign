/**
 * FrostPage — FROST (RFC 9591) threshold-signature demo.
 *
 * Simulates a complete t-of-n FROST signing session in-browser:
 *
 *   Step 1 · Setup     — dealer generates N key shares (trusted dealer model)
 *   Step 2 · Round 1   — each threshold participant generates nonces + commitment
 *   Step 3 · Sign Pkg  — aggregator collects commitments + message → SigningPackage
 *   Step 4 · Round 2   — each participant produces a SignatureShare
 *   Step 5 · Result    — aggregator combines shares → final Ed25519 signature
 *
 * The resulting signature (64 bytes) is a standard Ed25519 sig indistinguishable
 * from a single-signer one — it can be verified or broadcast on Solana without
 * any FROST tooling.
 */

import { useState, useCallback } from "react";

// ─── WASM accessor ────────────────────────────────────────────────────────────

function getWasm(): Record<string, unknown> {
  return (
    ((globalThis as Record<string, unknown>).__airsign_wasm__ as
      | Record<string, unknown>
      | undefined) ?? {}
  );
}

// Minimal WASM type interfaces
interface WasmFrostDealerClass {
  new (): never;
  generate(n: number, threshold: number): WasmFrostDealerHandle;
}
interface WasmFrostDealerHandle {
  setup_json(): string;
  key_packages_json(): string;
  pubkey_package_json(): string;
}

interface WasmFrostParticipantClass {
  new (keyPackageJson: string, identifier: number): WasmFrostParticipantHandle;
}
interface WasmFrostParticipantHandle {
  round1(): string;
  round2(noncesJson: string, signingPackageJson: string): string;
  identifier(): number;
}

interface WasmFrostAggregatorClass {
  new (
    pubkeyPackageJson: string,
    threshold: number,
    totalParticipants: number
  ): WasmFrostAggregatorHandle;
}
interface WasmFrostAggregatorHandle {
  add_commitment(r1Json: string): void;
  commitment_count(): number;
  build_signing_package(messageHex: string): string;
  add_share(r2Json: string): void;
  share_count(): number;
  aggregate(signingPackageJson: string): string;
  reset(): void;
}

function getDealer(): WasmFrostDealerClass {
  return getWasm()["WasmFrostDealer"] as WasmFrostDealerClass;
}
function getParticipantClass(): WasmFrostParticipantClass {
  return getWasm()["WasmFrostParticipant"] as WasmFrostParticipantClass;
}
function getAggregatorClass(): WasmFrostAggregatorClass {
  return getWasm()["WasmFrostAggregator"] as WasmFrostAggregatorClass;
}

function hexEncode(s: string): string {
  return Array.from(new TextEncoder().encode(s))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

// ─── Types ────────────────────────────────────────────────────────────────────

type Stage = "setup" | "round1" | "pkg" | "round2" | "result";

interface R1Data {
  participantId: number;
  /** Stays private — never shown in full */
  noncesJson: string;
  /** Sent to aggregator */
  commitmentsJson: string;
}

interface R2Data {
  participantId: number;
  shareJson: string;
}

interface FrostResult {
  signature_hex: string;
  verifying_key_hex: string;
  message_hex: string;
  threshold: number;
  total_participants: number;
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

function CopyButton({ text, label = "Copy" }: { text: string; label?: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <button
      className="btn btn-sm"
      style={{ marginLeft: 8 }}
      onClick={() => {
        void navigator.clipboard.writeText(text);
        setCopied(true);
        setTimeout(() => setCopied(false), 1500);
      }}
    >
      {copied ? "✓ Copied" : label}
    </button>
  );
}

function JsonBox({
  label,
  value,
  rows = 4,
  private: isPrivate = false,
}: {
  label: string;
  value: string;
  rows?: number;
  private?: boolean;
}) {
  return (
    <div style={{ marginBottom: 12 }}>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          marginBottom: 4,
          gap: 4,
        }}
      >
        <span className="label">
          {isPrivate ? "🔒 " : ""}
          {label}
        </span>
        {isPrivate && (
          <span
            style={{
              fontSize: 11,
              background: "#fef3c7",
              color: "#92400e",
              borderRadius: 4,
              padding: "1px 6px",
            }}
          >
            PRIVATE — never share
          </span>
        )}
        {!isPrivate && <CopyButton text={value} />}
      </div>
      <textarea
        readOnly
        value={value}
        rows={rows}
        style={{
          width: "100%",
          fontFamily: "monospace",
          fontSize: 11,
          background: isPrivate ? "#fef3c7" : "#0d1117",
          color: isPrivate ? "#78350f" : "#c9d1d9",
          border: isPrivate ? "1px solid #fbbf24" : "1px solid #30363d",
          borderRadius: 6,
          padding: 8,
          resize: "vertical",
          boxSizing: "border-box",
        }}
      />
    </div>
  );
}

function StepBadge({
  n,
  active,
  done,
}: {
  n: number;
  active: boolean;
  done: boolean;
}) {
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        width: 28,
        height: 28,
        borderRadius: "50%",
        background: done ? "#22c55e" : active ? "#3b82f6" : "#374151",
        color: "#fff",
        fontWeight: 700,
        fontSize: 13,
        marginRight: 8,
        flexShrink: 0,
      }}
    >
      {done ? "✓" : n}
    </span>
  );
}

// ─── Main component ───────────────────────────────────────────────────────────

export function FrostPage() {
  // ── Setup form ──────────────────────────────────────────────────────────────
  const [n, setN] = useState(3);
  const [threshold, setThreshold] = useState(2);
  const [message, setMessage] = useState("Transfer 1 SOL to treasury");

  // ── Session state ───────────────────────────────────────────────────────────
  const [stage, setStage] = useState<Stage>("setup");
  const [error, setError] = useState<string | null>(null);

  // Setup outputs
  const [keyPackages, setKeyPackages] = useState<string[]>([]);
  const [pubkeyPackageJson, setPubkeyPackageJson] = useState("");
  const [setupJson, setSetupJson] = useState("");

  // Round-1 outputs (indexed 0 = participant 1)
  const [r1Data, setR1Data] = useState<(R1Data | null)[]>([]);

  // Signing package
  const [signingPackageJson, setSigningPackageJson] = useState("");

  // Round-2 outputs
  const [r2Data, setR2Data] = useState<(R2Data | null)[]>([]);

  // Final result
  const [result, setResult] = useState<FrostResult | null>(null);

  // ── Step 1: Generate key setup ───────────────────────────────────────────────

  const handleGenerate = useCallback(() => {
    setError(null);
    try {
      const Dealer = getDealer();
      const handle = Dealer.generate(n, threshold);
      const setup = JSON.parse(handle.setup_json()) as {
        n: number;
        threshold: number;
        key_packages: string[];
        pubkey_package: string;
      };

      setKeyPackages(setup.key_packages);
      setPubkeyPackageJson(setup.pubkey_package);
      setSetupJson(handle.setup_json());
      setR1Data(new Array(threshold).fill(null));
      setR2Data(new Array(threshold).fill(null));
      setSigningPackageJson("");
      setResult(null);
      setStage("round1");
    } catch (e) {
      setError(String(e));
    }
  }, [n, threshold]);

  // ── Step 2: Run Round 1 for participant i (0-indexed) ────────────────────────

  const handleRound1 = useCallback(
    (idx: number) => {
      setError(null);
      try {
        const ParticipantCls = getParticipantClass();
        // participant identifier is 1-indexed
        const p = new ParticipantCls(keyPackages[idx], idx + 1);
        const r1Json = p.round1();
        const r1 = JSON.parse(r1Json) as {
          identifier: number;
          nonces_json: string;
          commitments_json: string;
        };
        setR1Data((prev) => {
          const next = [...prev];
          next[idx] = {
            participantId: r1.identifier,
            noncesJson: r1.nonces_json,
            commitmentsJson: r1.commitments_json,
          };
          return next;
        });
      } catch (e) {
        setError(String(e));
      }
    },
    [keyPackages]
  );

  // ── Step 3: Build signing package ────────────────────────────────────────────

  const handleBuildPkg = useCallback(() => {
    setError(null);
    try {
      const AggCls = getAggregatorClass();
      const agg = new AggCls(pubkeyPackageJson, threshold, n);

      for (const r1 of r1Data) {
        if (!r1) throw new Error("Not all Round-1 outputs are available.");
        // Build the wrapper JSON the aggregator's add_commitment expects
        const r1Wrapper = JSON.stringify({
          identifier: r1.participantId,
          nonces_json: r1.noncesJson,
          commitments_json: r1.commitmentsJson,
        });
        agg.add_commitment(r1Wrapper);
      }

      const msgHex = hexEncode(message);
      const pkg = agg.build_signing_package(msgHex);
      setSigningPackageJson(pkg);
      setStage("round2");
    } catch (e) {
      setError(String(e));
    }
  }, [pubkeyPackageJson, threshold, n, r1Data, message]);

  // ── Step 4: Run Round 2 for participant i (0-indexed) ────────────────────────

  const handleRound2 = useCallback(
    (idx: number) => {
      setError(null);
      const r1 = r1Data[idx];
      if (!r1) {
        setError(`Participant ${idx + 1} has no Round-1 output.`);
        return;
      }
      try {
        const ParticipantCls = getParticipantClass();
        const p = new ParticipantCls(keyPackages[idx], idx + 1);
        const r2Json = p.round2(r1.noncesJson, signingPackageJson);
        const r2 = JSON.parse(r2Json) as {
          identifier: number;
          share_json: string;
        };
        setR2Data((prev) => {
          const next = [...prev];
          next[idx] = {
            participantId: r2.identifier,
            shareJson: r2.share_json,
          };
          return next;
        });
      } catch (e) {
        setError(String(e));
      }
    },
    [keyPackages, r1Data, signingPackageJson]
  );

  // ── Step 5: Aggregate shares ─────────────────────────────────────────────────

  const handleAggregate = useCallback(() => {
    setError(null);
    try {
      const AggCls = getAggregatorClass();
      const agg = new AggCls(pubkeyPackageJson, threshold, n);

      for (const r2 of r2Data) {
        if (!r2) throw new Error("Not all Round-2 shares are available.");
        agg.add_share(
          JSON.stringify({
            identifier: r2.participantId,
            share_json: r2.shareJson,
          })
        );
      }

      const resultJson = agg.aggregate(signingPackageJson);
      setResult(JSON.parse(resultJson) as FrostResult);
      setStage("result");
    } catch (e) {
      setError(String(e));
    }
  }, [pubkeyPackageJson, threshold, n, r2Data, signingPackageJson]);

  // ── Helpers ──────────────────────────────────────────────────────────────────

  const allRound1Done = r1Data.length > 0 && r1Data.every(Boolean);
  const allRound2Done = r2Data.length > 0 && r2Data.every(Boolean);

  const stageIndex: Record<Stage, number> = {
    setup: 0,
    round1: 1,
    pkg: 2,
    round2: 3,
    result: 4,
  };
  const si = stageIndex[stage];

  // ─── Render ────────────────────────────────────────────────────────────────

  return (
    <div className="page">
      {/* Header */}
      <div style={{ marginBottom: 24 }}>
        <h2 className="section-title">❄️ FROST Threshold Signatures (RFC 9591)</h2>
        <p className="hint">
          FROST produces a standard Ed25519 signature from M-of-N key shares — no
          single device ever holds the full private key. The resulting signature is
          indistinguishable from a regular Ed25519 sig and can be used directly
          on Solana.
        </p>
      </div>

      {/* Step progress bar */}
      <div
        style={{
          display: "flex",
          gap: 8,
          marginBottom: 28,
          flexWrap: "wrap",
        }}
      >
        {[
          "1 · Setup",
          "2 · Round 1",
          "3 · Sign Pkg",
          "4 · Round 2",
          "5 · Result",
        ].map((label, i) => (
          <div
            key={label}
            style={{
              display: "flex",
              alignItems: "center",
              opacity: si >= i ? 1 : 0.35,
            }}
          >
            <StepBadge n={i + 1} active={si === i} done={si > i} />
            <span style={{ fontSize: 13, color: si === i ? "#93c5fd" : "#9ca3af" }}>
              {label}
            </span>
            {i < 4 && (
              <span style={{ marginLeft: 8, color: "#4b5563" }}>→</span>
            )}
          </div>
        ))}
      </div>

      {/* Error banner */}
      {error && (
        <div className="alert alert-err" style={{ marginBottom: 16 }}>
          ⚠️ {error}
        </div>
      )}

      {/* ── Step 1: Setup ────────────────────────────────────────────────────── */}
      <section className="card" style={{ marginBottom: 20 }}>
        <h3 className="card-title">
          <StepBadge n={1} active={si === 0} done={si > 0} />
          Trusted-Dealer Key Setup
        </h3>

        <div style={{ display: "flex", gap: 16, flexWrap: "wrap", marginBottom: 16 }}>
          <label className="field">
            <span className="label">Total signers (N) — min 2</span>
            <input
              type="number"
              className="input"
              min={2}
              max={10}
              value={n}
              disabled={stage !== "setup"}
              onChange={(e) => {
                const v = Math.max(2, Math.min(10, Number(e.target.value)));
                setN(v);
                if (threshold > v) setThreshold(v);
              }}
            />
          </label>
          <label className="field">
            <span className="label">Threshold (M) — min 2</span>
            <input
              type="number"
              className="input"
              min={2}
              max={n}
              value={threshold}
              disabled={stage !== "setup"}
              onChange={(e) => {
                const v = Math.max(2, Math.min(n, Number(e.target.value)));
                setThreshold(v);
              }}
            />
          </label>
        </div>

        <p className="hint" style={{ marginBottom: 12 }}>
          The dealer generates N key shares from OS randomness. In a real deployment
          each key share is transmitted to its participant via QR stream on an
          air-gapped machine. Here all shares are held in browser memory for the demo.
        </p>

        {stage === "setup" && (
          <button className="btn btn-primary" onClick={handleGenerate}>
            🎲 Generate {threshold}-of-{n} Key Setup
          </button>
        )}

        {stage !== "setup" && (
          <>
            <div className="alert alert-ok" style={{ marginBottom: 12 }}>
              ✅ Setup complete — {threshold}-of-{n} · {keyPackages.length} key
              shares generated
            </div>
            <JsonBox
              label={`pubkey_package (shared with all participants)`}
              value={pubkeyPackageJson}
              rows={3}
            />
            {keyPackages.slice(0, threshold).map((kp, i) => (
              <JsonBox
                key={i}
                label={`key_packages[${i}] — Participant ${i + 1}`}
                value={kp}
                rows={3}
                private
              />
            ))}
            <button
              className="btn btn-sm"
              style={{ marginTop: 8 }}
              onClick={() => {
                setStage("setup");
                setKeyPackages([]);
                setPubkeyPackageJson("");
                setSetupJson("");
                setR1Data([]);
                setR2Data([]);
                setSigningPackageJson("");
                setResult(null);
                setError(null);
              }}
            >
              ↺ Reset
            </button>
          </>
        )}
      </section>

      {/* ── Step 2: Round 1 ──────────────────────────────────────────────────── */}
      {si >= 1 && (
        <section className="card" style={{ marginBottom: 20 }}>
          <h3 className="card-title">
            <StepBadge n={2} active={si === 1} done={si > 1} />
            Round 1 — Commit (each participant generates nonces)
          </h3>
          <p className="hint" style={{ marginBottom: 12 }}>
            Each participant generates a random nonce pair. The <strong>nonces</strong>{" "}
            stay private on the participant's device. The <strong>commitments</strong>{" "}
            are sent to the aggregator.
          </p>

          {Array.from({ length: threshold }, (_, i) => (
            <div
              key={i}
              style={{
                border: "1px solid #21262d",
                borderRadius: 8,
                padding: 12,
                marginBottom: 12,
              }}
            >
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  marginBottom: 8,
                  gap: 8,
                }}
              >
                <strong style={{ color: "#e2e8f0" }}>
                  Participant {i + 1}
                </strong>
                {r1Data[i] ? (
                  <span className="badge badge-ok">✓ committed</span>
                ) : (
                  <span className="badge badge-warn">pending</span>
                )}
              </div>

              {r1Data[i] ? (
                <>
                  <JsonBox
                    label="nonces_json"
                    value={r1Data[i]!.noncesJson}
                    rows={3}
                    private
                  />
                  <JsonBox
                    label="commitments_json (→ aggregator)"
                    value={r1Data[i]!.commitmentsJson}
                    rows={3}
                  />
                </>
              ) : (
                <button
                  className="btn btn-primary"
                  disabled={si > 1}
                  onClick={() => handleRound1(i)}
                >
                  ▶ Run Round 1 for Participant {i + 1}
                </button>
              )}
            </div>
          ))}

          {allRound1Done && si === 1 && (
            <button
              className="btn btn-primary"
              style={{ marginTop: 4 }}
              onClick={() => setStage("pkg")}
            >
              → Proceed to Build Signing Package
            </button>
          )}
        </section>
      )}

      {/* ── Step 3: Build signing package ────────────────────────────────────── */}
      {si >= 2 && (
        <section className="card" style={{ marginBottom: 20 }}>
          <h3 className="card-title">
            <StepBadge n={3} active={si === 2} done={si > 2} />
            Aggregator — Build Signing Package
          </h3>
          <p className="hint" style={{ marginBottom: 12 }}>
            The aggregator collects all Round-1 commitments and the message to sign,
            then produces a <code>SigningPackage</code> broadcast to every participant.
          </p>

          <label className="field" style={{ marginBottom: 12 }}>
            <span className="label">Message to sign</span>
            <input
              type="text"
              className="input"
              value={message}
              disabled={si > 2}
              onChange={(e) => setMessage(e.target.value)}
            />
          </label>

          {si === 2 && (
            <button className="btn btn-primary" onClick={handleBuildPkg}>
              🔧 Build Signing Package
            </button>
          )}

          {signingPackageJson && (
            <JsonBox
              label="signing_package_json (→ broadcast to all participants)"
              value={signingPackageJson}
              rows={5}
            />
          )}
        </section>
      )}

      {/* ── Step 4: Round 2 ──────────────────────────────────────────────────── */}
      {si >= 3 && (
        <section className="card" style={{ marginBottom: 20 }}>
          <h3 className="card-title">
            <StepBadge n={4} active={si === 3} done={si > 3} />
            Round 2 — Sign (each participant produces a share)
          </h3>
          <p className="hint" style={{ marginBottom: 12 }}>
            Each participant combines their private nonces from Round 1 with the
            aggregator's <code>SigningPackage</code> to produce a{" "}
            <code>SignatureShare</code>, which is sent back to the aggregator.
          </p>

          {Array.from({ length: threshold }, (_, i) => (
            <div
              key={i}
              style={{
                border: "1px solid #21262d",
                borderRadius: 8,
                padding: 12,
                marginBottom: 12,
              }}
            >
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  marginBottom: 8,
                  gap: 8,
                }}
              >
                <strong style={{ color: "#e2e8f0" }}>
                  Participant {i + 1}
                </strong>
                {r2Data[i] ? (
                  <span className="badge badge-ok">✓ signed</span>
                ) : (
                  <span className="badge badge-warn">pending</span>
                )}
              </div>

              {r2Data[i] ? (
                <JsonBox
                  label="share_json (→ aggregator)"
                  value={r2Data[i]!.shareJson}
                  rows={3}
                />
              ) : (
                <button
                  className="btn btn-primary"
                  disabled={si > 3}
                  onClick={() => handleRound2(i)}
                >
                  ✍️ Run Round 2 for Participant {i + 1}
                </button>
              )}
            </div>
          ))}

          {allRound2Done && si === 3 && (
            <button
              className="btn btn-primary"
              style={{ marginTop: 4 }}
              onClick={handleAggregate}
            >
              🔗 Aggregate Shares → Final Signature
            </button>
          )}
        </section>
      )}

      {/* ── Step 5: Result ───────────────────────────────────────────────────── */}
      {stage === "result" && result && (
        <section className="card" style={{ marginBottom: 20 }}>
          <h3 className="card-title">
            <StepBadge n={5} active={false} done />
            ✅ Final Ed25519 Signature
          </h3>

          <div className="alert alert-ok" style={{ marginBottom: 16 }}>
            🎉 FROST {result.threshold}-of-{result.total_participants} signing
            complete! The signature below is a standard Ed25519 sig — verified and
            ready to submit on Solana.
          </div>

          <div style={{ display: "grid", gap: 12 }}>
            <div>
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  marginBottom: 4,
                }}
              >
                <span className="label">signature_hex (64 bytes)</span>
                <CopyButton text={result.signature_hex} />
              </div>
              <code
                style={{
                  display: "block",
                  wordBreak: "break-all",
                  background: "#0d1117",
                  color: "#4ade80",
                  border: "1px solid #166534",
                  borderRadius: 6,
                  padding: "8px 12px",
                  fontSize: 12,
                  fontFamily: "monospace",
                }}
              >
                {result.signature_hex}
              </code>
            </div>

            <div>
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  marginBottom: 4,
                }}
              >
                <span className="label">verifying_key_hex (group public key, 32 bytes)</span>
                <CopyButton text={result.verifying_key_hex} />
              </div>
              <code
                style={{
                  display: "block",
                  wordBreak: "break-all",
                  background: "#0d1117",
                  color: "#60a5fa",
                  border: "1px solid #1e3a5f",
                  borderRadius: 6,
                  padding: "8px 12px",
                  fontSize: 12,
                  fontFamily: "monospace",
                }}
              >
                {result.verifying_key_hex}
              </code>
            </div>

            <div>
              <span className="label">message_hex</span>
              <code
                style={{
                  display: "block",
                  wordBreak: "break-all",
                  background: "#0d1117",
                  color: "#c9d1d9",
                  border: "1px solid #30363d",
                  borderRadius: 6,
                  padding: "8px 12px",
                  fontSize: 12,
                  fontFamily: "monospace",
                  marginTop: 4,
                }}
              >
                {result.message_hex}
              </code>
            </div>

            <div style={{ display: "flex", gap: 24 }}>
              <div>
                <span className="label">Threshold</span>
                <p style={{ color: "#e2e8f0", margin: "2px 0 0", fontSize: 18, fontWeight: 700 }}>
                  {result.threshold}
                </p>
              </div>
              <div>
                <span className="label">Total Participants</span>
                <p style={{ color: "#e2e8f0", margin: "2px 0 0", fontSize: 18, fontWeight: 700 }}>
                  {result.total_participants}
                </p>
              </div>
              <div>
                <span className="label">Sig length</span>
                <p style={{ color: "#4ade80", margin: "2px 0 0", fontSize: 18, fontWeight: 700 }}>
                  {result.signature_hex.length / 2} bytes
                </p>
              </div>
            </div>
          </div>

          <button
            className="btn btn-sm"
            style={{ marginTop: 16 }}
            onClick={() => {
              setStage("setup");
              setKeyPackages([]);
              setPubkeyPackageJson("");
              setSetupJson("");
              setR1Data([]);
              setR2Data([]);
              setSigningPackageJson("");
              setResult(null);
              setError(null);
            }}
          >
            ↺ Start New Session
          </button>
        </section>
      )}
    </div>
  );
}