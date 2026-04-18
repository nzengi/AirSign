import { useRouter } from "expo-router";
import React, { useEffect, useState } from "react";
import { StyleSheet, Text, TouchableOpacity, View } from "react-native";
import QrAnimator from "../src/components/QrAnimator";

/**
 * Stub: fountain-encode signed bytes into base64 QR frames.
 * Real implementation calls airsign_core.fountain_encode(signedBytes).
 */
function stubFountainFrames(): string[] {
  // Return a small set of placeholder frames so the animator is exercisable
  // without the native module.
  const payload = btoa(
    JSON.stringify({ sig: "PLACEHOLDER_SIGNATURE_BASE64", version: 1 })
  );
  return Array.from({ length: 8 }, (_, i) =>
    btoa(JSON.stringify({ idx: i, total: 8, data: payload }))
  );
}

export default function DisplayScreen() {
  const router = useRouter();
  const [frames, setFrames] = useState<string[]>([]);
  const [fps, setFps] = useState(5);

  useEffect(() => {
    setFrames(stubFountainFrames());
  }, []);

  return (
    <View style={styles.container}>
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
            <Text style={styles.placeholderText}>Preparing frames…</Text>
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

      <Text style={styles.frameCount}>{frames.length} fountain frames</Text>

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
  },
  placeholderText: {
    color: "#4b5563",
    fontSize: 14,
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