import { CameraView, useCameraPermissions } from "expo-camera";
import React, { useEffect, useRef, useState } from "react";
import { StyleSheet, Text, TouchableOpacity, View } from "react-native";

interface Props {
  onFrame: (data: string) => void;
  /** If true the scanner is paused (e.g. while processing a frame) */
  paused?: boolean;
}

/**
 * QrScanner — wraps expo-camera's QR scanning.
 *
 * Each unique QR payload is emitted once via `onFrame`. The caller is
 * responsible for deduplication at the fountain-decoder level.
 */
export default function QrScanner({ onFrame, paused = false }: Props) {
  const [permission, requestPermission] = useCameraPermissions();
  const lastData = useRef<string | null>(null);
  const [torchOn, setTorchOn] = useState(false);

  useEffect(() => {
    if (!permission?.granted) {
      void requestPermission();
    }
  }, [permission, requestPermission]);

  if (!permission) {
    return (
      <View style={styles.center}>
        <Text style={styles.dimText}>Requesting camera permission…</Text>
      </View>
    );
  }

  if (!permission.granted) {
    return (
      <View style={styles.center}>
        <Text style={styles.errorText}>Camera permission denied.</Text>
        <TouchableOpacity style={styles.button} onPress={requestPermission}>
          <Text style={styles.buttonText}>Grant Permission</Text>
        </TouchableOpacity>
      </View>
    );
  }

  return (
    <View style={styles.container}>
      <CameraView
        style={StyleSheet.absoluteFill}
        facing="back"
        enableTorch={torchOn}
        barcodeScannerSettings={{ barcodeTypes: ["qr"] }}
        onBarcodeScanned={
          paused
            ? undefined
            : ({ data }: { data: string }) => {
                // Deduplicate: only emit if this frame differs from the last
                if (data !== lastData.current) {
                  lastData.current = data;
                  onFrame(data);
                }
              }
        }
      />

      {/* Viewfinder overlay */}
      <View style={styles.overlay} pointerEvents="none">
        <View style={styles.cutout} />
      </View>

      {/* Torch toggle */}
      <TouchableOpacity
        style={styles.torchButton}
        onPress={() => setTorchOn((v: boolean) => !v)}
      >
        <Text style={styles.torchText}>{torchOn ? "🔦 Off" : "🔦 On"}</Text>
      </TouchableOpacity>

      {paused && (
        <View style={styles.pausedBanner}>
          <Text style={styles.pausedText}>Processing…</Text>
        </View>
      )}
    </View>
  );
}

const CUTOUT = 260;

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: "#000",
  },
  center: {
    flex: 1,
    alignItems: "center",
    justifyContent: "center",
    backgroundColor: "#0a0a0a",
    gap: 12,
  },
  dimText: {
    color: "#6b7280",
    fontSize: 15,
  },
  errorText: {
    color: "#ef4444",
    fontSize: 15,
  },
  button: {
    backgroundColor: "#1d4ed8",
    paddingHorizontal: 20,
    paddingVertical: 10,
    borderRadius: 8,
  },
  buttonText: {
    color: "#fff",
    fontWeight: "600",
  },
  overlay: {
    ...StyleSheet.absoluteFillObject,
    alignItems: "center",
    justifyContent: "center",
    backgroundColor: "rgba(0,0,0,0.5)",
  },
  cutout: {
    width: CUTOUT,
    height: CUTOUT,
    borderWidth: 2,
    borderColor: "#3b82f6",
    borderRadius: 12,
    backgroundColor: "transparent",
    // "punch a hole" — the overlay bg is behind this, so we need the shadow
    shadowColor: "#3b82f6",
    shadowOpacity: 0.8,
    shadowRadius: 8,
    shadowOffset: { width: 0, height: 0 },
  },
  torchButton: {
    position: "absolute",
    bottom: 40,
    right: 24,
    backgroundColor: "rgba(0,0,0,0.6)",
    paddingHorizontal: 16,
    paddingVertical: 8,
    borderRadius: 20,
  },
  torchText: {
    color: "#fff",
    fontSize: 14,
  },
  pausedBanner: {
    position: "absolute",
    top: 0,
    left: 0,
    right: 0,
    backgroundColor: "#1d4ed8cc",
    paddingVertical: 8,
    alignItems: "center",
  },
  pausedText: {
    color: "#fff",
    fontWeight: "600",
  },
});