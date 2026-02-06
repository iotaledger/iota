#!/bin/bash

# Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# network-fuzz.sh — deterministic network fuzzing for validator clusters
# Host-level iptables drops + in-namespace tc

set -euo pipefail
IFS=$'\n\t'

PID_REAPPLY=""
PID_RESTART=""
PID_REBAL=""

# ---------------------------------------------------------------------
# Timestamped log default
# ---------------------------------------------------------------------
TIMESTAMP="$(date +%Y%m%d-%H%M%S)"
LOG_FILE="logs/fuzz_script_${TIMESTAMP}.log"
echo "Log file will be: $LOG_FILE"
mkdir -p "$(dirname "$LOG_FILE")"

log(){ echo "$(date -Iseconds) $1" | tee -a "$LOG_FILE" >/dev/null; }

# ---------------------------------------------------------------------
# Defaults
# ---------------------------------------------------------------------
NUMBER_VALIDATORS=4
SEED=42
PERCENT_BLOCK=0       # % unordered pairs to block (bidirectional)
PERCENT_LOSS=0        # % nodes with 1–5% loss
PERCENT_RESTART=0     # % nodes to restart per round
RESTART_DURATION=120  # seconds a subset of validators is stopped during each normal round
TOPOLOGY="geo-low"     # random | geo-high | geo-low | ring | star | non-triangle
ROUND_SPAN=0          # duration of rounds, defaults to  2*RESTART_DURATION

# healing (env override from dual-run)
HEAL_EVERY_ROUND=${HEAL_EVERY_ROUND:-0}   # 0 = disabled
HEAL_NUM_ROUNDS=${HEAL_NUM_ROUNDS:-0}

# optional TTL for whole run (0 = no TTL)
TTL_SECS=0

# ---------------------------------------------------------------------
# Args
# ---------------------------------------------------------------------
while (( "$#" )); do
  case "${1:-}" in
    -n) NUMBER_VALIDATORS="$2"; shift 2;;
    -s) SEED="$2"; shift 2;;
    -b) PERCENT_BLOCK="$2"; shift 2;;
    -l) PERCENT_LOSS="$2"; shift 2;;
    -r) PERCENT_RESTART="$2"; shift 2;;
    -d) RESTART_DURATION="$2"; shift 2;;
    -o) LOG_FILE="$2"; shift 2;;
    -t|--topology) TOPOLOGY="$2"; shift 2;;
    --round-span) ROUND_SPAN="$2"; shift 2;;
    --ttl) TTL_SECS="$2"; shift 2;;
    -h|--help)
      echo "Usage: $0 [-n N] [-s SEED] [-b %block] [-l %loss] [-r %restart] [-d restart_s] [-o logfile] [-t topology] [--round-span secs] [--ttl secs]"
      exit 0;;
    *) break;;
  esac
done

# re-open log file if -o was given after first echo
mkdir -p "$(dirname "$LOG_FILE")"

# ---------------------------------------------------------------------
# Global lock & stopfile
# ---------------------------------------------------------------------
exec {LOCKFD}>/tmp/network-fuzz.lock
STOPFILE="/tmp/network-fuzz.stop"

# single-instance guard
exec {FUZZ_LOCKFD}>/tmp/network-fuzz-single.lock
if ! flock -n "$FUZZ_LOCKFD"; then
  echo "Another network-fuzz.sh is already running. Exiting."
  exit 1
fi

