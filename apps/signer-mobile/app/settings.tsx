import * as SecureStore from "expo-secure-store";
import React, { useEffect, useState } from "react";
import {
  ScrollView,
  StyleSheet,
  Switch,
  Text,
  TouchableOpacity,
  View,
} from "react-native";

// ── Storage key & defaults ─────────────────────────────────────────────────

const SETTINGS_KEY = "airsign_settings_v1";
const CLUSTER_PREF_KEY = "airsign_preferred_cluster";

type ClusterOption = "mainnet-beta" | "devnet" | "testnet";
const CLUSTER_OPTIONS: ClusterOption[] = ["mainnet-beta", "devnet", "testnet"];
const CLUSTER_LABELS: Record<ClusterOption, string> = {
  "mainnet-beta": "Mainnet",
  devnet: "Devnet",
  testnet: "Testnet",
};
const CLUSTER_COLORS: Record<ClusterOption, string> = {
  "mainnet-beta": "#ef4444",
  devnet: "#3b82f6",
  testnet: "#f59e0b",
};

interface AppSettings {
  fps: number;
  qrSize: number;
  frostThreshold: number;
  frostTotal: number;
  requireBiometrics: boolean;
  showRawData: boolean;
}

const DEFAULTS: AppSettings = {
  fps: 5,
  qrSize: 280,
  frostThreshold: 2,
  frostTotal: 3,
  requireBiometrics: true,
  showRawData: false,
};

async function loadSettings(): Promise<AppSettings> {
  try {
    const raw = await SecureStore.getItemAsync(SETTINGS_KEY);
    if (!raw) return DEFAULTS;
    return { ...DEFAULTS, ...(JSON.parse(raw) as Partial<AppSettings>) };
  } catch {
    return DEFAULTS;
  }
}

async function saveSettings(s: AppSettings): Promise<void> {
  try {
    await SecureStore.setItemAsync(SETTINGS_KEY, JSON.stringify(s));
  } catch {
    // best-effort
  }
}

// ── StepperRow component ───────────────────────────────────────────────────

interface RowProps {
  label: string;
  value: string;
  onInc: () => void;
  onDec: () => void;
  unit?: string;
}

function StepperRow({ label, value, onInc, onDec, unit = "" }: RowProps) {
  return (
    <View style={styles.row}>
      <Text style={styles.rowLabel}>{label}</Text>
      <View style={styles.stepper}>
        <TouchableOpacity style={styles.stepBtn} onPress={onDec}>
          <Text style={styles.stepBtnText}>−</Text>
        </TouchableOpacity>
        <Text style={styles.stepValue}>
          {value}
          {unit}
        </Text>
        <TouchableOpacity style={styles.stepBtn} onPress={onInc}>
          <Text style={styles.stepBtnText}>+</Text>
        </TouchableOpacity>
      </View>
    </View>
  );
}

// ── Main screen ───────────────────────────────────────────────────────────

