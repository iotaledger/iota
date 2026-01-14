#!/bin/bash

# Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# Orchestrate: build images -> bootstrap -> run -> apply latencies -> fuzz -> wait/save logs
# Run from: iota/dev-tools/iota-private-network/experiments/

set -euo pipefail


# Prevent concurrent runs of run-all-fuzz.sh
exec {RUNALL_LOCKFD}>/tmp/run-all-fuzz.lock
if ! flock -n "$RUNALL_LOCKFD"; then
  echo "Another run-all-fuzz.sh is already running. Exiting."
  exit 1
fi

# --- Pre-clean lingering fuzzers & state (safe, idempotent) ---
echo "[run-all-fuzz] Pre-clean: killing old fuzzers and clearing state..."

# 1) Kill any existing network-fuzz.sh process groups (root-owned too)
sudo pkill -TERM -f 'network-fuzz\.sh' 2>/dev/null || true
sleep 1
sudo pkill -KILL -f 'network-fuzz\.sh' 2>/dev/null || true

# 2) Remove old stop/lock files (they auto-release FDs, but be thorough)
sudo rm -f /tmp/network-fuzz.stop /tmp/network-fuzz.lock /tmp/network-fuzz-single.lock /tmp/run-all-fuzz.lock 2>/dev/null || true

# 3) Clear iptables fuzz rules (v4) if any
nums=$(sudo iptables -L DOCKER-USER -n --line-numbers 2>/dev/null | awk '/fuzzdrop:/{print $1}' | sort -rn)
if [ -n "$nums" ]; then
  while read -r n; do [ -n "$n" ] && sudo iptables -D DOCKER-USER "$n" 2>/dev/null || true; done <<< "$nums"
fi

# 4) Clear tc qdiscs inside validators (adjust upper bound to your max)
for i in $(seq 1 30); do
  pid=$(docker inspect -f '{{.State.Pid}}' "validator-$i" 2>/dev/null || true)
  [ -z "$pid" ] || [ "$pid" = "0" ] && continue
  sudo nsenter -t "$pid" -n tc qdisc del dev eth0 root 2>/dev/null || true
done

# 5) Wait a moment until no fuzzers are shown anymore (race guard)
timeout=5
while pgrep -af 'network-fuzz\.sh' >/dev/null && [ $timeout -gt 0 ]; do
  sleep 1; timeout=$((timeout-1))
done
if pgrep -af 'network-fuzz\.sh' >/dev/null; then
  echo "[run-all-fuzz] WARNING: fuzzers still detected; proceeding but results may be tainted."
else
  echo "[run-all-fuzz] Pre-clean complete."
fi

PARENT_BASHPID=${BASHPID}
CLEANING=false

# =================== CONSTANTS ===================
DEFAULT_NUM_VALIDATORS=4
DEFAULT_PROTOCOL="mysticeti"
DEFAULT_BUILD=true
DEFAULT_TOPOLOGY="false"         # maps to fuzz topology: true|false|ring|star|non-triangle|random|geo-high|geo-low
DEFAULT_SEED=42
DEFAULT_PERCENT_BLOCK=0
DEFAULT_PERCENT_LOSS=0
DEFAULT_PERCENT_RESTART=0
DEFAULT_RUN_DURATION=3600        # 1h
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOG_DIR="$SCRIPT_DIR/logs"
LOG_INTERVAL=60
DEFAULT_NETWORK_METRIC=false
DEFAULT_SPAMMER_ENABLE=false
DEFAULT_SPAMMER_TPS=10
DEFAULT_SPAMMER_SIZE="10KiB"
DEFAULT_SPAMMER_TYPE="stress"

# Fuzz extras (env-overridable)
DEFAULT_FUZZ_TTL="${FUZZ_TTL:-0}"                 # seconds; 0 = disabled
DEFAULT_FUZZ_ROUND_SPAN="${FUZZ_ROUND_SPAN:-0}"   # seconds; 0 = 2*RESTART_DURATION in fuzz
DEFAULT_FUZZ_RESTART_DURATION="${FUZZ_RESTART_DURATION:-120}"  # seconds
# healing (env-overridable)
DEFAULT_HEAL_EVERY_ROUND="${HEAL_EVERY_ROUND:-0}"   # 0 = disabled
DEFAULT_HEAL_NUM_ROUNDS="${HEAL_NUM_ROUNDS:-0}"