# ---------------------------------------------------------------------
# Deterministic PRNG
# ---------------------------------------------------------------------
PRNG_STATE=$(( (SEED ^ 0x9E3779B9) & 0xFFFFFFFF ))
prng_next_u32(){
  local x=$PRNG_STATE
  x=$(( (x ^ (x << 13)) & 0xFFFFFFFF ))
  x=$(( (x ^ (x >> 17)) & 0xFFFFFFFF ))
  x=$(( (x ^ (x << 5)) & 0xFFFFFFFF ))
  PRNG_STATE=$x
  echo $x
}
rand_range(){ local lo=$1 hi=$2; echo $(( lo + ($(prng_next_u32) % (hi - lo + 1)) )); }
det_shuffle(){ local -a a=("$@"); local n=${#a[@]}; for ((i=n-1;i>0;i--)); do j=$(rand_range 0 "$i"); tmp=${a[i]}; a[i]=${a[j]}; a[j]=$tmp; done; printf '%s\n' "${a[@]}"; }
derive_round_seed(){ local r=$1; echo $(( (SEED + r * 2654435761) & 0xFFFFFFFF )); }

# ---------------------------------------------------------------------
# Validators + IPs
# ---------------------------------------------------------------------
validators=(); for i in $(seq 1 "$NUMBER_VALIDATORS"); do validators+=(validator-"$i"); done
all_idx=($(seq 0 $((NUMBER_VALIDATORS-1))))
declare -A VALIDATOR_IP

get_container_ip(){
  local name=$1
  docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$name" 2>/dev/null || true
}

refresh_all_ips(){
  for i in "${all_idx[@]}"; do
    local v=${validators[$i]}
    VALIDATOR_IP["$i"]="$(get_container_ip "$v")"
  done
}

# ---------------------------------------------------------------------
# Latency tables
# ---------------------------------------------------------------------
RTT_GEO=(
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
RTT_RING=(
  "0 15 30 60 120 240 120 60 30 15"
  "15 0 15 30 60 120 240 120 60 30"
  "30 15 0 15 30 60 120 240 120 60"
  "60 30 15 0 15 30 60 120 240 120"
  "120 60 30 15 0 15 30 60 120 240"
  "240 120 60 30 15 0 15 30 60 120"
  "120 240 120 60 30 15 0 15 30 60"
  "60 120 240 120 60 30 15 0 15 30"
  "30 60 120 240 120 60 30 15 0 15"
  "15 30 60 120 240 120 60 30 15 0"
)
RTT_STAR=(
  "0 25 25 25 25 25 25 25 25 25"
  "25 0 200 200 200 200 200 200 200 200"
  "25 200 0 200 200 200 200 200 200 200"
  "25 200 200 0 200 200 200 200 200 200"
  "25 200 200 200 0 200 200 200 200 200"
  "25 200 200 200 200 0 200 200 200 200"
  "25 200 200 200 200 200 0 200 200 200"
  "25 200 200 200 200 200 200 0 200 200"
  "25 200 200 200 200 200 200 200 0 200"
  "25 200 200 200 200 200 200 200 200 0"
)
RTT_NONTRI=(
  "0 500 20 25 450 30 470 35 460 40"
  "500 0 25 460 30 450 35 440 40 430"
  "20 25 0 510 30 490 35 480 40 470"
  "25 460 510 0 20 480 25 470 30 460"
  "450 30 30 20 0 520 25 510 30 500"
  "30 450 490 480 520 0 20 490 25 480"
  "470 35 35 25 25 20 0 530 20 520"
  "35 440 480 470 510 490 530 0 15 510"
  "460 40 40 30 30 25 20 15 0 540"
  "40 430 470 460 500 480 520 510 540 0"
)

rtt_lookup(){
  local i=$1 j=$2; local -n tab=$3; local size=${#tab[@]}
  local ii=$(( i % size )); local jj=$(( j % size ))
  IFS=' ' read -r -a row <<< "${tab[$ii]}"
  echo "${row[$jj]}"
}

# ---------------------------------------------------------------------
# State tables for topology and enforcement
# ---------------------------------------------------------------------
# BLOCK_EDGE["i|j"]  → 1 if validators i and j should be blocked logically
#                     (i.e. no communication between these indices in this round)
#                     This is an in-memory description of the intended network cut.
#
# LAT_MS["i|j"]      → simulated RTT between validators i and j, in milliseconds.
#
# LOSS_PCT_NODE["i"] → packet-loss percentage (1–5 %) applied via tc to node i.
#
# HOST_DROPS["ipA|ipB"] → 1 if an iptables DROP rule is currently active between
#                          container IPs ipA and ipB (both directions installed).
#                          Used to track the *actual* host-level enforcement so that
#                          cleanup and re-application can remove exactly th
declare -A BLOCK_EDGE      # "i|j" -> 0/1
declare -A LAT_MS          # "i|j" -> ms
declare -A LOSS_PCT_NODE   # "i"   -> 0 or [1..5]
declare -A HOST_DROPS      # "ipA|ipB" -> 1 (both dirs installed)

# ---------------------------------------------------------------------
# Build latencies
# ---------------------------------------------------------------------
build_latencies(){
  local mode="$1" MAX_LAT=399 base raw
  for i in "${all_idx[@]}"; do
    for j in "${all_idx[@]}"; do
      if [[ $i -eq $j ]]; then
        LAT_MS["$i|$j"]=0
        continue
      fi
      case "$mode" in
        geo-high)      raw=$(rtt_lookup "$i" "$j" RTT_GEO); base=$(( raw / 2 )); (( base < 1 )) && base=1 ;;
        geo-low)       raw=$(rtt_lookup "$i" "$j" RTT_GEO); base=$(( raw / 8 )); (( base < 1 )) && base=1 ;;
        ring)          base=$(rtt_lookup "$i" "$j" RTT_RING) ;;
        star)          base=$(rtt_lookup "$i" "$j" RTT_STAR) ;;
        non-triangle)  base=$(rtt_lookup "$i" "$j" RTT_NONTRI) ;;
        random|*)      base=$(rand_range 20 "$MAX_LAT") ;;
      esac
      (( base > MAX_LAT )) && base=$MAX_LAT
      LAT_MS["$i|$j"]=$base
    done
  done
}

