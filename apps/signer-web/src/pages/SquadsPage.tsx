import { useState } from "react";

// ─── Types ────────────────────────────────────────────────────────────────────

type Tab = "pda" | "create" | "approve" | "propose" | "addMember" | "removeMember" | "changeThreshold";

interface JsonResult {
  ok: boolean;
  data: unknown;
}

// ─── helpers ──────────────────────────────────────────────────────────────────

const SQUADS_V4_PROGRAM = "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf";

function sha256Disc(name: string): string {
  // We cannot run native crypto synchronously in a browser snippet without
  // Web Crypto, so we render a placeholder that the user can replace after
  // calling the CLI.  The real discriminator is computed by the Rust library.
  return `global:${name}  (computed by CLI)`;
}

function clsx(...classes: (string | undefined | false)[]) {
  return classes.filter(Boolean).join(" ");
}

// ─── Sub-forms ────────────────────────────────────────────────────────────────

function PdaForm() {
  const [createKey, setCreateKey] = useState("");
  const [result, setResult] = useState<JsonResult | null>(null);

  function run() {
    if (!createKey.trim()) return;
    setResult({
      ok: true,
      data: {
        note: "Copy this create-key and run: airsign squads pda --create-key <KEY>",
        create_key: createKey.trim(),
        program_id: SQUADS_V4_PROGRAM,
        seeds: ["multisig", createKey.trim()],
      },
    });
  }

  return (
    <section className="form-section">
      <h3>Derive PDAs</h3>
      <p className="hint">
        Derive the multisig and default vault PDAs from an ephemeral create key.
      </p>
      <label>
        Create Key (base58 pubkey)
        <input
          type="text"
          value={createKey}
          onChange={(e) => setCreateKey(e.target.value)}
          placeholder="4wTQ…"
        />
      </label>
      <button onClick={run} disabled={!createKey.trim()}>
        Derive PDAs
      </button>

      <CliHint cmd={`airsign squads pda --create-key ${createKey || "<CREATE_KEY>"}`} />
      <ResultBox result={result} />
    </section>
  );
}

function CreateForm() {
  const [createKey, setCreateKey] = useState("");
  const [members, setMembers] = useState("");
  const [threshold, setThreshold] = useState(2);
  const [timeLock, setTimeLock] = useState(0);
  const [memo, setMemo] = useState("");
  const [result, setResult] = useState<JsonResult | null>(null);

  function run() {
    if (!createKey.trim() || !members.trim()) return;
    const memberList = members.split(",").map((m) => m.trim());
    if (threshold < 1 || threshold > memberList.length) {
      setResult({ ok: false, data: `Threshold must be between 1 and ${memberList.length}` });
      return;
    }
    setResult({
      ok: true,
      data: {
        note: "Run the CLI command below to produce the actual instruction JSON.",
        create_key: createKey.trim(),
        members: memberList,
        threshold,
        time_lock: timeLock,
        memo: memo || null,
      },
    });
  }

  const memberCount = members.split(",").filter((m) => m.trim()).length;
  const cliCmd = [
    `airsign squads create`,
    `--create-key ${createKey || "<CREATE_KEY>"}`,
    `--members ${members || "<PUBKEY,...>"}`,
    `--threshold ${threshold}`,
    timeLock ? `--time-lock ${timeLock}` : "",
    memo ? `--memo "${memo}"` : "",
  ]
    .filter(Boolean)
    .join(" \\\n  ");

  return (
    <section className="form-section">
      <h3>Create Multisig</h3>
      <p className="hint">
        Build a <code>multisig_create_v2</code> instruction. Use{" "}
        <code>voter:</code>, <code>initiator:</code>, or <code>executor:</code> prefixes for
        limited permissions; no prefix = full permissions.
      </p>

      <label>
        Create Key (base58)
        <input value={createKey} onChange={(e) => setCreateKey(e.target.value)} placeholder="4wTQ…" />
      </label>
      <label>
        Members (comma-separated, optional prefixes)
        <input
          value={members}
          onChange={(e) => setMembers(e.target.value)}
          placeholder="Alice…,voter:Bob…,Carol…"
        />
      </label>
      <div className="row">
        <label>
          Threshold (M-of-{memberCount || "N"})
          <input
            type="number"
            min={1}
            max={memberCount || 99}
            value={threshold}
            onChange={(e) => setThreshold(Number(e.target.value))}
          />
        </label>
        <label>
          Time-lock (seconds)
          <input type="number" min={0} value={timeLock} onChange={(e) => setTimeLock(Number(e.target.value))} />
        </label>
      </div>
      <label>
        Memo (optional)
        <input value={memo} onChange={(e) => setMemo(e.target.value)} placeholder="My treasury" />
      </label>
      <button onClick={run} disabled={!createKey.trim() || !members.trim()}>
        Build Instruction JSON
      </button>

      <CliHint cmd={cliCmd} />
      <ResultBox result={result} />
    </section>
  );
}

