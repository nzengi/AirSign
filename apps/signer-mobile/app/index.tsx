import { useRouter } from "expo-router";
import React from "react";
import {
  ScrollView,
  StyleSheet,
  Text,
  TouchableOpacity,
  View,
} from "react-native";

interface NavCardProps {
  title: string;
  description: string;
  icon: string;
  onPress: () => void;
  accent?: string;
}

function NavCard({ title, description, icon, onPress, accent = "#3b82f6" }: NavCardProps) {
  return (
    <TouchableOpacity style={[styles.card, { borderLeftColor: accent }]} onPress={onPress} activeOpacity={0.7}>
      <Text style={styles.cardIcon}>{icon}</Text>
      <View style={styles.cardText}>
        <Text style={styles.cardTitle}>{title}</Text>
        <Text style={styles.cardDesc}>{description}</Text>
      </View>
      <Text style={styles.chevron}>›</Text>
    </TouchableOpacity>
  );
}

export default function HomeScreen() {
  const router = useRouter();

  return (
    <ScrollView style={styles.container} contentContainerStyle={styles.content}>
      {/* Header */}
      <View style={styles.header}>
        <Text style={styles.logo}>🔐 AirSign</Text>
        <Text style={styles.tagline}>
          Air-gapped Solana signer · Airplane mode enforced
        </Text>
      </View>

      {/* Offline badge */}
      <View style={styles.offlineBadge}>
        <Text style={styles.offlineDot}>●</Text>
        <Text style={styles.offlineText}>Device is offline — signing enabled</Text>
      </View>

      {/* Navigation */}
      <Text style={styles.sectionLabel}>SIGN A TRANSACTION</Text>

      <NavCard
        icon="📷"
        title="Scan Transaction"
        description="Scan QR codes from the online machine to receive an unsigned transaction"
        accent="#3b82f6"
        onPress={() => router.push("/scan")}
      />

      <NavCard
        icon="📡"
        title="Show Signature"
        description="Display the signed transaction as animated QR codes for the online machine to scan"
        accent="#22c55e"
        onPress={() => router.push("/display")}
      />

      <Text style={styles.sectionLabel}>MANAGE</Text>

      <NavCard
        icon="🗝️"
        title="Key Management"
        description="Generate, import, export, or delete Ed25519 signing keypairs"
        accent="#a855f7"
        onPress={() => router.push("/keystore")}
      />

      <NavCard
        icon="📋"
        title="Signing History"
        description="Audit log of every transaction signed on this device"
        accent="#f59e0b"
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        onPress={() => router.push("/history" as any)}
      />

      <Text style={styles.sectionLabel}>ADVANCED</Text>

      <NavCard
        icon="❄️"
        title="FROST Threshold Signing"
        description="t-of-n threshold signatures via FROST RFC 9591 — Dealer, Participant & Sign"
        accent="#60a5fa"
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        onPress={() => router.push("/frost" as any)}
      />

      <NavCard
        icon="🏛️"
        title="Squads Multisig"
        description="Propose and approve Squads v4 vault transactions air-gapped"
        accent="#a78bfa"
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        onPress={() => router.push("/squads" as any)}
      />

      <NavCard
        icon="⚙️"
        title="Settings"
        description="QR frame rate, FROST threshold configuration, and more"
        accent="#6b7280"
        onPress={() => router.push("/settings")}
      />

      <Text style={styles.footer}>
        AirSign v1.0.0 · github.com/nzengi/AirSign
      </Text>
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: "#0a0a0a",
  },
  content: {
    padding: 20,
    paddingBottom: 40,
  },
  header: {
    alignItems: "center",
    paddingVertical: 32,
  },
  logo: {
    fontSize: 40,
    marginBottom: 8,
  },
  tagline: {
    color: "#6b7280",
    fontSize: 13,
    textAlign: "center",
  },
  offlineBadge: {
    flexDirection: "row",
    alignItems: "center",
    backgroundColor: "#052e16",
    borderRadius: 8,
    paddingHorizontal: 12,
    paddingVertical: 8,
    marginBottom: 24,
    gap: 8,
  },
  offlineDot: {
    color: "#22c55e",
    fontSize: 10,
  },
  offlineText: {
    color: "#22c55e",
    fontSize: 13,
    fontWeight: "600",
  },
  sectionLabel: {
    color: "#4b5563",
    fontSize: 11,
    fontWeight: "700",
    letterSpacing: 1.2,
    marginBottom: 8,
    marginTop: 4,
  },
  card: {
    backgroundColor: "#111827",
    borderRadius: 12,
    borderLeftWidth: 3,
    flexDirection: "row",
    alignItems: "center",
    padding: 16,
    marginBottom: 10,
    gap: 12,
  },
  cardIcon: {
    fontSize: 28,
    width: 36,
    textAlign: "center",
  },
  cardText: {
    flex: 1,
  },
  cardTitle: {
    color: "#f9fafb",
    fontSize: 16,
    fontWeight: "600",
    marginBottom: 4,
  },
  cardDesc: {
    color: "#6b7280",
    fontSize: 12,
    lineHeight: 18,
  },
  chevron: {
    color: "#374151",
    fontSize: 24,
    fontWeight: "300",
  },
  footer: {
    color: "#374151",
    fontSize: 11,
    textAlign: "center",
    marginTop: 32,
  },
});