#!/bin/bash

# Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# Orchestrate: build images -> bootstrap -> run -> apply latencies -> fuzz -> wait/save logs
# Run from: iota/dev-tools/iota-private-network/experiments/

set -euo pipefail
PARENT_BASHPID=${BASHPID}
CLEANING=false

# =================== CONSTANTS ===================
DEFAULT_NUM_VALIDATORS=4
DEFAULT_PROTOCOL="starfish"
DEFAULT_BUILD=true
DEFAULT_GEODISTRIBUTED=false
DEFAULT_SEED=42
DEFAULT_PERCENT_BLOCK=0       # percent chance to block a connection
DEFAULT_PERCENT_LOSS=0       # percent chance to apply netem loss
DEFAULT_PERCENT_RESTART=0     # percent chance to restart a validator
DEFAULT_RESTART_DURATION=120  # default restart duration in seconds
DEFAULT_RESTART_TIMEOUT=60    # default timeout before restarting (seconds)
DEFAULT_RESTART_MODE="preserve-consensus"  # restart mode: preserve-consensus | full-reset | simple-restart
DEFAULT_EPOCH_DURATION_MS=1200000  # default epoch duration: 20 minutes (in milliseconds)
DEFAULT_RUN_DURATION=3600  # default sleep at end: 1 hour
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOG_DIR="$SCRIPT_DIR/logs" # directory with logs
LOG_INTERVAL=60           # save logs every 60 seconds
DEFAULT_NETWORK_METRIC=false
DEFAULT_SPAMMER_ENABLE=false
DEFAULT_SPAMMER_TPS=10
DEFAULT_SPAMMER_SIZE="10KiB"
DEFAULT_SPAMMER_TYPE="stress"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# Set the root directory of the private network (one level up from experiments)
NETWORK_DIR="$(dirname "$SCRIPT_DIR")"
CLEANUP_SCRIPT="$NETWORK_DIR/cleanup.sh"
# ==================================================

# --- Trap termination and normal exit safely ---
CLEANED_UP=false
cleanup_and_kill() {
    # Ensure only the original parent shell runs the EXIT trap (avoid subshell-triggered cleanup)
    if [ "${BASHPID}" != "${PARENT_BASHPID}" ]; then
          return
    fi

    # Re-entrancy guard: run cleanup only once and ignore further signals during teardown
    if [ "${CLEANING}" = true ]; then
        log "cleanup already running; returning"
        return
    fi
    CLEANING=true

    # Disable traps for SIGINT/SIGTERM/EXIT to avoid recursive invocations while cleaning
    trap - SIGINT SIGTERM EXIT
    # Ignore further SIGINT/SIGTERM from the terminal during teardown
    trap '' SIGINT SIGTERM

    # Allow cleanup commands to continue even if one fails
    set +e
    # Ensure log targets exist even if EXIT happens before normal init
    : "${SCRIPT_DIR:=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)}"
    : "${LOG_DIR:=${SCRIPT_DIR}/logs}"
    : "${LOG_FILE:=${LOG_DIR}/experiment_script_latest.log}"
    mkdir -p "$LOG_DIR" 2>/dev/null || true
    touch "$LOG_FILE" 2>/dev/null || true
    log "BEGIN cleanup_and_kill (pid=$$, user=$(id -un))"
    kill_spammer_processes || true
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
        log "Stopping all background scripts and validators..."
        # Stop background jobs started by this script without signaling the shell itself
        if pids=$(pgrep -P $$); then
          kill $pids &>/dev/null || true
        fi
        log "Delegating Docker teardown to external script: $CLEANUP_SCRIPT"

        if [ -f "$CLEANUP_SCRIPT" ]; then
          if command -v sudo >/dev/null 2>&1; then
            log "Running teardown with sudo: $CLEANUP_SCRIPT"
            if ! sudo bash -lc "cd '$NETWORK_DIR' && ./$(basename "$CLEANUP_SCRIPT")"; then
              rc=$?
              log "ERROR: cleanup script failed with exit code $rc (sudo)"
            else
              log "External cleanup script finished successfully (sudo)."
            fi
          else
            log "sudo not found; running teardown without sudo: $CLEANUP_SCRIPT"
            if ! (cd "$NETWORK_DIR" && "$CLEANUP_SCRIPT"); then
              rc=$?
              log "ERROR: cleanup script failed with exit code $rc (no sudo)"
            else
              log "External cleanup script finished successfully (no sudo)."
            fi
          fi
        else
          log "FATAL: External cleanup script not found at $CLEANUP_SCRIPT. Containers may persist."
        fi
    fi
    log "Cleanup complete. Exiting."
    exit 0
}