function ApproveForm() {
  const [multisig, setMultisig] = useState("");
  const [txIndex, setTxIndex] = useState(1);
  const [approver, setApprover] = useState("");
  const [memo, setMemo] = useState("");
  const [result, setResult] = useState<JsonResult | null>(null);

  function run() {
    if (!multisig.trim() || !approver.trim()) return;
    setResult({
      ok: true,
      data: {
        note: "Run the CLI command to produce an AirSign QR payload for signing.",
        multisig_pda: multisig.trim(),
        transaction_index: txIndex,
        approver: approver.trim(),
        memo: memo || null,
      },
    });
  }

  const cliCmd = [
    `airsign squads approve`,
    `--multisig ${multisig || "<MULTISIG_PDA>"}`,
    `--tx-index ${txIndex}`,
    `--approver ${approver || "<APPROVER_PUBKEY>"}`,
    memo ? `--memo "${memo}"` : "",
  ]
    .filter(Boolean)
    .join(" \\\n  ");

  return (
    <section className="form-section">
      <h3>Approve Proposal</h3>
      <p className="hint">
        Build a <code>proposal_approve</code> instruction wrapped in an AirSign QR payload.
        Pipe the CLI output to <code>airsign send</code> to transmit it to the air-gapped signer.
      </p>

      <label>
        Multisig PDA
        <input value={multisig} onChange={(e) => setMultisig(e.target.value)} placeholder="SQDS…" />
      </label>
      <label>
        Transaction / Proposal Index
        <input type="number" min={1} value={txIndex} onChange={(e) => setTxIndex(Number(e.target.value))} />
      </label>
      <label>
        Approver Public Key
        <input value={approver} onChange={(e) => setApprover(e.target.value)} placeholder="4wTQ…" />
      </label>
      <label>
        Memo (optional)
        <input value={memo} onChange={(e) => setMemo(e.target.value)} placeholder="" />
      </label>
      <button onClick={run} disabled={!multisig.trim() || !approver.trim()}>
        Build Approval Payload
      </button>

      <CliHint cmd={cliCmd} />
      <ResultBox result={result} />
    </section>
  );
}

