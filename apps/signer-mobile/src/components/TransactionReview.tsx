import React from "react";
import {
  ScrollView,
  StyleSheet,
  Text,
  TouchableOpacity,
  View,
} from "react-native";

export interface InstructionSummary {
  program: string;
  name: string;
  /** Risk flags emitted by the Transaction Inspector */
  flags: string[];
  accounts: { label: string; pubkey: string; signer: boolean; writable: boolean }[];
  dataHex?: string;
}

export interface TransactionSummary {
  /** Base58 fee payer pubkey */
  feePayer: string;
  /** Estimated network fee in lamports */
  feeLamports?: number;
  recentBlockhash: string;
  instructions: InstructionSummary[];
  /** Overall risk level computed by the inspector */
  riskLevel: "safe" | "warn" | "critical";
}

interface Props {
  tx: TransactionSummary;
  onApprove: () => void;
  onReject: () => void;
  approving?: boolean;
}

const RISK_COLOR: Record<TransactionSummary["riskLevel"], string> = {
  safe: "#22c55e",
  warn: "#f59e0b",
  critical: "#ef4444",
};

const RISK_LABEL: Record<TransactionSummary["riskLevel"], string> = {
  safe: "✅ Safe",
  warn: "⚠️  Review Carefully",
  critical: "🚨 High Risk",
};

export default function TransactionReview({
  tx,
  onApprove,
  onReject,
  approving = false,
}: Props) {
  const riskColor = RISK_COLOR[tx.riskLevel];

  return (
    <View style={styles.container}>
      {/* Risk banner */}
      <View style={[styles.riskBanner, { borderColor: riskColor }]}>
        <Text style={[styles.riskLabel, { color: riskColor }]}>
          {RISK_LABEL[tx.riskLevel]}
        </Text>
      </View>

      <ScrollView style={styles.scroll} contentContainerStyle={styles.scrollContent}>
        {/* Fee payer */}
        <Section title="Fee Payer">
          <Mono>{tx.feePayer}</Mono>
          {tx.feeLamports !== undefined && (
            <Text style={styles.dimText}>
              Fee: {(tx.feeLamports / 1e9).toFixed(6)} SOL
            </Text>
          )}
        </Section>

        {/* Recent blockhash */}
        <Section title="Recent Blockhash">
          <Mono>{tx.recentBlockhash}</Mono>
        </Section>

        {/* Instructions */}
        {tx.instructions.map((ix, i) => (
          <Section key={i} title={`Instruction ${i + 1}: ${ix.name}`}>
            <Text style={styles.dimText}>Program: {ix.program}</Text>

            {/* Risk flags */}
            {ix.flags.length > 0 && (
              <View style={styles.flagsBox}>
                {ix.flags.map((f, fi) => (
                  <Text key={fi} style={styles.flagText}>
                    ⚠️ {f}
                  </Text>
                ))}
              </View>
            )}

            {/* Accounts */}
            {ix.accounts.map((acc, ai) => (
              <View key={ai} style={styles.accountRow}>
                <Text style={styles.accountLabel}>{acc.label}</Text>
                <Mono>{acc.pubkey}</Mono>
                <View style={styles.badges}>
                  {acc.signer && <Badge text="signer" color="#3b82f6" />}
                  {acc.writable && <Badge text="writable" color="#f59e0b" />}
                </View>
              </View>
            ))}

            {/* Instruction data */}
            {ix.dataHex && (
              <View style={styles.dataBox}>
                <Text style={styles.dimText}>Data (hex):</Text>
                <Mono>{ix.dataHex}</Mono>
              </View>
            )}
          </Section>
        ))}
      </ScrollView>

      {/* Action buttons */}
      <View style={styles.actions}>
        <TouchableOpacity
          style={[styles.actionBtn, styles.rejectBtn]}
          onPress={onReject}
          disabled={approving}
        >
          <Text style={styles.rejectText}>Reject</Text>
        </TouchableOpacity>

        <TouchableOpacity
          style={[
            styles.actionBtn,
            styles.approveBtn,
            tx.riskLevel === "critical" && styles.approveBtnCritical,
            approving && styles.approveBtnDisabled,
          ]}
          onPress={onApprove}
          disabled={approving}
        >
          <Text style={styles.approveText}>
            {approving ? "Signing…" : tx.riskLevel === "critical" ? "Sign Anyway" : "Sign"}
          </Text>
        </TouchableOpacity>
      </View>
    </View>
  );
}

