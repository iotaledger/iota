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

set -uo pipefail

# first arg is the released commit, defaults to `origin/mainnet`
RELEASE_COMMIT=${1:-origin/mainnet}

# second arg is the release candidate commit, defaults to origin/develop
RELEASE_CANDIDATE_COMMIT=${2:-origin/develop}

# Abort if git repo is dirty
if ! git diff-index --quiet HEAD --; then
  echo "ERROR: Git repo is dirty, aborting"
  git diff-index --name-only HEAD --
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
echo "Release commit: $RELEASE_COMMIT"
echo "Candidate commit: $RELEASE_CANDIDATE_COMMIT"

REPO_ROOT=$(git rev-parse --show-toplevel)
cd "$REPO_ROOT"

# check if binaries have already been built
if [ -f "$WORKING_DIR/iota-node-release" ] && [ -f "$WORKING_DIR/iota-release" ] && [ -f "$WORKING_DIR/iota-node-candidate" ]; then
  echo "Binaries already built, skipping build"
else
  echo "Building iota-node and iota at $RELEASE_COMMIT"

  # remember current commit
  CURRENT_COMMIT=$(git rev-parse HEAD)

  git checkout $RELEASE_COMMIT || exit 1
  cargo build --bin iota-node --bin iota || exit 1
  cp ./target/debug/iota-node "$WORKING_DIR/iota-node-release"
  cp ./target/debug/iota "$WORKING_DIR/iota-release"

  echo "Building iota-node at $RELEASE_CANDIDATE_COMMIT"
  git checkout $RELEASE_CANDIDATE_COMMIT || exit 1
  cargo build --bin iota-node || exit 1
  cp ./target/debug/iota-node "$WORKING_DIR/iota-node-candidate"

  echo "returning to $CURRENT_COMMIT"
  git checkout $CURRENT_COMMIT || exit 1
fi

echo ""
echo "=== Binary info ==="
ls -lh "$WORKING_DIR/iota-node-release" "$WORKING_DIR/iota-node-candidate" "$WORKING_DIR/iota-release"
echo ""

export IOTA_CONFIG_DIR="$WORKING_DIR/config"
rm -rf "$IOTA_CONFIG_DIR"

echo "=== Running genesis ==="
if ! "$WORKING_DIR/iota-release" genesis --epoch-duration-ms 20000 --committee-size 4; then
  echo "ERROR: Genesis command failed with exit code $?"
  exit 1
fi

LOG_DIR="$WORKING_DIR/logs"

mkdir -p "$LOG_DIR"

# read all configs in the config dir to an array, sorted for deterministic ordering
CONFIGS=()
while IFS= read -r -d '' file; do
  CONFIGS+=("$file")
done < <(find "$IOTA_CONFIG_DIR" -name "127.0.0.1*.yaml" -print0 | sort -z)