function ProposeForm() {
  const [multisig, setMultisig] = useState("");
  const [creator, setCreator] = useState("");
  const [txIndex, setTxIndex] = useState(1);
  const [message, setMessage] = useState("");
  const [vaultIndex, setVaultIndex] = useState(0);
  const [memo, setMemo] = useState("");
  const [result, setResult] = useState<JsonResult | null>(null);

  function run() {
    if (!multisig.trim() || !creator.trim() || !message.trim()) return;
    setResult({
      ok: true,
      data: {
        note: "Run the CLI command to produce vault_transaction_create + proposal_create instruction JSON.",
        multisig_pda: multisig.trim(),
        creator: creator.trim(),
        transaction_index: txIndex,
        vault_index: vaultIndex,
        transaction_message_b64: message.trim(),
        memo: memo || null,
      },
    });
  }

  const cliCmd = [
    `airsign squads propose`,
    `--multisig ${multisig || "<MULTISIG_PDA>"}`,
    `--creator ${creator || "<CREATOR_PUBKEY>"}`,
    `--tx-index ${txIndex}`,
    `--vault-index ${vaultIndex}`,
    `--message ${message ? `"${message.slice(0, 20)}…"` : "<BASE64_MSG>"}`,
    memo ? `--memo "${memo}"` : "",
  ]
    .filter(Boolean)
    .join(" \\\n  ");

  return (
    <section className="form-section">
      <h3>Propose Vault Transaction</h3>
      <p className="hint">
        Builds a <code>vault_transaction_create</code> + <code>proposal_create</code> instruction
        pair. The inner transaction message must be base64-encoded bincode{" "}
        <code>solana_sdk::message::Message</code>.
      </p>

      <label>
        Multisig PDA
        <input value={multisig} onChange={(e) => setMultisig(e.target.value)} placeholder="SQDS…" />
      </label>
      <label>
        Creator / Fee-payer
        <input value={creator} onChange={(e) => setCreator(e.target.value)} placeholder="4wTQ…" />
      </label>
      <div className="row">
        <label>
          Transaction Index
          <input type="number" min={1} value={txIndex} onChange={(e) => setTxIndex(Number(e.target.value))} />
        </label>
        <label>
          Vault Index
          <input type="number" min={0} value={vaultIndex} onChange={(e) => setVaultIndex(Number(e.target.value))} />
        </label>
      </div>
      <label>
        Transaction Message (base64)
        <textarea
          rows={3}
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          placeholder="AQID…"
        />
      </label>
      <label>
        Memo (optional)
        <input value={memo} onChange={(e) => setMemo(e.target.value)} />
      </label>
      <button onClick={run} disabled={!multisig.trim() || !creator.trim() || !message.trim()}>
        Build Propose Instructions
      </button>

      <CliHint cmd={cliCmd} />
      <ResultBox result={result} />
    </section>
  );
}

function AddMemberForm() {
  const [multisig, setMultisig] = useState("");
  const [creator, setCreator] = useState("");
  const [txIndex, setTxIndex] = useState(1);
  const [member, setMember] = useState("");
  const [perm, setPerm] = useState<"full" | "voter" | "initiator" | "executor">("full");
  const [memo, setMemo] = useState("");
  const [result, setResult] = useState<JsonResult | null>(null);

  function run() {
    if (!multisig.trim() || !creator.trim() || !member.trim()) return;
    const prefixed = perm === "full" ? member.trim() : `${perm}:${member.trim()}`;
    setResult({
      ok: true,
      data: {
        note: "Run the CLI command to produce the config_transaction_create instruction JSON.",
        multisig_pda: multisig.trim(),
        creator: creator.trim(),
        tx_index: txIndex,
        new_member: prefixed,
        memo: memo || null,
      },
    });
  }

  const prefixed = perm === "full" ? member || "<PUBKEY>" : `${perm}:${member || "<PUBKEY>"}`;
  const cliCmd = [
    `airsign squads add-member`,
    `--multisig ${multisig || "<MULTISIG_PDA>"}`,
    `--creator ${creator || "<CREATOR_PUBKEY>"}`,
    `--tx-index ${txIndex}`,
    `--member ${prefixed}`,
    memo ? `--memo "${memo}"` : "",
  ]
    .filter(Boolean)
    .join(" \\\n  ");

  return (
    <section className="form-section">
      <h3>Add Member</h3>
      <p className="hint">Build a config transaction that adds a new member to the multisig.</p>

      <label>Multisig PDA<input value={multisig} onChange={(e) => setMultisig(e.target.value)} placeholder="SQDS…" /></label>
      <label>Creator / Fee-payer<input value={creator} onChange={(e) => setCreator(e.target.value)} placeholder="4wTQ…" /></label>
      <label>Config Tx Index<input type="number" min={1} value={txIndex} onChange={(e) => setTxIndex(Number(e.target.value))} /></label>
      <label>New Member Public Key<input value={member} onChange={(e) => setMember(e.target.value)} placeholder="9xRz…" /></label>
      <label>
        Permissions
        <select value={perm} onChange={(e) => setPerm(e.target.value as typeof perm)}>
          <option value="full">Full (Proposer + Voter + Executor)</option>
          <option value="voter">Voter only</option>
          <option value="initiator">Initiator only</option>
          <option value="executor">Executor only</option>
        </select>
      </label>
      <label>Memo (optional)<input value={memo} onChange={(e) => setMemo(e.target.value)} /></label>
      <button onClick={run} disabled={!multisig.trim() || !creator.trim() || !member.trim()}>
        Build Add-Member Instruction
      </button>

      <CliHint cmd={cliCmd} />
      <ResultBox result={result} />
    </section>
  );
}

