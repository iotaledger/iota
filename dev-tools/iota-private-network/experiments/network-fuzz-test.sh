#!/bin/bash
set -euo pipefail
IFS=$'\n\t'
SEED=${SEED:-$(date +%s)}
RANDOM=$SEED
echo "Seeding RANDOM with $SEED"

# network-fuzz-test.sh
# Run a 24h fuzzy random network disruption test across validators.


# === LOCKING: Prevent multiple instances ===
LOCKFILE="/tmp/network-fuzz.lock"
if [ -e "$LOCKFILE" ]; then
  echo "❌ Fuzz test already running (lockfile exists)."
  exit 1
fi
trap 'rm -f "$LOCKFILE"' EXIT
touch "$LOCKFILE"

# === CONFIGURATION ===
duration_total=$((24 * 60 * 60))  # 24 hours
start_time=$(date +%s)
end_time=$((start_time + duration_total))

validators=(validator-1 validator-2 validator-3 validator-4)

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


  #  For each validator pair A-B, randomly apply one of five blocking actions with 1/20 probability each
  duration=$((RANDOM % 30 + 30))
  for ((i=0; i<${#validators[@]}; i++)); do
    for ((j=i+1; j<${#validators[@]}; j++)); do
      A=${validators[i]}
      B=${validators[j]}
      # bidirectional block
      if (( RANDOM % 20 == 0 )); then
        log "Blocking bidirectional traffic between $A and $B for ${duration}s"
        (iptables_block_bidirectional "$A" "$B"; sleep $duration; iptables_unblock_bidirectional "$A" "$B") &
      fi
      # outgoing from A to B
      if (( RANDOM % 20 == 1 )); then
        log "Blocking outgoing traffic from $A to $B for ${duration}s"
        (iptables_block "$A" "$B"; sleep $duration; iptables_unblock "$A" "$B") &
      fi
      # incoming to A from B
      if (( RANDOM % 20 == 2 )); then
        log "Blocking incoming traffic to $A from $B for ${duration}s"
        (iptables_block_incoming "$A" "$B"; sleep $duration; iptables_unblock_incoming "$A" "$B") &
      fi
      # outgoing from B to A
      if (( RANDOM % 20 == 3 )); then
        log "Blocking outgoing traffic from $B to $A for ${duration}s"
        (iptables_block "$B" "$A"; sleep $duration; iptables_unblock "$B" "$A") &
      fi
      # incoming to B from A
      if (( RANDOM % 20 == 4 )); then
        log "Blocking incoming traffic to $B from $A for ${duration}s"
        (iptables_block_incoming "$B" "$A"; sleep $duration; iptables_unblock_incoming "$B" "$A") &
      fi
    done
  done

  # Loop through validators
    for v in "${validators[@]}"; do
      duration=$((RANDOM % 30 + 30)) # between 30 and 60 seconds
      # randomly decide to stop validator or not
       if (( RANDOM % 10 )); then
         # stop and restart validator for the generated duration
        (restart_validator "$v" "$duration") &
        else
          # apply random package loss
          loss=$((RANDOM % 101))
          log "Applying ${loss}% packet loss to $v for ${duration}s"
           (netem_loss "$v" "$loss" "$duration") &
        fi
    done
done

# === CLEANUP ===
log "Cleaning up all validators"
for v in "${validators[@]}"; do
  docker unpause "$v" 2>/dev/null || true
  docker run --rm --privileged --net container:"$v" gaiadocker/iproute2 qdisc del dev eth0 root 2>/dev/null || true
  docker run --rm --privileged --net container:"$v" nicolaka/netshoot sh -c "iptables -F" 2>/dev/null || true
done

log "Fuzz test completed"