# Kill any lingering spammer processes and remove locks
kill_spammer_processes() {
    log "Killing lingering spammer processes (if any) and removing locks..."
    # Stop the stress benchmark container by name if it exists
    docker stop stress-benchmark >/dev/null 2>&1 || true
    # kill common spammer process forms
    pkill -9 -f 'iota-spammer spammer spam' 2>/dev/null || true
    pkill -9 -f 'cargo run --release -- spammer spam' 2>/dev/null || true
    pkill -9 -f 'cargo run --release -- spammer spam' 2>/dev/null || true
    pkill -9 -f 'cargo run --release --.* stress' 2>/dev/null || true
    pkill -9 -f 'spamming_fuzz_test.sh' 2>/dev/null || true
    pkill -9 -f 'network-benchmark.sh' 2>/dev/null || true

    # also remove per-user and global lock files
    rm -f /tmp/spammer-*.lock 2>/dev/null || true
    rm -f /tmp/spammer.lock 2>/dev/null || true
}

trap cleanup_and_kill SIGINT SIGTERM EXIT

# --- Prepare log directory ---
mkdir -p "$LOG_DIR"

# --- Initial timestamp for the log file ---
LOG_FILE="$LOG_DIR/experiment_script_latest.log"

# --- Overwrite the log file at the beginning ---
: > "$LOG_FILE"

# --- Logging helper ---
log() {
    echo "$(date -Iseconds) $1" | tee -a "$LOG_FILE"
}

# --- Usage ---
usage() {
  echo "Usage: $0 [-n num_validators(4..30)] [-p protocol(mysticeti|starfish)] [-b build_images(true|false)]"
  echo "          [-g geodistributed(true|false)] [-s seed(number)] [-x percent_block_connection(0..100)] [-l percent_loss_packets(0..100)]"
  echo "          [-t run_duration_seconds] [-d restart_duration_seconds] [-r percent_restart(0..100)]"
  echo "          [-w restart_timeout_seconds] [-M restart_mode(preserve-consensus|full-reset|simple-restart)]"
  echo "          [-E epoch_duration_ms] [-m flag_to_output_network_statistics]"
  echo "          [-S spammer_enable(true|false)] [-T spammer_tps(number)] [-Z spammer_size_per_tx(KiB)] [-C spammer_type(iota-spammer|stress)]"
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
RESTART_DURATION=$DEFAULT_RESTART_DURATION
RESTART_TIMEOUT=$DEFAULT_RESTART_TIMEOUT
RESTART_MODE=$DEFAULT_RESTART_MODE
EPOCH_DURATION_MS=$DEFAULT_EPOCH_DURATION_MS
RUN_DURATION=$DEFAULT_RUN_DURATION
NETWORK_METRIC=$DEFAULT_NETWORK_METRIC
SPAMMER_ENABLE=$DEFAULT_SPAMMER_ENABLE
SPAMMER_TPS=$DEFAULT_SPAMMER_TPS
SPAMMER_SIZE_PER_TX=$DEFAULT_SPAMMER_SIZE
SPAMMER_TYPE=$DEFAULT_SPAMMER_TYPE

# --- Parse command-line arguments ---
while getopts ":n:p:b:g:s:x:l:t:d:r:w:M:E:mS:T:Z:C:h" opt; do
  case "$opt" in
    n) NUM_VALIDATORS="$OPTARG" ;;
    p) PROTOCOL="$OPTARG" ;;
    b) BUILD="$OPTARG" ;;
    g) GEODISTRIBUTED="$OPTARG" ;;
    s) SEED="$OPTARG" ;;
    x) PERCENT_BLOCK="$OPTARG" ;;
    l) PERCENT_LOSS="$OPTARG" ;;
    t) RUN_DURATION="$OPTARG" ;;
    d) RESTART_DURATION="$OPTARG" ;;
    r) PERCENT_RESTART="$OPTARG" ;;
    w) RESTART_TIMEOUT="$OPTARG" ;;
    M) RESTART_MODE="$OPTARG" ;;
    E) EPOCH_DURATION_MS="$OPTARG" ;;
    m) NETWORK_METRIC=true ;;
    S) SPAMMER_ENABLE="$OPTARG" ;;
    T) SPAMMER_TPS="$OPTARG" ;;
    Z) SPAMMER_SIZE_PER_TX="$OPTARG" ;;
    C) SPAMMER_TYPE="$OPTARG";;
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
log "Restart duration           : $RESTART_DURATION s"
log "Restart timeout            : $RESTART_TIMEOUT s"
log "Restart mode               : $RESTART_MODE"
log "Epoch duration             : $EPOCH_DURATION_MS ms"
log "Run experiments duration   : $RUN_DURATION s"
log "Network metrics enabled    : $NETWORK_METRIC"
log "Spammer enabled            : $SPAMMER_ENABLE"
if [ "$SPAMMER_ENABLE" = true ]; then
  log "Spammer type               : $SPAMMER_TYPE"
  log "Spammer TPS                : $SPAMMER_TPS"
  if [ "$SPAMMER_TYPE" = "iota-spammer" ]; then
    log "Spammer size per tx        : $SPAMMER_SIZE_PER_TX"
  fi
