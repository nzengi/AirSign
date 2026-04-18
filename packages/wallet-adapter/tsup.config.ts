import { defineConfig } from "tsup";

export default defineConfig({
  entry: ["src/index.ts"],
  format: ["esm", "cjs"],
  dts: true,
  sourcemap: true,
  clean: true,
  splitting: false,
  treeshake: true,
  external: [
    "react",
    "react-dom",
    "@solana/web3.js",
    "@solana/wallet-adapter-base",
    "@airsign/react",
  ],
  esbuildOptions(options) {
    options.jsx = "automatic";
  },
});