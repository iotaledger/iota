#!/bin/bash

# Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# Apply fuzz disruptions deterministically using derived pseudorandom numbers
# Mimics latencies between docker containers via a built-in role-based model
# Supports packet loss, connection blocking, and periodic validator restarts
# Logs to a file and keeps running to reapply rules after container
# restarts; -D dumps the effective latency matrix and exits immediately.

set -euo pipefail
IFS=$'\n\t'


# --- Default configuration ---
NUMBER_VALIDATORS=4       # Number of validator containers
SEED=${SEED:-42}       # Seed for reproducibility of pseudorandom disruptions
PERCENT_BLOCK=0           # Percent chance to block a connection
PERCENT_LOSS=0           # Percent chance to apply packet loss
PERCENT_RESTART=0         # Percent of validators to stop and start in each restart round
RESTART_DURATION=120      # Seconds validators remain stopped during a restart
RESTART_TIMEOUT=60        # Seconds to verify a restarted validator is running
RESTART_MODE="preserve-consensus"  # restart mode: preserve-consensus | full-reset | simple-restart
GEODISTRIBUTED=false  # Large geodistributed latencies or small ones
LOG_FILE="logs/fuzz_script.log" # Output file for script
LATENCY_FILE=""  # Optional TSV overriding the built-in role-based model
DUMP_FILE=""     # Write the effective latency matrix as TSV and exit

# --- Command-line arguments ---
while getopts "g:n:s:b:l:r:d:w:M:o:L:D:" opt; do
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
    L) LATENCY_FILE="$OPTARG" ;;
    D) DUMP_FILE="$OPTARG" ;;
    *) echo "Usage: $0 [-n num_validators] [-s seed] [-b percent_block] [-l percent_packet_loss] [-r percent_restart] [-d restart_duration] [-w restart_timeout] [-M restart_mode(preserve-consensus|full-reset|simple-restart)] [-g geodistributed_bool] [-o logfile] [-L latency_matrix.tsv] [-D dump_matrix.tsv]"; exit 1 ;;
  esac
done
shift $((OPTIND-1))



# --- Logging helper ---
log() {
    echo "$(date -Iseconds) $1" >> "$LOG_FILE"
}


# --- Per-run lock directory ---
# apply_and_mark serializes the edges sharing one source container's tc root via
# a per-source lockfile. These live in a self-owned directory next to the log
# file (not the shared, sticky /var/lock), and we sweep it at startup so stale
# files from a previous run — including one killed with SIGKILL, which bypasses
# any trap — can never make `exec 200>` fail and silently skip latency setup.
LOCK_DIR="$(dirname "$LOG_FILE")/network-benchmark-locks"
# Dump mode never takes locks and usually runs unprivileged, where sweeping
# root-owned lock files from a previous sudo run would fail under set -e.
if [ -z "$DUMP_FILE" ]; then
  rm -rf "$LOCK_DIR"
  mkdir -p "$LOCK_DIR"
fi


# --- Prepare validator list ---
validators=()
for i in $(seq 1 "$NUMBER_VALIDATORS"); do
  validators+=(validator-"$i")
done


# === Built-in role-based latency model ===
# Deterministic directed per-edge netem parameters for any validator count.
# Roles repeat every 10 validators (validator v has role (v-1) % 10):
#   role 0 (v1, v11, ...) : hub        - band member with mildly fast
#                           50-52 ms inbound spokes; anchors the round pace
#                           ~2 ms above the band quorum loop so direct blocks
#                           keep completing quorums (AddBlock) on fresh runs
#   roles 1-7             : band       - narrow asymmetric 48-54 ms mesh;
#                           direct full blocks complete quorums (AddBlock)
#   role 8 (v9, v19, ...) : follower   - 22 ms spoke from its decade hub plus
#                           88-96 ms directs; hub blocks complete its rounds
#                           via embedded headers (AddBlockHeader)
#   role 9 (v10, v20, ...): heavy tail - 540-659 +/- 150 ms deep volatile
#                           directs (corr 80: slow wander across ~390-810 ms),
#                           one 60 ms hub route delivered in netem slot
#                           bursts (100-146 ms at n=10, +2 ms per validator
#                           above 10) whose ~2-round batches interact with
#                           the 50 ms min block delay to skip rounds
#                           (block-rate spread), 70-95 ms outbound so its
#                           stale leader blocks never stall the quorum
# With -g false all delays and jitters are divided by 4 and slot clauses are
# dropped (legacy "small latencies" mode).

