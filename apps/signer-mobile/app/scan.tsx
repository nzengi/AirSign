import { useRouter } from "expo-router";
import * as SecureStore from "expo-secure-store";
import React, { useCallback, useEffect, useRef, useState } from "react";
import {
  ActivityIndicator,
  Modal,
  ScrollView,
  StyleSheet,
  Text,
  TouchableOpacity,
  View,
} from "react-native";
import QrScanner from "../src/components/QrScanner";
import TransactionReview, {
  TransactionSummary,
} from "../src/components/TransactionReview";
import { appendSigningLog } from "./history";
import AirSignCore from "../src/native/AirSignCore";

// expo-local-authentication requires native compilation and is not available in
// Expo Go. Import it lazily so the module still loads in Expo Go (biometrics
// are simply skipped when the native module is absent).
interface LocalAuthModule {
  hasHardwareAsync(): Promise<boolean>;
  isEnrolledAsync(): Promise<boolean>;
  authenticateAsync(opts: {
    promptMessage: string;
    fallbackLabel: string;
    cancelLabel: string;
    disableDeviceFallback: boolean;
  }): Promise<{ success: boolean; error?: string }>;
}
// eslint-disable-next-line @typescript-eslint/no-explicit-any
let LocalAuthentication: LocalAuthModule | null = null;
try {
  // eslint-disable-next-line @typescript-eslint/no-var-requires
  LocalAuthentication = require("expo-local-authentication") as LocalAuthModule;
} catch {
  // Expo Go — biometrics unavailable, signing proceeds without the auth gate
}

// ─────────────────────────────────────────────────────────────────────────────

type Phase = "selectKey" | "scanning" | "decoding" | "reviewing" | "signing" | "done";

interface KeyOption {
  id: string;
  label: string;
  pubkeyBase58: string;
}

const SETTINGS_KEY = "airsign_settings_v1";
const LABEL_STORE_KEY = "airsign_key_labels";
const CLUSTER_PREF_KEY = "airsign_preferred_cluster";

async function loadLabelMap(): Promise<Record<string, { label: string }>> {
  try {
    const raw = await SecureStore.getItemAsync(LABEL_STORE_KEY);
    if (!raw) return {};
    return JSON.parse(raw) as Record<string, { label: string }>;
  } catch {
    return {};
  }
}

async function readPreferredCluster(): Promise<string> {
  try {
    const val = await SecureStore.getItemAsync(CLUSTER_PREF_KEY);
    return val ?? "devnet";
  } catch {
    return "devnet";
  }
}

async function readRequireBiometrics(): Promise<boolean> {
  try {
    const raw = await SecureStore.getItemAsync(SETTINGS_KEY);
    if (!raw) return true; // default: on
    const parsed = JSON.parse(raw) as { requireBiometrics?: boolean };
    return parsed.requireBiometrics !== false;
  } catch {
    return true;
  }
}

async function checkBiometrics(): Promise<{ success: boolean; reason?: string }> {
  if (!LocalAuthentication) {
    // Native module not available (Expo Go) — allow through
    return { success: true };
  }
  const hasBiometrics = await LocalAuthentication.hasHardwareAsync();
  if (!hasBiometrics) {
    return { success: true };
  }
  const isEnrolled = await LocalAuthentication.isEnrolledAsync();
  if (!isEnrolled) {
    return { success: true };
  }
  const result = await LocalAuthentication.authenticateAsync({
    promptMessage: "Confirm signing with biometrics",
    fallbackLabel: "Use Passcode",
    cancelLabel: "Cancel",
    disableDeviceFallback: false,
  });
  if (result.success) return { success: true };
  return { success: false, reason: result.error ?? "Authentication cancelled" };
}

// ── Key selection modal ───────────────────────────────────────────────────────

function KeySelectModal({
  visible,
  keys,
  onSelect,
  onCancel,
}: {
  visible: boolean;
  keys: KeyOption[];
  onSelect: (key: KeyOption) => void;
  onCancel: () => void;
}) {
  return (
    <Modal visible={visible} transparent animationType="slide">
      <View style={styles.modalOverlay}>
        <View style={styles.modalSheet}>
          <Text style={styles.modalTitle}>Select Signing Key</Text>
          {keys.length === 0 ? (
            <Text style={styles.modalEmpty}>
              No keys found. Generate a key in Key Management first.
            </Text>
          ) : (
            <ScrollView>
              {keys.map((k) => (
                <TouchableOpacity
                  key={k.id}
                  style={styles.keyOption}
                  onPress={() => onSelect(k)}
                >
                  <Text style={styles.keyOptionLabel}>{k.label}</Text>
                  <Text
                    style={styles.keyOptionPubkey}
                    numberOfLines={1}
                    ellipsizeMode="middle"
                  >
                    {k.pubkeyBase58}
                  </Text>
                </TouchableOpacity>
              ))}
            </ScrollView>
          )}
          <TouchableOpacity style={styles.cancelBtn} onPress={onCancel}>
            <Text style={styles.cancelBtnText}>Cancel</Text>
          </TouchableOpacity>
        </View>
      </View>
    </Modal>
  );
}

