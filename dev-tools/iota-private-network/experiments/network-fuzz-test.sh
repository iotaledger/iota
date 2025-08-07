#!/bin/bash

# Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# Run a long-time fuzzy random network disruption test across validators.

set -euo pipefail
trap 'echo "Error! Cleaning up…"; cleanup_all' ERR
IFS=$'\n\t'


log() {
  echo "$(date -Iseconds) $1"
}

# Detect consensus protocol from validator-1 container
CONSENSUS_PROTOCOL=$(
  docker exec validator-1 sh -c 'env | grep "^CONSENSUS_PROTOCOL=" || true' \
  | sed -n 's/^CONSENSUS_PROTOCOL=//p'
)
case "${CONSENSUS_PROTOCOL:-}" in
  starfish|Starfish)
    PROTOCOL_NAME="Starfish"
    ;;
  *)
    PROTOCOL_NAME="Mysticeti"
    ;;
esac
log "Using consensus protocol: ${PROTOCOL_NAME}"

# === PROBABILITIES & DURATIONS ===
BLOCK_PROB=50      # 1 in BLOCK_PROB chance to trigger each block type per pair and disruption type
STOP_PROB=10       # percentage chance (0-100) to stop a validator
LOSS_PROB=25       # percentage chance (0-100) to apply packet loss
MIN_DURATION=60    # minimum disruption duration (seconds)
MAX_DURATION=300   # maximum disruption duration (seconds)

# Define total test duration as a constant (default: 1h)
TEST_DURATION=$((1 * 60 * 60))   # total fuzz test time in seconds (default: 1h)




# === LOCKING: Prevent multiple instances ===
LOCKFILE="/tmp/network-fuzz.lock"
if [ -e "$LOCKFILE" ]; then
  echo "Error: Fuzz test already running (lockfile exists)."
  exit 1
fi
trap 'rm -f "$LOCKFILE"' EXIT
touch "$LOCKFILE"

# === CONFIGURATION ===
# duration_total=$((1 * 60 * 60))  # 1 hours
duration_total=${TEST_DURATION}

NUM_VALIDATORS=4
SEED=${SEED:-42}
while getopts "n:s:" opt; do
  case "$opt" in
    n) NUM_VALIDATORS="$OPTARG" ;;
    s) SEED="$OPTARG" ;;
    *) echo "Usage: $0 [-n num_validators] [-s seed]"; exit 1 ;;
  esac
done
shift $((OPTIND-1))

start_time=$(date +%s)
end_time=$((start_time + duration_total))

validators=()
LOG_DIR="./logs"
mkdir -p "$LOG_DIR"
echo "Logging into $LOG_DIR"

trap 'echo "Interrupted! Cleaning up…"; cleanup_all; exit 1' INT TERM EXIT
# Capture all script stdout/stderr to the script log
TIMESTAMP_START=$(date +%Y%m%d-%H%M%S)
SCRIPT_LOG="$LOG_DIR/fuzz-test-script-$TIMESTAMP_START.log"
exec > >(tee -a "$SCRIPT_LOG") 2>&1

# Seed RANDOM after logging is set up
echo "Seeding RANDOM with $SEED"
RANDOM=$SEED
for i in $(seq 1 "$NUM_VALIDATORS"); do
  validators+=(validator-"$i")
done

# Announce test start with selected validator count
echo "Starting network fuzz test with ${NUM_VALIDATORS} validators"

cleanup_all() {
  log "Cleaning up all validators"
  for v in "${validators[@]}"; do
    docker unpause "$v" 2>/dev/null || true
    docker run --rm --privileged --net container:"$v" gaiadocker/iproute2 qdisc del dev eth0 root 2>/dev/null || true
    docker run --rm --privileged --net container:"$v" nicolaka/netshoot sh -c "iptables -F" 2>/dev/null || true
    docker run --rm --privileged --net container:"$v" nicolaka/netshoot \
          sh -c "tc qdisc del dev eth0 root || true; iptables -t mangle -F" >/dev/null 2>&1 || true
  done
}

# === ACTION HELPERS ===

# == Pause - restart ==

pause_validator() {
  local v=$1 d=$2
  log "Pausing $v for ${d}s"
  docker pause "$v"
  sleep $d
  docker unpause "$v"
  log "Unpaused $v"
}