NETWORK_DIR="$(dirname "$SCRIPT_DIR")"
CLEANUP_SCRIPT="$NETWORK_DIR/cleanup.sh"
# ==================================================

# --- helpers ---
container_exists() { docker ps -a --format '{{.Names}}' | grep -qx "$1"; }
container_running() { docker ps --format '{{.Names}}' | grep -qx "$1"; }

# --- Trap termination and normal exit safely ---
CLEANED_UP=false
cleanup_and_kill() {
    if [ "${BASHPID}" != "${PARENT_BASHPID}" ]; then
          return
    fi
    if [ "${CLEANING}" = true ]; then
        log "cleanup already running; returning"
        return
    fi
    CLEANING=true
    trap - SIGINT SIGTERM EXIT
    trap '' SIGINT SIGTERM

    set +e
    : "${SCRIPT_DIR:=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)}"
    : "${LOG_DIR:=${SCRIPT_DIR}/logs}"
    : "${LOG_FILE:=${LOG_DIR}/experiment_script_latest.log}"
    mkdir -p "$LOG_DIR" 2>/dev/null || true
    touch "$LOG_FILE" 2>/dev/null || true
    log "BEGIN cleanup_and_kill (pid=$$, user=$(id -un))"

    kill_spammer_processes || true

    if [ "$CLEANED_UP" = false ]; then
        if [ "$NETWORK_METRIC" = true ]; then
          echo "=== Final network stats for validators ==="
          for ((i=1; i<=NUM_VALIDATORS; i++)); do
              v="validator-$i"
              if container_running "$v"; then
                tx_bytes=$(docker exec "$v" cat /sys/class/net/eth0/statistics/tx_bytes 2>/dev/null || echo 0)
                rx_bytes=$(docker exec "$v" cat /sys/class/net/eth0/statistics/rx_bytes 2>/dev/null || echo 0)
                tx_packets=$(docker exec "$v" cat /sys/class/net/eth0/statistics/tx_packets 2>/dev/null || echo 0)
                rx_packets=$(docker exec "$v" cat /sys/class/net/eth0/statistics/rx_packets 2>/dev/null || echo 0)
                tx_mb=$(awk "BEGIN {printf \"%.2f\", $tx_bytes/1024/1024}")
                rx_mb=$(awk "BEGIN {printf \"%.2f\", $rx_bytes/1024/1024}")
                tx_packets_fmt=$(printf "%'d" "$tx_packets")
                rx_packets_fmt=$(printf "%'d" "$rx_packets")
                echo ">>> $v <<<"
                echo "TX: $tx_packets_fmt packets, $tx_mb MB"
                echo "RX: $rx_packets_fmt packets, $rx_mb MB"
                echo
              else
                echo ">>> $v <<< (not running)"
              fi
          done
        fi

        CLEANED_UP=true
        log "Stopping all background scripts and validators..."
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
    log "Killing lingering spammer and fuzz processes (if any) and removing locks..."

    # Stop stress benchmark container if present
    docker stop stress-benchmark >/dev/null 2>&1 || true

    # ---- Kill spammer processes ----
    pkill -9 -f 'iota-spammer spammer spam' 2>/dev/null || true
    pkill -9 -f 'cargo run --release -- spammer spam' 2>/dev/null || true
    pkill -9 -f 'cargo run --release --.* stress' 2>/dev/null || true
    pkill -9 -f 'spamming_fuzz_test.sh' 2>/dev/null || true
    pkill -9 -f 'network-fuzz.sh' 2>/dev/null || true

    # ---- Kill leftover fuzz/orchestrator loops ----
    pkill -TERM -f 'docker (logs|inspect|exec).*validator-' 2>/dev/null || true
    sleep 1
    pkill -KILL -f 'docker (logs|inspect|exec).*validator-' 2>/dev/null || true

    # If we ever invoked via sudo, sweep root-owned copies too
    if command -v sudo >/dev/null 2>&1; then
        sudo pkill -TERM -f 'network-fuzz\.sh' 2>/dev/null || true
        sudo pkill -TERM -f 'docker (logs|inspect|exec).*validator-' 2>/dev/null || true
        sleep 1
        sudo pkill -KILL -f 'network-fuzz\.sh' 2>/dev/null || true
        sudo pkill -KILL -f 'docker (logs|inspect|exec).*validator-' 2>/dev/null || true
    fi

    # ---- Remove old lock files ----
    rm -f /tmp/spammer-*.lock /tmp/spammer.lock /tmp/network-fuzz*.lock /tmp/run-all-fuzz.lock 2>/dev/null || true
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
  echo "          [-t topology(true|false|ring|star|non-triangle|random|geo-high|geo-low)] [-s seed(number)]"
  echo "          [-x percent_block_connection(0..100)] [-l percent_loss_packets(0..100)]"
  echo "          [-d run_duration_seconds] [-r percent_restart(0..100)] [-m]"
  echo "          [-S spammer_enable(true|false)] [-T spammer_tps(number)] [-Z spammer_size_per_tx(KiB)] [-C spammer_type(iota-spammer|stress)]"
  echo "Env overrides: FUZZ_TTL (seconds), FUZZ_ROUND_SPAN (seconds), FUZZ_RESTART_DURATION (seconds)"
}

