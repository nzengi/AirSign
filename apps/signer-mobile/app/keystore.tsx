import * as SecureStore from "expo-secure-store";
import React, { useCallback, useEffect, useRef, useState } from "react";
import {
  Alert,
  Modal,
  ScrollView,
  Share,
  StyleSheet,
  Text,
  TextInput,
  TouchableOpacity,
  View,
} from "react-native";
import AirSignCore from "../src/native/AirSignCore";

interface KeyEntry {
  id: string;
  label: string;
  pubkeyBase58: string;
  createdAt: string;
}

const LABEL_STORE_KEY = "airsign_key_labels";

async function loadLabelMap(): Promise<
  Record<string, { label: string; createdAt: string }>
> {
  try {
    const raw = await SecureStore.getItemAsync(LABEL_STORE_KEY);
    if (!raw) return {};
    return JSON.parse(raw) as Record<
      string,
      { label: string; createdAt: string }
    >;
  } catch {
    return {};
  }
}

async function saveLabelMap(
  map: Record<string, { label: string; createdAt: string }>
): Promise<void> {
  try {
    await SecureStore.setItemAsync(LABEL_STORE_KEY, JSON.stringify(map));
  } catch {
    // best-effort
  }
}

// ── Import Modal ──────────────────────────────────────────────────────────────

