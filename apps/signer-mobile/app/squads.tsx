/**
 * app/squads.tsx — Mobile Squads v4 multisig UI
 *
 * Three tabs:
 *   PDAs     — derive and display Squads multisig + vault PDAs
 *   Propose  — build a vault-transaction proposal and encode as QR
 *   Approve  — scan an approval-QR, co-sign, and display result QR
 *
 * Squads v4 program ID: SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf
 * All PDA derivation & instruction assembly is delegated to AirSignCore /
 * afterimage-squads crate via the WASM bridge.
 */

import { useRouter } from "expo-router";
import * as SecureStore from "expo-secure-store";
import React, { useCallback, useEffect, useState } from "react";
import {
  ActivityIndicator,
  Alert,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  TouchableOpacity,
  View,
} from "react-native";
import AirSignCore from "../src/native/AirSignCore";

// ── Constants ─────────────────────────────────────────────────────────────────

const SQUADS_PROGRAM_ID = "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf";
const SQUADS_CONFIG_KEY = "airsign_squads_config_v1";

// ── Types ─────────────────────────────────────────────────────────────────────

type Tab = "pdas" | "propose" | "approve";

interface SquadsConfig {
  multisigPda: string;
  vaultPda: string;
  createKey: string;
}

async function loadSquadsConfig(): Promise<SquadsConfig | null> {
  try {
    const raw = await SecureStore.getItemAsync(SQUADS_CONFIG_KEY);
    if (!raw) return null;
    return JSON.parse(raw) as SquadsConfig;
  } catch {
    return null;
  }
}

async function saveSquadsConfig(cfg: SquadsConfig): Promise<void> {
  await SecureStore.setItemAsync(SQUADS_CONFIG_KEY, JSON.stringify(cfg));
}

// ── Shared tab bar ────────────────────────────────────────────────────────────

function TabBar({ active, onChange }: { active: Tab; onChange: (t: Tab) => void }) {
  const tabs: { key: Tab; label: string }[] = [
    { key: "pdas", label: "PDAs" },
    { key: "propose", label: "Propose" },
    { key: "approve", label: "Approve" },
  ];
  return (
    <View style={styles.tabBar}>
      {tabs.map((t) => (
        <TouchableOpacity
          key={t.key}
          style={[styles.tabBtn, active === t.key && styles.tabBtnActive]}
          onPress={() => onChange(t.key)}
        >
          <Text style={[styles.tabBtnText, active === t.key && styles.tabBtnTextActive]}>
            {t.label}
          </Text>
        </TouchableOpacity>
      ))}
    </View>
  );
}

// ── PDA row display ───────────────────────────────────────────────────────────

function PdaRow({ label, value }: { label: string; value: string }) {
  return (
    <View style={styles.pdaRow}>
      <Text style={styles.pdaLabel}>{label}</Text>
      <Text style={styles.pdaValue} selectable numberOfLines={1} ellipsizeMode="middle">
        {value || "—"}
      </Text>
    </View>
  );
}

// ── PDAs tab ──────────────────────────────────────────────────────────────────

