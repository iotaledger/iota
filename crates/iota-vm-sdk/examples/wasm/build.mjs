// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Bundle app.ts (with @iota/iota-sdk) into a single browser ES module. The wasm
// package under ./pkg is kept external so its wasm-bindgen glue resolves the
// .wasm via import.meta.url at runtime rather than being inlined.
import { build } from "esbuild";

await build({
  entryPoints: ["app.ts"],
  bundle: true,
  format: "esm",
  target: "es2022",
  outfile: "app.bundle.js",
  external: ["./pkg/*"],
  define: {
    "process.env.NODE_ENV": '"production"',
    global: "globalThis",
  },
  logLevel: "info",
});
