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
sleep_max=400       # maximum wait between actions (seconds)

log() {
  echo "$(date -Iseconds) $1"
}

cleanup_all() {
  log "Cleaning up all validators"
  for v in "${validators[@]}"; do
    docker unpause $v 2>/dev/null || true
    docker run --rm --privileged --net container:$v gaiadocker/iproute2 qdisc del dev eth0 root 2>/dev/null || true
    docker run --rm --privileged --net container:$v nicolaka/netshoot sh -c "iptables -F" 2>/dev/null || true
  done
}

trap 'echo "Interrupted! Cleaning up…"; cleanup_all; exit 1' INT TERM

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

  # 1) choose random subset of validators
  selected_validators=()
  for val in "${validators[@]}"; do
    if (( RANDOM % 2 )); then
      selected_validators+=("$val")
    fi
  done
  # ensure at least one validator
  if [ ${#selected_validators[@]} -eq 0 ]; then
    selected_validators+=("${validators[RANDOM % ${#validators[@]}]}")
  fi

  # 2) for each selected validator, choose random subset of actions and execute
  for v in "${selected_validators[@]}"; do
    # choose random subset of actions
    actions=("pause" "restart" "netem" "iptables")
    selected_actions=()
    for act in "${actions[@]}"; do
      if (( RANDOM % 2 )); then
        selected_actions+=("$act")
      fi
    done
    # ensure at least one action for this validator
    if [ ${#selected_actions[@]} -eq 0 ]; then
      selected_actions+=("${actions[RANDOM % ${#actions[@]}]}")
    fi

    # execute each selected action on validator $v
    for act in "${selected_actions[@]}"; do
      case $act in
        "pause")
          d=$((RANDOM % 60 + 30))
          ( pause_validator "$v" "$d" ) &
          ;;
        "restart")
          ( restart_validator "$v" ) &
          ;;
        "netem")
          p=${loss_levels[RANDOM % ${#loss_levels[@]}]}
          d=$((RANDOM % 60 + 30))
          ( netem_loss "$v" "$p" "$d" ) &
          ;;
        "iptables")
          # select a random peer
          peers=()
          for p2 in "${validators[@]}"; do
            [[ "$p2" != "$v" ]] && peers+=("$p2")
          done
          b="${peers[RANDOM % ${#peers[@]}]}"
          d=$((RANDOM % 60 + 60))
          # run block/unblock in background
          ( iptables_block "$v" "$b"; sleep "$d"; iptables_unblock "$v" "$b" ) &
          ;;
      esac
    done
  done
done

# === CLEANUP ===
log "Cleaning up all validators"
for v in "${validators[@]}"; do
  docker unpause $v 2>/dev/null || true
  docker run --rm --privileged --net container:$v gaiadocker/iproute2 qdisc del dev eth0 root 2>/dev/null || true
  docker run --rm --privileged --net container:$v nicolaka/netshoot sh -c "iptables -F" 2>/dev/null || true
done

log "Fuzz test completed"