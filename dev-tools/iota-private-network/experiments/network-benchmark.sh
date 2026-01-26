#!/bin/bash

# Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# Apply fuzz disruptions deterministically using derived pseudorandom numbers
# Mimics latencies between docker containers
# Supports packet loss, connection blocking, and periodic validator restarts
# Logs to console only. Exits immediately, leaving disruptions applied.

set -euo pipefail
IFS=$'\n\t'


# --- Default configuration ---
NUMBER_VALIDATORS=4       # Number of validator containers
SEED=${SEED:-42}       # Seed for reproducibility of pseudorandom disruptions
PERCENT_BLOCK=0           # Percent chance to block a connection
PERCENT_LOSS=0           # Percent chance to apply packet loss
PERCENT_RESTART=0         # Percent of validators to stop and start after RESTART_DURATION seconds
RESTART_DURATION=120    # Seconds to stop validators during restart
RESTART_TIMEOUT=60      # Seconds to wait before restarting (timeout duration)
RESTART_MODE="preserve-consensus"  # restart mode: preserve-consensus | full-reset | simple-restart
GEODISTRIBUTED=false  # Large geodistributed latencies or small ones
LOG_FILE="logs/fuzz_script.log" # Output file for script

# --- Command-line arguments ---
while getopts "g:n:s:b:l:r:d:w:M:o:" opt; do
  case "$opt" in
    g) GEODISTRIBUTED="$OPTARG" ;;
    n) NUMBER_VALIDATORS="$OPTARG" ;;
    s) SEED="$OPTARG" ;;
    b) PERCENT_BLOCK="$OPTARG" ;;
    l) PERCENT_LOSS="$OPTARG" ;;
    r) PERCENT_RESTART="$OPTARG" ;;
    d) RESTART_DURATION="$OPTARG" ;;
    w) RESTART_TIMEOUT="$OPTARG" ;;
    M) RESTART_MODE="$OPTARG" ;;
    o) LOG_FILE="$OPTARG" ;;
    *) echo "Usage: $0 [-n num_validators] [-s seed] [-b percent_block] [-l percent_packet_loss] [-r percent_restart] [-d restart_duration] [-w restart_timeout] [-M restart_mode(preserve-consensus|full-reset|simple-restart)] [-g geodistributed_bool]"; exit 1 ;;
  esac
done
shift $((OPTIND-1))



# --- Logging helper ---
log() {
    echo "$(date -Iseconds) $1" >> "$LOG_FILE"
}


# --- Prepare validator list ---
validators=()
for i in $(seq 1 "$NUMBER_VALIDATORS"); do
  validators+=(validator-"$i")
done


# === RTT latency table ===
RTT_LATENCY_TABLE=(
  "1 14 104 112 198 65 68 110 201 146"
  "14 1 106 122 196 78 67 103 189 142"
  "104 106 1 215 281 163 29 50 143 238"
  "112 122 215 1 309 175 176 220 299 254"
  "198 196 281 309 1 137 254 268 150 101"
  "65 78 163 175 137 1 127 172 226 108"
  "68 67 29 176 254 127 1 38 125 199"
  "110 103 50 220 268 172 38 1 148 245"
  "201 189 143 299 150 226 125 148 1 140"
  "146 142 238 254 101 108 199 245 140 1"
)

# === Subfunctions ===

