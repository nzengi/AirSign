/**
 * AirplaneModeGuard
 *
 * Wraps the entire app and shows a blocking overlay whenever the device has
 * any reachable network interface. The signing UI is only accessible when
 * the device is fully offline (airplane mode or all radios disabled).
 *
 * Security note (KI-010): On rooted/jailbroken devices, expo-network's
 * reachability check can be spoofed. This guard is a UX safety net for
 * honest users, not a hard security boundary.
 */
import React, { useCallback, useEffect, useState } from "react";
import {
  ActivityIndicator,
  AppState,
  AppStateStatus,
  StyleSheet,
  Text,
  TouchableOpacity,
  View,
} from "react-native";
// expo-network is imported dynamically so the module is tree-shaken on web
// (expo-network is not available in browsers).
let Network: typeof import("expo-network") | null = null;
try {
  // eslint-disable-next-line @typescript-eslint/no-var-requires
  Network = require("expo-network");
} catch {
  // running in a web / test environment — skip network check
}

type NetworkStateType = {
  isInternetReachable?: boolean | null;
  isConnected?: boolean | null;
};

interface Props {
  children: React.ReactNode;
}

export default function AirplaneModeGuard({ children }: Props) {
  const [checking, setChecking] = useState(true);
  const [online, setOnline] = useState(false);

  const checkNetwork = useCallback(async () => {
    if (!Network) {
      // No network module — assume offline (safe default)
      setOnline(false);
      setChecking(false);
      return;
    }
    try {
      const state: NetworkStateType = await Network.getNetworkStateAsync();
      const reachable =
        state.isInternetReachable === true || state.isConnected === true;
      setOnline(reachable);
    } catch {
      // Error querying network → assume offline (safe default)
      setOnline(false);
    } finally {
      setChecking(false);
    }
  }, []);

  // Check on mount and whenever the app comes back to the foreground
  useEffect(() => {
    void checkNetwork();

    const subscription = AppState.addEventListener(
      "change",
      (state: AppStateStatus) => {
        if (state === "active") {
          setChecking(true);
          void checkNetwork();
        }
      }
    );

    // Re-check every 5 seconds while the app is active
    const interval = setInterval(() => {
      void checkNetwork();
    }, 5000);

    return () => {
      subscription.remove();
      clearInterval(interval);
    };
  }, [checkNetwork]);

  if (checking) {
    return (
      <View style={styles.overlay}>
        <ActivityIndicator size="large" color="#f59e0b" />
        <Text style={styles.overlayText}>Checking network status…</Text>
      </View>
    );
  }

  if (online) {
    return (
      <View style={styles.overlay}>
        <Text style={styles.warningIcon}>✈️</Text>
        <Text style={styles.overlayTitle}>Enable Airplane Mode</Text>
        <Text style={styles.overlayText}>
          AirSign detected an active network connection.{"\n"}
          To protect your private keys, signing is blocked until{"\n"}
          the device is fully offline.
        </Text>
        <Text style={styles.instructionText}>
          1. Open Settings{"\n"}
          2. Enable Airplane Mode{"\n"}
          3. Return to AirSign
        </Text>
        <TouchableOpacity
          style={styles.retryButton}
          onPress={() => {
            setChecking(true);
            void checkNetwork();
          }}
        >
          <Text style={styles.retryText}>Re-check Network</Text>
        </TouchableOpacity>
      </View>
    );
  }

  return <>{children}</>;
}

const styles = StyleSheet.create({
  overlay: {
    flex: 1,
    backgroundColor: "#0a0a0a",
    alignItems: "center",
    justifyContent: "center",
    padding: 32,
    gap: 16,
  },
  warningIcon: {
    fontSize: 64,
    marginBottom: 8,
  },
  overlayTitle: {
    color: "#f59e0b",
    fontSize: 22,
    fontWeight: "700",
    textAlign: "center",
  },
  overlayText: {
    color: "#9ca3af",
    fontSize: 15,
    textAlign: "center",
    lineHeight: 22,
  },
  instructionText: {
    color: "#d1d5db",
    fontSize: 14,
    lineHeight: 22,
    backgroundColor: "#111827",
    padding: 16,
    borderRadius: 8,
    alignSelf: "stretch",
  },
  retryButton: {
    backgroundColor: "#1d4ed8",
    paddingHorizontal: 24,
    paddingVertical: 12,
    borderRadius: 8,
    marginTop: 8,
  },
  retryText: {
    color: "#ffffff",
    fontSize: 15,
    fontWeight: "600",
  },
});