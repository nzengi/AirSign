/**
 * TransactionReview — renders a human-readable summary of a Solana transaction
 * together with risk flags produced by the AfterImage inspector.
 *
 * @example
 * ```tsx
 * <TransactionReview
 *   summary={inspectorOutput}
 *   showFields
 * />
 * ```
 */

import React from "react";
import type { TransactionReviewProps, RiskFlag, InstructionInfo } from "../types.js";

// ─── Helpers ──────────────────────────────────────────────────────────────────

function severityColor(sev: RiskFlag["severity"]): string {
  switch (sev) {
    case "HIGH":
      return "#ef4444";
    case "MEDIUM":
      return "#f97316";
    case "LOW":
      return "#eab308";
  }
}

function severityBg(sev: RiskFlag["severity"]): string {
  switch (sev) {
    case "HIGH":
      return "#fef2f2";
    case "MEDIUM":
      return "#fff7ed";
    case "LOW":
      return "#fefce8";
  }
}

function RiskBadge({ flag }: { flag: RiskFlag }) {
  return (
    <li
      style={{
        display: "flex",
        alignItems: "flex-start",
        gap: "0.5rem",
        padding: "0.5rem 0.75rem",
        borderRadius: "6px",
        background: severityBg(flag.severity),
        border: `1px solid ${severityColor(flag.severity)}30`,
        listStyle: "none",
      }}
    >
      <span
        style={{
          fontWeight: 700,
          fontSize: "0.65rem",
          textTransform: "uppercase",
          letterSpacing: "0.05em",
          color: severityColor(flag.severity),
          marginTop: "0.1rem",
          flexShrink: 0,
        }}
      >
        {flag.severity}
      </span>
      <span style={{ fontSize: "0.8rem", color: "#1e293b", lineHeight: 1.4 }}>
        {flag.message}
        {flag.code && (
          <code
            style={{
              marginLeft: "0.4rem",
              fontSize: "0.7rem",
              color: "#64748b",
              background: "#f1f5f9",
              padding: "0 0.25rem",
              borderRadius: "3px",
            }}
          >
            {flag.code}
          </code>
        )}
      </span>
    </li>
  );
}

function InstructionRow({
  ix,
  showFields,
}: {
  ix: InstructionInfo;
  showFields: boolean;
}) {
  return (
    <li
      style={{
        padding: "0.625rem 0.75rem",
        borderRadius: "6px",
        background: "#f8fafc",
        border: "1px solid #e2e8f0",
        listStyle: "none",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "baseline",
          gap: "0.5rem",
          flexWrap: "wrap",
        }}
      >
        <span
          style={{
            fontSize: "0.7rem",
            fontWeight: 600,
            textTransform: "uppercase",
            letterSpacing: "0.04em",
            color: "#6366f1",
            flexShrink: 0,
          }}
        >
          {ix.kind}
        </span>
        <span style={{ fontSize: "0.8rem", color: "#334155" }}>
          {ix.summary}
        </span>
      </div>

      {showFields && Object.keys(ix.fields).length > 0 && (
        <table
          style={{
            marginTop: "0.5rem",
            width: "100%",
            borderCollapse: "collapse",
            fontSize: "0.7rem",
            color: "#475569",
          }}
        >
          <tbody>
            {Object.entries(ix.fields).map(([k, v]) => (
              <tr key={k}>
                <td
                  style={{
                    paddingRight: "0.75rem",
                    fontWeight: 600,
                    whiteSpace: "nowrap",
                    verticalAlign: "top",
                    paddingBottom: "2px",
                  }}
                >
                  {k}
                </td>
                <td
                  style={{
                    fontFamily: "monospace",
                    wordBreak: "break-all",
                    verticalAlign: "top",
                    paddingBottom: "2px",
                  }}
                >
                  {v}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </li>
  );
}

// ─── Main component ───────────────────────────────────────────────────────────

export function TransactionReview({
  summary,
  showFields = false,
  className,
}: TransactionReviewProps) {
  const { instructions, riskFlags, hasHighRisk, signatureCount } = summary;

  return (
    <div
      className={className}
      style={{
        display: "flex",
        flexDirection: "column",
        gap: "1rem",
        fontFamily:
          '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
      }}
    >
      {/* Header */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: "0.75rem",
          flexWrap: "wrap",
        }}
      >
        <h3
          style={{
            margin: 0,
            fontSize: "1rem",
            fontWeight: 600,
            color: "#0f172a",
          }}
        >
          Transaction Review
        </h3>

        {/* Signature status */}
        <span
          style={{
            fontSize: "0.7rem",
            fontWeight: 600,
            padding: "0.15rem 0.5rem",
            borderRadius: "999px",
            background: signatureCount === 0 ? "#fef9c3" : "#dcfce7",
            color: signatureCount === 0 ? "#92400e" : "#166534",
            border: `1px solid ${signatureCount === 0 ? "#fde68a" : "#bbf7d0"}`,
          }}
        >
          {signatureCount === 0
            ? "Unsigned"
            : `${signatureCount} signature${signatureCount !== 1 ? "s" : ""}`}
        </span>

        {/* High-risk badge */}
        {hasHighRisk && (
          <span
            style={{
              fontSize: "0.7rem",
              fontWeight: 700,
              padding: "0.15rem 0.5rem",
              borderRadius: "999px",
              background: "#fef2f2",
              color: "#991b1b",
              border: "1px solid #fca5a5",
            }}
          >
            ⚠ High Risk
          </span>
        )}
      </div>

      {/* Risk flags */}
      {riskFlags.length > 0 && (
        <section aria-label="Risk flags">
          <p
            style={{
              margin: "0 0 0.4rem",
              fontSize: "0.75rem",
              fontWeight: 600,
              color: "#64748b",
              textTransform: "uppercase",
              letterSpacing: "0.05em",
            }}
          >
            Risk Flags
          </p>
          <ul style={{ margin: 0, padding: 0, display: "flex", flexDirection: "column", gap: "0.4rem" }}>
            {riskFlags.map((f, i) => (
              <RiskBadge key={`${f.code}-${i}`} flag={f} />
            ))}
          </ul>
        </section>
      )}

      {/* Instructions */}
      <section aria-label="Instructions">
        <p
          style={{
            margin: "0 0 0.4rem",
            fontSize: "0.75rem",
            fontWeight: 600,
            color: "#64748b",
            textTransform: "uppercase",
            letterSpacing: "0.05em",
          }}
        >
          Instructions ({instructions.length})
        </p>
        <ul
          style={{
            margin: 0,
            padding: 0,
            display: "flex",
            flexDirection: "column",
            gap: "0.4rem",
          }}
        >
          {instructions.map((ix, i) => (
            <InstructionRow
              key={`${ix.kind}-${i}`}
              ix={ix}
              showFields={showFields}
            />
          ))}
        </ul>
      </section>
    </div>
  );
}

TransactionReview.displayName = "TransactionReview";