# === Subfunctions ===

# edge_params(i, j)
# Echoes "delay_ms jitter_ms corr_pct loss_pct slot_min_ms slot_max_ms" for
# the directed edge validator-i -> validator-j (1-based, i != j).
edge_params() {
  local i=$1 j=$2
  local role_i=$(( (i - 1) % 10 )) role_j=$(( (j - 1) % 10 ))
  local hub_j=$(( j - (j - 1) % 10 ))
  local d
  # heavy-tail inbound: bursty hub spoke, deep fluctuating directs
  if [ "$role_j" -eq 9 ]; then
    if [ "$i" -eq "$hub_j" ]; then
      # Slot bounds scale with validator count: the band round gets longer as
      # N grows, so fixed 100-146 ms bursts skip fewer rounds and the
      # heavy-tail's block-rate deficit shrinks below the >=1 blk/s target.
      # +2 ms per validator above 10 restores it; n=10 stays exactly 100-146.
      local slot_shift=$(( NUMBER_VALIDATORS > 10 ? 2 * (NUMBER_VALIDATORS - 10) : 0 ))
      echo "60 3 0 0 $(( 100 + slot_shift )) $(( 146 + slot_shift ))"
    else
      echo "$(( 540 + (23 * i) % 120 )) 150 80 0 0 0"
    fi
    return
  fi
  # heavy-tail outbound: moderate, never stalls healthy quorums
  if [ "$role_i" -eq 9 ]; then
    echo "$(( 70 + (9 * j) % 26 )) 25 30 0 0 0"
    return
  fi
  # relay-follower inbound: hub spoke wins every round
  if [ "$role_j" -eq 8 ]; then
    if [ "$i" -eq "$hub_j" ]; then
      echo "22 2 30 0 0 0"
    else
      echo "$(( 88 + (3 * i) % 9 )) 8 30 0 0 0"
    fi
    return
  fi
  if [ "$role_i" -eq 8 ]; then
    echo "$(( 58 + (5 * j) % 9 )) 8 30 0 0 0"
    return
  fi
  # fast inbound spokes to the hub
  if [ "$role_j" -eq 0 ]; then
    echo "$(( 50 + i % 3 )) 3 30 0 0 0"
    return
  fi
  # ordinary band mesh
  d=$(( 48 + (3 * i + 5 * j) % 7 ))
  echo "$d $(( 3 + d % 3 )) 30 0 0 0"
}

# Fill the matrix arrays from the built-in model for all directed edges.
# Applies the -g false downscaling here so accessors stay pure lookups.
populate_builtin_matrix() {
  local i j d jit corr loss smin smax
  for ((i=1; i<=NUMBER_VALIDATORS; i++)); do
    for ((j=1; j<=NUMBER_VALIDATORS; j++)); do
      [ "$i" -eq "$j" ] && continue
      # Explicit IFS: the script-global IFS has no space, so the
      # space-separated edge_params output would not split otherwise.
      IFS=' ' read -r d jit corr loss smin smax <<< "$(edge_params "$i" "$j")"
      if [ "$GEODISTRIBUTED" != true ]; then
        d=$(( d / 4 )); [ "$d" -lt 1 ] && d=1
        jit=$(( jit / 4 ))
        smin=0; smax=0
      fi
      LATENCY_MATRIX[$i,$j]=$d
      JITTER_MATRIX[$i,$j]=$jit
      LOSS_MATRIX[$i,$j]=$loss
      CORR_MATRIX[$i,$j]=$corr
      SLOT_MIN_MATRIX[$i,$j]=$smin
      SLOT_MAX_MATRIX[$i,$j]=$smax
    done
  done
  log "Populated built-in role-based latency matrix for $NUMBER_VALIDATORS validators"
}