NUM_CONFIGS=${#CONFIGS[@]}
if [ "$NUM_CONFIGS" -lt 4 ]; then
  echo "ERROR: Expected at least 4 validator configs, found $NUM_CONFIGS"
  echo "Config dir contents:"
  ls -la "$IOTA_CONFIG_DIR/"
  exit 1
fi

echo ""
echo "=== Validator configs (sorted) ==="
for i in "${!CONFIGS[@]}"; do
  echo "  CONFIGS[$i]: ${CONFIGS[$i]}"
done
echo ""

export RUST_LOG=iota=debug,info

NODE_PIDS=()
NODE_NAMES=()

# 2 release nodes
echo "Starting release node-0..."
"$WORKING_DIR/iota-node-release" --config-path "${CONFIGS[0]}" > "$LOG_DIR/node-0.log" 2>&1 &
NODE_PIDS+=($!)
NODE_NAMES+=("node-0 (release)")

echo "Starting release node-1..."
"$WORKING_DIR/iota-node-release" --config-path "${CONFIGS[1]}" > "$LOG_DIR/node-1.log" 2>&1 &
NODE_PIDS+=($!)
NODE_NAMES+=("node-1 (release)")

# 2 candidate nodes
echo "Starting candidate node-2..."
"$WORKING_DIR/iota-node-candidate" --config-path "${CONFIGS[2]}" > "$LOG_DIR/node-2.log" 2>&1 &
NODE_PIDS+=($!)
NODE_NAMES+=("node-2 (candidate)")

echo "Starting candidate node-3..."
"$WORKING_DIR/iota-node-candidate" --config-path "${CONFIGS[3]}" > "$LOG_DIR/node-3.log" 2>&1 &
NODE_PIDS+=($!)
NODE_NAMES+=("node-3 (candidate)")

# and a fullnode
echo "Starting fullnode (release)..."
"$WORKING_DIR/iota-node-release" --config-path "$IOTA_CONFIG_DIR/fullnode.yaml" > "$LOG_DIR/fullnode.log" 2>&1 &
FULLNODE_PID=$!

echo ""
echo "=== Node PIDs ==="
for i in "${!NODE_PIDS[@]}"; do
  echo "  ${NODE_NAMES[$i]}: PID ${NODE_PIDS[$i]}"
done
echo "  fullnode (release): PID $FULLNODE_PID"
echo ""

# Check that all nodes are still alive after a brief startup period
sleep 5
echo "=== Node status after 5s ==="
EARLY_CRASH=false
for i in "${!NODE_PIDS[@]}"; do
  if kill -0 "${NODE_PIDS[$i]}" 2>/dev/null; then
    echo "  ${NODE_NAMES[$i]} (PID ${NODE_PIDS[$i]}): running"
  else
    wait "${NODE_PIDS[$i]}" 2>/dev/null
    EXIT_CODE=$?
    echo "  ${NODE_NAMES[$i]} (PID ${NODE_PIDS[$i]}): CRASHED (exit code $EXIT_CODE)"
    EARLY_CRASH=true
  fi
done
if kill -0 "$FULLNODE_PID" 2>/dev/null; then
  echo "  fullnode (PID $FULLNODE_PID): running"
else
  wait "$FULLNODE_PID" 2>/dev/null
  EXIT_CODE=$?
  echo "  fullnode (PID $FULLNODE_PID): CRASHED (exit code $EXIT_CODE)"
  EARLY_CRASH=true
fi
echo ""

if [ "$EARLY_CRASH" = true ]; then
  echo "ERROR: One or more nodes crashed within 5 seconds of startup!"
  echo ""
  echo "=== Log file sizes ==="
  for log in "$LOG_DIR"/*.log; do
    echo "  $(basename "$log"): $(wc -l < "$log") lines, $(du -h "$log" | cut -f1)"
  done
  echo ""
  echo "=== Last 100 lines of each log ==="
  for log in "$LOG_DIR"/*.log; do
    echo "--- $(basename "$log") ---"
    tail -100 "$log"
    echo ""
  done
  # Still kill any surviving processes
  for pid in "${NODE_PIDS[@]}" "$FULLNODE_PID"; do
    kill "$pid" 2>/dev/null
  done
  wait 2>/dev/null
  exit 1
fi

echo "Sleeping for 55 more seconds (60s total)..."
sleep 55

# Check node status before shutdown
echo "=== Node status before shutdown ==="
for i in "${!NODE_PIDS[@]}"; do
  if kill -0 "${NODE_PIDS[$i]}" 2>/dev/null; then
    echo "  ${NODE_NAMES[$i]} (PID ${NODE_PIDS[$i]}): running"
  else
    wait "${NODE_PIDS[$i]}" 2>/dev/null
    EXIT_CODE=$?
    echo "  ${NODE_NAMES[$i]} (PID ${NODE_PIDS[$i]}): EXITED (exit code $EXIT_CODE)"
  fi
done
if kill -0 "$FULLNODE_PID" 2>/dev/null; then
  echo "  fullnode (PID $FULLNODE_PID): running"
else
  wait "$FULLNODE_PID" 2>/dev/null
  EXIT_CODE=$?
  echo "  fullnode (PID $FULLNODE_PID): EXITED (exit code $EXIT_CODE)"
fi
echo ""

# Shutdown: send SIGINT first (allows graceful shutdown), then SIGTERM
echo "Shutting down nodes (SIGINT)..."
for pid in "${NODE_PIDS[@]}" "$FULLNODE_PID"; do
  kill -INT "$pid" 2>/dev/null
done
sleep 3

# SIGTERM any remaining
for pid in "${NODE_PIDS[@]}" "$FULLNODE_PID"; do
  if kill -0 "$pid" 2>/dev/null; then
    kill -TERM "$pid" 2>/dev/null
  fi
done

# Wait for all child processes to fully terminate and flush output
wait 2>/dev/null
echo "All nodes stopped."
echo ""

# Print log file sizes
echo "=== Log file sizes ==="
for log in "$LOG_DIR"/*.log; do
  echo "  $(basename "$log"): $(wc -l < "$log") lines, $(du -h "$log" | cut -f1)"
done
echo ""

# Validation helper
VALIDATION_FAILED=false

validate_log() {
  local pattern="$1"
  local logfile="$2"
  local label="$3"
  if grep -q "$pattern" "$logfile"; then
    echo "  PASS: '$pattern' found in $label"
    return 0
  else
    echo "  FAIL: '$pattern' NOT found in $label"
    VALIDATION_FAILED=true
    return 1
  fi
}

echo "=== Validation ==="
validate_log "Node State has been reconfigured" "$LOG_DIR/fullnode.log" "fullnode"
validate_log "random beacon: created" "$LOG_DIR/node-0.log" "node-0 (release)"
validate_log "random beacon: DKG complete" "$LOG_DIR/node-0.log" "node-0 (release)"
validate_log "random beacon: created" "$LOG_DIR/node-2.log" "node-2 (candidate)"
validate_log "random beacon: DKG complete" "$LOG_DIR/node-2.log" "node-2 (candidate)"
echo ""

if [ "$VALIDATION_FAILED" = true ]; then
  echo "=== VALIDATION FAILED ==="
  echo ""
  echo "=== Last 100 lines of each log ==="
  for log in "$LOG_DIR"/*.log; do
    echo "--- $(basename "$log") ---"
    tail -100 "$log"
    echo ""
  done

  echo "=== Errors and panics across all logs ==="
  for log in "$LOG_DIR"/*.log; do
    ERRORS=$(grep -c -iE "panic|error|fatal|SIGSEGV|SIGABRT" "$log" 2>/dev/null || true)
    if [ "$ERRORS" -gt 0 ]; then
      echo "--- $(basename "$log") ($ERRORS error lines) ---"
      grep -iE "panic|error|fatal|SIGSEGV|SIGABRT" "$log" | tail -30
      echo ""
    fi
  done
  exit 1
fi

echo "Cluster reconfigured successfully"

exit 0
