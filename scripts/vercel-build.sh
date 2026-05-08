#!/usr/bin/env bash
# Vercel build pipeline for the signer-web demo.
#
# Vercel build images do not ship with a Rust toolchain, so we install rustup,
# add the wasm32-unknown-unknown target, install wasm-pack, build the WASM
# bindings into apps/signer-web/src/wasm-pkg/ (where signer-web's wasm.ts
# imports them from), and finally run the existing tsc + vite build.
set -euo pipefail

log() { printf '\n\033[1;36m[vercel-build]\033[0m %s\n' "$*"; }

# ── 1. Rust toolchain (cached only on warm builds; cold installs ~30 s) ──────
if ! command -v rustup >/dev/null 2>&1; then
  log "installing rustup (stable, minimal profile)"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --default-toolchain stable --profile minimal --no-modify-path
fi
export PATH="$HOME/.cargo/bin:$PATH"

log "ensuring wasm32-unknown-unknown target is installed"
rustup target add wasm32-unknown-unknown

# ── 2. wasm-pack (built binary, no compile cost) ─────────────────────────────
if ! command -v wasm-pack >/dev/null 2>&1; then
  log "installing wasm-pack"
  curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
fi

# ── 3. Build the WASM bindings into the path signer-web expects ──────────────
log "building afterimage-wasm (release, target=web)"
wasm-pack build crates/afterimage-wasm \
  --release \
  --target web \
  --out-dir ../../apps/signer-web/src/wasm-pkg

# ── 4. Run the signer-web build (tsc --noEmit && vite build) ─────────────────
log "building signer-web (tsc + vite)"
npm run build --workspace=apps/signer-web

log "done"
