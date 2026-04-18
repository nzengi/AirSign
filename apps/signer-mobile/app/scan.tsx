import { useRouter } from "expo-router";
import React, { useCallback, useState } from "react";
import { StyleSheet, Text, View } from "react-native";
import QrScanner from "../src/components/QrScanner";
import TransactionReview, {
  TransactionSummary,
} from "../src/components/TransactionReview";

type Phase = "scanning" | "reviewing" | "signing" | "done";

/** Stub: parse fountain frame and accumulate until complete */
function accumulateFrame(
  _frames: string[],
  frame: string
): { frames: string[]; complete: boolean; payload?: string } {
  const frames = [..._frames, frame];
  // Real implementation delegates to the airsign-core native module.
  // For the scaffold we treat any single frame that is valid base64 as complete.
  try {
    atob(frame.replace(/^data:[^;]+;base64,/, ""));
    return { frames, complete: true, payload: frame };
  } catch {
    return { frames, complete: false };
  }
}

/** Stub: parse the decoded payload into a TransactionSummary */
function parseTransactionPayload(payload: string): TransactionSummary {
  // Real implementation calls airsign_core.inspect_transaction(payload).
  // Scaffold returns a dummy summary so the UI is exercisable.
  void payload;
  return {
    feePayer: "11111111111111111111111111111111",
    feeLamports: 5000,
    recentBlockhash: "4vJ9JU1bJJE96FWSJKvHsmmFADCg4gpZQff4P3bkLKi",
    riskLevel: "warn",
    instructions: [
      {
        program: "11111111111111111111111111111111",
        name: "SystemProgram::Transfer",
        flags: ["Transfers SOL — verify recipient address"],
        accounts: [
          {
            label: "from",
            pubkey: "11111111111111111111111111111111",
            signer: true,
            writable: true,
          },
          {
            label: "to",
            pubkey: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            signer: false,
            writable: true,
          },
        ],
        dataHex: "02000000404b4c00000000",
      },
    ],
  };
}

export default function ScanScreen() {
  const router = useRouter();
  const [phase, setPhase] = useState<Phase>("scanning");
  const [frames, setFrames] = useState<string[]>([]);
  const [progress, setProgress] = useState(0);
  const [tx, setTx] = useState<TransactionSummary | null>(null);

  const handleFrame = useCallback(
    (data: string) => {
      const result = accumulateFrame(frames, data);
      setFrames(result.frames);
      setProgress(Math.min(result.frames.length / 10, 1)); // rough estimate

      if (result.complete && result.payload) {
        const summary = parseTransactionPayload(result.payload);
        setTx(summary);
        setPhase("reviewing");
      }
    },
    [frames]
  );

  const handleApprove = useCallback(async () => {
    if (!tx) return;
    setPhase("signing");
    // Real: call airsign_core.sign(payload, keyId) → signed bytes → fountain encode
    await new Promise((r) => setTimeout(r, 800)); // simulate signing delay
    setPhase("done");
    router.replace("/display");
  }, [tx, router]);

  const handleReject = useCallback(() => {
    router.back();
  }, [router]);

  if (phase === "reviewing" && tx) {
    return (
      <TransactionReview
        tx={tx}
        onApprove={() => void handleApprove()}
        onReject={handleReject}
        approving={phase === "signing"}
      />
    );
  }

  return (
    <View style={styles.container}>
      {/* Progress bar */}
      {frames.length > 0 && (
        <View style={styles.progressBar}>
          <View style={[styles.progressFill, { width: `${progress * 100}%` }]} />
        </View>
      )}

      <QrScanner onFrame={handleFrame} paused={phase !== "scanning"} />

      <View style={styles.hint}>
        <Text style={styles.hintText}>
          {frames.length === 0
            ? "Point camera at the animated QR on the online machine"
            : `Received ${frames.length} frame${frames.length > 1 ? "s" : ""}…`}
        </Text>
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: "#000" },
  progressBar: {
    height: 3,
    backgroundColor: "#1f2937",
  },
  progressFill: {
    height: 3,
    backgroundColor: "#3b82f6",
  },
  hint: {
    backgroundColor: "#0a0a0acc",
    padding: 12,
    alignItems: "center",
  },
  hintText: {
    color: "#9ca3af",
    fontSize: 13,
  },
});