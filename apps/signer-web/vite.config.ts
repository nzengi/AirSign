import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { resolve } from "path";

// The WASM pkg is built by `wasm-pack build crates/afterimage-wasm --target web`
// and output to `crates/afterimage-wasm/pkg/`.  We copy it into public/wasm/ so
// Vite serves the binary correctly (WASM must be served as a real URL, not bundled).
export default defineConfig({
  plugins: [react()],
  base: "./",          // relative base for GitHub Pages deployment
  resolve: {
    alias: {
      // Allow `import ... from "@airsign/react/src/..."` during dev
      "@airsign/react": resolve(__dirname, "../../packages/react/src"),
    },
  },
  server: {
    headers: {
      // Required for SharedArrayBuffer / COOP if ever needed; harmless otherwise
      "Cross-Origin-Opener-Policy": "same-origin",
      "Cross-Origin-Embedder-Policy": "require-corp",
    },
  },
  optimizeDeps: {
    exclude: ["afterimage_wasm"],  // skip pre-bundling the WASM glue module
  },
  assetsInclude: ["**/*.wasm"],
});