# latency_from_table(i, j)
# Returns RTT between validator i and j from RTT table, scaled by GEODISTRIBUTED.
latency_from_table() {
  local i=$1 j=$2
  local size=${#RTT_LATENCY_TABLE[@]}
  local idx_i=$(( i % size ))
  local idx_j=$(( j % size ))
  IFS=' ' read -r -a row <<< "${RTT_LATENCY_TABLE[$idx_i]}"
  local val=${row[$idx_j]}

  local divisor
  if [ "$GEODISTRIBUTED" = true ]; then
    divisor=2
  else
    divisor=8
  fi

  local res=$(( val / divisor ))
  if [ "$res" -gt "$val" ]; then
    res=$MAX
  fi

  echo "$res"
}


# container_pid(container)
# Returns host PID of Docker container
container_pid() { docker inspect -f '{{.State.Pid}}' "$1"; }

# Apply latency and mark packets from container A → B
apply_and_mark() {
  local A=$1 B=$2
  local D=$3 J=$4
  local IPB pid
  local lockfile="/var/lock/apply_and_mark_${A}.lock"

  # Acquire exclusive lock for this container pair
  exec 200>"$lockfile"
  until flock -n 200; do
      sleep 0.1
  done

  # Get container PID and target IP
  pid=$(container_pid "$A")
  # Skip if container doesn't have a valid PID (not fully started yet)
  if [ -z "$pid" ] || [ "$pid" = "0" ]; then
    flock -u 200
    return 0
  fi

  IPB=$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$B")
  # Skip if unable to get IP address
  if [ -z "$IPB" ]; then
    flock -u 200
    return 0
  fi

  # Derive a per-destination mark from the validator index of B
  local idxB mark classid
  idxB=${B#validator-}
  mark=${idxB:-1}
  classid="1:$((100 + mark))"

  # Ensure a classful root qdisc exists once per container
  if ! nsenter -t "$pid" -n tc qdisc show dev eth0 2>/dev/null | grep -q "htb 1:"; then
    nsenter -t "$pid" -n tc qdisc del dev eth0 root 2>/dev/null || true
    nsenter -t "$pid" -n tc qdisc add dev eth0 root handle 1: htb default 1 2>/dev/null || \
      log "Warning: failed to create htb root qdisc for $A"
    nsenter -t "$pid" -n tc class add dev eth0 parent 1: classid 1:1 htb rate 1000mbit ceil 1000mbit 2>/dev/null || true
  fi

  # Mark packets A → B inside the container namespace (idempotent)
  if ! nsenter -t "$pid" -n iptables -t mangle -C OUTPUT -d "${IPB}" -j MARK --set-mark "$mark" 2>/dev/null; then
    nsenter -t "$pid" -n iptables -t mangle -A OUTPUT -d "${IPB}" -j MARK --set-mark "$mark" 2>/dev/null || \
      log "Warning: failed to mark traffic from $A → $B"
  fi

  # Create/update a dedicated class and netem qdisc for this destination
  nsenter -t "$pid" -n tc class replace dev eth0 parent 1: classid "$classid" htb rate 1000mbit ceil 1000mbit 2>/dev/null || true
  nsenter -t "$pid" -n tc qdisc replace dev eth0 parent "$classid" handle "${mark}0:" netem delay "${D}ms" "${J}ms" 2>/dev/null || \
    log "Warning: failed to apply latency to $A → $B"

  # Attach a filter that routes marked packets into the class
  if ! nsenter -t "$pid" -n tc filter show dev eth0 parent 1: 2>/dev/null | grep -q "fh $mark .* flowid $classid"; then
    nsenter -t "$pid" -n tc filter add dev eth0 parent 1: protocol ip handle "$mark" fw flowid "$classid" 2>/dev/null || \
      log "Warning: failed to attach tc filter for $A → $B"
  fi

  # Release lock automatically when function exits
  flock -u 200
}

# apply netem loss for packetes
apply_loss() {
  local A=$1 percent=$2
  local pid; pid=$(container_pid "$A")
  nsenter -t "$pid" -n tc qdisc del dev eth0 root 2>/dev/null || true
  nsenter -t "$pid" -n tc qdisc add dev eth0 root netem loss "${percent}%"
  log "Applied ${percent}% packet loss to $A"
}

# block connection between a given pair of addresses
block_connection() {
  local A=$1 B=$2
  local pid ipB
  ipB=$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$B")
  pid=$(container_pid "$A")
  nsenter -t "$pid" -n iptables -A OUTPUT -d "$ipB" -j DROP
  log "Blocked traffic $A → $B"
}

# Restart a validator container with configurable database handling.
# Supports three modes:
#   - preserve-consensus: Remove only authorities_db, keep consensus_db
#   - full-reset: Remove both authorities_db and consensus_db
#   - simple-restart: Don't remove any databases, clean docker restart only
restart_validator() {
 local v=$1 d=$2 timeout=${3:-60} mode=${4:-preserve-consensus}
 log "Stopping $v..."
 docker stop "$v" >/dev/null 2>&1

 local validator_num=${v#validator-}
 local base_path="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/data/validator-${validator_num}"

 case "$mode" in
   preserve-consensus)
     # Remove only authorities_db, keep consensus_db
     log "Restart mode: preserve-consensus (removing authorities_db only)"
     local db_path="$base_path/authorities_db"
     if [ -d "$db_path" ]; then
       log "Node database found at: $db_path (size: $(du -sh "$db_path" 2>/dev/null | cut -f1))"
       rm -rf "$db_path" || log "Error: Failed to remove node database"
       [ ! -d "$db_path" ] && log "Successfully deleted node database for $v"
     else
       log "Warning: Node database not found at $db_path"
     fi
     ;;

   full-reset)
     # Remove both databases
     log "Restart mode: full-reset (removing both authorities_db and consensus_db)"
     for db in authorities_db consensus_db; do
       local db_path="$base_path/$db"
       if [ -d "$db_path" ]; then
         log "Removing $db at: $db_path (size: $(du -sh "$db_path" 2>/dev/null | cut -f1))"
         rm -rf "$db_path" || log "Error: Failed to remove $db"
         [ ! -d "$db_path" ] && log "Successfully deleted $db for $v"
       else
         log "Warning: $db not found at $db_path"
       fi
     done
     ;;

   simple-restart)
     # Don't remove any databases
     log "Restart mode: simple-restart (no database deletion)"
     ;;

   *)
     log "Error: Unknown restart mode: $mode"
     ;;
 esac

 log "Waiting $timeout seconds before restarting $v..."
 sleep "$timeout"

 # Restart the validator
 docker start "$v" >/dev/null 2>&1
 log "Restarted $v"
}