function RemoveMemberForm() {
  const [multisig, setMultisig] = useState("");
  const [creator, setCreator] = useState("");
  const [txIndex, setTxIndex] = useState(1);
  const [member, setMember] = useState("");
  const [memo, setMemo] = useState("");
  const [result, setResult] = useState<JsonResult | null>(null);

  function run() {
    if (!multisig.trim() || !creator.trim() || !member.trim()) return;
    setResult({
      ok: true,
      data: {
        note: "Run the CLI command below to produce the instruction JSON.",
        multisig_pda: multisig.trim(),
        creator: creator.trim(),
        tx_index: txIndex,
        member_to_remove: member.trim(),
        memo: memo || null,
      },
    });
  }

  const cliCmd = [
    `airsign squads remove-member`,
    `--multisig ${multisig || "<MULTISIG_PDA>"}`,
    `--creator ${creator || "<CREATOR_PUBKEY>"}`,
    `--tx-index ${txIndex}`,
    `--member ${member || "<MEMBER_PUBKEY>"}`,
    memo ? `--memo "${memo}"` : "",
  ]
    .filter(Boolean)
    .join(" \\\n  ");

  return (
    <section className="form-section">
      <h3>Remove Member</h3>
      <p className="hint">Build a config transaction that removes a member from the multisig.</p>

      <label>Multisig PDA<input value={multisig} onChange={(e) => setMultisig(e.target.value)} placeholder="SQDS…" /></label>
      <label>Creator / Fee-payer<input value={creator} onChange={(e) => setCreator(e.target.value)} placeholder="4wTQ…" /></label>
      <label>Config Tx Index<input type="number" min={1} value={txIndex} onChange={(e) => setTxIndex(Number(e.target.value))} /></label>
      <label>Member to Remove<input value={member} onChange={(e) => setMember(e.target.value)} placeholder="9xRz…" /></label>
      <label>Memo (optional)<input value={memo} onChange={(e) => setMemo(e.target.value)} /></label>
      <button onClick={run} disabled={!multisig.trim() || !creator.trim() || !member.trim()}>
        Build Remove-Member Instruction
      </button>

      <CliHint cmd={cliCmd} />
      <ResultBox result={result} />
    </section>
  );
}

function ChangeThresholdForm() {
  const [multisig, setMultisig] = useState("");
  const [creator, setCreator] = useState("");
  const [txIndex, setTxIndex] = useState(1);
  const [threshold, setThreshold] = useState(2);
  const [memo, setMemo] = useState("");
  const [result, setResult] = useState<JsonResult | null>(null);

  function run() {
    if (!multisig.trim() || !creator.trim()) return;
    setResult({
      ok: true,
      data: {
        note: "Run the CLI command below to produce the instruction JSON.",
        multisig_pda: multisig.trim(),
        creator: creator.trim(),
        tx_index: txIndex,
        new_threshold: threshold,
        memo: memo || null,
      },
    });
  }

  const cliCmd = [
    `airsign squads change-threshold`,
    `--multisig ${multisig || "<MULTISIG_PDA>"}`,
    `--creator ${creator || "<CREATOR_PUBKEY>"}`,
    `--tx-index ${txIndex}`,
    `--threshold ${threshold}`,
    memo ? `--memo "${memo}"` : "",
  ]
    .filter(Boolean)
    .join(" \\\n  ");

  return (
    <section className="form-section">
      <h3>Change Threshold</h3>
      <p className="hint">Build a config transaction that updates the approval threshold.</p>

      <label>Multisig PDA<input value={multisig} onChange={(e) => setMultisig(e.target.value)} placeholder="SQDS…" /></label>
      <label>Creator / Fee-payer<input value={creator} onChange={(e) => setCreator(e.target.value)} placeholder="4wTQ…" /></label>
      <label>Config Tx Index<input type="number" min={1} value={txIndex} onChange={(e) => setTxIndex(Number(e.target.value))} /></label>
      <label>
        New Threshold
        <input type="number" min={1} value={threshold} onChange={(e) => setThreshold(Number(e.target.value))} />
      </label>
      <label>Memo (optional)<input value={memo} onChange={(e) => setMemo(e.target.value)} /></label>
      <button onClick={run} disabled={!multisig.trim() || !creator.trim()}>
        Build Change-Threshold Instruction
      </button>

      <CliHint cmd={cliCmd} />
      <ResultBox result={result} />
    </section>
  );
}

