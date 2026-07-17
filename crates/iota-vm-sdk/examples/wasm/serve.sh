#!/usr/bin/env bash
# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
#
# Build the wasm bundle and the JS app, then serve this example directory and
# print the URL to open. The page builds a staking transaction with the IOTA TS
# SDK against testnet and simulates it in the wasm Move VM.
#
# Requires:
#   - the wasm32 target (`rustup target add wasm32-unknown-unknown`) and the
#     wasm-bindgen CLI matching the crate's wasm-bindgen version
#     (`cargo install wasm-bindgen-cli --version <ver>`);
#   - Node.js + npm (to install @iota/iota-sdk and bundle the app with esbuild).
#
# Usage: ./serve.sh [port]            (default port 8000)
#        ./serve.sh --rebuild [port]  force a wasm rebuild first
set -euo pipefail

example_dir="$(cd "$(dirname "$0")" && pwd)"
crate_dir="$(cd "$example_dir/../.." && pwd)"

rebuild=false
if [[ "${1:-}" == "--rebuild" ]]; then
  rebuild=true
  shift
fi
port="${1:-8000}"
url="http://localhost:$port/"

# 1. Build the wasm bundle when it's missing, or when --rebuild is passed.
if [[ "$rebuild" == true || ! -f "$example_dir/pkg/iota_vm_sdk.js" ]]; then
  echo "Building wasm bundle…"
  cd "$crate_dir"
  out_dir="examples/wasm/pkg"
  target_dir="$(cargo metadata --format-version 1 --no-deps \
    | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')"
  wasm="$target_dir/wasm32-unknown-unknown/release/iota_vm_sdk.wasm"
  cargo build --lib --release --target wasm32-unknown-unknown --features wasm-bindgen
  wasm-bindgen "$wasm" --out-dir "$out_dir" --target web
fi

# 2. Install JS deps (once) and bundle the app with esbuild.
cd "$example_dir"
if [[ ! -d node_modules ]]; then
  echo "Installing JS dependencies…"
  npm install
fi
echo "Bundling app.ts…"
npm run build

# 3. Serve this directory (the page talks to testnet over HTTPS directly).
echo
echo "Serving on $url"
echo "Press Ctrl+C to stop."
echo
python3 -m http.server "$port"
