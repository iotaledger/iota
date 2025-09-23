#!/bin/bash

# Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# Orchestrate: build images -> bootstrap -> run -> apply latencies -> fuzz -> wait/save logs
# Run from: iota/dev-tools/iota-private-network/experiments/

set -euo pipefail

# =================== CONSTANTS ===================
DEFAULT_NUM_VALIDATORS=4
DEFAULT_PROTOCOL="mysticeti"
DEFAULT_BUILD=true
DEFAULT_GEODISTRIBUTED=false
DEFAULT_SEED=42
DEFAULT_PERCENT_BLOCK=0       # percent chance to block a connection
DEFAULT_PERCENT_LOSS=0       # percent chance to apply netem loss
DEFAULT_PERCENT_RESTART=0     # percent chance to restart a validator
DEFAULT_RUN_DURATION=3600  # default sleep at end: 1 hour
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOG_DIR="$SCRIPT_DIR/logs" # directory with logs
LOG_INTERVAL=60           # save logs every 5 minutes
DEFAULT_NETWORK_METRIC=false
# ==================================================

# --- Trap termination and normal exit safely ---
CLEANED_UP=false
cleanup_and_kill() {
    if [ "$CLEANED_UP" = false ]; then
        # --- Print final network statistics to terminal ---
        if [ "$NETWORK_METRIC" = true ]; then
          echo "=== Final network stats for validators ==="
          for ((i=1; i<=NUM_VALIDATORS; i++)); do
              v="validator-$i"
              tx_bytes=$(docker exec "$v" cat /sys/class/net/eth0/statistics/tx_bytes)
              rx_bytes=$(docker exec "$v" cat /sys/class/net/eth0/statistics/rx_bytes)
              tx_packets=$(docker exec "$v" cat /sys/class/net/eth0/statistics/tx_packets)
              rx_packets=$(docker exec "$v" cat /sys/class/net/eth0/statistics/rx_packets)

              # Convert bytes to MB (with 2 decimals)
              tx_mb=$(awk "BEGIN {printf \"%.2f\", $tx_bytes/1024/1024}")
              rx_mb=$(awk "BEGIN {printf \"%.2f\", $rx_bytes/1024/1024}")

              # Add thousand separators for packets
              tx_packets_fmt=$(printf "%'d" "$tx_packets")
              rx_packets_fmt=$(printf "%'d" "$rx_packets")

              echo ">>> $v <<<"
              echo "TX: $tx_packets_fmt packets, $tx_mb MB"
              echo "RX: $rx_packets_fmt packets, $rx_mb MB"
              echo
          done
        fi

        CLEANED_UP=true
        echo "Stopping all background scripts and validators..."
        kill -- -$$ &> /dev/null   # silently kill all children
        (cd .. && docker compose down &> /dev/null)  # silent cleanup
    fi
}

trap cleanup_and_kill SIGINT SIGTERM EXIT

# --- Prepare log directory ---
mkdir -p "$LOG_DIR"

# Initial timestamp for the log file
LOG_FILE="$LOG_DIR/experiment_script_latest.log"

# Overwrite the log file at the beginning
: > "$LOG_FILE"

# --- Logging helper ---
log() {
    echo "$(date -Iseconds) $1" | tee -a "$LOG_FILE"
}

# --- Usage ---
usage() {
  echo "Usage: $0 [-n num_validators(4..19)] [-p protocol(mysticeti|starfish)] [-b build_images(true|false)]"
  echo "          [-g geodistributed(true|false)] [-s seed(number)] [-x percent_block_connection(0..100)] [-l percent_loss_packets(0..100)]"
  echo "          [-t run_duration_seconds] [-r percent_restart(0..100)] [-m flag_to_output_network_statistics]"
}

# --- Default values ---
NUM_VALIDATORS=$DEFAULT_NUM_VALIDATORS
PROTOCOL=$DEFAULT_PROTOCOL
BUILD=$DEFAULT_BUILD
GEODISTRIBUTED=$DEFAULT_GEODISTRIBUTED
SEED=$DEFAULT_SEED
PERCENT_BLOCK=$DEFAULT_PERCENT_BLOCK
PERCENT_LOSS=$DEFAULT_PERCENT_LOSS
PERCENT_RESTART=$DEFAULT_PERCENT_RESTART
RUN_DURATION=$DEFAULT_RUN_DURATION
NETWORK_METRIC=$DEFAULT_NETWORK_METRIC

