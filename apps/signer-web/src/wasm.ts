/// <reference types="vite/client" />
/**
 * Bootstrap the AfterImage WASM module.
 *
 * The wasm-pack `--target web` artifacts live under `src/wasm-pkg/` so Vite
 * treats them as regular module/asset imports. This is the canonical pattern
 * for wasm-bindgen + Vite — works in dev (no `/public/` import error) and
 * in production (Vercel, Netlify, GitHub Pages sub-paths) without code
 * changes. No `Function` constructor, no eval, CSP-clean.
 *
 * The .wasm binary is referenced via `?url` so Vite returns its served URL
 * rather than inlining it; we pass that URL to wasm-bindgen's init().
 */

// Static import of the wasm-bindgen JS glue (regular ES module).
// eslint-disable-next-line import/no-unresolved
import init from "./wasm-pkg/afterimage_wasm.js";
// Static import of the .wasm binary URL via Vite's `?url` suffix.
// eslint-disable-next-line import/no-unresolved
import wasmBinaryUrl from "./wasm-pkg/afterimage_wasm_bg.wasm?url";
// Re-import the same module's namespace so we can stash it on globalThis
// for legacy pages that still read `(globalThis as any).__airsign_wasm__`.
// eslint-disable-next-line import/no-unresolved
import * as wasmGlue from "./wasm-pkg/afterimage_wasm.js";

let initialised = false;
let initPromise: Promise<void> | null = null;
let lastWasmUrl: string | null = null;

export interface WasmInitFailure {
  url: string;
  fetchStatus: number | null;
  fetchContentType: string | null;
  cause: string;
}

export async function initWasm(): Promise<void> {
  if (initialised) return;
  if (initPromise) return initPromise;

  initPromise = (async () => {
    lastWasmUrl = wasmBinaryUrl;
    try {
      await init({ module_or_path: wasmBinaryUrl });
    } catch (err) {
      // wasm-bindgen swallows the underlying fetch failure mode; do a probe
      // ourselves so the UI can show why (404, wrong mime type, COEP block,
      // etc.) instead of a generic "failed to compile" string.
      const probe = await probeWasmUrl(wasmBinaryUrl);
      const cause = err instanceof Error ? err.message : String(err);
      const failure: WasmInitFailure = {
        url: wasmBinaryUrl,
        fetchStatus: probe.status,
        fetchContentType: probe.contentType,
        cause,
      };
      const annotated = new Error(formatWasmFailure(failure));
      (annotated as Error & { details?: WasmInitFailure }).details = failure;
      throw annotated;
    }
    (globalThis as Record<string, unknown>).__airsign_wasm__ = wasmGlue;
    initialised = true;
  })();

  return initPromise;
}

export function isWasmReady(): boolean {
  return initialised;
}

export function getWasmUrl(): string | null {
  return lastWasmUrl;
}

async function probeWasmUrl(
  url: string,
): Promise<{ status: number | null; contentType: string | null }> {
  try {
    const res = await fetch(url, { method: "GET", cache: "no-store" });
    return { status: res.status, contentType: res.headers.get("content-type") };
  } catch {
    return { status: null, contentType: null };
  }
}

function formatWasmFailure(f: WasmInitFailure): string {
  const parts = [`WASM init failed (${f.cause}).`];
  parts.push(`URL: ${f.url}`);
  if (f.fetchStatus !== null) parts.push(`HTTP ${f.fetchStatus}`);
  if (f.fetchContentType) parts.push(`content-type: ${f.fetchContentType}`);
  if (f.fetchStatus === 404) {
    parts.push("→ wasm asset not deployed; check vite `base` and Vercel output dir");
  } else if (f.fetchContentType && !f.fetchContentType.includes("wasm")) {
    parts.push("→ wrong MIME type; ensure vercel.json sets application/wasm");
  }
  return parts.join(" · ");
}