# Write the effective matrix as a TSV (same format -L consumes) and return.
dump_matrix() {
  local file=$1
  local i j
  # C locale: %.2f must emit dot decimals regardless of the host LC_NUMERIC,
  # since the -L loss/corr guards and TSV consumers expect "0.00".
  local LC_ALL=C
  {
    echo "# latency-matrix n=$NUMBER_VALIDATORS model=role-based geodistributed=$GEODISTRIBUTED"
    echo "# roles repeat every 10 validators: hub / band x7 / relay-follower / heavy-tail"
    echo "# src	dst	delay_ms	jitter_ms	loss_pct	corr_pct	slot_min_ms	slot_max_ms"
    for ((i=1; i<=NUMBER_VALIDATORS; i++)); do
      for ((j=1; j<=NUMBER_VALIDATORS; j++)); do
        [ "$i" -eq "$j" ] && continue
        # Same miss defaults as the accessors, so dumping a partial -L
        # matrix works instead of tripping set -u.
        printf '%s\t%s\t%s\t%s\t%.2f\t%.0f\t%s\t%s\n' \
          "$i" "$j" \
          "${LATENCY_MATRIX[$i,$j]:-1}" "${JITTER_MATRIX[$i,$j]:-0}" \
          "${LOSS_MATRIX[$i,$j]:-0}" "${CORR_MATRIX[$i,$j]:-0}" \
          "${SLOT_MIN_MATRIX[$i,$j]:-0}" "${SLOT_MAX_MATRIX[$i,$j]:-0}"
      done
    done
  } > "$file"
}

# --- Optional latency matrix loaded from -L <file> ---
# TSV with one row per directed edge:
# `src \t dst \t rtt_ms \t jitter_ms \t loss_pct \t corr_pct \t slot_min_ms \t slot_max_ms`.
# The 5th-8th columns are optional. Indices are 1-based (validator-1 = 1).
# Comment lines starting with `#` and blank lines are ignored. When a
# (src,dst) lookup misses a partially specified matrix the accessors fall
# back to 1 ms delay and zero jitter/loss/correlation/slot.
# Non-zero slot columns emit `netem ... slot <min>ms <max>ms`, batching
# delivery into bursts spaced uniformly in [min, max] (kernel >= 4.16).
declare -A LATENCY_MATRIX
declare -A JITTER_MATRIX
declare -A LOSS_MATRIX
declare -A CORR_MATRIX
declare -A SLOT_MIN_MATRIX
declare -A SLOT_MAX_MATRIX

load_latency_matrix() {
  local file=$1
  if [ ! -f "$file" ]; then
    log "Latency matrix file not found: $file (falling back to built-in table)"
    return 1
  fi
  local count=0
  local src dst rtt jit loss corr slot_min slot_max
  while IFS=$' \t' read -r src dst rtt jit loss corr slot_min slot_max _rest; do
    [[ -z "${src:-}" || "${src:0:1}" == "#" ]] && continue
    LATENCY_MATRIX[$src,$dst]=$rtt
    JITTER_MATRIX[$src,$dst]=$jit
    # Optional columns: older TSVs (no loss/corr/slot columns) get 0 here, so
    # existing callers see no behavior change.
    LOSS_MATRIX[$src,$dst]=${loss:-0}
    CORR_MATRIX[$src,$dst]=${corr:-0}
    SLOT_MIN_MATRIX[$src,$dst]=${slot_min:-0}
    SLOT_MAX_MATRIX[$src,$dst]=${slot_max:-0}
    count=$(( count + 1 ))
  done < "$file"
  log "Loaded $count edges from latency matrix $file"
  return 0
}

