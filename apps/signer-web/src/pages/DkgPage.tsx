/**
 * DkgPage — Trustless Distributed Key Generation (DKG) tab.
 *
 * Demonstrates a full FROST RFC 9591 DKG session inside the browser:
 *   Round 1  → each simulated participant generates a commitment
 *   Round 2  → each participant processes all commitments and emits directed packages
 *   Finish   → each participant assembles their final key share
 *
 * All cryptography runs in the WASM module (afterimage-wasm / afterimage-dkg).
 * No server required — the "coordinator" role is played by this React state.
 */

import { useRef, useState } from "react";

// ─── WASM accessor (same pattern as FrostPage) ────────────────────────────────
function getWasm(): Record<string, unknown> {
  return (
    ((globalThis as Record<string, unknown>).__airsign_wasm__ as
      | Record<string, unknown>
      | undefined) ?? {}
  );
}

// ── types mirroring Rust DkgRound1Output / DkgRound2Output / DkgOutput ────────

interface Round1Output {
  identifier: number;
  round1_package_json: string;
  secret_package_json: string;  // kept client-side only, never displayed
}

interface Round2PackageEntry {
  recipient_identifier: number;
  package_json: string;
}

interface Round2Output {
  identifier: number;
  secret_package_json: string;
  round2_packages: Round2PackageEntry[];
}

interface DkgOutput {
  identifier: number;
  key_package_json: string;      // private
  pubkey_package_json: string;   // public
  group_pubkey_hex: string;
}

// ── helpers ───────────────────────────────────────────────────────────────────

function shortHex(hex: string, chars = 16): string {
  if (hex.length <= chars * 2) return hex;
  return hex.slice(0, chars) + "…" + hex.slice(-chars);
}

// ── component ─────────────────────────────────────────────────────────────────

