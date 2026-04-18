import * as SecureStore from "expo-secure-store";
import { useRouter } from "expo-router";
import React, { useCallback, useEffect, useState } from "react";
import {
  Alert,
  ScrollView,
  StyleSheet,
  Text,
  TouchableOpacity,
  View,
} from "react-native";

const KEY_INDEX_STORE = "airsign_key_index";

interface KeyEntry {
  id: string;
  label: string;
  pubkey: string;
  createdAt: string;
}

async function loadKeyIndex(): Promise<KeyEntry[]> {
  const raw = await SecureStore.getItemAsync(KEY_INDEX_STORE);
  if (!raw) return [];
  try {
    return JSON.parse(raw) as KeyEntry[];
  } catch {
    return [];
  }
}

async function saveKeyIndex(entries: KeyEntry[]): Promise<void> {
  await SecureStore.setItemAsync(KEY_INDEX_STORE, JSON.stringify(entries));
}

/** Stub: generate a new Ed25519 keypair via the native module */
async function generateKeypair(): Promise<{ pubkey: string; id: string }> {
  // Real: const kp = await AirSignCore.generateKeypair();
  const mockPubkey = Array.from({ length: 32 }, () =>
    Math.floor(Math.random() * 256)
      .toString(16)
      .padStart(2, "0")
  ).join("");
  return { pubkey: mockPubkey, id: `key_${Date.now()}` };
}

/** Stub: delete keypair from secure storage */
async function deleteKeypair(id: string): Promise<void> {
  // Real: await AirSignCore.deleteKeypair(id);
  await SecureStore.deleteItemAsync(`airsign_kp_${id}`);
}

export default function KeystoreScreen() {
  const router = useRouter();
  const [keys, setKeys] = useState<KeyEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [generating, setGenerating] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    setKeys(await loadKeyIndex());
    setLoading(false);
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const handleGenerate = useCallback(async () => {
    setGenerating(true);
    try {
      const { pubkey, id } = await generateKeypair();
      const entry: KeyEntry = {
        id,
        label: `Key ${keys.length + 1}`,
        pubkey,
        createdAt: new Date().toISOString(),
      };
      const updated = [...keys, entry];
      await saveKeyIndex(updated);
      setKeys(updated);
    } finally {
      setGenerating(false);
    }
  }, [keys]);

  const handleDelete = useCallback(
    (entry: KeyEntry) => {
      Alert.alert(
        "Delete Key",
        `Delete "${entry.label}"?\n\nThis cannot be undone. Make sure you have a backup.`,
        [
          { text: "Cancel", style: "cancel" },
          {
            text: "Delete",
            style: "destructive",
            onPress: async () => {
              await deleteKeypair(entry.id);
              const updated = keys.filter((k: KeyEntry) => k.id !== entry.id);
              await saveKeyIndex(updated);
              setKeys(updated);
            },
          },
        ]
      );
    },
    [keys]
  );

  return (
    <ScrollView style={styles.container} contentContainerStyle={styles.content}>
      <Text style={styles.sectionLabel}>SIGNING KEYS</Text>

      {loading ? (
        <Text style={styles.dimText}>Loading…</Text>
      ) : keys.length === 0 ? (
        <View style={styles.emptyBox}>
          <Text style={styles.emptyIcon}>🗝️</Text>
          <Text style={styles.emptyTitle}>No keys yet</Text>
          <Text style={styles.emptyDesc}>
            Generate a new Ed25519 keypair to start signing transactions.
          </Text>
        </View>
      ) : (
        keys.map((key: KeyEntry) => (
          <View key={key.id} style={styles.keyCard}>
            <View style={styles.keyInfo}>
              <Text style={styles.keyLabel}>{key.label}</Text>
              <Text style={styles.keyPubkey} numberOfLines={1} ellipsizeMode="middle">
                {key.pubkey}
              </Text>
              <Text style={styles.keyDate}>
                Created {new Date(key.createdAt).toLocaleDateString()}
              </Text>
            </View>
            <TouchableOpacity
              style={styles.deleteBtn}
              onPress={() => handleDelete(key)}
            >
              <Text style={styles.deleteBtnText}>🗑️</Text>
            </TouchableOpacity>
          </View>
        ))
      )}

      <TouchableOpacity
        style={[styles.generateBtn, generating && styles.generateBtnDisabled]}
        onPress={() => void handleGenerate()}
        disabled={generating}
      >
        <Text style={styles.generateBtnText}>
          {generating ? "Generating…" : "+ Generate New Key"}
        </Text>
      </TouchableOpacity>

      <View style={styles.warningBox}>
        <Text style={styles.warningTitle}>⚠️ Security Reminders</Text>
        <Text style={styles.warningText}>
          • Private keys are stored in the OS secure keychain{"\n"}
          • Keys never leave this device{"\n"}
          • Back up your seed phrase before deleting a key{"\n"}
          • This device must remain in airplane mode at all times
        </Text>
      </View>
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: "#0a0a0a" },
  content: { padding: 20, paddingBottom: 40 },
  sectionLabel: {
    color: "#4b5563",
    fontSize: 11,
    fontWeight: "700",
    letterSpacing: 1.2,
    marginBottom: 12,
  },
  dimText: { color: "#6b7280", fontSize: 14 },
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
  keyCard: {
    backgroundColor: "#111827",
    borderRadius: 10,
    padding: 14,
    marginBottom: 10,
    flexDirection: "row",
    alignItems: "center",
    gap: 12,
  },
  keyInfo: { flex: 1 },
  keyLabel: { color: "#f9fafb", fontSize: 15, fontWeight: "600", marginBottom: 4 },
  keyPubkey: {
    fontFamily: "monospace",
    color: "#6b7280",
    fontSize: 11,
    marginBottom: 4,
  },
  keyDate: { color: "#374151", fontSize: 11 },
  deleteBtn: {
    padding: 8,
    borderRadius: 8,
    backgroundColor: "#1f2937",
  },
  deleteBtnText: { fontSize: 18 },
  generateBtn: {
    backgroundColor: "#1d4ed8",
    borderRadius: 10,
    paddingVertical: 14,
    alignItems: "center",
    marginTop: 8,
    marginBottom: 24,
  },
  generateBtnDisabled: { opacity: 0.5 },
  generateBtnText: { color: "#fff", fontSize: 16, fontWeight: "600" },
  warningBox: {
    backgroundColor: "#1c1410",
    borderRadius: 10,
    padding: 16,
    borderWidth: 1,
    borderColor: "#92400e",
  },
  warningTitle: { color: "#f59e0b", fontSize: 13, fontWeight: "700", marginBottom: 8 },
  warningText: { color: "#d97706", fontSize: 12, lineHeight: 20 },
});