// ── Sub-components ──────────────────────────────────────────────────────────

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <View style={sectionStyles.container}>
      <Text style={sectionStyles.title}>{title}</Text>
      {children}
    </View>
  );
}

function Mono({ children }: { children: string }) {
  return <Text style={monoStyles.text} numberOfLines={2} ellipsizeMode="middle">{children}</Text>;
}

function Badge({ text, color }: { text: string; color: string }) {
  return (
    <View style={[badgeStyles.badge, { borderColor: color }]}>
      <Text style={[badgeStyles.text, { color }]}>{text}</Text>
    </View>
  );
}

// ── Styles ───────────────────────────────────────────────────────────────────

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: "#0a0a0a",
  },
  riskBanner: {
    borderWidth: 1,
    borderRadius: 8,
    margin: 16,
    padding: 12,
    alignItems: "center",
  },
  riskLabel: {
    fontSize: 16,
    fontWeight: "700",
  },
  scroll: {
    flex: 1,
  },
  scrollContent: {
    paddingBottom: 16,
  },
  dimText: {
    color: "#6b7280",
    fontSize: 12,
    marginTop: 4,
  },
  flagsBox: {
    backgroundColor: "#1c1410",
    borderRadius: 6,
    padding: 8,
    marginTop: 6,
    gap: 4,
  },
  flagText: {
    color: "#f59e0b",
    fontSize: 12,
  },
  accountRow: {
    marginTop: 8,
    paddingTop: 8,
    borderTopWidth: StyleSheet.hairlineWidth,
    borderTopColor: "#1f2937",
  },
  accountLabel: {
    color: "#9ca3af",
    fontSize: 11,
    marginBottom: 2,
  },
  badges: {
    flexDirection: "row",
    gap: 6,
    marginTop: 4,
  },
  dataBox: {
    marginTop: 8,
    backgroundColor: "#111827",
    borderRadius: 6,
    padding: 8,
  },
  actions: {
    flexDirection: "row",
    gap: 12,
    padding: 16,
    paddingBottom: 32,
    borderTopWidth: StyleSheet.hairlineWidth,
    borderTopColor: "#1f2937",
  },
  actionBtn: {
    flex: 1,
    paddingVertical: 14,
    borderRadius: 10,
    alignItems: "center",
  },
  rejectBtn: {
    backgroundColor: "#1f2937",
  },
  rejectText: {
    color: "#e5e7eb",
    fontSize: 16,
    fontWeight: "600",
  },
  approveBtn: {
    backgroundColor: "#16a34a",
  },
  approveBtnCritical: {
    backgroundColor: "#b91c1c",
  },
  approveBtnDisabled: {
    opacity: 0.5,
  },
  approveText: {
    color: "#ffffff",
    fontSize: 16,
    fontWeight: "700",
  },
});

const sectionStyles = StyleSheet.create({
  container: {
    marginHorizontal: 16,
    marginTop: 12,
    backgroundColor: "#111827",
    borderRadius: 10,
    padding: 12,
  },
  title: {
    color: "#e5e7eb",
    fontSize: 13,
    fontWeight: "600",
    marginBottom: 6,
    textTransform: "uppercase",
    letterSpacing: 0.5,
  },
});

const monoStyles = StyleSheet.create({
  text: {
    fontFamily: "monospace",
    fontSize: 11,
    color: "#d1d5db",
  },
});

const badgeStyles = StyleSheet.create({
  badge: {
    borderWidth: 1,
    borderRadius: 4,
    paddingHorizontal: 6,
    paddingVertical: 2,
  },
  text: {
    fontSize: 10,
    fontWeight: "600",
  },
});