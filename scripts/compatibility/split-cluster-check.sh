#!/bin/bash
# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# Runs a cluster with 2 validators built at one commit, and 2 at another, and
# verifies that the cluster can produce checkpoints and reconfigure
#
# Usage:
#
# WORKING_DIR=/tmp/split-cluster-check ./scripts/compatibility/split-cluster-check.sh
#
# You can then re-run using the same WORKING_DIR to skip building the binaries
# every time. If you omit WORKING_DIR, a temp dir will be created and used.

# first arg is the released commit, defaults to `origin/mainnet`
RELEASE_COMMIT=${1:-origin/mainnet}

# second arg is the release candidate commit, defaults to origin/develop
RELEASE_CANDIDATE_COMMIT=${2:-origin/develop}

# Abort if git repo is dirty
if ! git diff-index --quiet HEAD --; then
  echo "Git repo is dirty, aborting"
  exit 1
fi

# if WORKING_DIR is not set, create a temp dir
if [ -z "$WORKING_DIR" ]; then
  WORKING_DIR=$(mktemp -d)
else
  # if WORKING_DIR is set but doesn't exist, create it
  mkdir -p "$WORKING_DIR"
fi

echo "Using working dir $WORKING_DIR"

REPO_ROOT=$(git rev-parse --show-toplevel)
cd "$REPO_ROOT"

# check if binaries have already been built
if [ -f "$WORKING_DIR/iota-node-release" ] && [ -f "$WORKING_DIR/iota-localnet-release" ] && [ -f "$WORKING_DIR/iota-node-candidate" ]; then
  echo "Binaries already built, skipping build"
else
  echo "Building iota-node and iota-localnet at $RELEASE_COMMIT"

  # remember current commit
  CURRENT_COMMIT=$(git rev-parse HEAD)

  git checkout $RELEASE_COMMIT || exit 1
  cargo build --bin iota-node --bin iota-localnet || exit 1
  cp ./target/debug/iota-node "$WORKING_DIR/iota-node-release"
  cp ./target/debug/iota-localnet "$WORKING_DIR/iota-localnet-release"

  echo "Building iota-node at $RELEASE_CANDIDATE_COMMIT"
  git checkout $RELEASE_CANDIDATE_COMMIT || exit 1
  cargo build --bin iota-node || exit 1
  cp ./target/debug/iota-node "$WORKING_DIR/iota-node-candidate"

  echo "returning to $CURRENT_COMMIT"
  git checkout $CURRENT_COMMIT || exit 1
fi

export IOTA_CONFIG_DIR="$WORKING_DIR/config"
rm -rf "$IOTA_CONFIG_DIR"

"$WORKING_DIR/iota-localnet-release" genesis --epoch-duration-ms 20000 --committee-size 4

LOG_DIR="$WORKING_DIR/logs"

mkdir -p "$LOG_DIR"

# read all configs in the config dir to an array
CONFIGS=()
while IFS= read -r -d '' file; do
  CONFIGS+=("$file")
done < <(find "$IOTA_CONFIG_DIR" -name "127.0.0.1*.yaml" -print0)

export RUST_LOG=iota=debug,info

# Track child PIDs (and which binary version each runs) so we can report
# liveness and tear the cluster down cleanly.
NODE_PIDS=()
NODE_NAMES=()
NODE_VERSIONS=()

start_node() {
  local name=$1 version=$2 binary=$3 config=$4
  "$binary" --config-path "$config" > "$LOG_DIR/$name.log" 2>&1 &
  local pid=$!
  NODE_PIDS+=("$pid")
  NODE_NAMES+=("$name")
  NODE_VERSIONS+=("$version")
  echo "Started $name ($version) with PID $pid"
}

# Tear the cluster down on any exit (success, timeout, or early node death).
cleanup() {
  echo "shutting down nodes"
  pkill -P $$ 2>/dev/null
  wait 2>/dev/null
}
trap cleanup EXIT

# 2 release nodes, 2 candidate nodes, and a release fullnode.
start_node node-0 release "$WORKING_DIR/iota-node-release" "${CONFIGS[0]}"
start_node node-1 release "$WORKING_DIR/iota-node-release" "${CONFIGS[1]}"
start_node node-2 candidate "$WORKING_DIR/iota-node-candidate" "${CONFIGS[2]}"
start_node node-3 candidate "$WORKING_DIR/iota-node-candidate" "${CONFIGS[3]}"
start_node fullnode release "$WORKING_DIR/iota-node-release" "$IOTA_CONFIG_DIR/fullnode.yaml"