# ---------------------------------------------------------------------
# Loss per node
# ---------------------------------------------------------------------
select_losses(){
  local NUM_LOSS=$(( (NUMBER_VALIDATORS * PERCENT_LOSS + 50) / 100 ))
  local loss_candidates=($(det_shuffle "${all_idx[@]}"))
  for k in "${all_idx[@]}"; do LOSS_PCT_NODE["$k"]=0; done
  for ((t=0; t<NUM_LOSS; t++)); do
    local node=${loss_candidates[$t]}
    LOSS_PCT_NODE["$node"]=$(rand_range 1 5)
  done
}

# ---------------------------------------------------------------------
# tc on each container (via nsenter)
# ---------------------------------------------------------------------
apply_node_qdisc(){
  local v_idx="$1"
  local v=${validators[$v_idx]}
  local pid; pid=$(docker inspect -f '{{.State.Pid}}' "$v" 2>/dev/null || true)
  [[ -z "$pid" || "$pid" = "0" ]] && return 0

  # Ensure a classful root qdisc exists once per container
  if ! sudo nsenter -t "$pid" -n tc qdisc show dev eth0 2>/dev/null | grep -q "htb 1:"; then
    sudo nsenter -t "$pid" -n tc qdisc del dev eth0 root 2>/dev/null || true
    sudo nsenter -t "$pid" -n tc qdisc add dev eth0 root handle 1: htb default 1 2>/dev/null || true
    sudo nsenter -t "$pid" -n tc class add dev eth0 parent 1: classid 1:1 htb rate 1000mbit ceil 1000mbit 2>/dev/null || true
  fi

  # Clear existing filters so we can re-attach per-destination ones
  sudo nsenter -t "$pid" -n tc filter del dev eth0 parent 1: 2>/dev/null || true

  local loss=${LOSS_PCT_NODE["$v_idx"]:-0}

  # Apply per-destination netem based on LAT_MS[i|j]
  local j ipB base jitter classid
  for j in "${all_idx[@]}"; do
    [[ $v_idx -eq $j ]] && continue

    ipB=${VALIDATOR_IP["$j"]}
    [[ -z "$ipB" ]] && continue

    base=${LAT_MS["$v_idx|$j"]}
    [[ -z "$base" ]] && base=0
    jitter=$(( (base>5) ? 3 : 1 ))
    classid="1:$((100 + j))"

    sudo nsenter -t "$pid" -n tc class replace dev eth0 parent 1: classid "$classid" htb rate 1000mbit ceil 1000mbit 2>/dev/null || true

    if (( loss > 0 )); then
      sudo nsenter -t "$pid" -n tc qdisc replace dev eth0 parent "$classid" netem delay "${base}ms" "${jitter}ms" loss "${loss}%" 2>/dev/null || true
    else
      sudo nsenter -t "$pid" -n tc qdisc replace dev eth0 parent "$classid" netem delay "${base}ms" "${jitter}ms" 2>/dev/null || true
    fi

    sudo nsenter -t "$pid" -n tc filter add dev eth0 parent 1: protocol ip u32 match ip dst "${ipB}/32" flowid "$classid" 2>/dev/null || true
  done
}