export default function SettingsScreen() {
  const [ready, setReady] = useState(false);
  const [fps, setFps] = useState(DEFAULTS.fps);
  const [qrSize, setQrSize] = useState(DEFAULTS.qrSize);
  const [frostThreshold, setFrostThreshold] = useState(DEFAULTS.frostThreshold);
  const [frostTotal, setFrostTotal] = useState(DEFAULTS.frostTotal);
  const [requireBiometrics, setRequireBiometrics] = useState(DEFAULTS.requireBiometrics);
  const [showRawData, setShowRawData] = useState(DEFAULTS.showRawData);
  const [cluster, setCluster] = useState<ClusterOption>("devnet");

  // ── Load persisted settings on mount ────────────────────────────────────

  useEffect(() => {
    void (async () => {
      const s = await loadSettings();
      setFps(s.fps);
      setQrSize(s.qrSize);
      setFrostThreshold(s.frostThreshold);
      setFrostTotal(s.frostTotal);
      setRequireBiometrics(s.requireBiometrics);
      setShowRawData(s.showRawData);
      // Load cluster separately
      const storedCluster = await SecureStore.getItemAsync(CLUSTER_PREF_KEY);
      if (storedCluster && CLUSTER_OPTIONS.includes(storedCluster as ClusterOption)) {
        setCluster(storedCluster as ClusterOption);
      }
      setReady(true);
    })();
  }, []);

  // ── Persist whenever any value changes (after initial load) ─────────────

  useEffect(() => {
    if (!ready) return;
    void saveSettings({
      fps,
      qrSize,
      frostThreshold,
      frostTotal,
      requireBiometrics,
      showRawData,
    });
  }, [ready, fps, qrSize, frostThreshold, frostTotal, requireBiometrics, showRawData]);

  const handleClusterChange = (c: ClusterOption) => {
    setCluster(c);
    void SecureStore.setItemAsync(CLUSTER_PREF_KEY, c);
  };

  return (
    <ScrollView style={styles.container} contentContainerStyle={styles.content}>
      {/* QR Transport */}
      <Text style={styles.sectionLabel}>QR TRANSPORT</Text>
      <View style={styles.card}>
        <StepperRow
          label="Animation speed"
          value={String(fps)}
          unit=" fps"
          onInc={() => setFps((v) => Math.min(v + 1, 30))}
          onDec={() => setFps((v) => Math.max(v - 1, 1))}
        />
        <View style={styles.divider} />
        <StepperRow
          label="QR code size"
          value={String(qrSize)}
          unit=" px"
          onInc={() => setQrSize((v) => Math.min(v + 20, 400))}
          onDec={() => setQrSize((v) => Math.max(v - 20, 160))}
        />
      </View>

      {/* FROST */}
      <Text style={styles.sectionLabel}>FROST THRESHOLD</Text>
      <View style={styles.card}>
        <StepperRow
          label="Threshold (t)"
          value={String(frostThreshold)}
          onInc={() => setFrostThreshold((v) => Math.min(v + 1, frostTotal))}
          onDec={() => setFrostThreshold((v) => Math.max(v - 1, 2))}
        />
        <View style={styles.divider} />
        <StepperRow
          label="Participants (n)"
          value={String(frostTotal)}
          onInc={() => setFrostTotal((v) => Math.min(v + 1, 10))}
          onDec={() =>
            setFrostTotal((v) => {
              const next = Math.max(v - 1, frostThreshold);
              return next;
            })
          }
        />
        <Text style={styles.hint}>
          Requires {frostThreshold}-of-{frostTotal} participants to sign
        </Text>
      </View>

      {/* Security */}
      <Text style={styles.sectionLabel}>SECURITY</Text>
      <View style={styles.card}>
        <View style={styles.switchRow}>
          <View style={styles.switchLabel}>
            <Text style={styles.rowLabel}>Require biometrics</Text>
            <Text style={styles.rowHint}>
              Face ID / fingerprint before signing
            </Text>
          </View>
          <Switch
            value={requireBiometrics}
            onValueChange={setRequireBiometrics}
            trackColor={{ false: "#374151", true: "#1d4ed8" }}
            thumbColor="#ffffff"
          />
        </View>
        <View style={styles.divider} />
        <View style={styles.switchRow}>
          <View style={styles.switchLabel}>
            <Text style={styles.rowLabel}>Show raw instruction data</Text>
            <Text style={styles.rowHint}>
              Display raw hex in transaction review
            </Text>
          </View>
          <Switch
            value={showRawData}
            onValueChange={setShowRawData}
            trackColor={{ false: "#374151", true: "#1d4ed8" }}
            thumbColor="#ffffff"
          />
        </View>
      </View>

      {/* Network */}
      <Text style={styles.sectionLabel}>NETWORK</Text>
      <View style={styles.card}>
        <View style={styles.clusterRow}>
          {CLUSTER_OPTIONS.map((c, i) => (
            <TouchableOpacity
              key={c}
              style={[
                styles.clusterBtn,
                cluster === c && { backgroundColor: CLUSTER_COLORS[c] + "33", borderColor: CLUSTER_COLORS[c] },
                i === 0 && { borderTopLeftRadius: 10, borderBottomLeftRadius: 10 },
                i === CLUSTER_OPTIONS.length - 1 && { borderTopRightRadius: 10, borderBottomRightRadius: 10 },
              ]}
              onPress={() => handleClusterChange(c)}
            >
              <View style={[styles.clusterDot, { backgroundColor: cluster === c ? CLUSTER_COLORS[c] : "#374151" }]} />
              <Text style={[styles.clusterBtnText, cluster === c && { color: CLUSTER_COLORS[c] }]}>
                {CLUSTER_LABELS[c]}
              </Text>
            </TouchableOpacity>
          ))}
        </View>
        <Text style={styles.hint}>
          {cluster === "mainnet-beta"
            ? "⚠️  Mainnet — real SOL will be transferred"
            : cluster === "testnet"
            ? "Testnet — test tokens, no real value"
            : "Devnet — development & testing"}
        </Text>
      </View>

      {/* About */}
      <Text style={styles.sectionLabel}>ABOUT</Text>
      <View style={styles.card}>
        <View style={styles.row}>
          <Text style={styles.rowLabel}>Version</Text>
          <Text style={styles.rowValue}>1.0.0</Text>
        </View>
        <View style={styles.divider} />
        <View style={styles.row}>
          <Text style={styles.rowLabel}>Protocol</Text>
          <Text style={styles.rowValue}>AirSign v6</Text>
        </View>
        <View style={styles.divider} />
        <View style={styles.row}>
          <Text style={styles.rowLabel}>Cryptography</Text>
          <Text style={styles.rowValue}>Ed25519 · FROST RFC 9591</Text>
        </View>
        <View style={styles.divider} />
        <View style={styles.row}>
          <Text style={styles.rowLabel}>Source</Text>
          <Text style={styles.rowValue}>github.com/nzengi/AirSign</Text>
        </View>
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
    marginBottom: 8,
    marginTop: 16,
  },
  card: {
    backgroundColor: "#111827",
    borderRadius: 12,
  },
  divider: {
    height: StyleSheet.hairlineWidth,
    backgroundColor: "#1f2937",
    marginHorizontal: 16,
  },
  row: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: 16,
    paddingVertical: 14,
  },
  rowLabel: { color: "#e5e7eb", fontSize: 15 },
  rowValue: { color: "#6b7280", fontSize: 14 },
  rowHint: { color: "#4b5563", fontSize: 11, marginTop: 2 },
  hint: {
    color: "#4b5563",
    fontSize: 11,
    paddingHorizontal: 16,
    paddingBottom: 12,
  },
  stepper: {
    flexDirection: "row",
    alignItems: "center",
    gap: 8,
  },
  stepBtn: {
    width: 32,
    height: 32,
    borderRadius: 8,
    backgroundColor: "#1f2937",
    alignItems: "center",
    justifyContent: "center",
  },
  stepBtnText: { color: "#e5e7eb", fontSize: 20, lineHeight: 24 },
  stepValue: { color: "#e5e7eb", fontSize: 15, minWidth: 52, textAlign: "center" },
  clusterRow: {
    flexDirection: "row",
    margin: 12,
    borderRadius: 10,
    overflow: "hidden",
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: "#1f2937",
  },
  clusterBtn: {
    flex: 1,
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "center",
    paddingVertical: 12,
    gap: 6,
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: "#1f2937",
  },
  clusterDot: { width: 8, height: 8, borderRadius: 4 },
  clusterBtnText: { color: "#6b7280", fontSize: 13, fontWeight: "600" },
  switchRow: {
    flexDirection: "row",
    alignItems: "center",
    paddingHorizontal: 16,
    paddingVertical: 14,
    gap: 12,
  },
  switchLabel: { flex: 1 },
});