# latency_for / jitter_for / loss_for / corr_for take validator NAMES
# (validator-1, validator-2, ...) and return ms / ms / percent. The matrix
# arrays are always populated before use (either from -L or the built-in
# role-based model), so these are pure lookups.
latency_for() {
  local A=$1 B=$2
  local src=${A#validator-} dst=${B#validator-}
  echo "${LATENCY_MATRIX[$src,$dst]:-1}"
}

jitter_for() {
  local A=$1 B=$2
  local src=${A#validator-} dst=${B#validator-}
  echo "${JITTER_MATRIX[$src,$dst]:-0}"
}

loss_for() {
  local A=$1 B=$2
  local src=${A#validator-} dst=${B#validator-}
  if [ -n "${LOSS_MATRIX[$src,$dst]:-}" ]; then
    echo "${LOSS_MATRIX[$src,$dst]}"
  else
    echo "0"
  fi
}

corr_for() {
  local A=$1 B=$2
  local src=${A#validator-} dst=${B#validator-}
  if [ -n "${CORR_MATRIX[$src,$dst]:-}" ]; then
    echo "${CORR_MATRIX[$src,$dst]}"
  else
    echo "0"
  fi
}

slot_min_for() {
  local A=$1 B=$2
  local src=${A#validator-} dst=${B#validator-}
  echo "${SLOT_MIN_MATRIX[$src,$dst]:-0}"
}

slot_max_for() {
  local A=$1 B=$2
  local src=${A#validator-} dst=${B#validator-}
  echo "${SLOT_MAX_MATRIX[$src,$dst]:-0}"
}


# container_pid(container)
# Returns host PID of Docker container
container_pid() { docker inspect -f '{{.State.Pid}}' "$1"; }

# Apply latency and mark packets from container A → B.
# Args: A B delay_ms jitter_ms [loss_pct] [corr_pct] [slot_min_ms] [slot_max_ms].
# Optional values default to 0. When loss is 0 the `loss` netem keyword is
# omitted entirely (some kernels treat `loss 0%` as enabling the loss
# accounting machinery even with zero drop rate). Correlation is also omitted
# when 0, and the slot clause is omitted unless both slot bounds are positive.
apply_and_mark() {
  local A=$1 B=$2
  local D=$3 J=$4
  local L=${5:-0}
  local C=${6:-0}
  local SMIN=${7:-0}
  local SMAX=${8:-0}
  local IPB pid
  local lockfile="$LOCK_DIR/apply_and_mark_${A}.lock"

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

  # Ensure a classful root qdisc exists once per container. The root is only
  # ever deleted on a SUCCESSFUL read that shows a non-htb root: treating a
  # transient `tc show` failure as "no root" used to del the root here and
  # silently wipe every netem qdisc already applied on this source.
  local qdisc_show
  if ! qdisc_show=$(nsenter -t "$pid" -n tc qdisc show dev eth0 2>/dev/null); then
    log "Warning: tc qdisc show failed for $A; skipping root-qdisc check"
  elif ! grep -q "htb 1:" <<< "$qdisc_show"; then
    nsenter -t "$pid" -n tc qdisc del dev eth0 root 2>/dev/null || true
    nsenter -t "$pid" -n tc qdisc add dev eth0 root handle 1: htb default 1 2>/dev/null || \
      log "Warning: failed to create htb root qdisc for $A"
    nsenter -t "$pid" -n tc class add dev eth0 parent 1: classid 1:1 htb rate 1000mbit ceil 1000mbit 2>/dev/null || true
  fi

  # Mark packets A → B inside the container namespace (idempotent).
  # `-w 5` makes iptables wait up to 5s for the host-shared /run/xtables.lock
  # instead of returning EAGAIN — needed because nsenter + iptables across
  # many netns still contend a single host-level xtables lock file.
  local ipt_err
  if ! ipt_err=$(nsenter -t "$pid" -n iptables -w 5 -t mangle -C OUTPUT -d "${IPB}" -j MARK --set-mark "$mark" 2>&1); then
    if ! ipt_err=$(nsenter -t "$pid" -n iptables -w 5 -t mangle -A OUTPUT -d "${IPB}" -j MARK --set-mark "$mark" 2>&1); then
      log "Warning: failed to mark traffic from $A → $B: $ipt_err"
    fi
  fi

  # Create/update a dedicated class and netem qdisc for this destination.
  # Loss is appended only when non-zero — see header comment on apply_and_mark.
  nsenter -t "$pid" -n tc class replace dev eth0 parent 1: classid "$classid" htb rate 1000mbit ceil 1000mbit 2>/dev/null || true
  local tc_err
  local delay_args=(delay "${D}ms" "${J}ms")
  # Correlation is only meaningful with non-zero jitter; with J=0 tc may
  # reject the qdisc and the edge would silently lose its latency.
  if [ "$J" != "0" ] && [ "$C" != "0" ] && [ "$C" != "0.0" ] && [ "$C" != "0.00" ]; then
    delay_args+=("${C}%")
  fi
  # Slot batching: deliver queued packets in bursts spaced U(SMIN, SMAX) ms.
  if [ "${SMIN%.*}" -gt 0 ] 2>/dev/null && [ "${SMAX%.*}" -gt 0 ] 2>/dev/null; then
    delay_args+=(slot "${SMIN}ms" "${SMAX}ms")
  fi
  if [ "$L" = "0" ] || [ "$L" = "0.0" ] || [ "$L" = "0.00" ]; then
    tc_err=$(nsenter -t "$pid" -n tc qdisc replace dev eth0 parent "$classid" handle "${mark}0:" netem "${delay_args[@]}" 2>&1) || \
      log "Warning: failed to apply latency to $A → $B: $tc_err"
  else
    tc_err=$(nsenter -t "$pid" -n tc qdisc replace dev eth0 parent "$classid" handle "${mark}0:" netem "${delay_args[@]}" loss "${L}%" 2>&1) || \
      log "Warning: failed to apply latency+loss to $A → $B: $tc_err"
  fi

  # Attach a filter that routes marked packets into the class. `tc filter show`
  # prints `handle 0x<hex>` so match on the hex-formatted mark; otherwise the
  # grep never matches and we re-add the filter every call.
  local mark_hex
  printf -v mark_hex '%x' "$mark"
  if ! nsenter -t "$pid" -n tc filter show dev eth0 parent 1: 2>/dev/null | grep -q "handle 0x${mark_hex} .* flowid ${classid}"; then
    tc_err=$(nsenter -t "$pid" -n tc filter add dev eth0 parent 1: protocol ip handle "$mark" fw flowid "$classid" 2>&1) || \
      log "Warning: failed to attach tc filter for $A → $B: $tc_err"
  fi

  # Release lock automatically when function exits
  flock -u 200
}

# Combine matrix loss with source-wide fuzz loss as independent probabilities.
effective_loss_for() {
  local A=$1 B=$2
  local base extra
  base=$(loss_for "$A" "$B")
  extra=${fuzz_loss_amount["$A"]:-0}
  awk -v base="$base" -v extra="$extra" \
    'BEGIN { printf "%.2f", 100 - ((100 - base) * (100 - extra) / 100) }'
}

# Apply source-wide fuzz loss without replacing the per-edge latency tree.
apply_loss() {
  local A=$1 percent=$2
  local B D J L C SMIN SMAX
  fuzz_loss_amount["$A"]=$percent
  for B in "${validators[@]}"; do
    [ "$A" = "$B" ] && continue
    D=$(latency_for "$A" "$B")
    J=$(jitter_for "$A" "$B")
    L=$(effective_loss_for "$A" "$B")
    C=$(corr_for "$A" "$B")
    SMIN=$(slot_min_for "$A" "$B")
    SMAX=$(slot_max_for "$A" "$B")
    apply_and_mark "$A" "$B" "$D" "$J" "$L" "$C" "$SMIN" "$SMAX"
  done
  log "Applied ${percent}% source-wide packet loss to $A without removing latency"
}

# Record a blocked target for later reapplication after container restarts.
# Values are newline-delimited because this script sets IFS to newline+tab.
record_block_target() {
  local A=$1 B=$2
  if [ -n "${fuzz_block_targets[$A]:-}" ]; then
    fuzz_block_targets["$A"]+=$'\n'"$B"
  else
    fuzz_block_targets["$A"]="$B"
  fi
}

# block connection between a given pair of addresses
block_connection() {
  local A=$1 B=$2
  local pid ipB
  ipB=$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$B")
  pid=$(container_pid "$A")
  # Idempotent (`-C` before `-A`): the reapply watcher re-invokes this after
  # container restarts, and repeated calls must not stack duplicate rules.
  if ! nsenter -t "$pid" -n iptables -w 5 -C OUTPUT -d "$ipB" -j DROP 2>/dev/null; then
    nsenter -t "$pid" -n iptables -w 5 -A OUTPUT -d "$ipB" -j DROP
  fi
  log "Blocked traffic $A → $B"
}

# Restart a validator container with configurable database handling.
# Supports three modes:
#   - preserve-consensus: Remove only authorities_db, keep consensus_db
#   - full-reset: Remove both authorities_db and consensus_db
#   - simple-restart: Don't remove any databases, clean docker restart only
restart_validator() {
 local v=$1 stop_duration=$2 timeout=${3:-60} mode=${4:-preserve-consensus}
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

 log "Keeping $v stopped for $stop_duration seconds..."
 sleep "$stop_duration"

 docker start "$v" >/dev/null 2>&1
 local deadline=$((SECONDS + timeout))
 while [ "$SECONDS" -lt "$deadline" ]; do
   if [ "$(docker inspect -f '{{.State.Running}}' "$v" 2>/dev/null || true)" = "true" ]; then
     log "Restarted $v"
     return 0
   fi
   sleep 1
 done
 log "Error: $v did not remain running within ${timeout}s after restart"
 return 1
}

# apply fuzz network conditions
initially_apply_fuzz() {
  for ((i=0; i<NUMBER_VALIDATORS; i++)); do
     A=${validators[i]}


    for ((j=i+1; j<NUMBER_VALIDATORS; j++)); do

      B=${validators[j]}

      r_block_A=$(( RANDOM % 100 ))
      r_block_B=$(( RANDOM % 100 ))


      if (( r_block_A < PERCENT_BLOCK )); then
        block_connection "$A" "$B"
        record_block_target "$A" "$B"
      fi
      if (( r_block_B < PERCENT_BLOCK )); then
        block_connection "$B" "$A"
        record_block_target "$B" "$A"
      fi
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
  # One background worker per SOURCE validator; each worker applies all its
  # outbound rules (one per peer) sequentially against its own netns. Caps live concurrency
  # to NUMBER_VALIDATORS instead of NUMBER_VALIDATORS*(NUMBER_VALIDATORS-1),
  # which previously caused silent /run/xtables.lock contention and left a
  # random ~30% of (src,dst) pairs without their netem qdisc.
  for ((i=0; i<${#validators[@]}; i++)); do
    A=${validators[i]}
    (
      for ((j=0; j<${#validators[@]}; j++)); do
        [ "$i" -eq "$j" ] && continue
        B=${validators[j]}
        D=$(latency_for "$A" "$B")
        J=$(jitter_for "$A" "$B")
        L=$(loss_for "$A" "$B")
        C=$(corr_for "$A" "$B")
        SMIN=$(slot_min_for "$A" "$B")
        SMAX=$(slot_max_for "$A" "$B")
        log "Injecting ${D}ms±${J}ms latency corr=${C}% loss=${L}% slot=${SMIN}-${SMAX}ms $A → $B"
        apply_and_mark "$A" "$B" "$D" "$J" "$L" "$C" "$SMIN" "$SMAX"
      done
    ) &
  done
  wait
  log "Initial latency application complete"
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

            # Compare against the expected netem count: a partial wipe (some
            # qdiscs lost, others surviving) must heal too, not only the
            # all-gone case after a container restart. Source-wide fuzz loss
            # is carried by every per-edge netem qdisc, so latency remains
            # active and the expected count is always n-1.
            local expected=$(( ${#validators[@]} - 1 ))
            netem_count=$(nsenter -t "$pid" -n tc qdisc show dev eth0 2>/dev/null | grep -c "netem" || true)
            if [ "${netem_count:-0}" -lt "$expected" ]; then
                log "Reapplying latency + fuzz for $v (netem ${netem_count:-0}/$expected — container restarted or tc removed)"

                # --- Reapply latency ---
                for u in "${validators[@]}"; do
                    [ "$v" = "$u" ] && continue
                    D=$(latency_for "$v" "$u")
                    J=$(jitter_for "$v" "$u")
                    L=$(effective_loss_for "$v" "$u")
                    C=$(corr_for "$v" "$u")
                    SMIN=$(slot_min_for "$v" "$u")
                    SMAX=$(slot_max_for "$v" "$u")
                    apply_and_mark "$v" "$u" "$D" "$J" "$L" "$C" "$SMIN" "$SMAX" &
                done
                wait

                # --- Reapply fuzz (blocking) --- guarded: a transient
                # nsenter/iptables failure must not abort the watcher loop
                # under set -e.
                for target in ${fuzz_block_targets["$v"]}; do
                    block_connection "$v" "$target" || true
                done
            fi
        done
        sleep 1
    done
}

# === Main ===
log "Starting fuzz manager"
RANDOM=$SEED

# Load the -L matrix override if provided; otherwise compute the built-in
# role-based model. A missing/unreadable -L file also falls back to it.
if [ -z "$LATENCY_FILE" ] || ! load_latency_matrix "$LATENCY_FILE"; then
  populate_builtin_matrix
fi

# Dump-and-exit mode: write the effective matrix (for logging, inspection,
# or as a -L input) without touching docker or netem state.
if [ -n "$DUMP_FILE" ]; then
  dump_matrix "$DUMP_FILE"
  log "Dumped latency matrix to $DUMP_FILE"
  exit 0
fi

# Initially set latencies
initially_apply_latency

# Initially set fuzz rules
initially_apply_fuzz

# Reapply latencies and fuzz rules every second
reapply_latencies_and_fuzz_loop &

# Restart validator loop
restart_loop &


wait