# --- Parse command-line arguments ---
while getopts ":n:p:b:g:s:x:l:t:r:hm" opt; do
  case "$opt" in
    n) NUM_VALIDATORS="$OPTARG" ;;
    p) PROTOCOL="$OPTARG" ;;
    b) BUILD="$OPTARG" ;;
    g) GEODISTRIBUTED="$OPTARG" ;;
    s) SEED="$OPTARG" ;;
    x) PERCENT_BLOCK="$OPTARG" ;;
    l) PERCENT_LOSS="$OPTARG" ;;
    t) RUN_DURATION="$OPTARG" ;;
    r) PERCENT_RESTART="$OPTARG" ;;
    m) NETWORK_METRIC=true ;;
    h) usage; exit 0 ;;
    \?) usage; exit 2 ;;
    :)  usage; exit 2 ;;
  esac
done
shift $((OPTIND-1))

# --- Ensure correct directory ---
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
[[ "$(basename "$SCRIPT_DIR")" != "experiments" ]] && { log "Error: run from experiments/"; exit 1; }

# --- Summary ---
log "=== SUMMARY ==="
log "Number of validators       : $NUM_VALIDATORS"
log "Consensus protocol         : $PROTOCOL"
log "Rebuild images             : $BUILD"
log "Geodistributed network     : $GEODISTRIBUTED"
log "Seed                       : $SEED"
log "Percent block connection   : $PERCENT_BLOCK"
log "Percent netem loss         : $PERCENT_LOSS"
log "Percent restart validator  : $PERCENT_RESTART"
log "Run experiments duration   : $RUN_DURATION s"
log "==========================="

# --- 1) Build images (optional) ---
if [ "$BUILD" = true ]; then
  (cd ../../../docker/iota-node && ./build.sh -t iota-node)
  (cd ../../../docker/iota-tools && ./build.sh -t iota-tools)
  (cd ../../../docker/iota-indexer && ./build.sh -t iota-indexer)
else
  log "Skipping image builds"
fi

# --- 2) Bootstrap network ---
(cd .. && ./bootstrap.sh -n "$NUM_VALIDATORS")

# --- 3) Bring up docker network ---
(cd .. && ./run.sh -n "$NUM_VALIDATORS" -p "$PROTOCOL")


log "Sleep 5s to boot validators..."
sleep 5

# --- 4) Run grafana dashboard if not already running ---
GRAFANA_DIR="../../grafana-local"
cd "$GRAFANA_DIR" || { log "Grafana folder not found"; exit 1; }

# Check if any Grafana container is already running
if docker compose ps --services --filter "status=running" | grep -q grafana; then
  log "Grafana already running, skipping start"
else
  log "Starting Grafana dashboard..."
  docker compose up -d
fi
log "Grafana URL: http://localhost:3000/dashboards"
cd - >/dev/null

# --- 5) Launch combined latency + fuzz watcher in background ---
./network-fuzz-disruption.sh \
    -n "$NUM_VALIDATORS" \
    -s "$SEED" \
    -b "$PERCENT_BLOCK" \
    -l "$PERCENT_LOSS" \
    -r "$PERCENT_RESTART" \
    -g "$GEODISTRIBUTED" \
    -o "$LOG_FILE" &

# --- 6) Run for specified duration, periodically saving logs ---
log "Running experiments for $RUN_DURATION seconds, saving logs every $LOG_INTERVAL seconds..."
start_time=$(date +%s)
end_time=$((start_time + RUN_DURATION))

while [[ $(date +%s) -lt $end_time ]]; do
  for ((i=1; i<=NUM_VALIDATORS; i++)); do
    v="validator-$i"
    docker logs "$v" &> "$LOG_DIR/exp-${v}-latest.log"
  done
  sleep "$LOG_INTERVAL"
done

# --- Final log save with timestamp ---
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
for ((i=1; i<=NUM_VALIDATORS; i++)); do
  v="validator-$i"

  # Save final validator log with timestamp
  docker logs "$v" &> "$LOG_DIR/experiment-${v}-${TIMESTAMP}.log"

  # Keep the latest symlink-like copy updated
  cp "$LOG_DIR/experiment-${v}-${TIMESTAMP}.log" "$LOG_DIR/experiment-${v}-latest.log"

  log "Saved final log for $v to $LOG_DIR/experiment-${v}-${TIMESTAMP}.log"
done

# Copy main experiment log with timestamp
cp "$LOG_FILE" "$LOG_DIR/experiment_script_${TIMESTAMP}.log"

log "All steps completed. Cleanup will run on script exit."
