/**
 * Bootstrap the AfterImage WASM module.
 * The compiled binary is served from /wasm/ (copied from crates/afterimage-wasm/pkg/).
 */

let initialised = false;
let initPromise: Promise<void> | null = null;

export async function initWasm(): Promise<void> {
  if (initialised) return;
  if (initPromise) return initPromise;

  initPromise = (async () => {
    // Dynamic import via Function constructor bypasses TS module-resolution
    // checking while still generating a native ES dynamic import at runtime.
    // The path is a public-dir URL, not a TS module — this is intentional.
    const dynamicImport = new Function("url", "return import(url)") as
      (url: string) => Promise<Record<string, unknown>>;
    const glue = await dynamicImport("/wasm/afterimage_wasm.js");

    // wasm-pack --target web generates an `init` default export
    const init = glue["default"] as (wasmUrl: string) => Promise<void>;
    await init("/wasm/afterimage_wasm_bg.wasm");

    // Expose the module on globalThis so pages can call WasmSendSession etc.
    (globalThis as Record<string, unknown>).__airsign_wasm__ = glue;
    initialised = true;
  })();

  return initPromise;
}

export function isWasmReady(): boolean {
  return initialised;
}