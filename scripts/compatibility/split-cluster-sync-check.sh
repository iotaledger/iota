#!/bin/bash
# (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# Enhanced split-cluster test that specifically triggers and tests Synchronizer and CommitSyncer
#
# This script runs a cluster with 3 validators built at the release commit and 1 validator
# built at the candidate commit. The candidate validator is started late (once the
# initial quorum has built up a commit backlog) to trigger synchronization components.
#
# Usage:
#
# WORKING_DIR=/tmp/split-cluster-sync-check ./scripts/compatibility/split-cluster-sync-check.sh
#
# You can then re-run using the same WORKING_DIR to skip building the binaries
# every time. If you omit WORKING_DIR, a temp dir will be created and used.
#
# Test scenario:
# - Staggered node startup: candidate node started late, once the initial quorum has accumulated enough rounds for synchronization

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

# remember current commit
CURRENT_COMMIT=$(git rev-parse HEAD)

# check if binaries have already been built
if [ -f "$WORKING_DIR/iota-node-release" ] && [ -f "$WORKING_DIR/iota-localnet-release" ] && [ -f "$WORKING_DIR/iota-node-candidate" ]; then
  echo "Binaries already built, skipping build"
else
  echo "Building iota-node and iota-localnet at $RELEASE_COMMIT"

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

"$WORKING_DIR/iota-localnet-release" genesis --epoch-duration-ms 600000 --committee-size 4

LOG_DIR="$WORKING_DIR/logs"
METRICS_DIR="$WORKING_DIR/metrics"

mkdir -p "$LOG_DIR"
mkdir -p "$METRICS_DIR"

# read all configs in the config dir to an array
CONFIGS=()
while IFS= read -r -d '' file; do
  CONFIGS+=("$file")
done < <(find "$IOTA_CONFIG_DIR" -name "127.0.0.1*.yaml" -print0)

export RUST_LOG=iota=debug,info

# Track child PIDs (and which binary version each runs) so we can report
# liveness and tear the cluster down cleanly. node-3 is appended only once it
# starts (late), so the warmup liveness check naturally covers just the initial
# quorum.
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

# Cleanup function to kill child processes on exit
cleanup() {
  echo "Cleaning up..."
  pkill -P $$
  wait 2>/dev/null
}
trap cleanup EXIT

# Helper function to get metrics port from config
get_metrics_port() {
  local config_file=$1
  grep "metrics-address:" "$config_file" | awk -F: '{print $NF}' | tr -d ' "'
}

# Helper function to get metrics from a node
get_metrics() {
  local config_file=$1
  local output_file=$2
  local port=$(get_metrics_port "$config_file")
  if [ -n "$port" ]; then
    curl -s "http://127.0.0.1:$port/metrics" > "$output_file" 2>/dev/null || true
  else
    echo "Error: Could not find metrics port in $config_file" >&2
  fi
}

# Helper function to extract metric value (for single-value metrics)
get_metric_value() {
  local file=$1
  local metric_name=$2
  local value=$(grep "^${metric_name} " "$file" 2>/dev/null | tail -1 | awk '{print $2}')
  # Ensure we return a number, default to 0 if empty
  if [ -z "$value" ] || ! [[ "$value" =~ ^[0-9]+(\.[0-9]+)?$ ]]; then
    echo "0"
  else
    echo "$value"
  fi
}

# Helper function to sum metric values across all labels
sum_metric_values() {
  local file=$1
  local metric_name=$2
  local sum=$(grep "^${metric_name}{" "$file" 2>/dev/null | awk '{sum+=$2} END {print sum}')
  # Ensure we return a number, default to 0 if empty
  if [ -z "$sum" ] || ! [[ "$sum" =~ ^[0-9]+(\.[0-9]+)?$ ]]; then
    echo "0"
  else
    echo "$sum"
  fi
}

# Track failures
FAILURES=()

POLL_INTERVAL=${POLL_INTERVAL:-5}
# node-0..2 warm up for this long to build a commit backlog large enough that
# node-3 must use the Synchronizer/CommitSyncer to catch up; with a small backlog
# node-3 catches up through live consensus and exercises neither.
WARMUP_SECS=${WARMUP_SECS:-180}
# Upper bound on the poll for node-3 to catch up past the baseline commit index.
CATCHUP_MAX_WAIT=${CATCHUP_MAX_WAIT:-180}

# Echo "<name> (<version>, pid <pid>)" of the first dead node among those
# started so far and return 0; return 1 if all are alive.
first_dead_node() {
  local i
  for i in "${!NODE_NAMES[@]}"; do
    if ! kill -0 "${NODE_PIDS[$i]}" 2>/dev/null; then
      echo "${NODE_NAMES[$i]} (${NODE_VERSIONS[$i]}, pid ${NODE_PIDS[$i]})"
      return 0
    fi
  done
  return 1
}

