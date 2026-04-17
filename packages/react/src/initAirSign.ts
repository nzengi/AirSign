/**
 * initAirSign — initialises the AfterImage WASM module and stores it on
 * `globalThis.__airsign_wasm__` so that hooks and components can pick it up
 * without requiring every consumer to pass the module around manually.
 *
 * Call this **once** at application bootstrap, before rendering any AirSign
 * components:
 *
 * ```ts
 * // app/layout.tsx  (Next.js app router) or  main.tsx  (Vite)
 * import { initAirSign } from "@airsign/react";
 *
 * await initAirSign();
 * ```
 *
 * The function is idempotent — subsequent calls return the already-loaded
 * module immediately.
 */

import type { AirSignWasm } from "./types.js";

declare global {
  // eslint-disable-next-line no-var
  var __airsign_wasm__: AirSignWasm | undefined;
}

let initPromise: Promise<AirSignWasm> | null = null;

/**
 * Load and initialise the AirSign WASM module.
 *
 * @param wasmUrl - Optional URL to the `.wasm` binary.  When omitted the
 *   bundler-default path is used (works with Vite, Next.js, webpack 5+).
 */
export async function initAirSign(wasmUrl?: string): Promise<AirSignWasm> {
  // Already loaded — return cached module.
  if (globalThis.__airsign_wasm__) {
    return globalThis.__airsign_wasm__;
  }

  // Deduplicate concurrent calls.
  if (initPromise) {
    return initPromise;
  }

  initPromise = (async () => {
    // Dynamic import keeps this SSR-safe.  The `@airsign/wasm` package is the
    // wasm-pack bundler output — it may not have TypeScript declarations in the
    // monorepo until after `wasm-pack build` is run, so we use a string
    // expression to prevent tsc from resolving the module at build time.
    const wasmPkg = "@airsign/wasm";
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const mod: any = await import(/* @vite-ignore */ wasmPkg);

    // The wasm-pack bundler target exports a default `init` function.
    const init = mod.default as (url?: string) => Promise<void>;
    await init(wasmUrl);

    const wasm = mod as unknown as AirSignWasm;
    globalThis.__airsign_wasm__ = wasm;
    return wasm;
  })();

  return initPromise;
}

/**
 * Returns `true` if the WASM module has already been loaded.
 */
export function isAirSignReady(): boolean {
  return !!globalThis.__airsign_wasm__;
}

/**
 * Return the cached WASM module, or `null` if not yet initialised.
 */
export function getAirSignWasm(): AirSignWasm | null {
  return globalThis.__airsign_wasm__ ?? null;
}