// ─── Shared UI components ─────────────────────────────────────────────────────

function CliHint({ cmd }: { cmd: string }) {
  const [copied, setCopied] = useState(false);

  function copy() {
    navigator.clipboard.writeText(cmd).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  }

  return (
    <div className="cli-hint">
      <span className="cli-label">CLI</span>
      <pre>{cmd}</pre>
      <button className="copy-btn" onClick={copy}>
        {copied ? "✓ copied" : "Copy"}
      </button>
    </div>
  );
}

function ResultBox({ result }: { result: JsonResult | null }) {
  if (!result) return null;
  return (
    <div className={clsx("result-box", result.ok ? "result-ok" : "result-err")}>
      <pre>{JSON.stringify(result.data, null, 2)}</pre>
    </div>
  );
}

// ─── Main page ────────────────────────────────────────────────────────────────

const TABS: { id: Tab; label: string }[] = [
  { id: "pda", label: "PDAs" },
  { id: "create", label: "Create" },
  { id: "approve", label: "Approve" },
  { id: "propose", label: "Propose" },
  { id: "addMember", label: "Add Member" },
  { id: "removeMember", label: "Remove Member" },
  { id: "changeThreshold", label: "Change Threshold" },
];

export default function SquadsPage() {
  const [tab, setTab] = useState<Tab>("pda");

  return (
    <div className="page squads-page">
      <h2>Squads v4 Multisig</h2>
      <p className="page-desc">
        Build Squads v4 multisig instructions <em>completely offline</em>. Copy the generated CLI
        command to your air-gapped machine, pipe the JSON output to{" "}
        <code>airsign send</code>, and scan the QR stream on the signing device.
      </p>

      {/* Tab bar */}
      <div className="tab-bar">
        {TABS.map((t) => (
          <button
            key={t.id}
            className={clsx("tab-btn", tab === t.id && "tab-btn--active")}
            onClick={() => setTab(t.id)}
          >
            {t.label}
          </button>
        ))}
      </div>

      {/* Active form */}
      <div className="tab-content">
        {tab === "pda" && <PdaForm />}
        {tab === "create" && <CreateForm />}
        {tab === "approve" && <ApproveForm />}
        {tab === "propose" && <ProposeForm />}
        {tab === "addMember" && <AddMemberForm />}
        {tab === "removeMember" && <RemoveMemberForm />}
        {tab === "changeThreshold" && <ChangeThresholdForm />}
      </div>

      {/* Reference */}
      <details className="reference">
        <summary>Squads v4 Program Reference</summary>
        <table>
          <thead>
            <tr>
              <th>Instruction</th>
              <th>Discriminator (global:…)</th>
            </tr>
          </thead>
          <tbody>
            {[
              "multisig_create_v2",
              "vault_transaction_create",
              "proposal_create",
              "proposal_approve",
              "proposal_reject",
              "vault_transaction_execute",
              "config_transaction_create",
            ].map((name) => (
              <tr key={name}>
                <td><code>{name}</code></td>
                <td><code>{sha256Disc(name)}</code></td>
              </tr>
            ))}
          </tbody>
        </table>
        <p>
          Program ID:{" "}
          <a
            href={`https://explorer.solana.com/address/${SQUADS_V4_PROGRAM}`}
            target="_blank"
            rel="noreferrer"
          >
            {SQUADS_V4_PROGRAM}
          </a>
        </p>
      </details>
    </div>
  );
}