# Dump the tail of every started node's log for triage on failure.
dump_log_tails() {
  local i
  echo "===== LOG TAILS (last 50 lines) ====="
  for i in "${!NODE_NAMES[@]}"; do
    echo "----- ${NODE_NAMES[$i]}.log (${NODE_VERSIONS[$i]}) -----"
    tail -n 50 "$LOG_DIR/${NODE_NAMES[$i]}.log" 2>/dev/null
  done
}

echo "=== Phase 1: Initial Quorum Startup (3 release nodes) ==="
echo "Starting nodes 0-2 with release binary to establish quorum..."

start_node node-0 release "$WORKING_DIR/iota-node-release" "${CONFIGS[0]}"
start_node node-1 release "$WORKING_DIR/iota-node-release" "${CONFIGS[1]}"
start_node node-2 release "$WORKING_DIR/iota-node-release" "${CONFIGS[2]}"
start_node fullnode release "$WORKING_DIR/iota-node-release" "$IOTA_CONFIG_DIR/fullnode.yaml"

echo "Building a commit backlog for ${WARMUP_SECS}s before starting node-3..."
SECONDS=0
while [ "$SECONDS" -lt "$WARMUP_SECS" ]; do
  if dead=$(first_dead_node); then
    echo "ERROR: $dead exited early during warmup after ${SECONDS}s"
    dump_log_tails
    exit 1
  fi
  sleep "$POLL_INTERVAL"
done

# Capture the baseline commit index that node-3 will have to catch up past.
get_metrics "${CONFIGS[0]}" "$METRICS_DIR/node-0-before-node3.txt"
INITIAL_COMMIT_INDEX=$(get_metric_value "$METRICS_DIR/node-0-before-node3.txt" "consensus_last_commit_index")
echo "Initial commit index on node-0: $INITIAL_COMMIT_INDEX (after ${WARMUP_SECS}s)"
if [ "$INITIAL_COMMIT_INDEX" -le 0 ]; then
  echo "ERROR: initial quorum produced no commits in ${WARMUP_SECS}s; cannot validate catch-up against a zero baseline"
  dump_log_tails
  exit 1
fi

echo "Consensus protocol: Starfish"

echo -e "\n=== Phase 2: Late Start of Candidate Node ==="
echo "Starting node-3 (candidate) - should trigger synchronization to catch up..."

# Start the 4th node with candidate binary (late joiner).
start_node node-3 candidate "$WORKING_DIR/iota-node-candidate" "${CONFIGS[3]}"

echo -e "\n=== Checking Node-3 After Initial Sync ==="
echo "Waiting for node-3 to catch up past commit index $INITIAL_COMMIT_INDEX (up to ${CATCHUP_MAX_WAIT}s)..."
SECONDS=0
NODE3_COMMIT_AFTER_JOIN=0
while true; do
  get_metrics "${CONFIGS[3]}" "$METRICS_DIR/node-3-after-join.txt"
  NODE3_COMMIT_AFTER_JOIN=$(get_metric_value "$METRICS_DIR/node-3-after-join.txt" "consensus_last_commit_index")
  if [ "$NODE3_COMMIT_AFTER_JOIN" -gt "$INITIAL_COMMIT_INDEX" ]; then
    break
  fi
  if dead=$(first_dead_node); then
    echo "ERROR: $dead exited early during catch-up after ${SECONDS}s"
    dump_log_tails
    exit 1
  fi
  # Stop once the deadline passes, but only after the check above so a catch-up
  # that first happens during the final poll interval is not missed.
  if [ "$SECONDS" -ge "$CATCHUP_MAX_WAIT" ]; then break; fi
  sleep "$POLL_INTERVAL"
done
echo "node-3 commit index after ${SECONDS}s: $NODE3_COMMIT_AFTER_JOIN"

# Debug: Check if metrics file exists and has content
if [ ! -s "$METRICS_DIR/node-3-after-join.txt" ]; then
  echo "⚠ Warning: Metrics file is empty or doesn't exist. Checking available metrics..."
  echo "Sample of available metrics (first 20 lines):"
  head -20 "$METRICS_DIR/node-3-after-join.txt" 2>/dev/null || echo "  File does not exist or is empty"
fi

# Starfish: commit_sync_fetched_commits is labeled by source (commit_sync, fast_commit_sync), so sum across labels
NODE3_COMMIT_SYNC=$(sum_metric_values "$METRICS_DIR/node-3-after-join.txt" "consensus_commit_sync_fetched_commits")
NODE3_HEADER_SYNC=$(sum_metric_values "$METRICS_DIR/node-3-after-join.txt" "consensus_synchronizer_fetched_block_headers_by_peer")
NODE3_TXN_SYNC=$(sum_metric_values "$METRICS_DIR/node-3-after-join.txt" "consensus_transaction_synchronizer_fetched_transactions_by_peer")
NODE3_COMMIT_SYNC_TXN_SIZE=$(sum_metric_values "$METRICS_DIR/node-3-after-join.txt" "consensus_commit_sync_total_fetched_transactions_size")