apply_all_latencies_and_loss_once(){
  log "Applying tc to all validators"
  refresh_all_ips
  for i in "${all_idx[@]}"; do
    apply_node_qdisc "$i"
  done
}

# ---------------------------------------------------------------------
# HOST-LEVEL iptables helpers
# ---------------------------------------------------------------------
ensure_docker_user_chain(){
  # ensure chain exists
  sudo iptables -nL DOCKER-USER >/dev/null 2>&1 || sudo iptables -N DOCKER-USER
  # ensure FORWARD jumps to it (sometimes missing)
  if ! sudo iptables -C FORWARD -j DOCKER-USER >/dev/null 2>&1; then
    sudo iptables -I FORWARD -j DOCKER-USER || true
  fi
}

host_add_drop_pair(){
  local ipA="$1" ipB="$2"
  [[ -z "$ipA" || -z "$ipB" ]] && return 0

  # forward: A -> B
  if ! sudo iptables -C DOCKER-USER -s "$ipA" -d "$ipB" -j DROP 2>/dev/null; then
    sudo iptables -A DOCKER-USER -s "$ipA" -d "$ipB" -j DROP -m comment --comment "fuzzdrop:$ipA->$ipB" || true
  fi
  # backward: B -> A
  if ! sudo iptables -C DOCKER-USER -s "$ipB" -d "$ipA" -j DROP 2>/dev/null; then
    sudo iptables -A DOCKER-USER -s "$ipB" -d "$ipA" -j DROP -m comment --comment "fuzzdrop:$ipB->$ipA" || true
  fi

  HOST_DROPS["$ipA|$ipB"]=1
}

host_clear_all_drops(){
  #  delete all fuzzdrop rules, bottom-up
  local nums
  nums=$(sudo iptables -L DOCKER-USER -n --line-numbers 2>/dev/null \
           | awk '/fuzzdrop:/{print $1}' \
           | sort -rn)
  if [ -n "$nums" ]; then
    while read -r num; do
      [ -z "$num" ] && continue
      sudo iptables -D DOCKER-USER "$num" 2>/dev/null || true
    done <<< "$nums"
  fi

  HOST_DROPS=()
  log "Cleared fuzzdrop rules from DOCKER-USER"
}

apply_all_blocks_host(){
  ensure_docker_user_chain
  refresh_all_ips
  for i in "${all_idx[@]}"; do
    for j in "${all_idx[@]}"; do
      [[ $i -eq $j ]] && continue
      if [[ ${BLOCK_EDGE["$i|$j"]} -eq 1 ]]; then
        local ipA=${VALIDATOR_IP["$i"]}
        local ipB=${VALIDATOR_IP["$j"]}
        host_add_drop_pair "$ipA" "$ipB"
      fi
    done
  done
  log "Applied host-level blocks"
}