fi
log "==========================="

# --- Pre-clean to ensure a fresh start (explicit, not via trap) ---
log "Pre-clean: invoking cleanup to ensure a fresh state before starting experiments..."
if [ -f "$CLEANUP_SCRIPT" ]; then
  if command -v sudo >/dev/null 2>&1; then
    log "Pre-clean (sudo): $CLEANUP_SCRIPT"
    if ! sudo bash -lc "cd '$NETWORK_DIR' && ./$(basename "$CLEANUP_SCRIPT")"; then
      rc=$?
      log "ERROR: pre-clean failed with exit code $rc — continuing anyway"
    else
      log "Pre-clean completed successfully."
    fi
  else
    log "Pre-clean (no sudo): $CLEANUP_SCRIPT"
    if ! (cd "$NETWORK_DIR" && "$CLEANUP_SCRIPT"); then
      rc=$?
      log "ERROR: pre-clean failed with exit code $rc — continuing anyway"
    else
      log "Pre-clean completed successfully."
    fi
  fi
else
  log "WARNING: cleanup script not found at $CLEANUP_SCRIPT; skipping pre-clean."
fi
# --- Ensure no old spammer instances are running before we begin ---
kill_spammer_processes || true

# --- 1) Build images (optional) ---
if [ "$BUILD" = true ]; then
  (cd ../../../docker/iota-node && ./build.sh -t iota-node)
  (cd ../../../docker/iota-tools && ./build.sh -t iota-tools)
  (cd ../../../docker/iota-indexer && ./build.sh -t iota-indexer)
else
  log "Skipping image builds"
fi

# --- 2) Bootstrap network ---
(cd .. && ./bootstrap.sh -n "$NUM_VALIDATORS" -e "$EPOCH_DURATION_MS")

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
./network-benchmark.sh \
    -n "$NUM_VALIDATORS" \
    -s "$SEED" \
    -b "$PERCENT_BLOCK" \
    -l "$PERCENT_LOSS" \
    -r "$PERCENT_RESTART" \
    -d "$RESTART_DURATION" \
    -w "$RESTART_TIMEOUT" \
    -M "$RESTART_MODE" \
    -g "$GEODISTRIBUTED" \
    -o "$LOG_FILE" &