restart_validator() {
  local v=$1 d=$2
  docker stop "$v" >/dev/null 2>&1
  sleep $d
  docker start "$v" >/dev/null 2>&1
  log "Restarted $v"
}

# == Netem loss ==

netem_loss() {
  local v=$1 p=$2 d=$3
  log "Applying ${p}% packet loss to $v for ${d}s"
  docker run --rm --privileged --net container:"$v" gaiadocker/iproute2 qdisc del dev eth0 root 2>/dev/null || true
  docker run --rm --privileged --net container:"$v" gaiadocker/iproute2 qdisc add dev eth0 root netem loss ${p}%
  sleep $d
  docker run --rm --privileged --net container:"$v" gaiadocker/iproute2 qdisc del dev eth0 root
  log "Cleared netem loss on $v"
}

# == iptables ==

iptables_block() {
  local A=$1 B=$2
  local ipB
  ipB=$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$B")
  log "Blocking outbound traffic from $A to $B ($ipB)"
  docker run --rm --privileged --net container:"$A" nicolaka/netshoot sh -c "
    iptables -A OUTPUT -d $ipB -j DROP
  "
}

iptables_block_incoming() {
  local A=$1 B=$2
  local ipB
  ipB=$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$B")
  log "Blocking inbound traffic to $A from $B ($ipB)"
  docker run --rm --privileged --net container:"$A" nicolaka/netshoot sh -c "
    iptables -A INPUT -s $ipB -j DROP
  "
}

iptables_block_bidirectional() {
  local A=$1 B=$2
  iptables_block "$A" "$B"
  iptables_block_incoming "$A" "$B"
}

iptables_unblock() {
  local A=$1 B=$2
  local ipB
  ipB=$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$B")
  log "Unblocking outbound traffic from $A to $B ($ipB)"
  docker run --rm --privileged --net container:"$A" nicolaka/netshoot sh -c "
    iptables -D OUTPUT -d $ipB -j DROP 2>/dev/null || true
  "
}

iptables_unblock_incoming() {
  local A=$1 B=$2
  local ipB
  ipB=$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$B")
  log "Unblocking inbound traffic to $A from $B ($ipB)"
  docker run --rm --privileged --net container:"$A" nicolaka/netshoot sh -c "
    iptables -D INPUT -s $ipB -j DROP 2>/dev/null || true
  "
}

iptables_unblock_bidirectional() {
  local A=$1 B=$2
  iptables_unblock "$A" "$B"
  iptables_unblock_incoming "$A" "$B"
}

# == Latency ==

mark_pair() {
  local A=$1 B=$2
  local IPB
  IPB=$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$B")
  docker run --rm --privileged --net container:"$A" nicolaka/netshoot \
    sh -c "iptables -t mangle -A OUTPUT -d ${IPB} -j MARK --set-mark 1"
}

apply_latency() {
  local A=$1 delay=$2 jitter=$3
  docker run --rm --privileged --net container:"$A" nicolaka/netshoot \
    sh -c "tc qdisc del dev eth0 root 2>/dev/null || true; \
           tc qdisc add dev eth0 root netem delay ${delay} ${jitter}"
}

# == Clear all latency ==
clear_latency() {
  log "Clearing all latency rules"
  for v in "${validators[@]}"; do
    docker run --rm --privileged --net container:"$v" nicolaka/netshoot sh -c "
      tc qdisc del dev eth0 root 2>/dev/null || true
      iptables -t mangle -F
    " 2>/dev/null || true
  done
}

# === FUZZ LOOP ===
log "Starting fuzz test (duration: ${TEST_DURATION}s, validators: ${NUM_VALIDATORS})"
log "Warmup sleep for 30s"
sleep 30

