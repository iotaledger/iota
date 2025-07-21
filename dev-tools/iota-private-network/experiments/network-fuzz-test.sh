#!/bin/bash

# Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail
IFS=$'\n\t'
SEED=${SEED:-$(date +%s)}
RANDOM=$SEED
echo "Seeding RANDOM with $SEED"

# Run a 24h fuzzy random network disruption test across validators.


# === LOCKING: Prevent multiple instances ===
LOCKFILE="/tmp/network-fuzz.lock"
if [ -e "$LOCKFILE" ]; then
  echo "Error: Fuzz test already running (lockfile exists)."
  exit 1
fi
trap 'rm -f "$LOCKFILE"' EXIT
touch "$LOCKFILE"

# === CONFIGURATION ===
duration_total=$((24 * 60 * 60))  # 24 hours

# Parse optional -n flag for number of validators (default 4)
NUM_VALIDATORS=4
while getopts "n:" opt; do
  case "$opt" in
    n) NUM_VALIDATORS="$OPTARG" ;;
    *) echo "Usage: $0 [-n num_validators]"; exit 1 ;;
  esac
done
shift $((OPTIND-1))

start_time=$(date +%s)
end_time=$((start_time + duration_total))

# Build validators array based on NUM_VALIDATORS
validators=()
for i in $(seq 1 "$NUM_VALIDATORS"); do
  validators+=(validator-"$i")
done

# Announce test start with selected validator count
echo "Starting network fuzz test with ${NUM_VALIDATORS} validators"

log() {
  echo "$(date -Iseconds) $1"
}

cleanup_all() {
  log "Cleaning up all validators"
  for v in "${validators[@]}"; do
    docker unpause "$v" 2>/dev/null || true
    docker run --rm --privileged --net container:"$v" gaiadocker/iproute2 qdisc del dev eth0 root 2>/dev/null || true
    docker run --rm --privileged --net container:"$v" nicolaka/netshoot sh -c "iptables -F" 2>/dev/null || true
  done
}

trap 'echo "Interrupted! Cleaning up…"; cleanup_all; exit 1' INT TERM

# === ACTION HELPERS ===

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
  log "Stopping $v for ${d}s"
  docker stop "$v"
  sleep $d
  docker start "$v"
  log "Restarted $v"
}

netem_loss() {
  local v=$1 p=$2 d=$3
  log "Applying ${p}% packet loss to $v for ${d}s"
  docker run --rm --privileged --net container:"$v" gaiadocker/iproute2 qdisc add dev eth0 root netem loss ${p}%
  sleep $d
  docker run --rm --privileged --net container:"$v" gaiadocker/iproute2 qdisc del dev eth0 root
  log "Cleared netem loss on $v"
}

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

# === FUZZ LOOP ===
log "Starting 24h fuzz test"
while [[ $(date +%s) -lt $end_time ]]; do
  # recovery wait
  log "Recovery sleep for 60s"
  sleep 60


  #  For each validator pair A-B, randomly apply one of five blocking actions with 1/50 probability each
  duration=$((RANDOM % 120 + 60))
  for ((i=0; i<${#validators[@]}; i++)); do
    for ((j=i+1; j<${#validators[@]}; j++)); do
      A=${validators[i]}
      B=${validators[j]}
      # bidirectional block
      if (( RANDOM % 50 == 0 )); then
        log "Blocking bidirectional traffic between $A and $B for ${duration}s"
        (iptables_block_bidirectional "$A" "$B"; sleep $duration; iptables_unblock_bidirectional "$A" "$B") &
      fi
      # outgoing from A to B
      if (( RANDOM % 50 == 1 )); then
        log "Blocking outgoing traffic from $A to $B for ${duration}s"
        (iptables_block "$A" "$B"; sleep $duration; iptables_unblock "$A" "$B") &
      fi
      # incoming to A from B
      if (( RANDOM % 50 == 2 )); then
        log "Blocking incoming traffic to $A from $B for ${duration}s"
        (iptables_block_incoming "$A" "$B"; sleep $duration; iptables_unblock_incoming "$A" "$B") &
      fi
      # outgoing from B to A
      if (( RANDOM % 50 == 3 )); then
        log "Blocking outgoing traffic from $B to $A for ${duration}s"
        (iptables_block "$B" "$A"; sleep $duration; iptables_unblock "$B" "$A") &
      fi
      # incoming to B from A
      if (( RANDOM % 50 == 4 )); then
        log "Blocking incoming traffic to $B from $A for ${duration}s"
        (iptables_block_incoming "$B" "$A"; sleep $duration; iptables_unblock_incoming "$B" "$A") &
      fi
    done
  done

  # Loop through validators
    for v in "${validators[@]}"; do
      duration=$((RANDOM % 120 + 60)) # between 120 and 180 seconds
      loss=$((RANDOM % 41 + 10))       # 10–50% loss
      r=$((RANDOM % 100))
        if   (( r < 10 )); then
          log "Stopping $v for ${duration}s"
          (restart_validator "$v" "$duration") &
        elif (( r < 25 )); then
          log "Applying ${loss}% packet loss to $v for ${duration}s"
          (netem_loss "$v" "$loss" "$duration") &
        else
          log "No disruption on $v"
        fi
    done
  sleep 1
  log "Experiments running for 180s"
  sleep 180
done

# === CLEANUP ===
log "Cleaning up all validators"
for v in "${validators[@]}"; do
  docker unpause "$v" 2>/dev/null || true
  docker run --rm --privileged --net container:"$v" gaiadocker/iproute2 qdisc del dev eth0 root 2>/dev/null || true
  docker run --rm --privileged --net container:"$v" nicolaka/netshoot sh -c "iptables -F" 2>/dev/null || true
done

log "Fuzz test completed"