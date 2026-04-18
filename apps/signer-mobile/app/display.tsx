import { useLocalSearchParams, useRouter } from "expo-router";
import React, { useEffect, useState } from "react";
import { StyleSheet, Text, TouchableOpacity, View } from "react-native";
import QrAnimator from "../src/components/QrAnimator";
import AirSignCore from "../src/native/AirSignCore";

export default function DisplayScreen() {
  const router = useRouter();
  const params = useLocalSearchParams<{ framesJson?: string; signedPayload?: string; cluster?: string }>();
  const cluster = params.cluster ?? "devnet";
  const [frames, setFrames] = useState<string[]>([]);
  const [fps, setFps] = useState(5);
  const [status, setStatus] = useState("Preparing frames…");

  useEffect(() => {
    void (async () => {
      // Priority 1: frames already computed by scan.tsx and passed as JSON
      if (params.framesJson) {
        try {
          const parsed = JSON.parse(params.framesJson) as string[];
          if (Array.isArray(parsed) && parsed.length > 0) {
            setFrames(parsed);
            setStatus("");
            return;
          }
        } catch {
          // fall through
        }
      }

      // Priority 2: raw signed payload passed — encode it here
      if (params.signedPayload) {
        try {
          setStatus("Encoding fountain frames…");
          const result = await AirSignCore.fountainEncode(params.signedPayload, 16);
          const enc = result as { frames?: string[] };
          const computed = enc.frames ?? [params.signedPayload];
          setFrames(computed);
          setStatus("");
          return;
        } catch (err) {
          setStatus(`Encode error: ${String(err)}`);
          return;
        }
      }

      // Fallback: no data — show error
      setStatus("No signed transaction data received.");
    })();
  }, [params.framesJson, params.signedPayload]);

  const clusterColor =
    cluster === "mainnet-beta" ? "#ef4444" : cluster === "testnet" ? "#f59e0b" : "#3b82f6";
  const clusterLabel =
    cluster === "mainnet-beta" ? "Mainnet" : cluster === "testnet" ? "Testnet" : "Devnet";

  return (
    <View style={styles.container}>
      {/* Cluster badge */}
      <View style={[styles.clusterBadge, { borderColor: clusterColor }]}>
        <View style={[styles.clusterDot, { backgroundColor: clusterColor }]} />
        <Text style={[styles.clusterText, { color: clusterColor }]}>{clusterLabel}</Text>
      </View>

      <Text style={styles.title}>Scan with the online machine</Text>
      <Text style={styles.subtitle}>
        Hold this screen towards the camera on your online computer.{"\n"}
        The online machine will scan all frames automatically.
      </Text>

      <View style={styles.animatorWrapper}>
        {frames.length > 0 ? (
          <QrAnimator frames={frames} frameIntervalMs={Math.round(1000 / fps)} size={300} />
        ) : (
          <View style={styles.placeholder}>
            <Text style={styles.placeholderText}>{status || "Preparing…"}</Text>
          </View>
        )}
      </View>

      {/* Frame rate controls */}
      <View style={styles.fpsRow}>
        <Text style={styles.fpsLabel}>Speed: {fps} fps</Text>
        <View style={styles.fpsButtons}>
          {[2, 5, 10, 15].map((f) => (
            <TouchableOpacity
              key={f}
              style={[styles.fpsBtn, fps === f && styles.fpsBtnActive]}
              onPress={() => setFps(f)}
            >
              <Text style={[styles.fpsBtnText, fps === f && styles.fpsBtnTextActive]}>
                {f}
              </Text>
            </TouchableOpacity>
          ))}
        </View>
      </View>

      {frames.length > 0 && (
        <Text style={styles.frameCount}>{frames.length} fountain frames</Text>
      )}

      <TouchableOpacity style={styles.doneButton} onPress={() => router.replace("/")}>
        <Text style={styles.doneText}>Done — Return Home</Text>
      </TouchableOpacity>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: "#0a0a0a",
    alignItems: "center",
    padding: 20,
    paddingTop: 32,
  },
  title: {
    color: "#f9fafb",
    fontSize: 18,
    fontWeight: "700",
    textAlign: "center",
    marginBottom: 8,
  },
  subtitle: {
    color: "#6b7280",
    fontSize: 13,
    textAlign: "center",
    lineHeight: 20,
    marginBottom: 24,
  },
  animatorWrapper: {
    marginBottom: 24,
  },
  placeholder: {
    width: 300,
    height: 300,
    backgroundColor: "#111827",
    borderRadius: 8,
    alignItems: "center",
    justifyContent: "center",
    padding: 16,
  },
  placeholderText: {
    color: "#4b5563",
    fontSize: 14,
    textAlign: "center",
  },
  fpsRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: 12,
    marginBottom: 8,
  },
  fpsLabel: {
    color: "#9ca3af",
    fontSize: 13,
    width: 80,
  },
  fpsButtons: {
    flexDirection: "row",
    gap: 8,
  },
  fpsBtn: {
    borderWidth: 1,
    borderColor: "#374151",
    borderRadius: 6,
    paddingHorizontal: 12,
    paddingVertical: 6,
  },
  fpsBtnActive: {
    borderColor: "#3b82f6",
    backgroundColor: "#1e3a5f",
  },
  fpsBtnText: {
    color: "#6b7280",
    fontSize: 13,
  },
  fpsBtnTextActive: {
    color: "#60a5fa",
    fontWeight: "600",
  },
  clusterBadge: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
    borderWidth: 1,
    borderRadius: 20,
    paddingHorizontal: 12,
    paddingVertical: 4,
    marginBottom: 16,
  },
  clusterDot: { width: 7, height: 7, borderRadius: 4 },
  clusterText: { fontSize: 12, fontWeight: "700", letterSpacing: 0.5 },
  frameCount: {
    color: "#374151",
    fontSize: 11,
    marginBottom: 32,
  },
  doneButton: {
    backgroundColor: "#1f2937",
    paddingHorizontal: 32,
    paddingVertical: 14,
    borderRadius: 10,
    width: "100%",
    alignItems: "center",
  },
  doneText: {
    color: "#e5e7eb",
    fontSize: 16,
    fontWeight: "600",
  },
});