# --- Default values ---
NUM_VALIDATORS=$DEFAULT_NUM_VALIDATORS
PROTOCOL=$DEFAULT_PROTOCOL
BUILD=$DEFAULT_BUILD
TOPOLOGY=$DEFAULT_TOPOLOGY
SEED=$DEFAULT_SEED
PERCENT_BLOCK=$DEFAULT_PERCENT_BLOCK
PERCENT_LOSS=$DEFAULT_PERCENT_LOSS
PERCENT_RESTART=$DEFAULT_PERCENT_RESTART
RUN_DURATION=$DEFAULT_RUN_DURATION
NETWORK_METRIC=$DEFAULT_NETWORK_METRIC
SPAMMER_ENABLE=$DEFAULT_SPAMMER_ENABLE
SPAMMER_TPS=$DEFAULT_SPAMMER_TPS
SPAMMER_SIZE_PER_TX=$DEFAULT_SPAMMER_SIZE
SPAMMER_TYPE=$DEFAULT_SPAMMER_TYPE
FUZZ_TTL=$DEFAULT_FUZZ_TTL
FUZZ_ROUND_SPAN=$DEFAULT_FUZZ_ROUND_SPAN
FUZZ_RESTART_DURATION=$DEFAULT_FUZZ_RESTART_DURATION
HEAL_EVERY_ROUND=$DEFAULT_HEAL_EVERY_ROUND
HEAL_NUM_ROUNDS=$DEFAULT_HEAL_NUM_ROUNDS

# --- Parse command-line arguments ---
# NOTE: -t is TOPOLOGY, -d is RUN DURATION
while getopts ":n:p:b:t:s:x:l:d:r:mS:T:Z:C:h" opt; do
  case "$opt" in
    n) NUM_VALIDATORS="$OPTARG" ;;
    p) PROTOCOL="$OPTARG" ;;
    b) BUILD="$OPTARG" ;;
    t) TOPOLOGY="$OPTARG" ;;
    s) SEED="$OPTARG" ;;
    x) PERCENT_BLOCK="$OPTARG" ;;
    l) PERCENT_LOSS="$OPTARG" ;;
    d) RUN_DURATION="$OPTARG" ;;
    r) PERCENT_RESTART="$OPTARG" ;;
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

