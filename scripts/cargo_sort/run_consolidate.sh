#!/bin/bash

set -e  # Exit immediately if any command fails

source ./python_cmd.sh

# Run consolidate, ignoring external-crates and nre
$PYTHON_CMD cargo_sort.py --consolidate-deps \
  --strict \
  --strict-ignore "*:docs/examples/rust" \
  --strict-ignore "*:examples/tic-tac-toe/cli" \
  --strict-ignore "*:examples/custom-indexer/rust" \
  --strict-ignore "*:sdk/move-bytecode-template" \
  --strict-ignore "tonic:crates/telemetry-subscribers" \
  --strict-ignore "prost:crates/telemetry-subscribers" \
  --strict-ignore "syn:crates/iota-proc-macros" \
  --strict-ignore "syn:crates/iota-proto-build" \
  --keep-in-workspace fastcrypto-vdf \
  --ignore external-crates