echo "Node-3 metrics after initial sync:"
echo "  last_commit_index: $NODE3_COMMIT_AFTER_JOIN"
echo "  commit_sync_fetched_commits (sum): $NODE3_COMMIT_SYNC"
echo "  synchronizer_fetched_block_headers_by_peer (sum): $NODE3_HEADER_SYNC"
echo "  commit_sync_total_fetched_transactions_size: $NODE3_COMMIT_SYNC_TXN_SIZE"
echo "  transaction_synchronizer_fetched_transactions_by_peer (sum): $NODE3_TXN_SYNC"

# Check 1: Node-3 caught up past initial commit index
if [ "$NODE3_COMMIT_AFTER_JOIN" -le "$INITIAL_COMMIT_INDEX" ]; then
  FAILURES+=("FAIL: Node-3 did not catch up after late start within ${CATCHUP_MAX_WAIT}s (node-3: $NODE3_COMMIT_AFTER_JOIN, initial node-0: $INITIAL_COMMIT_INDEX)")
else
  echo "✓ Node-3 caught up past initial commit index"
fi

# Check 2: Header synchronizer was active
if [ "$NODE3_HEADER_SYNC" -le 0 ]; then
  FAILURES+=("FAIL: Header synchronizer was not active (consensus_synchronizer_fetched_block_headers_by_peer = $NODE3_HEADER_SYNC)")
else
  echo "✓ Header synchronizer was active (fetched $NODE3_HEADER_SYNC block headers)"
fi

# Check 3: Commit syncer was active
if [ "$NODE3_COMMIT_SYNC" -le 0 ]; then
  FAILURES+=("FAIL: Commit syncer was not active (consensus_commit_sync_fetched_commits = $NODE3_COMMIT_SYNC)")
else
  echo "✓ Commit syncer was active (fetched $NODE3_COMMIT_SYNC commits)"
fi

# Check 4: Transactions can come from commit syncer or transaction synchronizer
if [ "$NODE3_COMMIT_SYNC_TXN_SIZE" -le 0 ] && [ "$NODE3_TXN_SYNC" -le 0 ]; then
  FAILURES+=("FAIL: No transactions were fetched (commit_sync: $NODE3_COMMIT_SYNC_TXN_SIZE bytes, txn_sync: $NODE3_TXN_SYNC)")
else
  echo "✓ Transactions were fetched (commit_sync: $NODE3_COMMIT_SYNC_TXN_SIZE bytes, txn_sync: $NODE3_TXN_SYNC transactions)"
fi

# Capture final metrics from all nodes
get_metrics "${CONFIGS[0]}" "$METRICS_DIR/node-0-final.txt"
get_metrics "${CONFIGS[1]}" "$METRICS_DIR/node-1-final.txt"
get_metrics "${CONFIGS[2]}" "$METRICS_DIR/node-2-final.txt"
get_metrics "${CONFIGS[3]}" "$METRICS_DIR/node-3-final.txt"

FINAL_NODE0_COMMIT=$(get_metric_value "$METRICS_DIR/node-0-final.txt" "consensus_last_commit_index")
FINAL_NODE1_COMMIT=$(get_metric_value "$METRICS_DIR/node-1-final.txt" "consensus_last_commit_index")
FINAL_NODE2_COMMIT=$(get_metric_value "$METRICS_DIR/node-2-final.txt" "consensus_last_commit_index")
FINAL_NODE3_COMMIT=$(get_metric_value "$METRICS_DIR/node-3-final.txt" "consensus_last_commit_index")

echo -e "\n=== Final Commit Indices ==="
echo "  Node-0 (release): $FINAL_NODE0_COMMIT"
echo "  Node-1 (release): $FINAL_NODE1_COMMIT"
echo "  Node-2 (release): $FINAL_NODE2_COMMIT"
echo "  Node-3 (candidate): $FINAL_NODE3_COMMIT"

# The cluster is torn down by the EXIT trap.

# Print summary
echo -e "\n=== Test Summary ==="

if [ ${#FAILURES[@]} -eq 0 ]; then
  echo "✓ All checks passed!"
  echo ""
  echo "Successfully verified:"
  echo "  - Split-cluster with 3 release + 1 candidate node (consensus: Starfish)"
  echo "  - Candidate node synchronized after late start (joined once a commit backlog had built up):"
  echo "    • Header Synchronizer: fetched $NODE3_HEADER_SYNC block headers"
  echo "    • Commit Syncer: fetched $NODE3_COMMIT_SYNC commits ($NODE3_COMMIT_SYNC_TXN_SIZE bytes txn, txn_sync: $NODE3_TXN_SYNC transactions)"
  echo "    • Caught up from commit $INITIAL_COMMIT_INDEX to $NODE3_COMMIT_AFTER_JOIN"
  echo "  - Synchronization protocols are compatible between release and candidate versions"
  echo ""
  echo "Metrics available in: $METRICS_DIR"
  exit 0
else
  echo "✗ Test failed with ${#FAILURES[@]} error(s):"
  for failure in "${FAILURES[@]}"; do
    echo "  $failure"
  done
  echo ""
  echo "Metrics available in: $METRICS_DIR"
  echo "Check metrics files for detailed sync statistics"
  exit 1
fi