# --- Basic validation ---
case "$RUN_DURATION" in ''|*[!0-9]*) echo "Error: -d RUN_DURATION must be an integer (seconds)"; exit 2;; esac
case "$PERCENT_BLOCK" in ''|*[!0-9]*) echo "Error: -x must be 0..100"; exit 2;; esac
case "$PERCENT_LOSS" in ''|*[!0-9]*) echo "Error: -l must be 0..100"; exit 2;; esac
case "$PERCENT_RESTART" in ''|*[!0-9]*) echo "Error: -r must be 0..100"; exit 2;; esac

# --- Ensure correct directory ---
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
[[ "$(basename "$SCRIPT_DIR")" != "experiments" ]] && { log "Error: run from experiments/"; exit 1; }

# --- Summary ---
log "=== SUMMARY ==="
log "Number of validators       : $NUM_VALIDATORS"
log "Consensus protocol         : $PROTOCOL"
log "Rebuild images             : $BUILD"
log "Topology flag              : $TOPOLOGY"
log "Seed                       : $SEED"
log "Percent block connection   : $PERCENT_BLOCK"
log "Percent netem loss         : $PERCENT_LOSS"
log "Percent restart validator  : $PERCENT_RESTART"
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
log "Fuzz TTL (env)             : ${FUZZ_TTL}"
log "Fuzz round span (env)      : ${FUZZ_ROUND_SPAN}"
log "Fuzz restart duration (env): ${FUZZ_RESTART_DURATION}"
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
(cd .. && ./bootstrap.sh -n "$NUM_VALIDATORS")

# --- 3) Bring up docker network ---
(cd .. && ./run.sh -n "$NUM_VALIDATORS" -p "$PROTOCOL")

log "Sleep 5s to boot validators..."
sleep 5

# --- 4) Run grafana dashboard if not already running ---
GRAFANA_DIR="../../grafana-local"
cd "$GRAFANA_DIR" || { log "Grafana folder not found"; exit 1; }

if docker compose ps --services --filter "status=running" | grep -q grafana; then
  log "Grafana already running, skipping start"
else
  log "Starting Grafana dashboard..."
  docker compose up -d
fi
log "Grafana URL: http://localhost:3000/dashboards"
cd - >/dev/null

# --- Map TOPOLOGY to fuzz topology ---
TOPOLOGY_ARG="$TOPOLOGY"
case "$TOPOLOGY_ARG" in
  true)  TOPOLOGY_ARG="geo-high" ;;
  false) TOPOLOGY_ARG="geo-low" ;;
  ring|star|non-triangle|random|geo-high|geo-low) ;;
  *)     TOPOLOGY_ARG="random" ;;
esac

# --- Fuzz script log (separate, timestamped by fuzz itself unless overridden) ---
FUZZ_TS="$(date +%Y%m%d-%H%M%S)"
FUZZ_LOG="$LOG_DIR/fuzz_${FUZZ_TS}.log"

# removing stop lock file
sudo rm -f /tmp/network-fuzz.stop /tmp/network-fuzz-single.lock 2>/dev/null || true


# --- extra pre-clean to avoid leaking previous fuzz runs ---
if command -v sudo >/dev/null 2>&1; then
  # kill any leftover fuzzers (root background jobs)
  sudo pkill -f './network-fuzz.sh' 2>/dev/null || true
  sudo pkill -f 'network-fuzz.sh' 2>/dev/null || true

  # clear old fuzzdrop rules if any
  sudo iptables -L DOCKER-USER -n --line-numbers 2>/dev/null \
    | awk '/fuzzdrop:/{print $1}' \
    | sort -rn \
    | while read -r num; do
        sudo iptables -D DOCKER-USER "$num" 2>/dev/null || true
      done
fi

# --- 5) Launch combined latency + fuzz watcher in background ---
HEAL_EVERY_ROUND="${HEAL_EVERY_ROUND}" \
HEAL_NUM_ROUNDS="${HEAL_NUM_ROUNDS}" \
./network-fuzz.sh \
    -n "$NUM_VALIDATORS" \
    -s "$SEED" \
    -b "$PERCENT_BLOCK" \
    -l "$PERCENT_LOSS" \
    -r "$PERCENT_RESTART" \
    -t "$TOPOLOGY_ARG" \
    -d "$FUZZ_RESTART_DURATION" \
    --round-span "${FUZZ_ROUND_SPAN}" \
    --ttl "${FUZZ_TTL}" \
    -o "$FUZZ_LOG" &

