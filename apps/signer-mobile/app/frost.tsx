/**
 * app/frost.tsx — Mobile FROST threshold-signing UI
 *
 * Three tabs:
 *   Dealer   — generate DKG key-shares (t-of-n), display each share as QR
 *   Participant — receive share QR, store it, produce a signing commitment
 *   Sign     — aggregate commitments + partial sigs into a final Ed25519 sig
 *
 * All heavy crypto is delegated to AirSignCore (afterimage-wasm via bridge).
 * This screen is UI / UX only — no raw key material is logged or displayed
 * beyond what the user explicitly requests.
 */

import { useRouter } from "expo-router";
import * as SecureStore from "expo-secure-store";
import React, { useCallback, useState } from "react";
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

// ── Types ─────────────────────────────────────────────────────────────────────

type Tab = "dealer" | "participant" | "sign";

interface ShareEntry {
  participantIndex: number;
  shareB64: string; // serialised FROST key-share (base64)
  groupPubkeyB64: string;
}

const FROST_SHARES_KEY = "airsign_frost_shares_v1";

async function loadShares(): Promise<ShareEntry[]> {
  try {
    const raw = await SecureStore.getItemAsync(FROST_SHARES_KEY);
    if (!raw) return [];
    return JSON.parse(raw) as ShareEntry[];
  } catch {
    return [];
  }
}

async function saveShares(shares: ShareEntry[]): Promise<void> {
  await SecureStore.setItemAsync(FROST_SHARES_KEY, JSON.stringify(shares));
}

// ── Shared tab bar ────────────────────────────────────────────────────────────

