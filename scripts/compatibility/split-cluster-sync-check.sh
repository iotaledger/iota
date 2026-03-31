#!/bin/bash
# (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# Enhanced split-cluster test that specifically triggers and tests Synchronizer and CommitSyncer
#
# This script runs a cluster with 3 validators built at the release commit and 1 validator
# built at the candidate commit. The candidate validator is started late (after 3 minutes)
# to trigger synchronization components.
#
# Usage:
#
# WORKING_DIR=/tmp/split-cluster-sync-check ./scripts/compatibility/split-cluster-sync-check.sh
#
# You can then re-run using the same WORKING_DIR to skip building the binaries
# every time. If you omit WORKING_DIR, a temp dir will be created and used.
#
# Test scenario:
# - Staggered node startup: candidate node started 3 minutes late to accumulate enough rounds for synchronization

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

export IOTA_CONFIG_DIR="$WORKING_DIR/config"
rm -rf "$IOTA_CONFIG_DIR"

"$WORKING_DIR/iota-release" genesis --epoch-duration-ms 600000 --committee-size 4

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

# Track PIDs for process management
NODE_PIDS=()

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

echo "=== Phase 1: Initial Quorum Startup (3 release nodes) ==="
echo "Starting nodes 0-2 with release binary to establish quorum..."

# Start first 3 nodes with release binary
"$WORKING_DIR/iota-node-release" --config-path "${CONFIGS[0]}" > "$LOG_DIR/node-0.log" 2>&1 &
NODE_PIDS[0]=$!
echo "Started node-0 (release) with PID ${NODE_PIDS[0]}"

"$WORKING_DIR/iota-node-release" --config-path "${CONFIGS[1]}" > "$LOG_DIR/node-1.log" 2>&1 &
NODE_PIDS[1]=$!
echo "Started node-1 (release) with PID ${NODE_PIDS[1]}"

"$WORKING_DIR/iota-node-release" --config-path "${CONFIGS[2]}" > "$LOG_DIR/node-2.log" 2>&1 &
NODE_PIDS[2]=$!
echo "Started node-2 (release) with PID ${NODE_PIDS[2]}"

# Start fullnode
"$WORKING_DIR/iota-node-release" --config-path "$IOTA_CONFIG_DIR/fullnode.yaml" > "$LOG_DIR/fullnode.log" 2>&1 &
FULLNODE_PID=$!
echo "Started fullnode with PID $FULLNODE_PID"

echo "Waiting 3 minutes (180 seconds) for initial quorum to accumulate rounds..."
sleep 180

# Capture initial metrics from node-0
get_metrics "${CONFIGS[0]}" "$METRICS_DIR/node-0-before-node3.txt"
INITIAL_COMMIT_INDEX=$(get_metric_value "$METRICS_DIR/node-0-before-node3.txt" "consensus_last_commit_index")
echo "Initial commit index on node-0: $INITIAL_COMMIT_INDEX"

# Detect consensus protocol (Starfish vs Mysticeti) from the release node log.
# The consensus manager logs "Starting consensus protocol Mysticeti ..." or
# "Starting consensus protocol Starfish ..." on every epoch start, making this
# a reliable signal independent of any metric values.
if grep -q "Starting consensus protocol Starfish" "$LOG_DIR/node-0.log" 2>/dev/null; then
  CONSENSUS_TYPE="starfish"
  echo "Detected consensus protocol: Starfish"
else
  CONSENSUS_TYPE="mysticeti"
  echo "Detected consensus protocol: Mysticeti"
fi

echo -e "\n=== Phase 2: Late Start of Candidate Node ==="
echo "Starting node-3 (candidate) - should trigger synchronization to catch up..."

# Start the 4th node with candidate binary (late joiner)
"$WORKING_DIR/iota-node-candidate" --config-path "${CONFIGS[3]}" > "$LOG_DIR/node-3.log" 2>&1 &
NODE_PIDS[3]=$!
echo "Started node-3 (candidate) with PID ${NODE_PIDS[3]}"