function PdasTab() {
  const [createKey, setCreateKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [config, setConfig] = useState<SquadsConfig | null>(null);

  useEffect(() => {
    void loadSquadsConfig().then((c) => {
      if (c) setConfig(c);
    });
  }, []);

  const handleDerive = useCallback(async () => {
    const key = createKey.trim();
    if (!key) {
      Alert.alert("No create key", "Enter the base58 create key (any unique pubkey).");
      return;
    }
    setBusy(true);
    try {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const result = await (AirSignCore as any).squadsDerivePdas(key, SQUADS_PROGRAM_ID);
      const res = result as { multisigPda?: string; vaultPda?: string };
      const cfg: SquadsConfig = {
        multisigPda: res.multisigPda ?? "",
        vaultPda: res.vaultPda ?? "",
        createKey: key,
      };
      await saveSquadsConfig(cfg);
      setConfig(cfg);
    } catch (err) {
      Alert.alert("PDA derivation error", String(err));
    } finally {
      setBusy(false);
    }
  }, [createKey]);

  return (
    <ScrollView style={styles.tabContent} contentContainerStyle={{ padding: 16 }}>
      <Text style={styles.sectionTitle}>Squads Multisig PDAs</Text>
      <Text style={styles.body}>
        Enter the "create key" — a unique public key that seeds the multisig
        PDA. This is deterministic: the same create key always produces the
        same multisig and vault addresses.
      </Text>

      <Text style={styles.label}>Create Key (base58 pubkey)</Text>
      <TextInput
        style={styles.input}
        value={createKey}
        onChangeText={setCreateKey}
        autoCapitalize="none"
        autoCorrect={false}
        placeholder="Base58 public key…"
        placeholderTextColor="#4b5563"
      />

      <TouchableOpacity
        style={[styles.primaryBtn, busy && styles.primaryBtnDisabled]}
        onPress={() => void handleDerive()}
        disabled={busy}
      >
        {busy ? (
          <ActivityIndicator color="#fff" />
        ) : (
          <Text style={styles.primaryBtnText}>Derive PDAs</Text>
        )}
      </TouchableOpacity>

      {config && (
        <View style={{ marginTop: 20 }}>
          <Text style={styles.sectionTitle}>Derived Addresses</Text>
          <View style={styles.card}>
            <PdaRow label="Create Key" value={config.createKey} />
            <View style={styles.divider} />
            <PdaRow label="Multisig PDA" value={config.multisigPda} />
            <View style={styles.divider} />
            <PdaRow label="Vault PDA (index 0)" value={config.vaultPda} />
          </View>
          <Text style={styles.hint}>
            Program: {SQUADS_PROGRAM_ID.slice(0, 16)}…
          </Text>
        </View>
      )}
    </ScrollView>
  );
}

// ── Propose tab ───────────────────────────────────────────────────────────────

function ProposeTab() {
  const router = useRouter();
  const [busy, setBusy] = useState(false);
  const [recipient, setRecipient] = useState("");
  const [lamports, setLamports] = useState("");
  const [config, setConfig] = useState<SquadsConfig | null>(null);
  const [proposerKeyId, setProposerKeyId] = useState("");
  const [keyIds, setKeyIds] = useState<string[]>([]);

  useEffect(() => {
    void (async () => {
      const cfg = await loadSquadsConfig();
      if (cfg) setConfig(cfg);
      const ids: string[] = await AirSignCore.listKeypairIds();
      setKeyIds(ids);
      if (ids.length > 0) setProposerKeyId(ids[0]);
    })();
  }, []);

  const handlePropose = useCallback(async () => {
    if (!config) {
      Alert.alert("No PDAs", "Derive multisig PDAs in the PDAs tab first.");
      return;
    }
    if (!recipient.trim() || !lamports.trim()) {
      Alert.alert("Missing fields", "Fill in recipient and amount.");
      return;
    }
    if (!proposerKeyId) {
      Alert.alert("No key", "Generate a key in Key Management first.");
      return;
    }
    const lamportsNum = parseInt(lamports, 10);
    if (isNaN(lamportsNum) || lamportsNum <= 0) {
      Alert.alert("Invalid amount", "Enter a positive lamport amount.");
      return;
    }
    setBusy(true);
    try {
      // Build the vault-transaction proposal instruction bytes
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const result = await (AirSignCore as any).squadsProposalBuild(
        config.multisigPda,
        config.vaultPda,
        recipient.trim(),
        lamportsNum,
        SQUADS_PROGRAM_ID
      );
      const res = result as { txBase64?: string };
      const txBase64 = res.txBase64 ?? "";

      // Sign the proposal transaction with the proposer's key
      const signResult = await AirSignCore.signTransaction(proposerKeyId, txBase64);
      const signed = signResult as { signedTxBase64?: string };
      const signedTx = signed.signedTxBase64 ?? txBase64;

      // Fountain-encode
      const encResult = await AirSignCore.fountainEncode(signedTx, 16);
      const enc = encResult as { frames?: string[] };
      const frames = enc.frames ?? [signedTx];

      router.push({
        pathname: "/display",
        params: { framesJson: JSON.stringify(frames) },
      });
    } catch (err) {
      Alert.alert("Proposal error", String(err));
    } finally {
      setBusy(false);
    }
  }, [config, recipient, lamports, proposerKeyId, router]);

  return (
    <ScrollView style={styles.tabContent} contentContainerStyle={{ padding: 16 }}>
      <Text style={styles.sectionTitle}>Propose Vault Transaction</Text>
      <Text style={styles.body}>
        Build a SOL transfer from the Squads vault. The signed proposal will be
        displayed as a QR code for the online machine to submit.
      </Text>

      {!config && (
        <View style={styles.warnCard}>
          <Text style={styles.warnText}>
            ⚠️  No multisig configured. Go to the PDAs tab first.
          </Text>
        </View>
      )}

      {config && (
        <View style={styles.card}>
          <PdaRow label="Multisig" value={config.multisigPda} />
          <View style={styles.divider} />
          <PdaRow label="Vault" value={config.vaultPda} />
        </View>
      )}

      <Text style={styles.label}>Recipient (base58)</Text>
      <TextInput
        style={styles.input}
        value={recipient}
        onChangeText={setRecipient}
        autoCapitalize="none"
        autoCorrect={false}
        placeholder="Recipient pubkey…"
        placeholderTextColor="#4b5563"
      />

      <Text style={styles.label}>Amount (lamports)</Text>
      <TextInput
        style={styles.input}
        value={lamports}
        onChangeText={setLamports}
        keyboardType="number-pad"
        placeholder="e.g. 1000000 = 0.001 SOL"
        placeholderTextColor="#4b5563"
      />

      {keyIds.length > 0 && (
        <>
          <Text style={styles.label}>Proposer key</Text>
          <ScrollView horizontal showsHorizontalScrollIndicator={false}>
            <View style={{ flexDirection: "row", gap: 8, marginBottom: 4 }}>
              {keyIds.map((id) => (
                <TouchableOpacity
                  key={id}
                  style={[
                    styles.keyChip,
                    proposerKeyId === id && styles.keyChipActive,
                  ]}
                  onPress={() => setProposerKeyId(id)}
                >
                  <Text style={[styles.keyChipText, proposerKeyId === id && styles.keyChipTextActive]}>
                    {id.slice(0, 8)}…
                  </Text>
                </TouchableOpacity>
              ))}
            </View>
          </ScrollView>
        </>
      )}

      <TouchableOpacity
        style={[styles.primaryBtn, (busy || !config) && styles.primaryBtnDisabled]}
        onPress={() => void handlePropose()}
        disabled={busy || !config}
      >
        {busy ? (
          <ActivityIndicator color="#fff" />
        ) : (
          <Text style={styles.primaryBtnText}>Build & Sign Proposal →</Text>
        )}
      </TouchableOpacity>
    </ScrollView>
  );
}

// ── Approve tab ───────────────────────────────────────────────────────────────

function ApproveTab() {
  const router = useRouter();
  const [busy, setBusy] = useState(false);
  const [txB64Input, setTxB64Input] = useState("");
  const [approverKeyId, setApproverKeyId] = useState("");
  const [keyIds, setKeyIds] = useState<string[]>([]);

  useEffect(() => {
    void AirSignCore.listKeypairIds().then((ids) => {
      const idList = ids as string[];
      setKeyIds(idList);
      if (idList.length > 0) setApproverKeyId(idList[0]);
    });
  }, []);

  const handleApprove = useCallback(async () => {
    if (!txB64Input.trim()) {
      Alert.alert("No transaction", "Paste the base64 proposal transaction to approve.");
      return;
    }
    if (!approverKeyId) {
      Alert.alert("No key", "Select a signing key.");
      return;
    }
    setBusy(true);
    try {
      // Build approval instruction
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const config = await loadSquadsConfig();
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const result = await (AirSignCore as any).squadsApprovalBuild(
        config?.multisigPda ?? "",
        txB64Input.trim(),
        SQUADS_PROGRAM_ID
      );
      const res = result as { txBase64?: string };
      const approvalTx = res.txBase64 ?? txB64Input.trim();

      // Co-sign with approver key
      const signResult = await AirSignCore.signTransaction(approverKeyId, approvalTx);
      const signed = signResult as { signedTxBase64?: string };
      const signedTx = signed.signedTxBase64 ?? approvalTx;

      // Fountain-encode
      const encResult = await AirSignCore.fountainEncode(signedTx, 16);
      const enc = encResult as { frames?: string[] };
      const frames = enc.frames ?? [signedTx];

      router.push({
        pathname: "/display",
        params: { framesJson: JSON.stringify(frames) },
      });
    } catch (err) {
      Alert.alert("Approval error", String(err));
    } finally {
      setBusy(false);
    }
  }, [txB64Input, approverKeyId, router]);

  return (
    <ScrollView style={styles.tabContent} contentContainerStyle={{ padding: 16 }}>
      <Text style={styles.sectionTitle}>Approve Vault Transaction</Text>
      <Text style={styles.body}>
        Paste the base64 proposal transaction received from another member.
        Your approval signature will be encoded as a QR for submission.
      </Text>

      <Text style={styles.label}>Proposal transaction (base64)</Text>
      <TextInput
        style={[styles.input, { height: 100, textAlignVertical: "top" }]}
        value={txB64Input}
        onChangeText={setTxB64Input}
        multiline
        autoCapitalize="none"
        autoCorrect={false}
        placeholder="Paste base64-encoded proposal tx…"
        placeholderTextColor="#4b5563"
      />

      {keyIds.length > 0 && (
        <>
          <Text style={styles.label}>Approver key</Text>
          <ScrollView horizontal showsHorizontalScrollIndicator={false}>
            <View style={{ flexDirection: "row", gap: 8, marginBottom: 4 }}>
              {keyIds.map((id) => (
                <TouchableOpacity
                  key={id}
                  style={[
                    styles.keyChip,
                    approverKeyId === id && styles.keyChipActive,
                  ]}
                  onPress={() => setApproverKeyId(id)}
                >
                  <Text style={[styles.keyChipText, approverKeyId === id && styles.keyChipTextActive]}>
                    {id.slice(0, 8)}…
                  </Text>
                </TouchableOpacity>
              ))}
            </View>
          </ScrollView>
        </>
      )}

      <TouchableOpacity
        style={[styles.primaryBtn, busy && styles.primaryBtnDisabled]}
        onPress={() => void handleApprove()}
        disabled={busy}
      >
        {busy ? (
          <ActivityIndicator color="#fff" />
        ) : (
          <Text style={styles.primaryBtnText}>Sign Approval →</Text>
        )}
      </TouchableOpacity>

      <Text style={styles.hint}>
        The signed approval transaction will be displayed as an animated QR.
        Scan it with the online machine to submit to the Squads program.
      </Text>
    </ScrollView>
  );
}

// ── Root screen ───────────────────────────────────────────────────────────────

export default function SquadsScreen() {
  const [tab, setTab] = useState<Tab>("pdas");

  return (
    <View style={styles.container}>
      <TabBar active={tab} onChange={setTab} />
      {tab === "pdas" && <PdasTab />}
      {tab === "propose" && <ProposeTab />}
      {tab === "approve" && <ApproveTab />}
    </View>
  );
}

// ── Styles ────────────────────────────────────────────────────────────────────

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: "#0a0a0a" },
  tabBar: {
    flexDirection: "row",
    backgroundColor: "#111827",
    borderBottomWidth: StyleSheet.hairlineWidth,
    borderBottomColor: "#1f2937",
  },
  tabBtn: {
    flex: 1,
    paddingVertical: 14,
    alignItems: "center",
  },
  tabBtnActive: {
    borderBottomWidth: 2,
    borderBottomColor: "#8b5cf6",
  },
  tabBtnText: { color: "#6b7280", fontSize: 14, fontWeight: "600" },
  tabBtnTextActive: { color: "#a78bfa" },
  tabContent: { flex: 1 },
  sectionTitle: {
    color: "#f9fafb",
    fontSize: 16,
    fontWeight: "700",
    marginBottom: 8,
  },
  body: {
    color: "#9ca3af",
    fontSize: 13,
    lineHeight: 20,
    marginBottom: 16,
  },
  label: {
    color: "#9ca3af",
    fontSize: 12,
    fontWeight: "600",
    marginBottom: 6,
    marginTop: 12,
  },
  hint: { color: "#4b5563", fontSize: 11, marginTop: 8 },
  input: {
    backgroundColor: "#111827",
    borderRadius: 8,
    borderWidth: 1,
    borderColor: "#1f2937",
    color: "#e5e7eb",
    fontSize: 14,
    paddingHorizontal: 12,
    paddingVertical: 10,
  },
  card: {
    backgroundColor: "#111827",
    borderRadius: 10,
    marginTop: 12,
    borderWidth: 1,
    borderColor: "#1f2937",
  },
  pdaRow: {
    paddingHorizontal: 14,
    paddingVertical: 12,
  },
  pdaLabel: {
    color: "#6b7280",
    fontSize: 11,
    fontWeight: "600",
    textTransform: "uppercase",
    letterSpacing: 0.6,
    marginBottom: 4,
  },
  pdaValue: {
    color: "#e5e7eb",
    fontSize: 12,
    fontFamily: "monospace",
  },
  divider: {
    height: StyleSheet.hairlineWidth,
    backgroundColor: "#1f2937",
    marginHorizontal: 14,
  },
  primaryBtn: {
    backgroundColor: "#5b21b6",
    borderRadius: 10,
    paddingVertical: 14,
    alignItems: "center",
    marginTop: 16,
  },
  primaryBtnDisabled: { opacity: 0.5 },
  primaryBtnText: { color: "#fff", fontSize: 15, fontWeight: "700" },
  warnCard: {
    backgroundColor: "#431407",
    borderRadius: 10,
    padding: 14,
    marginBottom: 12,
    borderWidth: 1,
    borderColor: "#b45309",
  },
  warnText: { color: "#fbbf24", fontSize: 13 },
  keyChip: {
    paddingHorizontal: 12,
    paddingVertical: 8,
    borderRadius: 20,
    backgroundColor: "#1f2937",
    borderWidth: 1,
    borderColor: "#374151",
  },
  keyChipActive: {
    borderColor: "#8b5cf6",
    backgroundColor: "#2e1065",
  },
  keyChipText: { color: "#9ca3af", fontSize: 12, fontFamily: "monospace" },
  keyChipTextActive: { color: "#c4b5fd" },
});