# apply fuzz network conditions
initially_apply_fuzz() {
  for ((i=0; i<NUMBER_VALIDATORS; i++)); do
     A=${validators[i]}


    for ((j=i+1; j<NUMBER_VALIDATORS; j++)); do

      B=${validators[j]}

      r_block_A=$(( RANDOM % 100 ))
      r_block_B=$(( RANDOM % 100 ))


      (( r_block_A < PERCENT_BLOCK )) && block_connection "$A" "$B"
      (( r_block_B < PERCENT_BLOCK )) && block_connection "$B" "$A"
    done
  done

  num_to_apply_loss=$(( (NUMBER_VALIDATORS * PERCENT_LOSS + 50) / 100 ))

  indices=($(seq 0 $((NUMBER_VALIDATORS - 1))))
  # Shuffle indices
  for ((i=NUMBER_VALIDATORS-1; i>0; i--)); do
    j=$(( RANDOM % (i+1) ))
    tmp=${indices[i]}
    indices[i]=${indices[j]}
    indices[j]=$tmp
  done

  # Apply netem loss for packets to chosen validators
  for ((k=0; k<num_to_apply_loss; k++)); do
    A=${validators[indices[k]]}
    LOSS=$((RANDOM % 31 + 10 ))
    apply_loss "$A" "$LOSS"
  done
}

restart_loop() {
  sleep "$RESTART_DURATION"
  if (( PERCENT_RESTART == 0 )); then
    log "PERCENT_RESTART=0, skipping validator restarts"
    return
  fi

  while true; do
    num_to_restart=$(( (NUMBER_VALIDATORS * PERCENT_RESTART + 50) / 100 ))
    log "Restart round: $num_to_restart validators (duration=$RESTART_DURATION)"

    indices=($(seq 0 $((NUMBER_VALIDATORS - 1))))
    # Shuffle indices
    for ((i=NUMBER_VALIDATORS-1; i>0; i--)); do
      j=$(( RANDOM % (i+1) ))
      tmp=${indices[i]}
      indices[i]=${indices[j]}
      indices[j]=$tmp
    done

    # Restart chosen validators
    for ((k=0; k<num_to_restart; k++)); do
      v=${validators[indices[k]]}  # <-- fixed
      restart_validator "$v" "$RESTART_DURATION" "$RESTART_TIMEOUT" "$RESTART_MODE" &
    done
    log "Don't change restarts for duration=$(( 2 * RESTART_DURATION ))"
    sleep $(( 2 * RESTART_DURATION ))
  done
}


