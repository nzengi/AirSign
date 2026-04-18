import { Stack } from "expo-router";
import { StatusBar } from "expo-status-bar";
import { SafeAreaProvider } from "react-native-safe-area-context";
import AirplaneModeGuard from "../src/components/AirplaneModeGuard";

export default function RootLayout() {
  return (
    <SafeAreaProvider>
      <StatusBar style="light" />
      <AirplaneModeGuard>
        <Stack
          screenOptions={{
            headerStyle: { backgroundColor: "#0a0a0a" },
            headerTintColor: "#ffffff",
            headerTitleStyle: { fontWeight: "700" },
            contentStyle: { backgroundColor: "#0a0a0a" },
          }}
        >
          <Stack.Screen name="index" options={{ title: "AirSign Signer" }} />
          <Stack.Screen name="scan" options={{ title: "Scan Transaction" }} />
          <Stack.Screen name="display" options={{ title: "Show Signature" }} />
          <Stack.Screen name="keystore" options={{ title: "Key Management" }} />
          <Stack.Screen name="settings" options={{ title: "Settings" }} />
        </Stack>
      </AirplaneModeGuard>
    </SafeAreaProvider>
  );
}