import * as SecureStore from "expo-secure-store";
import React, { useCallback, useEffect, useState } from "react";
import {
  Alert,
  ScrollView,
  StyleSheet,
  Text,
  TouchableOpacity,
  View,
} from "react-native";

// ── Types ─────────────────────────────────────────────────────────────────────

export interface SigningLogEntry {
  id: string;           // uuid-like: timestamp + random suffix
  signedAt: string;     // ISO timestamp
  keyLabel: string;     // label of the key used
  keyPubkey: string;    // pubkey base58 of the key used
  feePayer: string;     // from tx inspection
  feeLamports: number;
  riskLevel: "safe" | "warn" | "critical";
  instructionCount: number;
  instructionNames: string[]; // first few instruction names
  signatureBase58: string;    // the resulting signature
}

export const SIGNING_LOG_KEY = "airsign_signing_log_v1";
const MAX_LOG_ENTRIES = 200;

export async function appendSigningLog(entry: SigningLogEntry): Promise<void> {
  try {
    const raw = await SecureStore.getItemAsync(SIGNING_LOG_KEY);
    const existing: SigningLogEntry[] = raw ? (JSON.parse(raw) as SigningLogEntry[]) : [];
    // Prepend new entry, keep most recent MAX_LOG_ENTRIES
    const updated = [entry, ...existing].slice(0, MAX_LOG_ENTRIES);
    await SecureStore.setItemAsync(SIGNING_LOG_KEY, JSON.stringify(updated));
  } catch (err) {
    console.warn("[SigningLog] failed to append entry", err);
  }
}

async function loadSigningLog(): Promise<SigningLogEntry[]> {
  try {
    const raw = await SecureStore.getItemAsync(SIGNING_LOG_KEY);
    if (!raw) return [];
    return JSON.parse(raw) as SigningLogEntry[];
  } catch {
    return [];
  }
}

async function clearSigningLog(): Promise<void> {
  await SecureStore.deleteItemAsync(SIGNING_LOG_KEY);
}

// ── Risk badge ────────────────────────────────────────────────────────────────

function RiskBadge({ level }: { level: SigningLogEntry["riskLevel"] }) {
  const map = {
    safe: { bg: "#052e16", text: "#22c55e", label: "SAFE" },
    warn: { bg: "#422006", text: "#f59e0b", label: "WARN" },
    critical: { bg: "#450a0a", text: "#f87171", label: "CRITICAL" },
  };
  const c = map[level];
  return (
    <View style={[styles.badge, { backgroundColor: c.bg }]}>
      <Text style={[styles.badgeText, { color: c.text }]}>{c.label}</Text>
    </View>
  );
}

// ── Log entry card ────────────────────────────────────────────────────────────

function LogCard({ entry }: { entry: SigningLogEntry }) {
  const date = new Date(entry.signedAt);
  const dateStr = date.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
  const timeStr = date.toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
  });

  const instructionSummary =
    entry.instructionNames.length > 0
      ? entry.instructionNames.slice(0, 3).join(", ") +
        (entry.instructionNames.length > 3
          ? ` +${entry.instructionNames.length - 3} more`
          : "")
      : `${entry.instructionCount} instruction${entry.instructionCount !== 1 ? "s" : ""}`;

  return (
    <View style={styles.card}>
      <View style={styles.cardHeader}>
        <View style={styles.cardHeaderLeft}>
          <Text style={styles.cardDate}>
            {dateStr} · {timeStr}
          </Text>
          <RiskBadge level={entry.riskLevel} />
        </View>
        <Text style={styles.cardFee}>
          ~{(entry.feeLamports / 1e9).toFixed(6)} SOL fee
        </Text>
      </View>

      <View style={styles.cardRow}>
        <Text style={styles.cardRowLabel}>Key</Text>
        <Text style={styles.cardRowValue} numberOfLines={1}>
          {entry.keyLabel}
        </Text>
      </View>

      <View style={styles.cardRow}>
        <Text style={styles.cardRowLabel}>Fee Payer</Text>
        <Text
          style={[styles.cardRowValue, styles.mono]}
          numberOfLines={1}
          ellipsizeMode="middle"
        >
          {entry.feePayer}
        </Text>
      </View>

      <View style={styles.cardRow}>
        <Text style={styles.cardRowLabel}>Instructions</Text>
        <Text style={styles.cardRowValue} numberOfLines={1}>
          {instructionSummary}
        </Text>
      </View>

      <View style={styles.cardRow}>
        <Text style={styles.cardRowLabel}>Signature</Text>
        <Text
          style={[styles.cardRowValue, styles.mono, styles.dimValue]}
          numberOfLines={1}
          ellipsizeMode="middle"
        >
          {entry.signatureBase58}
        </Text>
      </View>
    </View>
  );
}

