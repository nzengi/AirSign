/**
 * MultisigPage — M-of-N air-gapped multi-signature orchestrator.
 *
 * Flow:
 *  1. User fills in the setup form (signers, threshold, tx, cluster, description).
 *  2. For each round, the page shows the request JSON to transmit to the next
 *     air-gapped signer (via QR stream / USB / SD card).
 *  3. The user pastes the response JSON received back from the signer.
 *  4. Once M responses have been ingested, the collected partial signatures are
 *     displayed — ready to be embedded into the Solana transaction and broadcast.
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

type OrchestratorHandle = {
  current_round(): number;
  signer_count(): number;
  threshold(): number;
  threshold_met(): boolean;
  progress(): number;
  current_signer_pubkey(): string | undefined;
  get_request_json(): string;
  ingest_response_json(json: string): boolean;
  get_partial_sigs_json(): string;
  get_transaction_b64(): string;
  nonce(): string;
};

function createOrchestrator(
  txB64: string,
  signers: string[],
  threshold: number,
  cluster: string,
  description: string
): OrchestratorHandle {
  const wasm = getWasm();
  const Cls = wasm["WasmMultiSignOrchestrator"] as new (
    txB64: string,
    signers: string[],
    threshold: number,
    cluster: string,
    description: string
  ) => OrchestratorHandle;
  return new Cls(txB64, signers, threshold, cluster, description);
}

// ─── Types ────────────────────────────────────────────────────────────────────

type SessionStage = "setup" | "signing" | "complete";

interface SignerEntry {
  pubkey: string;
}

// ─── Component ────────────────────────────────────────────────────────────────

export function MultisigPage() {
  // ── Setup form state ────────────────────────────────────────────────────────
  const [signers, setSigners] = useState<SignerEntry[]>([
    { pubkey: "" },
    { pubkey: "" },
  ]);
  const [threshold, setThreshold] = useState(2);
  const [txB64, setTxB64] = useState("");
  const [description, setDescription] = useState("Treasury transaction");
  const [cluster, setCluster] = useState("devnet");

  // ── Session state ───────────────────────────────────────────────────────────
  const [stage, setStage] = useState<SessionStage>("setup");
  const [orchestrator, setOrchestrator] = useState<OrchestratorHandle | null>(null);
  const [currentRound, setCurrentRound] = useState(1);
  const [requestJson, setRequestJson] = useState("");
  const [responseInput, setResponseInput] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [partialSigsJson, setPartialSigsJson] = useState("");
  const [copied, setCopied] = useState<string | null>(null);
  const [progressPct, setProgressPct] = useState(0);

  // ── Setup helpers ───────────────────────────────────────────────────────────

  const addSigner = useCallback(() => {
    setSigners((prev) => [...prev, { pubkey: "" }]);
  }, []);

  const removeSigner = useCallback((idx: number) => {
    setSigners((prev) => {
      const next = prev.filter((_, i) => i !== idx);
      return next.length > 0 ? next : [{ pubkey: "" }];
    });
  }, []);

  const updateSigner = useCallback((idx: number, pubkey: string) => {
    setSigners((prev) =>
      prev.map((s, i) => (i === idx ? { pubkey } : s))
    );
  }, []);

  // ── Session start ───────────────────────────────────────────────────────────

  const startSession = useCallback(() => {
    setError(null);
    const pubkeys = signers.map((s) => s.pubkey.trim()).filter(Boolean);
    if (pubkeys.length === 0) {
      setError("Enter at least one signer public key.");
      return;
    }
    if (threshold < 1 || threshold > pubkeys.length) {
      setError(`Threshold must be between 1 and ${pubkeys.length}.`);
      return;
    }
    if (!txB64.trim()) {
      setError("Paste or enter the Base64-encoded unsigned transaction.");
      return;
    }

    try {
      const orch = createOrchestrator(txB64.trim(), pubkeys, threshold, cluster, description);
      const reqJson = orch.get_request_json();
      setOrchestrator(orch);
      setCurrentRound(orch.current_round());
      setRequestJson(reqJson);
      setProgressPct(0);
      setStage("signing");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [signers, threshold, txB64, description, cluster]);

  // ── Ingest response ─────────────────────────────────────────────────────────

  const ingestResponse = useCallback(() => {
    if (!orchestrator) return;
    setError(null);

    const trimmed = responseInput.trim();
    if (!trimmed) {
      setError("Paste the signer's response JSON first.");
      return;
    }

    try {
      const done = orchestrator.ingest_response_json(trimmed);
      const pct = Math.min(orchestrator.progress() * 100, 100);
      setProgressPct(pct);
      setResponseInput("");

      if (done) {
        const sigs = orchestrator.get_partial_sigs_json();
        setPartialSigsJson(sigs);
        setStage("complete");
      } else {
        const nextReq = orchestrator.get_request_json();
        setCurrentRound(orchestrator.current_round());
        setRequestJson(nextReq);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [orchestrator, responseInput]);

  // ── Copy helper ─────────────────────────────────────────────────────────────

  const copyText = useCallback(
    async (text: string, key: string) => {
      try {
        await navigator.clipboard.writeText(text);
        setCopied(key);
        setTimeout(() => setCopied(null), 2000);
      } catch {
        setCopied(null);
      }
    },
    []
  );

  // ── Reset ───────────────────────────────────────────────────────────────────

  const reset = useCallback(() => {
    setOrchestrator(null);
    setStage("setup");
    setRequestJson("");
    setResponseInput("");
    setError(null);
    setPartialSigsJson("");
    setProgressPct(0);
    setCurrentRound(1);
  }, []);

  // ──────────────────────────────────────────────────────────────────────────
  // Render
  // ──────────────────────────────────────────────────────────────────────────

  return (
    <div className="page">
      <h2 className="section-title">🔏 M-of-N Multi-Signature Orchestrator</h2>

      {/* ── Progress bar (signing stage) ─────────────────────────────────── */}
      {stage === "signing" && orchestrator && (
        <div style={{ marginBottom: 20 }}>
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              fontSize: 13,
              color: "var(--text-muted)",
              marginBottom: 6,
            }}
          >
            <span>
              Round {currentRound} of {orchestrator.signer_count()} —{" "}
              {orchestrator.threshold() - Math.round(progressPct / 100 * orchestrator.threshold())} signature(s) still needed
            </span>
            <span>{Math.round(progressPct)}%</span>
          </div>
          <div
            style={{
              background: "var(--surface-2)",
              borderRadius: 8,
              height: 10,
              overflow: "hidden",
            }}
          >
            <div
              style={{
                height: "100%",
                width: `${progressPct}%`,
                background: "var(--accent)",
                borderRadius: 8,
                transition: "width 0.4s ease",
              }}
            />
          </div>
          {orchestrator.current_signer_pubkey() && (
            <p style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 6 }}>
              Waiting for:{" "}
              <code style={{ wordBreak: "break-all" }}>
                {orchestrator.current_signer_pubkey()}
              </code>
            </p>
          )}
        </div>
      )}

      {/* ── Error banner ─────────────────────────────────────────────────── */}
      {error && (
        <div className="alert alert-err" style={{ marginBottom: 16 }}>
          ⚠️ {error}
        </div>
      )}

      {/* ════════════════════════════════════════════════════════════════════
          STAGE: setup
          ════════════════════════════════════════════════════════════════════ */}
      {stage === "setup" && (
        <>
          <section className="card">
            <h3 className="card-title">Transaction</h3>

            <label className="field-label">Cluster</label>
            <select
              className="input"
              value={cluster}
              onChange={(e) => setCluster(e.target.value)}
            >
              <option value="mainnet-beta">mainnet-beta</option>
              <option value="devnet">devnet</option>
              <option value="testnet">testnet</option>
              <option value="localnet">localnet</option>
            </select>

            <label className="field-label" style={{ marginTop: 12 }}>
              Description
            </label>
            <input
              className="input"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="e.g. Treasury transfer 50 SOL to DAO"
            />

            <label className="field-label" style={{ marginTop: 12 }}>
              Unsigned transaction (Base64-encoded)
            </label>
            <textarea
              className="input"
              rows={4}
              value={txB64}
              onChange={(e) => setTxB64(e.target.value)}
              placeholder="Paste the Base64-encoded unsigned Solana transaction bytes here…"
              style={{ fontFamily: "monospace", fontSize: 12 }}
            />
          </section>

          <section className="card" style={{ marginTop: 16 }}>
            <h3 className="card-title">Signers</h3>

            {signers.map((s, i) => (
              <div
                key={i}
                style={{ display: "flex", gap: 8, marginBottom: 8, alignItems: "center" }}
              >
                <span
                  style={{
                    minWidth: 24,
                    fontSize: 13,
                    color: "var(--text-muted)",
                    textAlign: "right",
                  }}
                >
                  {i + 1}.
                </span>
                <input
                  className="input"
                  style={{ flex: 1 }}
                  value={s.pubkey}
                  onChange={(e) => updateSigner(i, e.target.value)}
                  placeholder={`Signer ${i + 1} public key (Base58)`}
                />
                {signers.length > 1 && (
                  <button
                    className="btn btn-sm"
                    style={{ background: "var(--err)", padding: "6px 10px" }}
                    onClick={() => removeSigner(i)}
                  >
                    ✕
                  </button>
                )}
              </div>
            ))}

            <button className="btn btn-sm" onClick={addSigner} style={{ marginTop: 4 }}>
              + Add signer
            </button>

            <div style={{ marginTop: 16 }}>
              <label className="field-label">
                Threshold M (signatures required out of {signers.length})
              </label>
              <input
                className="input"
                type="number"
                min={1}
                max={signers.length}
                value={threshold}
                onChange={(e) =>
                  setThreshold(Math.max(1, Math.min(signers.length, Number(e.target.value))))
                }
                style={{ width: 80 }}
              />
            </div>
          </section>

          <button
            className="btn btn-primary"
            style={{ marginTop: 20, width: "100%" }}
            onClick={startSession}
          >
            🚀 Start Multi-Sign Session
          </button>
        </>
      )}

      {/* ════════════════════════════════════════════════════════════════════
          STAGE: signing
          ════════════════════════════════════════════════════════════════════ */}
      {stage === "signing" && (
        <>
          <section className="card">
            <div
              style={{
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
                marginBottom: 8,
              }}
            >
              <h3 className="card-title" style={{ margin: 0 }}>
                Round {currentRound} — Request JSON
              </h3>
              <button
                className="btn btn-sm"
                onClick={() => copyText(requestJson, "request")}
              >
                {copied === "request" ? "✓ Copied!" : "📋 Copy"}
              </button>
            </div>
            <p style={{ fontSize: 13, color: "var(--text-muted)", marginBottom: 10 }}>
              Transmit this JSON to signer{" "}
              <strong>{orchestrator?.current_signer_pubkey()?.slice(0, 8)}…</strong>{" "}
              via QR stream, USB, or SD card. The signer uses the AirSign CLI or
              air-gapped web app to produce a response.
            </p>
            <textarea
              className="input"
              rows={12}
              readOnly
              value={requestJson}
              style={{ fontFamily: "monospace", fontSize: 11, background: "var(--surface-2)" }}
            />
          </section>

          <section className="card" style={{ marginTop: 16 }}>
            <h3 className="card-title">Paste Signer Response</h3>
            <p style={{ fontSize: 13, color: "var(--text-muted)", marginBottom: 10 }}>
              After the signer produces a response JSON, paste it here. The
              orchestrator verifies the nonce, round, and signer pubkey before
              accepting the partial signature.
            </p>
            <textarea
              className="input"
              rows={8}
              value={responseInput}
              onChange={(e) => setResponseInput(e.target.value)}
              placeholder='{"version":2,"nonce":"…","round":1,"signer_pubkey":"…","signature_b64":"…"}'
              style={{ fontFamily: "monospace", fontSize: 11 }}
            />
            <div style={{ display: "flex", gap: 10, marginTop: 12 }}>
              <button
                className="btn btn-primary"
                style={{ flex: 1 }}
                onClick={ingestResponse}
                disabled={!responseInput.trim()}
              >
                ✅ Accept Signature
              </button>
              <button className="btn" onClick={reset}>
                ✕ Abort
              </button>
            </div>
          </section>
        </>
      )}

      {/* ════════════════════════════════════════════════════════════════════
          STAGE: complete
          ════════════════════════════════════════════════════════════════════ */}
      {stage === "complete" && (
        <>
          <div className="alert alert-ok" style={{ marginBottom: 16 }}>
            ✅ Threshold reached — {orchestrator?.threshold()} of{" "}
            {orchestrator?.signer_count()} signatures collected.
          </div>

          <section className="card">
            <div
              style={{
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
                marginBottom: 8,
              }}
            >
              <h3 className="card-title" style={{ margin: 0 }}>
                Partial Signatures
              </h3>
              <button
                className="btn btn-sm"
                onClick={() => copyText(partialSigsJson, "sigs")}
              >
                {copied === "sigs" ? "✓ Copied!" : "📋 Copy JSON"}
              </button>
            </div>
            <p style={{ fontSize: 13, color: "var(--text-muted)", marginBottom: 10 }}>
              Pass these partial signatures to{" "}
              <code>@solana/web3.js</code> to embed them into the transaction
              before broadcasting to the cluster.
            </p>
            <textarea
              className="input"
              rows={14}
              readOnly
              value={partialSigsJson}
              style={{ fontFamily: "monospace", fontSize: 11, background: "var(--surface-2)" }}
            />
          </section>

          <section className="card" style={{ marginTop: 16 }}>
            <h3 className="card-title">Next Steps</h3>
            <ol style={{ paddingLeft: 18, lineHeight: 1.8, fontSize: 14 }}>
              <li>
                Decode the original transaction Base64 back to bytes using{" "}
                <code>Transaction.deserialize()</code>.
              </li>
              <li>
                For each entry in the partial signatures array, call{" "}
                <code>transaction.addSignature(pubkey, Buffer.from(sig_b64, "base64"))</code>.
              </li>
              <li>
                Verify the transaction is fully signed:{" "}
                <code>transaction.verifySignatures()</code>.
              </li>
              <li>
                Broadcast via <code>connection.sendRawTransaction(transaction.serialize())</code>.
              </li>
            </ol>
            <p style={{ fontSize: 13, color: "var(--text-muted)", marginTop: 12 }}>
              Session nonce:{" "}
              <code style={{ wordBreak: "break-all" }}>{orchestrator?.nonce()}</code>
            </p>
          </section>

          <button
            className="btn btn-primary"
            style={{ marginTop: 20, width: "100%" }}
            onClick={reset}
          >
            🔄 New Session
          </button>
        </>
      )}
    </div>
  );
}