# ---------------------------------------------------------------------
# Build exact % of bidirectional blocks
# ---------------------------------------------------------------------
build_blocks_bidir_exact(){
  for i in "${all_idx[@]}"; do
    for j in "${all_idx[@]}"; do
      BLOCK_EDGE["$i|$j"]=0
    done
  done

  local -a pairs=()
  for ((i=0;i<NUMBER_VALIDATORS;i++)); do
    for ((j=i+1;j<NUMBER_VALIDATORS;j++)); do
      pairs+=("$i|$j")
    done
  done

  local M=${#pairs[@]}
  local NUM_BLOCKS=$(( (M * PERCENT_BLOCK + 50) / 100 ))

  local -a shuffled
  mapfile -t shuffled < <(det_shuffle "${pairs[@]}")

  for ((k=0; k<NUM_BLOCKS && k<M; k++)); do
    local p="${shuffled[$k]}"
    local i="${p%%|*}"
    local j="${p##*|}"
    BLOCK_EDGE["$i|$j"]=1
    BLOCK_EDGE["$j|$i"]=1
  done

  log "Built bidirectional block set: ${NUM_BLOCKS}/${M} pairs (~${PERCENT_BLOCK}%)"
}

# ---------------------------------------------------------------------
# Watcher: reapply tc (not iptables — those are host-level)
# ---------------------------------------------------------------------
reapply_latencies_loop(){
  log "Starting tc reapply watcher"
  while true; do
    [[ -f "$STOPFILE" ]] && { log "Watcher stopping"; return 0; }
    refresh_all_ips
    for i in "${all_idx[@]}"; do
      apply_node_qdisc "$i"
    done
    sleep 5
  done
}

# ---------------------------------------------------------------------
# Restart plan
# ---------------------------------------------------------------------
#ROUNDS=1000
#declare -a RESTART_BATCH

# number of validators to restart in each restart round
# BATCH_SIZE=$(( (NUMBER_VALIDATORS * PERCENT_RESTART + 50) / 100 ))

#for r in $(seq 0 $((ROUNDS-1))); do
#  pick=($(det_shuffle "${all_idx[@]}"))
#  line=""
#  for ((t=0; t<BATCH_SIZE; t++)); do line+="${pick[$t]}"$'\n'; done
#  RESTART_BATCH[$r]="$line"
#done

restart_validator(){
  local v=$1 d=$2
  log "Stopping $v for ${d}s..."
  docker stop "$v" >/dev/null 2>&1 || true
  sleep "$d"
  docker start "$v" >/dev/null 2>&1 || true
  log "Restarted $v"
}

restart_loop(){
  # number of validators to restart in each restart round
  BATCH_SIZE=$(( (NUMBER_VALIDATORS * PERCENT_RESTART + 50) / 100 ))
  sleep "$RESTART_DURATION"
  local r=0
  while true; do
    [[ -f "$STOPFILE" ]] && { log "Restart loop stopping"; return 0; }

    local heal_round=0
    if (( HEAL_EVERY_ROUND > 0 )); then
      if (( (r % HEAL_EVERY_ROUND) < HEAL_NUM_ROUNDS )); then
        heal_round=1
      fi
    fi

    if (( heal_round == 1 )); then
      log "Restart loop: heal window r=$r — skip restarts"
    else
      if (( PERCENT_RESTART > 0 && BATCH_SIZE > 0 )); then
        # ---------- deterministic per-round PRNG ----------
        local local_seed=$(( (SEED + r * 2654435761) & 0xFFFFFFFF ))
        local local_state=$local_seed

        det_shuffle_local() {
          local -a arr=("$@")
          local n=${#arr[@]}
          local i j tmp
          for ((i=n-1; i>0; i--)); do
            # xorshift-style local PRNG (same idea as global one)
            local_state=$(( (local_state ^ (local_state << 13)) & 0xFFFFFFFF ))
            local_state=$(( (local_state ^ (local_state >> 17)) & 0xFFFFFFFF ))
            local_state=$(( (local_state ^ (local_state << 5)) & 0xFFFFFFFF ))
            j=$(( local_state % (i+1) ))
            tmp=${arr[i]}
            arr[i]=${arr[j]}
            arr[j]=$tmp
          done
          printf '%s\n' "${arr[@]}"
        }

        # get deterministic permutation for this round
        local picks
        mapfile -t picks < <(det_shuffle_local "${all_idx[@]}")

        local n_batch=$BATCH_SIZE
        (( n_batch > ${#picks[@]} )) && n_batch=${#picks[@]}

        log "Restart round r=$r: restarting ${n_batch} validators (indexes: ${picks[*]:0:n_batch})"

        local t idx v
        for ((t=0; t<n_batch; t++)); do
          idx="${picks[$t]}"
          v=${validators[$idx]}
          restart_validator "$v" "$RESTART_DURATION" &
        done
      else
        log "Restart loop: PERCENT_RESTART=0 or BATCH_SIZE=0"
      fi
    fi

    r=$((r+1))
    sleep $(( 2 * RESTART_DURATION ))
  done
}

# ---------------------------------------------------------------------
# Rebalance / heal loop
# ---------------------------------------------------------------------
rebalance_blocks_loop(){
  if (( ROUND_SPAN <= 0 )); then
    ROUND_SPAN=$(( 2 * RESTART_DURATION ))
  fi
  local r=0
  sleep "$ROUND_SPAN"
  while true; do
    [[ -f "$STOPFILE" ]] && { log "Rebalance loop stopping"; return 0; }

    flock -x "$LOCKFD"
    local old_state=$PRNG_STATE
    PRNG_STATE=$(derive_round_seed "$r")

    if (( HEAL_EVERY_ROUND > 0 && (r % HEAL_EVERY_ROUND) == 0 )); then
      host_clear_all_drops
      for i in "${all_idx[@]}"; do LOSS_PCT_NODE["$i"]=0; done
      for i in "${all_idx[@]}"; do apply_node_qdisc "$i"; done
      log "HEAL round r=$r: cleared all host iptables drops, kept tc"
    else
      build_blocks_bidir_exact
      host_clear_all_drops
      apply_all_blocks_host
      log "Rebalanced bidirectional blocks for round r=$r"
    fi

    PRNG_STATE=$old_state
    flock -u "$LOCKFD"

    r=$((r+1))
    sleep "$ROUND_SPAN"
  done
}

# ---------------------------------------------------------------------
# Cleanup
# ---------------------------------------------------------------------
cleanup() {
  log "Cleanup: removing host iptables rules and container tc"
  host_clear_all_drops
  for v in "${validators[@]}"; do
    local pid; pid=$(docker inspect -f '{{.State.Pid}}' "$v" 2>/dev/null || true)
    [[ -z "$pid" || "$pid" = "0" ]] && continue
    sudo nsenter -t "$pid" -n tc qdisc del dev eth0 root 2>/dev/null || true
  done

  # only kill/wait if PIDs are non-empty
  [[ -n "${PID_REAPPLY:-}" ]] && kill "$PID_REAPPLY" 2>/dev/null || true
  [[ -n "${PID_RESTART:-}" ]] && kill "$PID_RESTART" 2>/dev/null || true
  [[ -n "${PID_REBAL:-}"   ]] && kill "$PID_REBAL"   2>/dev/null || true

  [[ -n "${PID_REAPPLY:-}" ]] && wait "$PID_REAPPLY" 2>/dev/null || true
  [[ -n "${PID_RESTART:-}" ]] && wait "$PID_RESTART" 2>/dev/null || true
  [[ -n "${PID_REBAL:-}"   ]] && wait "$PID_REBAL"   2>/dev/null || true

  log "Cleanup complete"
}


# ---------------------------------------------------------------------
# Init
# ---------------------------------------------------------------------
log "Starting deterministic fuzz (N=$NUMBER_VALIDATORS, seed=$SEED, topology=$TOPOLOGY, round_span=${ROUND_SPAN}, p_block=${PERCENT_BLOCK}%, heal_every=${HEAL_EVERY_ROUND}, heal_num=${HEAL_NUM_ROUNDS}, ttl=${TTL_SECS}s)"
build_latencies "$TOPOLOGY"
select_losses
build_blocks_bidir_exact
apply_all_latencies_and_loss_once
ensure_docker_user_chain
apply_all_blocks_host

# ---------------------------------------------------------------------
# loops
# ---------------------------------------------------------------------
( reapply_latencies_loop ) & PID_REAPPLY=$!
( restart_loop ) & PID_RESTART=$!
( rebalance_blocks_loop ) & PID_REBAL=$!

# optional TTL watchdog
if (( TTL_SECS > 0 )); then
  (
    sleep "$TTL_SECS"
    log "TTL reached (${TTL_SECS}s) — creating stopfile"
    touch "$STOPFILE"
  ) &
fi

trap 'cleanup; kill -- -$$ 2>/dev/null || true' EXIT INT TERM

# keep script alive
while true; do
  [[ -f "$STOPFILE" ]] && { log "Stopfile present, exiting main loop"; exit 0; }
  sleep 60
done