while [[ $(date +%s) -lt $end_time ]]; do
  log "######################################"
  log "Starting latency injection phase"
    # apply to all pairs
    for ((i=0; i<${#validators[@]}; i++)); do
      for ((j=i+1; j<${#validators[@]}; j++)); do
        A=${validators[i]}
        B=${validators[j]}
        # pick random delays and jitters for each direction
        D1=$((RANDOM % 60 + 10)) J1=$((RANDOM % 30))
        D2=$((RANDOM % 50 + 10)) J2=$((RANDOM % 30))
        log "Injecting ${D1}ms±${J1}ms latency from $A to $B"
        mark_pair "$A" "$B"
        apply_latency "$A" "${D1}ms" "${J1}ms"
        log "Injecting ${D2}ms±${J2}ms latency from $B to $A"
        mark_pair "$B" "$A"
        apply_latency "$B" "${D2}ms" "${J2}ms"
      done
    done

  #  For each validator pair A-B, randomly apply one of five blocking actions with 1/50 probability each
  log "######################################"
  log "Start blocking traffic"
  duration=$(( RANDOM % (MAX_DURATION - MIN_DURATION + 1) + MIN_DURATION ))
  for ((i=0; i<${#validators[@]}; i++)); do
    for ((j=i+1; j<${#validators[@]}; j++)); do
      A=${validators[i]}
      B=${validators[j]}
      # bidirectional block
      if (( RANDOM % BLOCK_PROB == 0 )); then
        log "Blocking bidirectional traffic between $A and $B for ${duration}s"
        (iptables_block_bidirectional "$A" "$B"; sleep $duration; iptables_unblock_bidirectional "$A" "$B") &
      fi
      # outgoing from A to B
      if (( RANDOM % BLOCK_PROB == 1 )); then
        log "Blocking outgoing traffic from $A to $B for ${duration}s"
        (iptables_block "$A" "$B"; sleep $duration; iptables_unblock "$A" "$B") &
      fi
      # incoming to A from B
      if (( RANDOM % BLOCK_PROB == 2 )); then
        log "Blocking incoming traffic to $A from $B for ${duration}s"
        (iptables_block_incoming "$A" "$B"; sleep $duration; iptables_unblock_incoming "$A" "$B") &
      fi
      # outgoing from B to A
      if (( RANDOM % BLOCK_PROB == 3 )); then
        log "Blocking outgoing traffic from $B to $A for ${duration}s"
        (iptables_block "$B" "$A"; sleep $duration; iptables_unblock "$B" "$A") &
      fi
      # incoming to B from A
      if (( RANDOM % BLOCK_PROB == 4 )); then
        log "Blocking incoming traffic to $B from $A for ${duration}s"
        (iptables_block_incoming "$B" "$A"; sleep $duration; iptables_unblock_incoming "$B" "$A") &
      fi
    done
  done
  log "######################################"
  log "Start stopping validators and apply netem loss"
  # Loop through validators
    for v in "${validators[@]}"; do
      duration=$(( RANDOM % (MAX_DURATION - MIN_DURATION + 1) + MIN_DURATION )) # between MIN_DURATION and MAX_DURATION seconds
      loss=$((RANDOM % 41 + 10))       # 10–50% loss
      r=$(( RANDOM % 100 ))
        if (( r < STOP_PROB )); then
          log "Stopping $v for ${duration}s"
          (restart_validator "$v" "$duration") &
        elif (( r < LOSS_PROB )); then
          log "Applying ${loss}% packet loss to $v for ${duration}s"
          (netem_loss "$v" "$loss" "$duration") &
        else
          log "No disruption on $v"
        fi
    done
  sleep 1
  log "Experiments running for 300s"
  # Periodically overwrite intermediate logs
  for v in "${validators[@]}"; do
    docker logs "$v" &> "$LOG_DIR/fuzz-test-${v}-latest.log"
  done
  # Overwrite intermediate script log
  cp "$SCRIPT_LOG" "$LOG_DIR/fuzz-test-script-latest.log"
  sleep 300
    clear_latency
  # Recovery wait
  log "Recovery sleep for 60s"
    sleep 60
done

# === CLEANUP ===
log "Cleaning up all validators"
for v in "${validators[@]}"; do
  docker unpause "$v" 2>/dev/null || true
  docker run --rm --privileged --net container:"$v" gaiadocker/iproute2 qdisc del dev eth0 root 2>/dev/null || true
  docker run --rm --privileged --net container:"$v" nicolaka/netshoot sh -c "iptables -F" 2>/dev/null || true
done

# === SAVE LOGS ===
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
for v in "${validators[@]}"; do
  docker logs "$v" &> "$LOG_DIR/fuzz-test-$v-$TIMESTAMP.log"
  log "Saved logs for $v to $LOG_DIR/fuzz-test-$v-$TIMESTAMP.log"
done
cp "$SCRIPT_LOG" "$LOG_DIR/fuzz-test-script-latest.log"
  log "Saved script log to $LOG_DIR/fuzz-test-script-$TIMESTAMP.log"

log "Fuzz test completed and logs saved."