initially_apply_latency() {
  # --- Apply latencies for all pairs ---
  for ((i=0; i<${#validators[@]}; i++)); do
    for ((j=i+1; j<${#validators[@]}; j++)); do
      A=${validators[i]} B=${validators[j]}
      D1=$(latency_from_table $i $j)
      D2=$(latency_from_table $j $i)
      J1=$((RANDOM % 3)) J2=$((RANDOM % 3))
      log "Injecting ${D1}ms±${J1}ms latency $A → $B"
      log "Injecting ${D2}ms±${J2}ms latency $B → $A"
      apply_and_mark "$A" "$B" "$D1" "$J1" &
      apply_and_mark "$B" "$A" "$D2" "$J2" &
    done
  done
}
# --- State for fuzz ---
declare -A fuzz_block_targets  # validator -> list of blocked validators
declare -A fuzz_loss_amount    # validator -> netem loss %
for v in "${validators[@]}"; do
    fuzz_block_targets["$v"]=""    # empty string = no targets yet
    fuzz_loss_amount["$v"]=0       # default 0% loss
done

# reapply rules in case some validators are restarted
reapply_latencies_and_fuzz_loop() {
    sleep 1
    log "Starting latency + fuzz watcher loop"

    # Initialize fuzz state if empty
    if [ ${#fuzz_block_targets[@]} -eq 0 ]; then
        for ((i=0; i<NUMBER_VALIDATORS; i++)); do
            A=${validators[i]}

            # Decide which validators A blocks
            blocks=()
            for ((j=0; j<NUMBER_VALIDATORS; j++)); do
                [ "$i" -eq "$j" ] && continue
                (( RANDOM % 100 < PERCENT_BLOCK )) && blocks+=("${validators[j]}")
            done
            fuzz_block_targets["$A"]="${blocks[*]}"

            # Decide netem loss for A
            if (( RANDOM % 100 < PERCENT_LOSS )); then
                fuzz_loss_amount["$A"]=$(( RANDOM % 31 + 10 ))
            else
                fuzz_loss_amount["$A"]=0
            fi
        done
    fi
    sleep 1
    while true; do
        for v in "${validators[@]}"; do
            # Skip if container is not running
            if ! docker ps --format '{{.Names}}' | grep -q "^${v}\$"; then
                continue
            fi

            pid=$(container_pid "$v")
            # Skip if container doesn't have a valid PID yet (still starting up)
            if [ -z "$pid" ] || [ "$pid" = "0" ]; then
                continue
            fi

            if ! nsenter -t "$pid" -n tc qdisc show dev eth0 | grep -q "netem"; then
                log "Reapplying latency + fuzz for $v (container restarted or tc removed)"

                # --- Reapply latency ---
                for u in "${validators[@]}"; do
                    [ "$v" = "$u" ] && continue
                    v_idx=${v#validator-}
                    u_idx=${u#validator-}
                    D=$(latency_from_table "$((v_idx - 1))" "$((u_idx - 1))")
                    J=$((RANDOM % 3))
                    apply_and_mark "$v" "$u" "$D" "$J" &
                done

                # --- Reapply fuzz (blocking) ---
                for target in ${fuzz_block_targets["$v"]}; do
                    block_connection "$v" "$target"
                done

                # --- Reapply fuzz (netem loss) ---
                loss=${fuzz_loss_amount["$v"]}
                (( loss > 0 )) && apply_loss "$v" "$loss"
            fi
        done
        sleep 1
    done
}

# === Main ===
log "Starting fuzz manager"
RANDOM=$SEED

# Initially set latencies
initially_apply_latency

# Initially set fuzz rules
initially_apply_fuzz

# Reapply latencies and fuzz rules every second
reapply_latencies_and_fuzz_loop &

# Restart validator loop
restart_loop &


wait
