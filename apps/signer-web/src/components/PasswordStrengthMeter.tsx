/**
 * Inline password-strength meter.
 *
 * Reads entropy + categorical strength from the WASM backend (Q3 hardening)
 * and renders a thin colored bar with a numeric bits readout. Designed to
 * sit directly underneath the shared-password input on every page that
 * touches signing key material.
 */

import { useMemo } from "react";
import { assessPassword, type PasswordStrength } from "../lib/wasm-api.js";

interface Props {
  password: string;
  /** Render even when password is empty. Default: hide when empty. */
  alwaysVisible?: boolean;
  className?: string;
}

const COLORS: Record<PasswordStrength, { bar: string; text: string; border: string }> = {
  weak:     { bar: "var(--danger)",  text: "var(--danger)",  border: "rgba(234,84,85,0.35)" },
  medium:   { bar: "var(--warn)",    text: "var(--warn)",    border: "rgba(217,119,6,0.38)" },
  strong:   { bar: "var(--accent2)", text: "var(--accent2)", border: "rgba(0,43,91,0.30)" },
  paranoid: { bar: "var(--accent)",  text: "var(--accent)",  border: "rgba(234,84,85,0.40)" },
};

const STRENGTH_NOTE: Record<PasswordStrength, string> = {
  weak: "brute-forceable in days — extend the password before signing anything real",
  medium: "OK for devnet/demo · upgrade for mainnet",
  strong: "mainnet-ready under default Argon2id (~317 single-core-years to brute force)",
  paranoid: "survives even reduced KDF parameters",
};

export function PasswordStrengthMeter({ password, alwaysVisible, className }: Props) {
  const assessment = useMemo(() => assessPassword(password), [password]);
  if (!password && !alwaysVisible) return null;

  const { bits, strength, mainnetReady } = assessment;
  const fillPct = Math.min(100, Math.round((bits / 100) * 100));
  const c = COLORS[strength];

  return (
    <div className={`pw-meter${className ? " " + className : ""}`}>
      <div className="pw-meter-row">
        <span className="pw-meter-label" style={{ color: c.text, borderColor: c.border }}>
          {strength}
        </span>
        <span className="pw-meter-bits">≈ {bits.toFixed(1)} bits</span>
        <span className="pw-meter-note">
          {mainnetReady ? "✓ mainnet-ready" : "⚠ not mainnet-ready"}
        </span>
      </div>
      <div className="pw-meter-track">
        <div
          className="pw-meter-fill"
          style={{ width: `${fillPct}%`, background: c.bar }}
        />
      </div>
      <p className="pw-meter-hint">{STRENGTH_NOTE[strength]}</p>
    </div>
  );
}