// ── Main screen ───────────────────────────────────────────────────────────────

export default function ScanScreen() {
  const router = useRouter();
  const [phase, setPhase] = useState<Phase>("selectKey");
  const [keys, setKeys] = useState<KeyOption[]>([]);
  const [selectedKey, setSelectedKey] = useState<KeyOption | null>(null);
  const [sessionId] = useState(() => `scan_${Date.now()}`);
  const [frameCount, setFrameCount] = useState(0);
  const [progress, setProgress] = useState(0);
  const [tx, setTx] = useState<TransactionSummary | null>(null);
  const [statusMsg, setStatusMsg] = useState("");

  // Decoded payload cached in a ref — avoids re-querying the decoder on approve
  const payloadRef = useRef<string>("");

  // Load available keys on mount
  useEffect(() => {
    void (async () => {
      try {
        const ids: string[] = await AirSignCore.listKeypairIds();
        const labelMap = await loadLabelMap();
        const loaded: KeyOption[] = await Promise.all(
          ids.map(async (id, i) => {
            let pubkeyBase58 = "";
            try {
              const r = await AirSignCore.getPublicKey(id);
              pubkeyBase58 =
                typeof r === "object" && r !== null && "pubkeyBase58" in r
                  ? (r as { pubkeyBase58: string }).pubkeyBase58
                  : String(r);
            } catch {
              pubkeyBase58 = "(unavailable)";
            }
            const label = labelMap[id]?.label ?? `Key ${i + 1}`;
            return { id, label, pubkeyBase58 };
          })
        );
        setKeys(loaded);
      } catch (err) {
        console.error("[Scan] failed to load keys", err);
      }
    })();
  }, []);

  // Handle each QR frame
  const handleFrame = useCallback(
    async (frameBase64: string) => {
      if (phase !== "scanning") return;
      try {
        setPhase("decoding");

        let totalBlocks = 0;
        try {
          const hdr = JSON.parse(atob(frameBase64)) as { total?: number };
          if (typeof hdr.total === "number") totalBlocks = hdr.total;
        } catch {
          // raw frame — not JSON-wrapped
        }

        const result = await AirSignCore.fountainDecodeAdd(
          sessionId,
          frameBase64,
          totalBlocks
        );
        const decoded = result as {
          complete: boolean;
          payloadBase64?: string;
          framesReceived?: number;
        };

        const received = decoded.framesReceived ?? frameCount + 1;
        setFrameCount(received);
        if (totalBlocks > 0) {
          setProgress(Math.min(received / totalBlocks, 0.99));
        } else {
          setProgress(Math.min(received * 0.1, 0.9));
        }

        if (decoded.complete && decoded.payloadBase64) {
          setProgress(1);
          payloadRef.current = decoded.payloadBase64;

          setStatusMsg("Inspecting transaction…");
          const inspected = await AirSignCore.inspectTransaction(
            decoded.payloadBase64
          );
          const insp = inspected as unknown as {
            feePayer?: string;
            feeLamports?: number;
            recentBlockhash?: string;
            riskLevel?: string;
            instructions?: TransactionSummary["instructions"];
          };
          const summary: TransactionSummary = {
            feePayer: insp.feePayer ?? "",
            feeLamports: insp.feeLamports ?? 0,
            recentBlockhash: insp.recentBlockhash ?? "",
            riskLevel:
              (insp.riskLevel as TransactionSummary["riskLevel"]) ?? "safe",
            instructions: insp.instructions ?? [],
          };
          setTx(summary);
          setPhase("reviewing");
        } else {
          setPhase("scanning");
        }
      } catch (err) {
        console.error("[Scan] frame error", err);
        setPhase("scanning");
      }
    },
    [phase, sessionId, frameCount]
  );

  const handleApprove = useCallback(async () => {
    if (!tx || !selectedKey) return;

    // 1. Biometrics gate
    const requireBio = await readRequireBiometrics();
    if (requireBio) {
      setStatusMsg("Waiting for biometric confirmation…");
      setPhase("signing");
      const bio = await checkBiometrics();
      if (!bio.success) {
        setStatusMsg(`Biometric auth failed: ${bio.reason ?? "cancelled"}`);
        setPhase("reviewing");
        return;
      }
    } else {
      setPhase("signing");
    }

    setStatusMsg("Signing…");

    // 2. Get payload from ref
    const payload = payloadRef.current;
    if (!payload) {
      setStatusMsg("Error: transaction payload not available");
      setPhase("reviewing");
      return;
    }

    try {
      const signResult = await AirSignCore.signTransaction(
        selectedKey.id,
        payload
      );
      const signed = signResult as {
        signedTxBase64?: string;
        signatureBase58?: string;
      };
      const signedPayload =
        signed.signedTxBase64 ?? signed.signatureBase58 ?? "";

      // 3. Append to signing log (best-effort, non-blocking)
      void appendSigningLog({
        id: `${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
        signedAt: new Date().toISOString(),
        keyLabel: selectedKey.label,
        keyPubkey: selectedKey.pubkeyBase58,
        feePayer: tx.feePayer,
        feeLamports: tx.feeLamports ?? 0,
        riskLevel: tx.riskLevel,
        instructionCount: tx.instructions.length,
        instructionNames: tx.instructions.map((ix) => ix.name),
        signatureBase58: signed.signatureBase58 ?? signedPayload,
      });

      // 4. Embed cluster preference into signed payload for broadcaster
      const cluster = await readPreferredCluster();

      // 5. Fountain-encode signed tx (wrap with cluster metadata)
      const payloadWithCluster = JSON.stringify({
        signedTx: signedPayload,
        cluster,
      });
      const payloadB64 = btoa(payloadWithCluster);
      const encResult = await AirSignCore.fountainEncode(payloadB64, 16);
      const enc = encResult as { frames?: string[] };
      const frames = enc.frames ?? [payloadB64];

      router.replace({
        pathname: "/display",
        params: { framesJson: JSON.stringify(frames), cluster },
      });
    } catch (err) {
      setStatusMsg(`Signing failed: ${String(err)}`);
      setPhase("reviewing");
    }
  }, [tx, selectedKey, router]);

  const handleReject = useCallback(() => {
    router.back();
  }, [router]);

  // ── Render ────────────────────────────────────────────────────────────────

  if (phase === "selectKey") {
    return (
      <KeySelectModal
        visible
        keys={keys}
        onSelect={(k) => {
          setSelectedKey(k);
          setPhase("scanning");
        }}
        onCancel={() => router.back()}
      />
    );
  }

  if (phase === "reviewing" && tx) {
    return (
      <TransactionReview
        tx={tx}
        onApprove={() => void handleApprove()}
        onReject={handleReject}
        approving={false}
      />
    );
  }

  if (phase === "signing") {
    return (
      <View style={[styles.container, styles.centered]}>
        <ActivityIndicator size="large" color="#3b82f6" />
        <Text style={styles.statusText}>{statusMsg || "Signing…"}</Text>
      </View>
    );
  }

  return (
    <View style={styles.container}>
      {selectedKey && (
        <View style={styles.keyPill}>
          <Text style={styles.keyPillText}>🗝️ {selectedKey.label}</Text>
        </View>
      )}

      <View style={styles.progressBar}>
        <View style={[styles.progressFill, { width: `${progress * 100}%` }]} />
      </View>

      <QrScanner
        onFrame={(d) => void handleFrame(d)}
        paused={phase !== "scanning"}
      />

      <View style={styles.hint}>
        <Text style={styles.hintText}>
          {frameCount === 0
            ? "Point camera at the animated QR on the online machine"
            : `Received ${frameCount} frame${frameCount > 1 ? "s" : ""}… ${statusMsg}`}
        </Text>
      </View>
    </View>
  );
}

// ── Styles ────────────────────────────────────────────────────────────────────

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: "#000" },
  centered: { alignItems: "center", justifyContent: "center" },
  keyPill: {
    backgroundColor: "#1f2937",
    paddingHorizontal: 12,
    paddingVertical: 6,
    margin: 8,
    borderRadius: 20,
    alignSelf: "center",
  },
  keyPillText: { color: "#9ca3af", fontSize: 12 },
  progressBar: { height: 3, backgroundColor: "#1f2937" },
  progressFill: { height: 3, backgroundColor: "#3b82f6" },
  hint: {
    backgroundColor: "#0a0a0acc",
    padding: 12,
    alignItems: "center",
  },
  hintText: { color: "#9ca3af", fontSize: 13 },
  statusText: { color: "#9ca3af", fontSize: 14, marginTop: 16 },
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
    maxHeight: "70%",
  },
  modalTitle: {
    color: "#f9fafb",
    fontSize: 18,
    fontWeight: "700",
    marginBottom: 16,
    textAlign: "center",
  },
  modalEmpty: {
    color: "#6b7280",
    fontSize: 14,
    textAlign: "center",
    paddingVertical: 24,
  },
  keyOption: {
    backgroundColor: "#1f2937",
    borderRadius: 10,
    padding: 14,
    marginBottom: 10,
  },
  keyOptionLabel: {
    color: "#f9fafb",
    fontSize: 15,
    fontWeight: "600",
    marginBottom: 4,
  },
  keyOptionPubkey: { color: "#6b7280", fontSize: 11, fontFamily: "monospace" },
  cancelBtn: {
    marginTop: 12,
    paddingVertical: 14,
    alignItems: "center",
    backgroundColor: "#1f2937",
    borderRadius: 10,
  },
  cancelBtnText: { color: "#9ca3af", fontSize: 16 },
});