echo "Waiting 60 seconds for node-3 to catch up..."
sleep 60

# Capture metrics after node-3 joined and perform checks
echo -e "\n=== Checking Node-3 After Initial Sync ==="
get_metrics "${CONFIGS[3]}" "$METRICS_DIR/node-3-after-join.txt"

# Debug: Check if metrics file exists and has content
if [ ! -s "$METRICS_DIR/node-3-after-join.txt" ]; then
  echo "⚠ Warning: Metrics file is empty or doesn't exist. Checking available metrics..."
  echo "Sample of available metrics (first 20 lines):"
  head -20 "$METRICS_DIR/node-3-after-join.txt" 2>/dev/null || echo "  File does not exist or is empty"
fi

NODE3_COMMIT_AFTER_JOIN=$(get_metric_value "$METRICS_DIR/node-3-after-join.txt" "consensus_last_commit_index")

if [ "$CONSENSUS_TYPE" = "starfish" ]; then
  # Starfish: commit_sync_fetched_commits is labeled by source (commit_sync, fast_commit_sync), so sum across labels
  NODE3_COMMIT_SYNC=$(sum_metric_values "$METRICS_DIR/node-3-after-join.txt" "consensus_commit_sync_fetched_commits")
  NODE3_HEADER_SYNC=$(sum_metric_values "$METRICS_DIR/node-3-after-join.txt" "consensus_synchronizer_fetched_block_headers_by_peer")
  NODE3_TXN_SYNC=$(sum_metric_values "$METRICS_DIR/node-3-after-join.txt" "consensus_transaction_synchronizer_fetched_transactions_by_peer")
  NODE3_COMMIT_SYNC_TXN_SIZE=$(sum_metric_values "$METRICS_DIR/node-3-after-join.txt" "consensus_commit_sync_total_fetched_transactions_size")
else
  # Mysticeti: commit_sync_fetched_commits is labeled by authority, so sum across labels
  NODE3_COMMIT_SYNC=$(sum_metric_values "$METRICS_DIR/node-3-after-join.txt" "consensus_commit_sync_fetched_commits")
  NODE3_BLOCK_SYNC=$(sum_metric_values "$METRICS_DIR/node-3-after-join.txt" "consensus_synchronizer_fetched_blocks_by_peer")
  NODE3_COMMIT_SYNC_BLOCKS=$(sum_metric_values "$METRICS_DIR/node-3-after-join.txt" "consensus_commit_sync_fetched_blocks")
fi

echo "Node-3 metrics after initial sync:"
echo "  last_commit_index: $NODE3_COMMIT_AFTER_JOIN"
echo "  commit_sync_fetched_commits (sum): $NODE3_COMMIT_SYNC"
if [ "$CONSENSUS_TYPE" = "starfish" ]; then
  echo "  synchronizer_fetched_block_headers_by_peer (sum): $NODE3_HEADER_SYNC"
  echo "  commit_sync_total_fetched_transactions_size: $NODE3_COMMIT_SYNC_TXN_SIZE"
  echo "  transaction_synchronizer_fetched_transactions_by_peer (sum): $NODE3_TXN_SYNC"
else
  echo "  synchronizer_fetched_blocks_by_peer (sum): $NODE3_BLOCK_SYNC"
  echo "  commit_sync_fetched_blocks (sum): $NODE3_COMMIT_SYNC_BLOCKS"
fi

# Check 1: Node-3 caught up past initial commit index
if [ "$NODE3_COMMIT_AFTER_JOIN" -le "$INITIAL_COMMIT_INDEX" ]; then
  FAILURES+=("FAIL: Node-3 did not catch up after late start (node-3: $NODE3_COMMIT_AFTER_JOIN, initial node-0: $INITIAL_COMMIT_INDEX)")
else
  echo "✓ Node-3 caught up past initial commit index"
fi

