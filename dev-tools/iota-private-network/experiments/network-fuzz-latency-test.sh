#!/usr/bin/env bash
# Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail
IFS=$'\n\t'

# Default number of validators
NUM_VALIDATORS=${NUM_VALIDATORS:-4}
# Parse optional -n flag for number of validators
while getopts "n:" opt; do
  case "$opt" in
    n) NUM_VALIDATORS="$OPTARG" ;;
    *) echo "Usage: $0 [-n num_validators]"; exit 1 ;;
  esac
done
shift $((OPTIND -1))

# Initialize random seed
SEED=${SEED:-$(date +%s)}
RANDOM=$SEED
echo "Seeding RANDOM with $SEED"

# Build validators list
validators=()
for i in $(seq 1 "$NUM_VALIDATORS"); do
  validators+=(validator-"$i")
done

# Trap to clean up at exit
cleanup_latency() {
  for A in "${validators[@]}"; do
    # clear any qdisc on each validator
    docker run --rm --privileged --net container:"$A" nicolaka/netshoot \
      sh -c "tc qdisc del dev eth0 root || true; iptables -t mangle -F" >/dev/null 2>&1 || true
  done
}
trap 'cleanup_latency' EXIT

log() {
  echo "[$(date -Iseconds)] $*"
}

# Helper functions
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

clear_latency() {
  local A=$1 B=$2
  mark_pair_remove() {
    local A=$1 B=$2
    local IPB
    IPB=$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$B")
    docker run --rm --privileged --net container:"$A" nicolaka/netshoot \
      sh -c "iptables -t mangle -D OUTPUT -d ${IPB} -j MARK --set-mark 1 || true"
  }
  mark_pair_remove "$A" "$B"
  docker run --rm --privileged --net container:"$A" nicolaka/netshoot \
    sh -c "tc qdisc del dev eth0 root || true"
}

# Main loop: apply random latency per pair, run 180s, clear, rest 60s, repeat
while true; do
  log "Starting latency injection phase (180s)"
  # apply to all pairs
  for ((i=0; i<${#validators[@]}; i++)); do
    for ((j=i+1; j<${#validators[@]}; j++)); do
      A=${validators[i]}
      B=${validators[j]}
      # pick random delays and jitters for each direction
      D1=$((RANDOM % 50 + 10)) J1=$((RANDOM % 50))
      D2=$((RANDOM % 50 + 10)) J2=$((RANDOM % 50))
      log "Injecting ${D1}ms±${J1}ms latency from $A to $B"
      mark_pair "$A" "$B"
      apply_latency "$A" "${D1}ms" "${J1}ms"
      log "Injecting ${D2}ms±${J2}ms latency from $B to $A"
      mark_pair "$B" "$A"
      apply_latency "$B" "${D2}ms" "${J2}ms"
    done
  done

  sleep 180

  log "Clearing all latency rules"
  cleanup_latency

  log "Rest phase (60s)"
  sleep 60
done