# Progress markers checked on failure to show how far each node got through
# epoch-0 reconfiguration (crates/iota-node/src/lib.rs,
# crates/iota-core/src/epoch/randomness.rs).
DIAG_MARKERS=(
  "Creating checkpoint executor for epoch 1"
  "Finished executing all checkpoints in epoch"
  "Node State has been reconfigured"
  "random beacon: created"
  "random beacon: DKG complete"
)

dump_diagnostics() {
  local i label marker status
  echo "===== DIAGNOSTICS (elapsed ${SECONDS}s) ====="
  for i in "${!NODE_NAMES[@]}"; do
    if kill -0 "${NODE_PIDS[$i]}" 2>/dev/null; then status="ALIVE"; else status="DEAD"; fi
    echo "  ${NODE_NAMES[$i]} (${NODE_VERSIONS[$i]}, pid ${NODE_PIDS[$i]}): $status"
  done
  echo "===== MARKER MATRIX ====="
  for i in "${!NODE_NAMES[@]}"; do
    label="${NODE_NAMES[$i]} (${NODE_VERSIONS[$i]})"
    for marker in "${DIAG_MARKERS[@]}"; do
      if grep -q "$marker" "$LOG_DIR/${NODE_NAMES[$i]}.log" 2>/dev/null; then status="present"; else status="ABSENT "; fi
      printf '  %-20s | %-45s | %s\n' "$label" "$marker" "$status"
    done
  done
  echo "===== LOG TAILS (last 50 lines) ====="
  for i in "${!NODE_NAMES[@]}"; do
    echo "----- ${NODE_NAMES[$i]}.log (${NODE_VERSIONS[$i]}) -----"
    tail -n 50 "$LOG_DIR/${NODE_NAMES[$i]}.log" 2>/dev/null
  done
}

# Poll for the fullnode to finish reconfiguration rather than waiting a fixed
# 60s: with a 20s epoch the marker normally appears within a minute, but
# boot + DKG + end-of-epoch checkpoint + fullnode sync can take longer under CI
# load, which is the source of the historical flake. Polling keeps the
# slow-but-correct case green while still bounding the wait.
MAX_WAIT=${MAX_WAIT:-180}
POLL_INTERVAL=${POLL_INTERVAL:-5}
RECONFIG_MARKER="Node State has been reconfigured"

echo "Waiting up to ${MAX_WAIT}s for fullnode reconfiguration (polling every ${POLL_INTERVAL}s)"
SECONDS=0
reconfigured=0
while true; do
  if grep -q "$RECONFIG_MARKER" "$LOG_DIR/fullnode.log" 2>/dev/null; then
    echo "Fullnode reconfigured after ${SECONDS}s"
    reconfigured=1
    break
  fi
  # Fail fast if any node died rather than burning the whole timeout.
  for i in "${!NODE_NAMES[@]}"; do
    if ! kill -0 "${NODE_PIDS[$i]}" 2>/dev/null; then
      echo "ERROR: ${NODE_NAMES[$i]} exited early after ${SECONDS}s"
      dump_diagnostics
      exit 1
    fi
  done
  # Stop once the deadline passes, but only after the check above so a marker
  # that first appears during the final poll interval is not missed.
  if [ "$SECONDS" -ge "$MAX_WAIT" ]; then break; fi
  sleep "$POLL_INTERVAL"
done

if [ "$reconfigured" -ne 1 ]; then
  echo "ERROR: timed out after ${SECONDS}s waiting for '$RECONFIG_MARKER' in fullnode log"
  dump_diagnostics
  exit 1
fi

# Ensure the random beacon's DKG completes on both versions. node-0 (release)
# and node-2 (candidate) are each checked for both markers (2 nodes x 2 markers
# = 4 checks, same coverage as before). Report every missing marker before
# failing so one run shows the full picture.
dkg_ok=1
for node in node-0 node-2; do
  for marker in "random beacon: created" "random beacon: DKG complete"; do
    if ! grep -q "$marker" "$LOG_DIR/$node.log"; then
      echo "ERROR: could not find '$marker' in $node log"
      dkg_ok=0
    fi
  done
done

if [ "$dkg_ok" -ne 1 ]; then
  dump_diagnostics
  exit 1
fi

echo "Cluster reconfigured successfully"

# The EXIT trap shuts the cluster down.
exit 0