FUZZ_PID=$!
log "Fuzz script started (pid=$FUZZ_PID), log: $FUZZ_LOG"

# --- 6) Launch spammer if enabled ---
if [ "$SPAMMER_ENABLE" = true ]; then
    log "Starting faucet-1..."
    (cd .. && docker compose up -d faucet-1) || log "Warning: could not start faucet-1"
    log "Sleep 20s after faucet start..."
    sleep 20
    SPAMMER_DURATION=$((RUN_DURATION - 60))
    if [ "$SPAMMER_DURATION" -lt 10 ]; then SPAMMER_DURATION=10; fi

    if [ "$SPAMMER_TYPE" = "stress" ]; then
        log "Starting 'stress' benchmark with TPS=$SPAMMER_TPS, duration=${SPAMMER_DURATION}s..."
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
          log "Detected sudo; running spammer as $SUDO_USER"
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

# --- 7) Run for specified duration, periodically saving logs (guarded) ---
log "Running experiments for $RUN_DURATION seconds, saving logs every $LOG_INTERVAL seconds..."
start_time=$(date +%s)
end_time=$((start_time + RUN_DURATION))

while [[ $(date +%s) -lt $end_time ]]; do
  for ((i=1; i<=NUM_VALIDATORS; i++)); do
    v="validator-$i"
    if container_exists "$v"; then
      tmp="$LOG_DIR/.exp-${v}-latest.tmp"
      dest="$LOG_DIR/exp-${v}-latest.log"
      if docker logs "$v" > "$tmp" 2>&1; then
        mv -f "$tmp" "$dest"
      else
        rm -f "$tmp" 2>/dev/null || true
        # optional: log a warning once in a while
        # log "Warn: docker logs failed for $v (leaving previous $dest intact)"
      fi
    fi
  done
  sleep "$LOG_INTERVAL"
done

# --- 8) Final log save with timestamp (guarded) ---
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
for ((i=1; i<=NUM_VALIDATORS; i++)); do
  v="validator-$i"
  if container_exists "$v"; then
    tmp="$LOG_DIR/.experiment-${v}-${TIMESTAMP}.tmp"
    dest_ts="$LOG_DIR/experiment-${v}-${TIMESTAMP}.log"
    dest_latest="$LOG_DIR/experiment-${v}-latest.log"

    if docker logs "$v" > "$tmp" 2>&1; then
      mv -f "$tmp" "$dest_ts"
      cp -f "$dest_ts" "$dest_latest"
      log "Saved final log for $v to $dest_ts"
    else
      rm -f "$tmp" 2>/dev/null || true
      log "WARNING: final docker logs failed for $v; keeping previous latest file (if any)"
    fi
    log "Saved final log for $v to $LOG_DIR/experiment-${v}-${TIMESTAMP}.log"
  else
    log "Skipping final log for $v (container not present)"
  fi
done

# --- 9) Copy main experiment log with timestamp ---
cp "$LOG_FILE" "$LOG_DIR/experiment_script_${TIMESTAMP}.log" 2>/dev/null || true


# --- 10) Copy spammer log with timestamp (if enabled and present)
if [ "$SPAMMER_ENABLE" = true ] && [ -f "$LOG_DIR/spammer.log" ]; then
  cp "$LOG_DIR/spammer.log" "$LOG_DIR/experiment-spammer-${TIMESTAMP}.log"
  cp "$LOG_DIR/spammer.log" "$LOG_DIR/experiment-spammer-latest.log"
  log "Saved spammer log to $LOG_DIR/experiment-spammer-${TIMESTAMP}.log"
fi
log "All steps completed. Cleanup will run on script exit."