function ImportModal({
  visible,
  onImport,
  onCancel,
}: {
  visible: boolean;
  onImport: (privateKeyBase58: string, label: string) => Promise<void>;
  onCancel: () => void;
}) {
  const [pk, setPk] = useState("");
  const [label, setLabel] = useState("");
  const [busy, setBusy] = useState(false);

  const handleSubmit = async () => {
    const trimmed = pk.trim();
    if (!trimmed) {
      Alert.alert("Missing Key", "Paste your base58-encoded private key.");
      return;
    }
    setBusy(true);
    try {
      await onImport(trimmed, label.trim());
      setPk("");
      setLabel("");
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal visible={visible} transparent animationType="slide">
      <View style={styles.modalOverlay}>
        <View style={styles.modalSheet}>
          <Text style={styles.modalTitle}>Import Private Key</Text>

          <Text style={styles.fieldLabel}>Label (optional)</Text>
          <TextInput
            style={styles.input}
            value={label}
            onChangeText={setLabel}
            placeholder="e.g. Backup Key"
            placeholderTextColor="#4b5563"
            autoCapitalize="none"
          />

          <Text style={styles.fieldLabel}>Private Key (base58)</Text>
          <TextInput
            style={[styles.input, styles.monoInput]}
            value={pk}
            onChangeText={setPk}
            placeholder="5Kb8kLf9…"
            placeholderTextColor="#4b5563"
            autoCapitalize="none"
            autoCorrect={false}
            secureTextEntry
            multiline
          />

          <View style={styles.warningBox}>
            <Text style={styles.warningTitle}>⚠️ Security Warning</Text>
            <Text style={styles.warningText}>
              Never type a private key on an online device. Only import keys
              on this air-gapped device.
            </Text>
          </View>

          <TouchableOpacity
            style={[styles.primaryBtn, busy && styles.disabledBtn]}
            onPress={() => void handleSubmit()}
            disabled={busy}
          >
            <Text style={styles.primaryBtnText}>
              {busy ? "Importing…" : "Import Key"}
            </Text>
          </TouchableOpacity>

          <TouchableOpacity style={styles.cancelBtn} onPress={onCancel}>
            <Text style={styles.cancelBtnText}>Cancel</Text>
          </TouchableOpacity>
        </View>
      </View>
    </Modal>
  );
}

// ── Export Modal ──────────────────────────────────────────────────────────────

function ExportModal({
  visible,
  entry,
  onClose,
}: {
  visible: boolean;
  entry: KeyEntry | null;
  onClose: () => void;
}) {
  const [privateKey, setPrivateKey] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [copied, setCopied] = useState(false);

  // Reset on open
  useEffect(() => {
    if (visible) {
      setPrivateKey(null);
      setCopied(false);
    }
  }, [visible]);

  const handleReveal = async () => {
    if (!entry) return;
    setLoading(true);
    try {
      const pk = await AirSignCore.exportPrivateKey(entry.id);
      setPrivateKey(pk);
    } catch (err) {
      Alert.alert("Error", String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleCopy = async () => {
    if (!privateKey) return;
    try {
      await Share.share({ message: privateKey });
      setCopied(true);
      setTimeout(() => setCopied(false), 3000);
    } catch {
      // user dismissed share sheet — no-op
    }
  };

  return (
    <Modal visible={visible} transparent animationType="slide">
      <View style={styles.modalOverlay}>
        <View style={styles.modalSheet}>
          <Text style={styles.modalTitle}>Export Private Key</Text>
          {entry && (
            <Text style={styles.exportKeyLabel} numberOfLines={1} ellipsizeMode="middle">
              {entry.label} · {entry.pubkeyBase58}
            </Text>
          )}

          <View style={[styles.warningBox, { marginBottom: 16 }]}>
            <Text style={styles.warningTitle}>⚠️ DANGER — Never share this key</Text>
            <Text style={styles.warningText}>
              Anyone with this key can sign any transaction on your behalf.
              {"\n"}Only reveal on an air-gapped device.
            </Text>
          </View>

          {privateKey ? (
            <>
              <View style={styles.pkBox}>
                <Text style={styles.pkText} selectable>
                  {privateKey}
                </Text>
              </View>
              <TouchableOpacity
                style={[styles.primaryBtn, copied && styles.successBtn]}
                onPress={() => void handleCopy()}
              >
                <Text style={styles.primaryBtnText}>
                  {copied ? "✓ Copied to Clipboard" : "Copy to Clipboard"}
                </Text>
              </TouchableOpacity>
            </>
          ) : (
            <TouchableOpacity
              style={[styles.dangerBtn, loading && styles.disabledBtn]}
              onPress={() => void handleReveal()}
              disabled={loading}
            >
              <Text style={styles.dangerBtnText}>
                {loading ? "Loading…" : "Reveal Private Key"}
              </Text>
            </TouchableOpacity>
          )}

          <TouchableOpacity style={styles.cancelBtn} onPress={onClose}>
            <Text style={styles.cancelBtnText}>Close</Text>
          </TouchableOpacity>
        </View>
      </View>
    </Modal>
  );
}

// ── Rename Modal ──────────────────────────────────────────────────────────────

function RenameModal({
  visible,
  entry,
  onRename,
  onCancel,
}: {
  visible: boolean;
  entry: KeyEntry | null;
  onRename: (id: string, newLabel: string) => Promise<void>;
  onCancel: () => void;
}) {
  const [label, setLabel] = useState(entry?.label ?? "");
  const inputRef = useRef<TextInput>(null);

  useEffect(() => {
    if (visible && entry) {
      setLabel(entry.label);
      setTimeout(() => inputRef.current?.focus(), 100);
    }
  }, [visible, entry]);

  const handleSave = async () => {
    const trimmed = label.trim();
    if (!trimmed) return;
    if (!entry) return;
    await onRename(entry.id, trimmed);
  };

  return (
    <Modal visible={visible} transparent animationType="fade">
      <View style={styles.modalOverlay}>
        <View style={[styles.modalSheet, { gap: 12 }]}>
          <Text style={styles.modalTitle}>Rename Key</Text>
          <TextInput
            ref={inputRef}
            style={styles.input}
            value={label}
            onChangeText={setLabel}
            placeholder="Key label"
            placeholderTextColor="#4b5563"
            returnKeyType="done"
            onSubmitEditing={() => void handleSave()}
          />
          <TouchableOpacity
            style={styles.primaryBtn}
            onPress={() => void handleSave()}
          >
            <Text style={styles.primaryBtnText}>Save</Text>
          </TouchableOpacity>
          <TouchableOpacity style={styles.cancelBtn} onPress={onCancel}>
            <Text style={styles.cancelBtnText}>Cancel</Text>
          </TouchableOpacity>
        </View>
      </View>
    </Modal>
  );
}

// ── Main Screen ───────────────────────────────────────────────────────────────

export default function KeystoreScreen() {
  const [keys, setKeys] = useState<KeyEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [generating, setGenerating] = useState(false);

  const [showImport, setShowImport] = useState(false);
  const [exportEntry, setExportEntry] = useState<KeyEntry | null>(null);
  const [renameEntry, setRenameEntry] = useState<KeyEntry | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const ids: string[] = await AirSignCore.listKeypairIds();
      const labelMap = await loadLabelMap();

      const entries: KeyEntry[] = await Promise.all(
        ids.map(async (id, index) => {
          let pubkeyBase58 = "";
          try {
            const result = await AirSignCore.getPublicKey(id);
            pubkeyBase58 =
              typeof result === "object" &&
              result !== null &&
              "pubkeyBase58" in result
                ? (result as { pubkeyBase58: string }).pubkeyBase58
                : String(result);
          } catch {
            pubkeyBase58 = "(unavailable)";
          }
          const meta = labelMap[id] ?? {
            label: `Key ${index + 1}`,
            createdAt: new Date().toISOString(),
          };
          return { id, pubkeyBase58, label: meta.label, createdAt: meta.createdAt };
        })
      );
      setKeys(entries);
    } catch (err) {
      console.error("[Keystore] refresh failed", err);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // ── Generate ────────────────────────────────────────────────────────────────

  const handleGenerate = useCallback(async () => {
    setGenerating(true);
    try {
      const kp = await AirSignCore.generateKeypair();
      const id: string = (kp as { id: string }).id;
      const pubkeyBase58: string = (kp as { pubkeyBase58: string }).pubkeyBase58;

      const labelMap = await loadLabelMap();
      const label = `Key ${keys.length + 1}`;
      const createdAt = new Date().toISOString();
      labelMap[id] = { label, createdAt };
      await saveLabelMap(labelMap);

      setKeys((prev) => [...prev, { id, label, pubkeyBase58, createdAt }]);
    } catch (err) {
      Alert.alert("Error", `Failed to generate key: ${String(err)}`);
    } finally {
      setGenerating(false);
    }
  }, [keys.length]);

  // ── Import ──────────────────────────────────────────────────────────────────

  const handleImport = useCallback(
    async (privateKeyBase58: string, customLabel: string) => {
      try {
        const kp = await AirSignCore.importKeypair(privateKeyBase58);
        const id: string = (kp as { id: string }).id;
        const pubkeyBase58: string = (kp as { pubkeyBase58: string }).pubkeyBase58;

        const labelMap = await loadLabelMap();
        const label = customLabel || `Imported Key ${keys.length + 1}`;
        const createdAt = new Date().toISOString();
        labelMap[id] = { label, createdAt };
        await saveLabelMap(labelMap);

        setKeys((prev) => [...prev, { id, label, pubkeyBase58, createdAt }]);
        setShowImport(false);
        Alert.alert("Success", `Key imported successfully.\nPublic key: ${pubkeyBase58.slice(0, 16)}…`);
      } catch (err) {
        Alert.alert("Import Failed", String(err));
      }
    },
    [keys.length]
  );

  // ── Rename ──────────────────────────────────────────────────────────────────

  const handleRename = useCallback(async (id: string, newLabel: string) => {
    try {
      const labelMap = await loadLabelMap();
      const existing = labelMap[id] ?? { label: newLabel, createdAt: new Date().toISOString() };
      labelMap[id] = { ...existing, label: newLabel };
      await saveLabelMap(labelMap);
      setKeys((prev) =>
        prev.map((k) => (k.id === id ? { ...k, label: newLabel } : k))
      );
      setRenameEntry(null);
    } catch (err) {
      Alert.alert("Error", `Failed to rename key: ${String(err)}`);
    }
  }, []);

  // ── Delete ──────────────────────────────────────────────────────────────────

  const handleDelete = useCallback((entry: KeyEntry) => {
    Alert.alert(
      "Delete Key",
      `Delete "${entry.label}"?\n\nThis cannot be undone. Make sure you have a backup.`,
      [
        { text: "Cancel", style: "cancel" },
        {
          text: "Delete",
          style: "destructive",
          onPress: async () => {
            try {
              await AirSignCore.deleteKeypair(entry.id);
              const labelMap = await loadLabelMap();
              delete labelMap[entry.id];
              await saveLabelMap(labelMap);
              setKeys((prev) => prev.filter((k) => k.id !== entry.id));
            } catch (err) {
              Alert.alert("Error", `Failed to delete key: ${String(err)}`);
            }
          },
        },
      ]
    );
  }, []);

  // ── Copy pubkey ─────────────────────────────────────────────────────────────

  const handleCopyPubkey = useCallback(async (entry: KeyEntry) => {
    try {
      await Share.share({ message: entry.pubkeyBase58 });
    } catch {
      // user dismissed — no-op
    }
  }, []);

  // ── Render ──────────────────────────────────────────────────────────────────

  return (
    <>
      <ScrollView style={styles.container} contentContainerStyle={styles.content}>
        <Text style={styles.sectionLabel}>SIGNING KEYS</Text>

        {loading ? (
          <Text style={styles.dimText}>Loading…</Text>
        ) : keys.length === 0 ? (
          <View style={styles.emptyBox}>
            <Text style={styles.emptyIcon}>🗝️</Text>
            <Text style={styles.emptyTitle}>No keys yet</Text>
            <Text style={styles.emptyDesc}>
              Generate a new Ed25519 keypair or import an existing private key.
            </Text>
          </View>
        ) : (
          keys.map((key) => (
            <View key={key.id} style={styles.keyCard}>
              <View style={styles.keyInfo}>
                <TouchableOpacity onPress={() => setRenameEntry(key)}>
                  <Text style={styles.keyLabel}>
                    {key.label} <Text style={styles.editHint}>✎</Text>
                  </Text>
                </TouchableOpacity>
                <TouchableOpacity onPress={() => void handleCopyPubkey(key)}>
                  <Text
                    style={styles.keyPubkey}
                    numberOfLines={1}
                    ellipsizeMode="middle"
                  >
                    {key.pubkeyBase58}
                  </Text>
                </TouchableOpacity>
                <Text style={styles.keyDate}>
                  Created {new Date(key.createdAt).toLocaleDateString()}
                </Text>
              </View>

              <View style={styles.keyActions}>
                <TouchableOpacity
                  style={styles.actionBtn}
                  onPress={() => setExportEntry(key)}
                >
                  <Text style={styles.actionBtnText}>⬆️</Text>
                </TouchableOpacity>
                <TouchableOpacity
                  style={styles.actionBtn}
                  onPress={() => handleDelete(key)}
                >
                  <Text style={styles.actionBtnText}>🗑️</Text>
                </TouchableOpacity>
              </View>
            </View>
          ))
        )}

        {/* Action buttons */}
        <TouchableOpacity
          style={[styles.generateBtn, generating && styles.disabledBtn]}
          onPress={() => void handleGenerate()}
          disabled={generating}
        >
          <Text style={styles.generateBtnText}>
            {generating ? "Generating…" : "+ Generate New Key"}
          </Text>
        </TouchableOpacity>

        <TouchableOpacity
          style={styles.importBtn}
          onPress={() => setShowImport(true)}
        >
          <Text style={styles.importBtnText}>⬇ Import Existing Key</Text>
        </TouchableOpacity>

        <View style={styles.securityBox}>
          <Text style={styles.warningTitle}>⚠️ Security Reminders</Text>
          <Text style={styles.warningText}>
            • Private keys are stored in the OS Keychain (Secure Enclave){"\n"}
            • Keys never leave this device unencrypted{"\n"}
            • Tap a public key to copy it{"\n"}
            • Tap ✎ to rename, ⬆️ to export, 🗑️ to delete{"\n"}
            • This device must remain in airplane mode at all times
          </Text>
        </View>
      </ScrollView>

      <ImportModal
        visible={showImport}
        onImport={handleImport}
        onCancel={() => setShowImport(false)}
      />

      <ExportModal
        visible={exportEntry !== null}
        entry={exportEntry}
        onClose={() => setExportEntry(null)}
      />

      <RenameModal
        visible={renameEntry !== null}
        entry={renameEntry}
        onRename={handleRename}
        onCancel={() => setRenameEntry(null)}
      />
    </>
  );
}

// ── Styles ────────────────────────────────────────────────────────────────────

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
  editHint: { color: "#4b5563", fontSize: 12 },
  keyPubkey: {
    fontFamily: "monospace",
    color: "#6b7280",
    fontSize: 11,
    marginBottom: 4,
  },
  keyDate: { color: "#374151", fontSize: 11 },
  keyActions: { flexDirection: "row", gap: 6 },
  actionBtn: {
    padding: 8,
    borderRadius: 8,
    backgroundColor: "#1f2937",
  },
  actionBtnText: { fontSize: 18 },
  generateBtn: {
    backgroundColor: "#1d4ed8",
    borderRadius: 10,
    paddingVertical: 14,
    alignItems: "center",
    marginTop: 8,
    marginBottom: 10,
  },
  generateBtnText: { color: "#fff", fontSize: 16, fontWeight: "600" },
  importBtn: {
    backgroundColor: "#1f2937",
    borderRadius: 10,
    paddingVertical: 14,
    alignItems: "center",
    marginBottom: 24,
    borderWidth: 1,
    borderColor: "#374151",
  },
  importBtnText: { color: "#9ca3af", fontSize: 16, fontWeight: "600" },
  disabledBtn: { opacity: 0.5 },
  securityBox: {
    backgroundColor: "#1c1410",
    borderRadius: 10,
    padding: 16,
    borderWidth: 1,
    borderColor: "#92400e",
  },
  warningBox: {
    backgroundColor: "#1c1410",
    borderRadius: 8,
    padding: 12,
    borderWidth: 1,
    borderColor: "#92400e",
    marginBottom: 8,
  },
  warningTitle: { color: "#f59e0b", fontSize: 13, fontWeight: "700", marginBottom: 6 },
  warningText: { color: "#d97706", fontSize: 12, lineHeight: 20 },
  // Modal
  modalOverlay: {
    flex: 1,
    backgroundColor: "#000000aa",
    justifyContent: "flex-end",
  },
  modalSheet: {
    backgroundColor: "#111827",
    borderTopLeftRadius: 16,
    borderTopRightRadius: 16,
    padding: 20,
    maxHeight: "80%",
  },
  modalTitle: {
    color: "#f9fafb",
    fontSize: 18,
    fontWeight: "700",
    marginBottom: 16,
    textAlign: "center",
  },
  fieldLabel: {
    color: "#9ca3af",
    fontSize: 12,
    fontWeight: "600",
    marginBottom: 6,
    marginTop: 8,
  },
  input: {
    backgroundColor: "#1f2937",
    borderRadius: 8,
    color: "#f9fafb",
    fontSize: 14,
    padding: 12,
    marginBottom: 4,
  },
  monoInput: { fontFamily: "monospace", fontSize: 12, minHeight: 60 },
  primaryBtn: {
    backgroundColor: "#1d4ed8",
    borderRadius: 10,
    paddingVertical: 14,
    alignItems: "center",
    marginTop: 12,
  },
  primaryBtnText: { color: "#fff", fontSize: 16, fontWeight: "600" },
  successBtn: { backgroundColor: "#16a34a" },
  dangerBtn: {
    backgroundColor: "#7f1d1d",
    borderRadius: 10,
    paddingVertical: 14,
    alignItems: "center",
    marginTop: 12,
    borderWidth: 1,
    borderColor: "#dc2626",
  },
  dangerBtnText: { color: "#fca5a5", fontSize: 16, fontWeight: "600" },
  cancelBtn: {
    marginTop: 10,
    paddingVertical: 14,
    alignItems: "center",
    backgroundColor: "#1f2937",
    borderRadius: 10,
  },
  cancelBtnText: { color: "#9ca3af", fontSize: 16 },
  exportKeyLabel: {
    color: "#6b7280",
    fontSize: 11,
    fontFamily: "monospace",
    textAlign: "center",
    marginBottom: 12,
  },
  pkBox: {
    backgroundColor: "#0a0a0a",
    borderRadius: 8,
    padding: 14,
    marginBottom: 8,
    borderWidth: 1,
    borderColor: "#dc2626",
  },
  pkText: {
    color: "#fca5a5",
    fontFamily: "monospace",
    fontSize: 13,
    textAlign: "center",
    lineHeight: 20,
  },
});