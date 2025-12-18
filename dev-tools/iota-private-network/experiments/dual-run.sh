#!/usr/bin/env bash

# Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# dual-run.sh — run sets of experiments (Mysticeti + Starfish per step)
set -euo pipefail
IFS=$'\n\t'


# ---------------------------------------------------------------------
# Paths
# ---------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DOCKER_ROOT="$(cd "$SCRIPT_DIR/../../../docker" && pwd)"

echo "dual-run: script dir = $SCRIPT_DIR"
echo "dual-run: docker root = $DOCKER_ROOT"


# ---------------------------------------------------------------------
# Build images ONCE up front
# ---------------------------------------------------------------------
echo "dual-run: building docker images once (node, tools, indexer)..."

# Use sudo because your builds / docker usually run as root
if ! (cd "$DOCKER_ROOT/iota-node"    && sudo ./build.sh); then
  echo "dual-run: ERROR building iota-node image" >&2
  exit 1
fi

if ! (cd "$DOCKER_ROOT/iota-tools"   && sudo ./build.sh); then
  echo "dual-run: ERROR building iota-tools image" >&2
  exit 1
fi

if ! (cd "$DOCKER_ROOT/iota-indexer" && sudo ./build.sh); then
  echo "dual-run: ERROR building iota-indexer image" >&2
  exit 1
fi

echo "dual-run: docker images built successfully."

# fuzz cadence
FUZZ_ROUND_SPAN=300      # time (seconds) between between topology reshuffles

# healing: every 3rd fuzz round -> clear all drops, keep tc latencies, pause restarts
HEAL_EVERY_ROUND=2
HEAL_NUM_ROUNDS=1

NUM_VALIDATORS=10
TOPOLOGY="ring"
DURATION=3600              # seconds
SPAMMER=true
SPAMMER_TPS=100
SPAMMER_TYPE="stress"
PAUSE_BETWEEN_PROTOCOLS=60 # seconds
PAUSE_BETWEEN_STEPS=180 # seconds

# parameter sequences (same length)
R_LIST=(25 26 33 33)   # percent restarts
X_LIST=(10 15 10 10)   # percent block
L_LIST=(10 15 10 10)   # percent nodes with loss


run_experiment() {
  local PROTO="$1" R="$2" X="$3" L="$4"
  local ts; ts=$(date +%Y%m%d-%H%M%S)
  echo "=== ${ts}: starting ${PROTO} (r=${R} x=${X} l=${L}) ==="
  sudo -E \
    FUZZ_ROUND_SPAN="${FUZZ_ROUND_SPAN}" \
    HEAL_EVERY_ROUND="${HEAL_EVERY_ROUND}" \
    HEAL_NUM_ROUNDS="${HEAL_NUM_ROUNDS}" \
    ./run-all-fuzz.sh \
      -n "${NUM_VALIDATORS}" \
      -p "${PROTO}" \
      -t "${TOPOLOGY}" \
      -b false \
      -r "${R}" \
      -x "${X}" \
      -l "${L}" \
      -d "${DURATION}" \
      -S "${SPAMMER}" \
      -T "${SPAMMER_TPS}" \
      -C "${SPAMMER_TYPE}"
  echo "=== finished ${PROTO} (r=${R} x=${X} l=${L}) ==="
}

for i in "${!R_LIST[@]}"; do
  R=${R_LIST[$i]}
  X=${X_LIST[$i]}
  L=${L_LIST[$i]}
  echo
  echo ">>> Step $((i+1)): r=${R}, x=${X}, l=${L} — $(date +%Y%m%d-%H%M%S)"
  run_experiment "mysticeti" "$R" "$X" "$L"
  echo "Sleeping ${PAUSE_BETWEEN_PROTOCOLS}s before starfish..."
  sleep "${PAUSE_BETWEEN_PROTOCOLS}"

  run_experiment "starfish" "$R" "$X" "$L"
  echo "Step $((i+1)) complete. Sleeping ${PAUSE_BETWEEN_STEPS}s before next step..."
  sleep "${PAUSE_BETWEEN_STEPS}"
done

echo "All progressive fuzz runs completed."