# --- 6) Launch spammer if enabled ---
if [ "$SPAMMER_ENABLE" = true ]; then
    # Ensure faucet-1 is running (required by spammer)
    log "Starting faucet-1..."
    (cd .. && docker compose up -d faucet-1) || log "Warning: could not start faucet-1"
    log "Sleep 20s after faucet start..."
    sleep 20
    SPAMMER_DURATION=$((RUN_DURATION - 60))
    if [ "$SPAMMER_DURATION" -lt 10 ]; then
      SPAMMER_DURATION=10
    fi

    if [ "$SPAMMER_TYPE" = "stress" ]; then
            log "Starting 'stress' benchmark with TPS=$SPAMMER_TPS, duration=${SPAMMER_DURATION}s..."
            # This command runs the `stress` binary from the iota-tools image inside the docker network
            docker run -d --rm --name stress-benchmark \
              --network iota-private-network_iota-network \
              -v "$(pwd)/../configs/genesis/genesis.blob:/opt/iota/config/genesis.blob:ro" \
              -v "$(pwd)/../configs/faucet/iota.keystore:/opt/iota/config/iota.keystore:ro" \
              iotaledger/iota-tools /usr/local/bin/stress \
                --local false \
                --use-fullnode-for-execution true \
                --fullnode-rpc-addresses http://fullnode-1:9000 \
                --genesis-blob-path /opt/iota/config/genesis.blob \
                --keystore-path /opt/iota/config/iota.keystore \
                --primary-gas-owner-id 0x7cc6ff19b379d305b8363d9549269e388b8c1515772253ed4c868ee80b149ca0 \
                bench \
                --target-qps "$SPAMMER_TPS" \
                --in-flight-ratio 5 \
                --transfer-object 100


            # Follow the logs of the detached container and redirect to the spammer log file
            docker logs -f stress-benchmark > "$LOG_DIR/spammer.log" 2>&1 &


    elif [ "$SPAMMER_TYPE" = "iota-spammer" ]; then
            USER_HOME=$(getent passwd "${SUDO_USER:-$USER}" | cut -d: -f6)
            SPAMMER_SCRIPT="${SPAMMER_SCRIPT:-$USER_HOME/iota-spammer/scripts/spamming_fuzz_test.sh}"
            if [ ! -f "$SPAMMER_SCRIPT" ]; then
              log "Error: Spammer script not found at $SPAMMER_SCRIPT"
              exit 1
            fi
            log "Starting 'iota-spammer' with TPS=$SPAMMER_TPS, size per tx=$SPAMMER_SIZE_PER_TX, duration=${SPAMMER_DURATION}s..."
            if [ -n "${SUDO_USER:-}" ]; then
              log "Detected sudo; running spammer as $SUDO_USER to inherit user Rust toolchain"
              sudo -u "$SUDO_USER" -H bash "$SPAMMER_SCRIPT" \
                -T "$SPAMMER_TPS" \
                -s "$SPAMMER_SIZE_PER_TX" \
                -d "${SPAMMER_DURATION}s" \
                > "$LOG_DIR/spammer.log" 2>&1 &
            else
              bash "$SPAMMER_SCRIPT" \
                -T "$SPAMMER_TPS" \
                -s "$SPAMMER_SIZE_PER_TX" \
                -d "${SPAMMER_DURATION}s" \
                > "$LOG_DIR/spammer.log" 2>&1 &
            fi
        else
            log "Error: Unknown SPAMMER_TYPE '$SPAMMER_TYPE'. Must be 'iota-spammer' or 'stress'."
            exit 1
        fi

        SPAM_PID=$!
        log "Spammer started in background (pid=$SPAM_PID); logs: $LOG_DIR/spammer.log"
    fi

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

  docker logs "$v" &> "$LOG_DIR/experiment-${v}-${TIMESTAMP}.log"
  cp "$LOG_DIR/experiment-${v}-${TIMESTAMP}.log" "$LOG_DIR/experiment-${v}-latest.log"

  log "Saved final log for $v to $LOG_DIR/experiment-${v}-${TIMESTAMP}.log"
done

# Copy main experiment log with timestamp
cp "$LOG_FILE" "$LOG_DIR/experiment_script_${TIMESTAMP}.log"

# Copy spammer log with timestamp (if enabled and present)
if [ "$SPAMMER_ENABLE" = true ] && [ -f "$LOG_DIR/spammer.log" ]; then
  cp "$LOG_DIR/spammer.log" "$LOG_DIR/experiment-spammer-${TIMESTAMP}.log"
  cp "$LOG_DIR/spammer.log" "$LOG_DIR/experiment-spammer-latest.log"
  log "Saved spammer log to $LOG_DIR/experiment-spammer-${TIMESTAMP}.log"
fi

log "All steps completed. Cleanup will run on script exit."