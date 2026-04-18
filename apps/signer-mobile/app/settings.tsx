import React, { useState } from "react";
import {
  ScrollView,
  StyleSheet,
  Switch,
  Text,
  TouchableOpacity,
  View,
} from "react-native";

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

export default function SettingsScreen() {
  const [fps, setFps] = useState(5);
  const [qrSize, setQrSize] = useState(280);
  const [frostThreshold, setFrostThreshold] = useState(2);
  const [frostTotal, setFrostTotal] = useState(3);
  const [requireBiometrics, setRequireBiometrics] = useState(true);
  const [showRawData, setShowRawData] = useState(false);

  return (
    <ScrollView style={styles.container} contentContainerStyle={styles.content}>
      {/* QR Transport */}
      <Text style={styles.sectionLabel}>QR TRANSPORT</Text>
      <View style={styles.card}>
        <StepperRow
          label="Animation speed"
          value={String(fps)}
          unit=" fps"
          onInc={() => setFps((v: number) => Math.min(v + 1, 30))}
          onDec={() => setFps((v: number) => Math.max(v - 1, 1))}
        />
        <View style={styles.divider} />
        <StepperRow
          label="QR code size"
          value={String(qrSize)}
          unit=" px"
          onInc={() => setQrSize((v: number) => Math.min(v + 20, 400))}
          onDec={() => setQrSize((v: number) => Math.max(v - 20, 160))}
        />
      </View>

      {/* FROST */}
      <Text style={styles.sectionLabel}>FROST THRESHOLD</Text>
      <View style={styles.card}>
        <StepperRow
          label="Threshold (t)"
          value={String(frostThreshold)}
          onInc={() => setFrostThreshold((v: number) => Math.min(v + 1, frostTotal))}
          onDec={() => setFrostThreshold((v: number) => Math.max(v - 1, 2))}
        />
        <View style={styles.divider} />
        <StepperRow
          label="Participants (n)"
          value={String(frostTotal)}
          onInc={() => setFrostTotal((v: number) => Math.min(v + 1, 10))}
          onDec={() =>
            setFrostTotal((v: number) => {
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
  switchRow: {
    flexDirection: "row",
    alignItems: "center",
    paddingHorizontal: 16,
    paddingVertical: 14,
    gap: 12,
  },
  switchLabel: { flex: 1 },
});