// ── Main Screen ───────────────────────────────────────────────────────────────

export default function HistoryScreen() {
  const [entries, setEntries] = useState<SigningLogEntry[]>([]);
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    setLoading(true);
    const log = await loadSigningLog();
    setEntries(log);
    setLoading(false);
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const handleClear = useCallback(() => {
    Alert.alert(
      "Clear History",
      "Delete all signing history? This only removes the log — keys are unaffected.",
      [
        { text: "Cancel", style: "cancel" },
        {
          text: "Clear",
          style: "destructive",
          onPress: async () => {
            await clearSigningLog();
            setEntries([]);
          },
        },
      ]
    );
  }, []);

  return (
    <ScrollView style={styles.container} contentContainerStyle={styles.content}>
      <View style={styles.titleRow}>
        <Text style={styles.sectionLabel}>SIGNING HISTORY</Text>
        {entries.length > 0 && (
          <TouchableOpacity onPress={handleClear}>
            <Text style={styles.clearBtn}>Clear All</Text>
          </TouchableOpacity>
        )}
      </View>

      {loading ? (
        <Text style={styles.dimText}>Loading…</Text>
      ) : entries.length === 0 ? (
        <View style={styles.emptyBox}>
          <Text style={styles.emptyIcon}>📋</Text>
          <Text style={styles.emptyTitle}>No signing history</Text>
          <Text style={styles.emptyDesc}>
            Every transaction you sign will appear here as an audit trail.
          </Text>
        </View>
      ) : (
        <>
          <Text style={styles.countText}>
            {entries.length} signing event{entries.length !== 1 ? "s" : ""}
          </Text>
          {entries.map((e) => (
            <LogCard key={e.id} entry={e} />
          ))}
        </>
      )}

      <View style={styles.infoBox}>
        <Text style={styles.infoTitle}>ℹ️ About this log</Text>
        <Text style={styles.infoText}>
          This log is stored locally in the device's Secure Enclave. It is
          never transmitted. Maximum {MAX_LOG_ENTRIES} entries are retained.
        </Text>
      </View>
    </ScrollView>
  );
}

// ── Styles ────────────────────────────────────────────────────────────────────

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: "#0a0a0a" },
  content: { padding: 20, paddingBottom: 40 },
  titleRow: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "center",
    marginBottom: 12,
  },
  sectionLabel: {
    color: "#4b5563",
    fontSize: 11,
    fontWeight: "700",
    letterSpacing: 1.2,
  },
  clearBtn: {
    color: "#ef4444",
    fontSize: 13,
    fontWeight: "600",
  },
  dimText: { color: "#6b7280", fontSize: 14 },
  countText: {
    color: "#4b5563",
    fontSize: 12,
    marginBottom: 12,
  },
  emptyBox: {
    backgroundColor: "#111827",
    borderRadius: 12,
    padding: 32,
    alignItems: "center",
    gap: 8,
    marginBottom: 20,
  },
  emptyIcon: { fontSize: 40, marginBottom: 8 },
  emptyTitle: { color: "#e5e7eb", fontSize: 16, fontWeight: "600" },
  emptyDesc: {
    color: "#6b7280",
    fontSize: 13,
    textAlign: "center",
    lineHeight: 20,
  },
  card: {
    backgroundColor: "#111827",
    borderRadius: 10,
    padding: 14,
    marginBottom: 10,
    gap: 8,
  },
  cardHeader: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "flex-start",
    marginBottom: 4,
  },
  cardHeaderLeft: {
    flexDirection: "row",
    alignItems: "center",
    gap: 8,
  },
  cardDate: {
    color: "#9ca3af",
    fontSize: 12,
  },
  cardFee: {
    color: "#6b7280",
    fontSize: 11,
  },
  badge: {
    borderRadius: 4,
    paddingHorizontal: 6,
    paddingVertical: 2,
  },
  badgeText: {
    fontSize: 9,
    fontWeight: "700",
    letterSpacing: 0.5,
  },
  cardRow: {
    flexDirection: "row",
    gap: 8,
  },
  cardRowLabel: {
    color: "#4b5563",
    fontSize: 11,
    width: 80,
    flexShrink: 0,
  },
  cardRowValue: {
    color: "#d1d5db",
    fontSize: 11,
    flex: 1,
  },
  mono: { fontFamily: "monospace" },
  dimValue: { color: "#4b5563" },
  infoBox: {
    backgroundColor: "#0f172a",
    borderRadius: 10,
    padding: 14,
    marginTop: 8,
    borderWidth: 1,
    borderColor: "#1e3a5f",
  },
  infoTitle: {
    color: "#60a5fa",
    fontSize: 12,
    fontWeight: "700",
    marginBottom: 6,
  },
  infoText: {
    color: "#475569",
    fontSize: 11,
    lineHeight: 18,
  },
});