# Check 2: Synchronizer was active (protocol-specific)
if [ "$CONSENSUS_TYPE" = "starfish" ]; then
  # Starfish has a dedicated block header synchronizer
  if [ "$NODE3_HEADER_SYNC" -le 0 ]; then
    FAILURES+=("FAIL: Header synchronizer was not active (consensus_synchronizer_fetched_block_headers_by_peer = $NODE3_HEADER_SYNC)")
  else
    echo "✓ Header synchronizer was active (fetched $NODE3_HEADER_SYNC block headers)"
  fi
else
  # Mysticeti uses a block synchronizer (no separate header sync)
  if [ "$NODE3_BLOCK_SYNC" -le 0 ]; then
    FAILURES+=("FAIL: Block synchronizer was not active (consensus_synchronizer_fetched_blocks_by_peer = $NODE3_BLOCK_SYNC)")
  else
    echo "✓ Block synchronizer was active (fetched $NODE3_BLOCK_SYNC blocks)"
  fi
fi

# Check 3: Commit syncer was active (applies to both protocols)
if [ "$NODE3_COMMIT_SYNC" -le 0 ]; then
  FAILURES+=("FAIL: Commit syncer was not active (consensus_commit_sync_fetched_commits = $NODE3_COMMIT_SYNC)")
else
  echo "✓ Commit syncer was active (fetched $NODE3_COMMIT_SYNC commits)"
fi

# Check 4: Data was fetched via commit syncer (protocol-specific)
if [ "$CONSENSUS_TYPE" = "starfish" ]; then
  # Starfish: transactions can come from commit syncer or transaction synchronizer
  if [ "$NODE3_COMMIT_SYNC_TXN_SIZE" -le 0 ] && [ "$NODE3_TXN_SYNC" -le 0 ]; then
    FAILURES+=("FAIL: No transactions were fetched (commit_sync: $NODE3_COMMIT_SYNC_TXN_SIZE bytes, txn_sync: $NODE3_TXN_SYNC)")
  else
    echo "✓ Transactions were fetched (commit_sync: $NODE3_COMMIT_SYNC_TXN_SIZE bytes, txn_sync: $NODE3_TXN_SYNC transactions)"
  fi
else
  # Mysticeti: no transaction synchronizer; check blocks fetched via commit sync
  if [ "$NODE3_COMMIT_SYNC_BLOCKS" -le 0 ]; then
    FAILURES+=("FAIL: No blocks were fetched via commit sync (consensus_commit_sync_fetched_blocks = $NODE3_COMMIT_SYNC_BLOCKS)")
  else
    echo "✓ Blocks were fetched via commit sync ($NODE3_COMMIT_SYNC_BLOCKS blocks)"
  fi
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

echo -e "\n=== Shutting Down Cluster ==="
kill ${NODE_PIDS[0]} ${NODE_PIDS[1]} ${NODE_PIDS[2]} ${NODE_PIDS[3]} $FULLNODE_PID 2>/dev/null
pkill -P $$
wait 2>/dev/null

# Print summary
echo -e "\n=== Test Summary ==="

if [ ${#FAILURES[@]} -eq 0 ]; then
  echo "✓ All checks passed!"
  echo ""
  echo "Successfully verified:"
  echo "  - Split-cluster with 3 release + 1 candidate node (consensus: $CONSENSUS_TYPE)"
  echo "  - Candidate node synchronized after late start (3 minute delay):"
  if [ "$CONSENSUS_TYPE" = "starfish" ]; then
    echo "    • Header Synchronizer: fetched $NODE3_HEADER_SYNC block headers"
    echo "    • Commit Syncer: fetched $NODE3_COMMIT_SYNC commits ($NODE3_COMMIT_SYNC_TXN_SIZE bytes txn, txn_sync: $NODE3_TXN_SYNC transactions)"
  else
    echo "    • Block Synchronizer: fetched $NODE3_BLOCK_SYNC blocks"
    echo "    • Commit Syncer: fetched $NODE3_COMMIT_SYNC commits ($NODE3_COMMIT_SYNC_BLOCKS blocks)"
  fi
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