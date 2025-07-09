#!/bin/bash

# network-fuzz-test.sh
# Run a 24h fuzzy random network disruption test across validators.

# === CONFIGURATION ===
duration_total=$((24 * 60 * 60))  # 24 hours
start_time=$(date +%s)
end_time=$((start_time + duration_total))

validators=(validator-1 validator-2 validator-3 validator-4)
loss_levels=(10 30 50 70 90)
sleep_min=60        # minimum wait between actions (seconds)
sleep_max=600       # maximum wait between actions (seconds)

log() {
  echo "$(date -Iseconds) $1"
}

# === ACTION HELPERS ===

pause_validator() {
  local v=$1 d=$2
  log "Pausing $v for ${d}s"
  docker pause $v
  sleep $d
  docker unpause $v
  log "Unpaused $v"
}

restart_validator() {
  local v=$1
  log "Restarting $v"
  docker restart $v
  log "Restarted $v"
}

netem_loss() {
  local v=$1 p=$2 d=$3
  log "Applying ${p}% packet loss to $v for ${d}s"
  docker run --rm --privileged --net container:$v gaiadocker/iproute2 qdisc add dev eth0 root netem loss ${p}%
  sleep $d
  docker run --rm --privileged --net container:$v gaiadocker/iproute2 qdisc del dev eth0 root
  log "Cleared netem loss on $v"
}

iptables_block() {
  local A=$1 B=$2
  local ipB
  ipB=$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' $B)
  log "Blocking traffic A->$B on $A"
  docker run --rm --privileged --net container:$A nicolaka/netshoot sh -c "
    iptables -A OUTPUT -d $ipB -j DROP
  "
}

iptables_unblock() {
  local A=$1 B=$2
  local ipB
  ipB=$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' $B)
  log "Unblocking traffic A->$B on $A"
  docker run --rm --privileged --net container:$A nicolaka/netshoot sh -c "
    iptables -D OUTPUT -d $ipB -j DROP 2>/dev/null || true
  "
}

# === FUZZ LOOP ===
log "Starting 24h fuzz test"
while [[ $(date +%s) -lt $end_time ]]; do
  # random wait
  wait_time=$((RANDOM % (sleep_max - sleep_min + 1) + sleep_min))
  log "Sleeping for ${wait_time}s"
  sleep $wait_time

  # select a random action
  action=$((RANDOM % 4))
  # select a random validator
  v=${validators[RANDOM % ${#validators[@]}]}

  case $action in
    0)  # pause/unpause
      d=$((RANDOM % 60 + 30))
      pause_validator $v $d
      ;;
    1)  # restart
      restart_validator $v
      ;;
    2)  # netem packet loss
      p=${loss_levels[RANDOM % ${#loss_levels[@]}]}
      d=$((RANDOM % 60 + 30))
      netem_loss $v $p $d
      ;;
    3)  # iptables block/unblock with random peer
      peers=("${validators[@]/$v}")
      b=${peers[RANDOM % ${#peers[@]}]}
      iptables_block $v $b
      # block duration
      d=$((RANDOM % 60 + 60))
      sleep $d
      iptables_unblock $v $b
      ;;
  esac
done

# === CLEANUP ===
log "Cleaning up all validators"
for v in "${validators[@]}"; do
  docker unpause $v 2>/dev/null || true
  docker run --rm --privileged --net container:$v gaiadocker/iproute2 qdisc del dev eth0 root 2>/dev/null || true
  docker run --rm --privileged --net container:$v nicolaka/netshoot sh -c "iptables -F" 2>/dev/null || true
done

log "Fuzz test completed"