export default function DkgPage() {
  // ── config ─────────────────────────────────────────────────────────────────
  const [n, setN] = useState(3);
  const [threshold, setThreshold] = useState(2);

  // ── session state ──────────────────────────────────────────────────────────
  const [phase, setPhase] = useState<"idle" | "r1" | "r2" | "done" | "error">("idle");
  const [error, setError] = useState("");

  const [r1Outputs, setR1Outputs] = useState<Round1Output[]>([]);
  const [r2Outputs, setR2Outputs] = useState<Round2Output[]>([]);
  const [dkgOutputs, setDkgOutputs] = useState<DkgOutput[]>([]);

  // WasmDkgParticipant objects — one per participant (held in a ref so React
  // doesn't try to serialise them into the virtual DOM).
  const participantRefs = useRef<Record<number, unknown>>({});

  const [busy, setBusy] = useState(false);

  // ── round 1 ────────────────────────────────────────────────────────────────
  async function runRound1() {
    setBusy(true);
    setError("");
    try {
      const wasm = getWasm();
      const outputs: Round1Output[] = [];
      participantRefs.current = {};

      for (let id = 1; id <= n; id++) {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const p = new (wasm as any).WasmDkgParticipant(id, n, threshold);
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const json = (p as any).round1() as string;
        const out = JSON.parse(json) as Round1Output;
        outputs.push(out);
        participantRefs.current[id] = p;
      }

      setR1Outputs(outputs);
      setPhase("r1");
    } catch (e: unknown) {
      setError(String(e));
      setPhase("error");
    } finally {
      setBusy(false);
    }
  }

  // ── round 2 ────────────────────────────────────────────────────────────────
  async function runRound2() {
    setBusy(true);
    setError("");
    try {
      const allR1Json = JSON.stringify(r1Outputs);
      const outputs: Round2Output[] = [];

      for (let id = 1; id <= n; id++) {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const p = participantRefs.current[id] as any;
        const json = p.round2(allR1Json) as string;
        const out = JSON.parse(json) as Round2Output;
        outputs.push(out);
      }

      setR2Outputs(outputs);
      setPhase("r2");
    } catch (e: unknown) {
      setError(String(e));
      setPhase("error");
    } finally {
      setBusy(false);
    }
  }

  // ── finish ─────────────────────────────────────────────────────────────────
  async function runFinish() {
    setBusy(true);
    setError("");
    try {
      const allR1Json = JSON.stringify(r1Outputs);
      const allR2Json = JSON.stringify(r2Outputs);
      const outputs: DkgOutput[] = [];

      for (let id = 1; id <= n; id++) {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const p = participantRefs.current[id] as any;
        const json = p.finish(allR1Json, allR2Json) as string;
        const out = JSON.parse(json) as DkgOutput;
        outputs.push(out);
      }

      setDkgOutputs(outputs);
      setPhase("done");
    } catch (e: unknown) {
      setError(String(e));
      setPhase("error");
    } finally {
      setBusy(false);
    }
  }

  // ── reset ──────────────────────────────────────────────────────────────────
  function reset() {
    participantRefs.current = {};
    setR1Outputs([]);
    setR2Outputs([]);
    setDkgOutputs([]);
    setError("");
    setPhase("idle");
  }

  // ── validation ─────────────────────────────────────────────────────────────
  const configValid = n >= 2 && threshold >= 2 && threshold <= n;

  // ── render ─────────────────────────────────────────────────────────────────
  return (
    <div className="page dkg-page">
      <h2>Distributed Key Generation (DKG)</h2>
      <p className="subtitle">
        FROST RFC 9591 — trustless t-of-n threshold key setup without a trusted dealer.
        All cryptography runs locally in WebAssembly.
      </p>

      {/* ── config ── */}
      <section className="card">
        <h3>Session Parameters</h3>
        <div className="form-row">
          <label>
            Participants (n)
            <input
              type="number"
              min={2}
              max={10}
              value={n}
              disabled={phase !== "idle"}
              onChange={(e) => setN(Math.max(2, parseInt(e.target.value) || 2))}
            />
          </label>
          <label>
            Threshold (t)
            <input
              type="number"
              min={2}
              max={n}
              value={threshold}
              disabled={phase !== "idle"}
              onChange={(e) =>
                setThreshold(Math.max(2, Math.min(n, parseInt(e.target.value) || 2)))
              }
            />
          </label>
        </div>
        {!configValid && (
          <p className="warn">Require 2 ≤ t ≤ n.</p>
        )}
      </section>

      {/* ── progress steps ── */}
      <div className="dkg-steps">
        {(["idle", "r1", "r2", "done"] as const).map((s, i) => (
          <div
            key={s}
            className={`step ${
              phase === s
                ? "active"
                : ["idle", "r1", "r2", "done"].indexOf(phase) > i
                ? "done"
                : ""
            }`}
          >
            <span className="step-num">{i + 1}</span>
            <span className="step-label">
              {["Setup", "Round 1", "Round 2", "Finish"][i]}
            </span>
          </div>
        ))}
      </div>

      {/* ── action buttons ── */}
      <div className="action-bar">
        {phase === "idle" && (
          <button onClick={runRound1} disabled={busy || !configValid}>
            {busy ? "Working…" : "▶ Start Round 1"}
          </button>
        )}
        {phase === "r1" && (
          <button onClick={runRound2} disabled={busy}>
            {busy ? "Working…" : "▶ Run Round 2"}
          </button>
        )}
        {phase === "r2" && (
          <button onClick={runFinish} disabled={busy}>
            {busy ? "Working…" : "▶ Finish DKG"}
          </button>
        )}
        {(phase === "done" || phase === "error") && (
          <button onClick={reset}>↺ Reset</button>
        )}
      </div>

      {/* ── error ── */}
      {phase === "error" && (
        <div className="error-box">
          <strong>Error:</strong> {error}
        </div>
      )}

      {/* ── round 1 results ── */}
      {r1Outputs.length > 0 && (
        <section className="card">
          <h3>Round 1 — Public Commitments</h3>
          <p className="hint">
            Each participant broadcasts their <code>round1_package_json</code> to all
            peers. Secret packages remain private.
          </p>
          <table>
            <thead>
              <tr>
                <th>Participant</th>
                <th>Commitment (truncated)</th>
              </tr>
            </thead>
            <tbody>
              {r1Outputs.map((r) => (
                <tr key={r.identifier}>
                  <td>#{r.identifier}</td>
                  <td>
                    <code>{shortHex(btoa(r.round1_package_json).slice(0, 48))}…</code>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}

      {/* ── round 2 results ── */}
      {r2Outputs.length > 0 && (
        <section className="card">
          <h3>Round 2 — Directed Packages</h3>
          <p className="hint">
            Each sender emits one encrypted package per peer.  Packages are routed
            only to their intended recipient — never broadcast.
          </p>
          <table>
            <thead>
              <tr>
                <th>Sender</th>
                <th>Recipients</th>
                <th>Packages</th>
              </tr>
            </thead>
            <tbody>
              {r2Outputs.map((r) => (
                <tr key={r.identifier}>
                  <td>#{r.identifier}</td>
                  <td>
                    {r.round2_packages.map((p) => `#${p.recipient_identifier}`).join(", ")}
                  </td>
                  <td>{r.round2_packages.length}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}

      {/* ── final results ── */}
      {phase === "done" && dkgOutputs.length > 0 && (
        <section className="card success-card">
          <h3>DKG Complete ✓</h3>
          <p className="hint">
            All participants share the same group public key.  Each holds a unique
            private key share — no single party ever knew the full secret.
          </p>

          {/* Group public key — same for all */}
          <div className="group-key-box">
            <span className="label">Group Public Key (Solana-compatible Ed25519)</span>
            <code className="pubkey">{dkgOutputs[0].group_pubkey_hex}</code>
          </div>

          {/* Verify all pubkeys match */}
          {dkgOutputs.every((o) => o.group_pubkey_hex === dkgOutputs[0].group_pubkey_hex) ? (
            <p className="ok">✓ All {n} participants derived the same group public key.</p>
          ) : (
            <p className="warn">⚠ Group public key mismatch — this should never happen!</p>
          )}

          {/* Per-participant key shares */}
          <table>
            <thead>
              <tr>
                <th>Participant</th>
                <th>Key Share (private — truncated)</th>
              </tr>
            </thead>
            <tbody>
              {dkgOutputs.map((o) => (
                <tr key={o.identifier}>
                  <td>#{o.identifier}</td>
                  <td>
                    <code>{shortHex(btoa(o.key_package_json).slice(0, 48))}…</code>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>

          <p className="note">
            The key packages above are compatible with the FROST Signing tab — paste
            any participant's <em>key_package_json</em> there to sign a Solana
            transaction without a trusted dealer.
          </p>
        </section>
      )}
    </div>
  );
}