function TabBar({ active, onChange }: { active: Tab; onChange: (t: Tab) => void }) {
  const tabs: { key: Tab; label: string }[] = [
    { key: "dealer", label: "Dealer" },
    { key: "participant", label: "Participant" },
    { key: "sign", label: "Sign" },
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

// ── Dealer tab ────────────────────────────────────────────────────────────────

function DealerTab() {
  const [threshold, setThreshold] = useState("2");
  const [total, setTotal] = useState("3");
  const [busy, setBusy] = useState(false);
  const [shares, setShares] = useState<string[]>([]); // base64 per share

  const handleGenerate = useCallback(async () => {
    const t = parseInt(threshold, 10);
    const n = parseInt(total, 10);
    if (isNaN(t) || isNaN(n) || t < 2 || n < t || n > 10) {
      Alert.alert("Invalid parameters", "Need 2 ≤ threshold ≤ total ≤ 10");
      return;
    }
    setBusy(true);
    try {
      // AirSignCore.frostDealerSetup returns { shares: string[], groupPubkeyB64: string }
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const result = await (AirSignCore as any).frostDealerSetup(t, n);
      const res = result as { shares?: string[]; groupPubkeyB64?: string };
      const generatedShares: string[] = res.shares ?? [];
      const groupPub = res.groupPubkeyB64 ?? "";

      // Persist all shares locally (this device acts as dealer)
      const entries: ShareEntry[] = generatedShares.map((s, i) => ({
        participantIndex: i + 1,
        shareB64: s,
        groupPubkeyB64: groupPub,
      }));
      const existing = await loadShares();
      await saveShares([...existing, ...entries]);

      setShares(generatedShares);
    } catch (err) {
      Alert.alert("DKG Error", String(err));
    } finally {
      setBusy(false);
    }
  }, [threshold, total]);

  return (
    <ScrollView style={styles.tabContent} contentContainerStyle={{ padding: 16 }}>
      <Text style={styles.sectionTitle}>Generate Key Shares</Text>
      <Text style={styles.body}>
        As Dealer, you split a new group key into {total} shares using
        Shamir secret-sharing. Each participant receives one share — this device
        stores all shares for distribution.
      </Text>

      <View style={styles.inputRow}>
        <View style={styles.inputHalf}>
          <Text style={styles.label}>Threshold (t)</Text>
          <TextInput
            style={styles.input}
            value={threshold}
            onChangeText={setThreshold}
            keyboardType="number-pad"
            placeholderTextColor="#4b5563"
            placeholder="2"
          />
        </View>
        <View style={styles.inputHalf}>
          <Text style={styles.label}>Participants (n)</Text>
          <TextInput
            style={styles.input}
            value={total}
            onChangeText={setTotal}
            keyboardType="number-pad"
            placeholderTextColor="#4b5563"
            placeholder="3"
          />
        </View>
      </View>

      <Text style={styles.hint}>
        Any {threshold}-of-{total} participants can jointly sign.
      </Text>

      <TouchableOpacity
        style={[styles.primaryBtn, busy && styles.primaryBtnDisabled]}
        onPress={() => void handleGenerate()}
        disabled={busy}
      >
        {busy ? (
          <ActivityIndicator color="#fff" />
        ) : (
          <Text style={styles.primaryBtnText}>Generate Shares</Text>
        )}
      </TouchableOpacity>

      {shares.length > 0 && (
        <View style={{ marginTop: 20 }}>
          <Text style={styles.sectionTitle}>Generated {shares.length} shares</Text>
          {shares.map((s, i) => (
            <View key={i} style={styles.shareCard}>
              <Text style={styles.shareLabel}>Share {i + 1}</Text>
              <Text style={styles.shareData} numberOfLines={3} ellipsizeMode="middle">
                {s}
              </Text>
              <Text style={styles.hint}>
                Distribute this share to participant {i + 1} via secure channel (QR or encrypted file).
              </Text>
            </View>
          ))}
        </View>
      )}
    </ScrollView>
  );
}

// ── Participant tab ────────────────────────────────────────────────────────────

function ParticipantTab() {
  const [shareInput, setShareInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [commitment, setCommitment] = useState("");

  const handleImportShare = useCallback(async () => {
    const trimmed = shareInput.trim();
    if (!trimmed) {
      Alert.alert("No share entered", "Paste your key share from the dealer.");
      return;
    }
    setBusy(true);
    try {
      // Parse share JSON { participantIndex, shareB64, groupPubkeyB64 }
      const entry = JSON.parse(trimmed) as ShareEntry;
      const existing = await loadShares();
      // Avoid duplicates
      const deduped = existing.filter((e) => e.participantIndex !== entry.participantIndex);
      await saveShares([...deduped, entry]);
      Alert.alert("Share imported", `Participant ${entry.participantIndex} share stored securely.`);
      setShareInput("");
    } catch {
      Alert.alert("Invalid share", "Expected JSON with participantIndex, shareB64, groupPubkeyB64.");
    } finally {
      setBusy(false);
    }
  }, [shareInput]);

  const handleCommit = useCallback(async () => {
    setBusy(true);
    try {
      const shares = await loadShares();
      if (shares.length === 0) {
        Alert.alert("No shares", "Import your key share first.");
        return;
      }
      const share = shares[0]; // use first available share on this device
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const result = await (AirSignCore as any).frostCommit(share.shareB64);
      const res = result as { commitmentB64?: string };
      setCommitment(res.commitmentB64 ?? "");
    } catch (err) {
      Alert.alert("Commit Error", String(err));
    } finally {
      setBusy(false);
    }
  }, []);

  return (
    <ScrollView style={styles.tabContent} contentContainerStyle={{ padding: 16 }}>
      <Text style={styles.sectionTitle}>Import Key Share</Text>
      <Text style={styles.body}>
        Paste the JSON share you received from the Dealer, or scan it from a QR code.
      </Text>

      <TextInput
        style={[styles.input, { height: 100, textAlignVertical: "top" }]}
        value={shareInput}
        onChangeText={setShareInput}
        multiline
        placeholder='{"participantIndex":1,"shareB64":"...","groupPubkeyB64":"..."}'
        placeholderTextColor="#4b5563"
      />

      <TouchableOpacity
        style={[styles.primaryBtn, busy && styles.primaryBtnDisabled]}
        onPress={() => void handleImportShare()}
        disabled={busy}
      >
        {busy ? <ActivityIndicator color="#fff" /> : <Text style={styles.primaryBtnText}>Import Share</Text>}
      </TouchableOpacity>

      <View style={styles.divider} />

      <Text style={styles.sectionTitle}>Generate Signing Commitment</Text>
      <Text style={styles.body}>
        Before signing, each participant produces a nonce commitment. Share this
        with the aggregator (Dealer) out of band.
      </Text>

      <TouchableOpacity
        style={[styles.primaryBtn, busy && styles.primaryBtnDisabled]}
        onPress={() => void handleCommit()}
        disabled={busy}
      >
        {busy ? <ActivityIndicator color="#fff" /> : <Text style={styles.primaryBtnText}>Generate Commitment</Text>}
      </TouchableOpacity>

      {commitment.length > 0 && (
        <View style={styles.shareCard}>
          <Text style={styles.shareLabel}>Your Commitment</Text>
          <Text style={styles.shareData} numberOfLines={4} ellipsizeMode="middle">
            {commitment}
          </Text>
          <Text style={styles.hint}>Send this to the aggregator before signing.</Text>
        </View>
      )}
    </ScrollView>
  );
}

// ── Sign tab ──────────────────────────────────────────────────────────────────

function SignTab() {
  const router = useRouter();
  const [msgB64, setMsgB64] = useState("");
  const [commitmentsJson, setCommitmentsJson] = useState("");
  const [busy, setBusy] = useState(false);
  const [partialSig, setPartialSig] = useState("");
  const [finalSig, setFinalSig] = useState("");

  const handlePartialSign = useCallback(async () => {
    if (!msgB64.trim()) {
      Alert.alert("No message", "Enter the base64-encoded message to sign.");
      return;
    }
    setBusy(true);
    try {
      const shares = await loadShares();
      if (shares.length === 0) {
        Alert.alert("No shares", "Import a key share in the Participant tab first.");
        return;
      }
      const share = shares[0];
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const result = await (AirSignCore as any).frostPartialSign(
        share.shareB64,
        msgB64.trim(),
        commitmentsJson.trim() || "[]"
      );
      const res = result as { partialSigB64?: string };
      setPartialSig(res.partialSigB64 ?? "");
    } catch (err) {
      Alert.alert("Partial Sign Error", String(err));
    } finally {
      setBusy(false);
    }
  }, [msgB64, commitmentsJson]);

  const handleAggregate = useCallback(async () => {
    if (!partialSig) {
      Alert.alert("No partial sig", "Produce a partial signature first.");
      return;
    }
    setBusy(true);
    try {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const result = await (AirSignCore as any).frostAggregate(
        commitmentsJson.trim() || "[]",
        `[${partialSig}]`,
        msgB64.trim()
      );
      const res = result as { signatureB64?: string };
      setFinalSig(res.signatureB64 ?? "");
    } catch (err) {
      Alert.alert("Aggregation Error", String(err));
    } finally {
      setBusy(false);
    }
  }, [partialSig, commitmentsJson, msgB64]);

  const handleDisplaySig = useCallback(async () => {
    if (!finalSig) return;
    const encResult = await AirSignCore.fountainEncode(finalSig, 16);
    const enc = encResult as { frames?: string[] };
    const frames = enc.frames ?? [finalSig];
    router.push({
      pathname: "/display",
      params: { framesJson: JSON.stringify(frames) },
    });
  }, [finalSig, router]);

  return (
    <ScrollView style={styles.tabContent} contentContainerStyle={{ padding: 16 }}>
      <Text style={styles.sectionTitle}>FROST Signing</Text>
      <Text style={styles.body}>
        Paste the base64 transaction message and the JSON array of participant
        commitments, then produce your partial signature.
      </Text>

      <Text style={styles.label}>Message (base64)</Text>
      <TextInput
        style={[styles.input, { height: 80, textAlignVertical: "top" }]}
        value={msgB64}
        onChangeText={setMsgB64}
        multiline
        placeholder="base64-encoded transaction bytes"
        placeholderTextColor="#4b5563"
      />

      <Text style={styles.label}>Commitments JSON array</Text>
      <TextInput
        style={[styles.input, { height: 80, textAlignVertical: "top" }]}
        value={commitmentsJson}
        onChangeText={setCommitmentsJson}
        multiline
        placeholder='[{"index":1,"commitmentB64":"..."},...]'
        placeholderTextColor="#4b5563"
      />

      <TouchableOpacity
        style={[styles.primaryBtn, busy && styles.primaryBtnDisabled]}
        onPress={() => void handlePartialSign()}
        disabled={busy}
      >
        {busy ? <ActivityIndicator color="#fff" /> : <Text style={styles.primaryBtnText}>Produce Partial Signature</Text>}
      </TouchableOpacity>

      {partialSig.length > 0 && (
        <>
          <View style={styles.shareCard}>
            <Text style={styles.shareLabel}>Partial Signature</Text>
            <Text style={styles.shareData} numberOfLines={3} ellipsizeMode="middle">
              {partialSig}
            </Text>
            <Text style={styles.hint}>Share this with the aggregator.</Text>
          </View>

          <TouchableOpacity
            style={[styles.secondaryBtn, busy && styles.primaryBtnDisabled]}
            onPress={() => void handleAggregate()}
            disabled={busy}
          >
            {busy ? <ActivityIndicator color="#93c5fd" /> : <Text style={styles.secondaryBtnText}>Aggregate → Final Signature</Text>}
          </TouchableOpacity>
        </>
      )}

      {finalSig.length > 0 && (
        <>
          <View style={[styles.shareCard, { borderColor: "#22c55e33" }]}>
            <Text style={[styles.shareLabel, { color: "#22c55e" }]}>✓ Final Signature</Text>
            <Text style={styles.shareData} numberOfLines={3} ellipsizeMode="middle">
              {finalSig}
            </Text>
          </View>
          <TouchableOpacity style={styles.primaryBtn} onPress={() => void handleDisplaySig()}>
            <Text style={styles.primaryBtnText}>Display as QR →</Text>
          </TouchableOpacity>
        </>
      )}
    </ScrollView>
  );
}

// ── Root screen ───────────────────────────────────────────────────────────────

export default function FrostScreen() {
  const [tab, setTab] = useState<Tab>("dealer");

  return (
    <View style={styles.container}>
      <TabBar active={tab} onChange={setTab} />
      {tab === "dealer" && <DealerTab />}
      {tab === "participant" && <ParticipantTab />}
      {tab === "sign" && <SignTab />}
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
    borderBottomColor: "#3b82f6",
  },
  tabBtnText: { color: "#6b7280", fontSize: 14, fontWeight: "600" },
  tabBtnTextActive: { color: "#60a5fa" },
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
  hint: { color: "#4b5563", fontSize: 11, marginTop: 6 },
  inputRow: { flexDirection: "row", gap: 12 },
  inputHalf: { flex: 1 },
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
  primaryBtn: {
    backgroundColor: "#1d4ed8",
    borderRadius: 10,
    paddingVertical: 14,
    alignItems: "center",
    marginTop: 16,
  },
  primaryBtnDisabled: { opacity: 0.5 },
  primaryBtnText: { color: "#fff", fontSize: 15, fontWeight: "700" },
  secondaryBtn: {
    backgroundColor: "#1e3a5f",
    borderRadius: 10,
    paddingVertical: 14,
    alignItems: "center",
    marginTop: 12,
    borderWidth: 1,
    borderColor: "#3b82f6",
  },
  secondaryBtnText: { color: "#93c5fd", fontSize: 15, fontWeight: "600" },
  divider: {
    height: StyleSheet.hairlineWidth,
    backgroundColor: "#1f2937",
    marginVertical: 20,
  },
  shareCard: {
    backgroundColor: "#111827",
    borderRadius: 10,
    padding: 14,
    marginTop: 16,
    borderWidth: 1,
    borderColor: "#1f2937",
  },
  shareLabel: {
    color: "#9ca3af",
    fontSize: 11,
    fontWeight: "700",
    textTransform: "uppercase",
    letterSpacing: 0.8,
    marginBottom: 6,
  },
  shareData: {
    color: "#e5e7eb",
    fontSize: 11,
    fontFamily: "monospace",
    lineHeight: 18,
  },
});