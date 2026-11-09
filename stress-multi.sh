#!/usr/bin/env bash
# Run multiple stress.rs processes in parallel, each pinned to one fullnode.
# Works around iota-benchmark stress.rs's "random proxy chosen once per workload"
# behavior in bench_driver.rs:356.
#
# Outputs go to sweeps/latest/logs/multi-<utc-ts>/ — one parent dir per invocation, never overwritten.

set -uo pipefail

QPS_TOTAL="${QPS_TOTAL:-0}"
DURATION="${DURATION:-300s}"
WORKERS="${WORKERS:-12}"
IN_FLIGHT_RATIO="${IN_FLIGHT_RATIO:-10}"
BURST="${BURST:-0}"
INTERVAL="${INTERVAL:-0}"

# Parse INTERVAL (e.g. "20ms", "2s", "1m", or bare "1000" = ms) to an
# integer millisecond value. Mirrors DURATION's suffix-friendly style.
parse_ms() {
  local v="$1"
  if   [[ "$v" =~ ^([0-9]+)ms$ ]]; then echo "${BASH_REMATCH[1]}"
  elif [[ "$v" =~ ^([0-9]+)s$  ]]; then echo "$(( ${BASH_REMATCH[1]} * 1000 ))"
  elif [[ "$v" =~ ^([0-9]+)m$  ]]; then echo "$(( ${BASH_REMATCH[1]} * 60000 ))"
  elif [[ "$v" =~ ^[0-9]+$     ]]; then echo "$v"
  else echo "Error: cannot parse '$v' as duration (use Nms / Ns / Nm)" >&2; return 1
  fi
}
INTERVAL=$(parse_ms "$INTERVAL") || exit 1
# The metrics read from Prometheus, the measured pool shares and the
# consensus figures cover the spam window from MEASURE_FROM_SECS after its
# start, so the queue filling up at the start is left out. 0 covers the
# whole window. The time series always cover the whole window.
MEASURE_FROM_SECS="${MEASURE_FROM_SECS:-0}"
if ! [[ "$MEASURE_FROM_SECS" =~ ^[0-9]+$ ]] \
  || [ $(( ${DURATION%s} - MEASURE_FROM_SECS )) -lt 10 ]; then
  echo "Error: MEASURE_FROM_SECS=$MEASURE_FROM_SECS must be whole seconds that leave at least 10 s of DURATION=$DURATION to measure" >&2
  exit 1
fi
GAS_CHUNK_SIZE="${GAS_CHUNK_SIZE:-500}"
NUM_TRANSFER_ACCOUNTS="${NUM_TRANSFER_ACCOUNTS:-2}"
# Persistent gas-pool cache dir (survives across runs). Each subprocess gets
# its own cache file under this dir, keyed by primary_gas_owner index. To
# disable the cache, set GAS_POOL_CACHE_DIR="disable" (or any non-path that
# doesn't exist and can't be created).
# Default lives under sweeps/.gas-pool-cache/ (shared across regimes, NOT
# per-sweep — it's reused across iters to avoid re-paying the slow pay_iota
# loop). Override with GAS_POOL_CACHE_DIR=... if you want a cross-clone
# cache, e.g. $HOME/.stress-gas-pool.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GAS_POOL_CACHE_DIR="${GAS_POOL_CACHE_DIR:-$SCRIPT_DIR/sweeps/.gas-pool-cache}"
FULLNODES="${FULLNODES:-http://127.0.0.1:9000}"
PROM_URL="${PROM_URL:-http://localhost:9090}"

IFS=',' read -ra FN_ARR <<< "$FULLNODES"
# Number of stress subprocesses. Defaults to the number of fullnode URLs (one
# proc primarily targets one fullnode), but can be overridden — e.g.
# NUM_PROCS=8 FULLNODES=<4 urls> ... gives 8 procs sharing 4 fullnodes (2:1),
# which preserves client-side concurrency (more independent RPC clients, more
# burst races at the gate) even with a slimmer validator network.
N="${NUM_PROCS:-${#FN_ARR[@]}}"

# Honest pool (Option B for the anti-spam fairness experiment described in
# graduated-benefits.md). When HONEST_PROC_COUNT > 0, the LAST
# HONEST_PROC_COUNT procs in this invocation use the honest configuration
# (low QPS, no burst), and the first (N - HONEST_PROC_COUNT) procs use the
# existing spammer-style config (QPS_TOTAL, BURST, INTERVAL,
# IN_FLIGHT_RATIO, WORKERS). Default 0 keeps single-pool behavior so
# burst-sweep.sh and other callers are unaffected.
HONEST_PROC_COUNT="${HONEST_PROC_COUNT:-0}"
HONEST_QPS_PER_PROC="${HONEST_QPS_PER_PROC:-50}"
HONEST_BURST="${HONEST_BURST:-1}"
HONEST_INTERVAL="${HONEST_INTERVAL:-0}"
HONEST_INTERVAL=$(parse_ms "$HONEST_INTERVAL") || exit 1
HONEST_IFR="${HONEST_IFR:-4}"
HONEST_WORKERS="${HONEST_WORKERS:-4}"
# When false (legacy), honest is closed-loop: it self-throttles under
# load and our admission-rate denominator (QPS x duration) overstates
# actual arrivals — biasing first_pass_pct upward.  When true, honest
# fires at fixed QPS regardless of inflight, so first_pass_pct measures
# pure validator behavior (the canonical fairness probe).
HONEST_OPEN_LOOP="${HONEST_OPEN_LOOP:-false}"

# Second honest pool with same config but always closed-loop. Pairs with
# the (possibly open-loop) HONEST_* pool to capture both perspectives in
# one iter: open-loop = pure validator admission, closed-loop = real-client
# effective throughput. Set HONEST_CL_PROC_COUNT=0 to disable.
HONEST_CL_PROC_COUNT="${HONEST_CL_PROC_COUNT:-0}"

N_SPAMMER=$((N - HONEST_PROC_COUNT - HONEST_CL_PROC_COUNT))
if [ "$N_SPAMMER" -le 0 ]; then
  echo "Error: HONEST_PROC_COUNT=$HONEST_PROC_COUNT + HONEST_CL_PROC_COUNT=$HONEST_CL_PROC_COUNT >= NUM_PROCS=$N (no spammer procs left)" >&2
  exit 1
fi
# Boundary indices for pool dispatch in the proc launch loop below.
# Proc i belongs to: spammer if i < HONEST_OL_START, honest (the
# HONEST_OPEN_LOOP-configurable pool) if i < HONEST_CL_START,
# honest_cl (always closed-loop) otherwise.
HONEST_OL_START=$N_SPAMMER
HONEST_CL_START=$((N_SPAMMER + HONEST_PROC_COUNT))

# bench_driver.rs:407 splits per-proc target_qps across WORKERS and
# requires per-worker target_qps >= 1 to spawn each worker. Below the
# floor, some workers silently never start (especially fatal in Mode A
# where INTERVAL paces the bursts but QPS_TOTAL is still gating spawn).
MIN_QPS_TOTAL=$((N_SPAMMER * WORKERS))
if [ "$QPS_TOTAL" -eq 0 ]; then
  QPS_TOTAL=$MIN_QPS_TOTAL
  echo "=> QPS_TOTAL unset; using minimum $QPS_TOTAL (= N_SPAMMER=$N_SPAMMER × WORKERS=$WORKERS) so all workers spawn."
elif [ "$QPS_TOTAL" -lt "$MIN_QPS_TOTAL" ]; then
  echo "Error: QPS_TOTAL=$QPS_TOTAL is below the minimum $MIN_QPS_TOTAL (= N_SPAMMER=$N_SPAMMER × WORKERS=$WORKERS)." >&2
  echo "       bench_driver.rs requires per-worker target_qps >= 1 — some workers won't spawn." >&2
  echo "       Either bump QPS_TOTAL >= $MIN_QPS_TOTAL or reduce NUM_PROCS/WORKERS." >&2
  exit 1
fi
# QPS_PER is the per-spammer-proc QPS. With HONEST_PROC_COUNT=0 this
# collapses to the original QPS_TOTAL / N.
QPS_PER=$((QPS_TOTAL / N_SPAMMER))

# Offset into the GAS_OWNERS array. Set GAS_OWNERS_OFFSET=4 on the second
# machine in a 2-machine setup so it uses gas owners 4-7 (avoiding contention
# with machine 1 using 0-3).
GAS_OWNERS_OFFSET="${GAS_OWNERS_OFFSET:-0}"

# Benchmark gas-owner addresses — read dynamically from the benchmark
# keystore generated by bootstrap.sh's print-benchmark-accounts helper.
# The keystore is sorted (FileBasedKeystore::save) so the address order
# is stable across runs even though it differs from the deterministic
# benchmark_gas_keys() generation order. Stable order is all we need:
# process i always gets the i-th address.
KEYSTORE_PATH="$SCRIPT_DIR/dev-tools/iota-private-network/configs/genesis/benchmark.keystore"
if [ ! -f "$KEYSTORE_PATH" ]; then
  echo "Error: benchmark keystore not found at $KEYSTORE_PATH — run bootstrap.sh first" >&2
  exit 1
fi
mapfile -t GAS_OWNERS < <(python3 -c "
import json, sys
data = json.load(open('$KEYSTORE_PATH'))
for k in data['keys']:
    print(k['address'])
")
if [ $((GAS_OWNERS_OFFSET + N)) -gt "${#GAS_OWNERS[@]}" ]; then
  echo "Error: GAS_OWNERS_OFFSET=$GAS_OWNERS_OFFSET + N=$N exceeds ${#GAS_OWNERS[@]} defined gas owners" >&2
  exit 1
fi

# Phase timing — emit [T+Ns] markers at major boundaries so the per-iter
# overhead breakdown lands in sweep.log. Sibling of sweep.sh's mark().
# Filtered out of monitor.sh by default; grep "T+" sweep.log to inspect.
SM_T0=$(date +%s)
mark() { echo "  [T+$(($(date +%s) - SM_T0))s] [sm] $*"; }
mark "stress-multi start"

# The stress client is a prebuilt binary from the network-benchmark repo
# (see STRESS_BIN in stress-load-shedding.sh). Check it once here, so a
# missing or outdated binary fails before any gas setup.
STRESS_BIN="${STRESS_BIN:-$SCRIPT_DIR/../network-benchmark/target/release/stress}"
if [ ! -x "$STRESS_BIN" ]; then
  echo "Error: stress client not found at STRESS_BIN=$STRESS_BIN" >&2
  exit 1
fi
stress_help=$("$STRESS_BIN" --help 2>/dev/null || true)
if ! grep -q -- '--submit-once' <<<"$stress_help"; then
  echo "Error: $STRESS_BIN has no --submit-once option; it predates the load shedding client" >&2
  exit 1
fi
mark "stress client checked: $STRESS_BIN"

# Master timestamp + parent dir under sweeps/latest/logs/ so each
# invocation is preserved.
MASTER_TS=$(date -u +"%Y-%m-%dT%H-%M-%SZ")
PARENT_DIR="$SCRIPT_DIR/sweeps/latest/logs/multi-${MASTER_TS}"
mkdir -p "$PARENT_DIR"

# Barrier files: each subprocess writes its READY_FILE after setup, then waits
# for START_FILE to appear. We touch START_FILE once all are ready, so all
# spam windows begin simultaneously.
BARRIER_DIR="$PARENT_DIR/barrier"
mkdir -p "$BARRIER_DIR"
START_FILE="$BARRIER_DIR/go"

if [ "$HONEST_PROC_COUNT" -gt 0 ] || [ "$HONEST_CL_PROC_COUNT" -gt 0 ]; then
  echo "=> Launching $N stress.rs processes across pools:"
  echo "     spammer pool:   $N_SPAMMER procs @ QPS=$QPS_PER each (total=$QPS_TOTAL)"
  echo "                     BURST=$BURST INTERVAL=$INTERVAL"
  if [ "$HONEST_PROC_COUNT" -gt 0 ]; then
    echo "     honest pool:    $HONEST_PROC_COUNT proc(s) @ QPS=$HONEST_QPS_PER_PROC each open_loop=$HONEST_OPEN_LOOP"
    echo "                     BURST=$HONEST_BURST INTERVAL=$HONEST_INTERVAL"
  fi
  if [ "$HONEST_CL_PROC_COUNT" -gt 0 ]; then
    echo "     honest_cl pool: $HONEST_CL_PROC_COUNT proc(s) @ QPS=$HONEST_QPS_PER_PROC each open_loop=false (always closed-loop)"
    echo "                     BURST=$HONEST_BURST INTERVAL=$HONEST_INTERVAL"
  fi
else
  echo "=> Launching $N stress.rs processes, each at QPS=$QPS_PER (total=$QPS_TOTAL)"
fi
echo "=> Parent dir: $PARENT_DIR"
echo "=> Barrier: $BARRIER_DIR (start file: $START_FILE)"

pids=()
logs=()
runs_dirs=()
ready_files=()
pool_labels=()

# If this script is killed (Ctrl-C, SIGTERM, hangup), reap all subprocess
# groups so we don't leak orphan stress binaries holding their metrics ports.
cleanup_subprocesses() {
  local pid
  for pid in "${pids[@]}"; do kill -TERM -- -"$pid" 2>/dev/null || true; done
  sleep 1
  for pid in "${pids[@]}"; do kill -KILL -- -"$pid" 2>/dev/null || true; done
}
trap cleanup_subprocesses INT TERM HUP
for ((i=0; i<N; i++)); do
  # Round-robin primary fullnode assignment so N>len(FN_ARR) works
  # (e.g. NUM_PROCS=8 with 4 fullnodes → procs 0,4 share fn[0], etc.).
  fn="${FN_ARR[$((i % ${#FN_ARR[@]}))]}"
  log="$PARENT_DIR/process-$i.log"
  proc_runs="$PARENT_DIR/process-$i"
  ready_file="$BARRIER_DIR/process-$i.ready"
  logs+=("$log")
  runs_dirs+=("$proc_runs")
  ready_files+=("$ready_file")
  # Per-subprocess cache file (one per gas owner so they don't collide).
  cache_idx=$((GAS_OWNERS_OFFSET + i))
  if [ "$GAS_POOL_CACHE_DIR" = "disable" ] || [ -z "$GAS_POOL_CACHE_DIR" ]; then
    proc_cache=""
  else
    mkdir -p "$GAS_POOL_CACHE_DIR" 2>/dev/null
    proc_cache="$GAS_POOL_CACHE_DIR/owner-$cache_idx.json"
  fi
  # Pool dispatch: procs partition into three ranges. All pools share
  # the same barrier and fire their first activity at the same instant
  # once all N procs are ready. Both honest pools use IDENTICAL config
  # (QPS, burst, IFR, workers) — only `proc_open_loop` differs, so the
  # comparison isolates the loop-type variable.
  if [ "$i" -ge "$HONEST_CL_START" ]; then
    pool="honest_cl"
    proc_qps=$HONEST_QPS_PER_PROC
    proc_burst=$HONEST_BURST
    proc_barrier=$HONEST_INTERVAL
    proc_ifr=$HONEST_IFR
    proc_workers=$HONEST_WORKERS
    # Closed-loop by design — models a polite client that backs off
    # under load. Captures real-client effective throughput.
    proc_open_loop="false"
  elif [ "$i" -ge "$HONEST_OL_START" ]; then
    pool="honest"
    proc_qps=$HONEST_QPS_PER_PROC
    proc_burst=$HONEST_BURST
    proc_barrier=$HONEST_INTERVAL
    proc_ifr=$HONEST_IFR
    proc_workers=$HONEST_WORKERS
    # Open/closed-loop controlled by HONEST_OPEN_LOOP. When true (the
    # fairness-probe configuration) honest fires at fixed QPS regardless
    # of inflight, so first_pass_pct measures pure validator admission.
    proc_open_loop="$HONEST_OPEN_LOOP"
  else
    pool="spammer"
    proc_qps=$QPS_PER
    proc_burst=$BURST
    proc_barrier=$INTERVAL
    proc_ifr=$IN_FLIGHT_RATIO
    proc_workers=$WORKERS
    # Spammer pool may opt into open-loop via the OPEN_LOOP env var so
    # the validator gate sees sustained pressure even when per-tx
    # round-trip latency is high. Default is closed-loop (matches
    # historical behaviour).
    proc_open_loop="${OPEN_LOOP:-false}"
  fi
  pool_labels+=("$pool")
  # Spikes come from the first spammer process only, so that each spike has
  # the configured size.
  if [ "$i" -eq 0 ]; then
    proc_spike_size="${SPIKE_SIZE:-0}"
  else
    proc_spike_size=0
  fi
  echo "   process $i [$pool] → $fn  (qps=$proc_qps burst=$proc_burst open_loop=$proc_open_loop submit_once=${SUBMIT_ONCE:-true} spike_size=$proc_spike_size, log: $log, cache: ${proc_cache:-disabled})"
  STRESS_BIN="$STRESS_BIN" \
  SUBMIT_ONCE="${SUBMIT_ONCE:-true}" \
  MAX_OUTSTANDING_PER_WORKER="${MAX_OUTSTANDING_PER_WORKER:-}" \
  SPIKE_SIZE="$proc_spike_size" \
  SPIKE_AT="${SPIKE_AT:-0}" \
  SPIKE_REPEAT="${SPIKE_REPEAT:-1}" \
  SPIKE_INTERVAL="${SPIKE_INTERVAL:-0}" \
  QPS="$proc_qps" \
  DURATION="$DURATION" \
  WORKERS="$proc_workers" \
  IN_FLIGHT_RATIO="$proc_ifr" \
  BURST="$proc_burst" \
  OPEN_LOOP="$proc_open_loop" \
  OPEN_LOOP_MAX_INFLIGHT_PER_WORKER="${OPEN_LOOP_MAX_INFLIGHT_PER_WORKER:-}" \
  INITIAL_BURST="${INITIAL_BURST:-0}" \
  INTERVAL="$proc_barrier" \
  GAS_CHUNK_SIZE="$GAS_CHUNK_SIZE" \
  GAS_POOL_CACHE_PATH="$proc_cache" \
  NUM_TRANSFER_ACCOUNTS="$NUM_TRANSFER_ACCOUNTS" \
  NUM_TARGET_VALIDATORS="${NUM_TARGET_VALIDATORS:-0}" \
  FULLNODE_RPC="$fn" \
  FULLNODE_RPC_ALL="$FULLNODES" \
  USE_FULLNODE_FOR_EXECUTION="${USE_FULLNODE_FOR_EXECUTION:-false}" \
  USE_FULLNODE_FOR_RECONFIG="${USE_FULLNODE_FOR_RECONFIG:-false}" \
  CLIENT_METRIC_PORT=0 \
  PRIMARY_GAS_OWNER="${GAS_OWNERS[$((GAS_OWNERS_OFFSET + i))]}" \
  RUNS_DIR="$proc_runs" \
  READY_FILE="$ready_file" \
  START_FILE="$START_FILE" \
  setsid ./stress-load-shedding.sh > "$log" 2>&1 &
  pids+=($!)
  # Stagger subprocess launches so their warmup pay_iota calls don't pile up
  # at the validator gate simultaneously (with TD path engaged, validators
  # see direct concurrent admissions from all procs — no fullnode buffer).
  # 500ms × N procs spreads setup over ~12s, plenty for the gate to drain
  # between proc warmup starts. Spam still syncs at the barrier post-ready.
  # Override with PROC_LAUNCH_STAGGER_MS=0 to disable.
  if [ "${PROC_LAUNCH_STAGGER_MS:-500}" -gt 0 ] && [ "$i" -lt "$((N - 1))" ]; then
    sleep "$(awk "BEGIN{print ${PROC_LAUNCH_STAGGER_MS:-500}/1000}")"
  fi
done

mark "$N subprocesses launched"
# Wait for all subprocesses to finish their setup (write their ready file).
# Bail after BARRIER_TIMEOUT seconds in case a subprocess crashes during setup.
BARRIER_TIMEOUT="${BARRIER_TIMEOUT:-1800}"  # 30 min default
echo "=> Waiting for all $N processes to finish setup (barrier sync, timeout=${BARRIER_TIMEOUT}s)..."
all_ready=0
elapsed=0
while [ $all_ready -eq 0 ]; do
  all_ready=1
  ready_count=0
  dead_without_ready=()
  for ((i=0; i<N; i++)); do
    rf="${ready_files[$i]}"
    if [ -f "$rf" ]; then
      ready_count=$((ready_count + 1))
    else
      all_ready=0
      # Fail-fast: if this subprocess's pid is no longer alive but it
      # never wrote a ready file, it crashed during setup. Bail out.
      if ! kill -0 "${pids[$i]}" 2>/dev/null; then
        dead_without_ready+=("$i")
      fi
    fi
  done
  echo "   [${elapsed}s] $ready_count/$N ready"
  if [ $all_ready -eq 1 ]; then break; fi
  if [ "${#dead_without_ready[@]}" -gt 0 ]; then
    echo
    echo "=> FAIL-FAST: ${#dead_without_ready[@]} subprocess(es) died during setup without writing ready file."
    for i in "${dead_without_ready[@]}"; do
      echo "--- process $i (log: ${logs[$i]}) ---"
      sed -r 's/\x1b\[[0-9;]*[mGKH]//g' "${logs[$i]}" 2>/dev/null \
        | grep -iE 'panic|insufficient|error|fail' \
        | grep -v 'compile\|^warning' \
        | tail -5
    done
    # Reap any remaining subprocesses so we don't leave orphans.
    # Each subprocess was launched via `setsid`, so its pid is the leader of
    # its own process group — kill the whole group (negative pid) to reach
    # the bash wrapper, the `script` fork, and the `target/release/stress`
    # grandchild. A plain `kill $pid` would only signal the bash wrapper and
    # leave the stress binary orphaned (which would hog its metrics port
    # 808N on the next run, causing AddrInUse panics).
    for pid in "${pids[@]}"; do kill -TERM -- -"$pid" 2>/dev/null || true; done
    # Release the start file so any still-waiting children unblock and exit.
    : > "$START_FILE" 2>/dev/null || true
    sleep 1
    # Anything still alive after SIGTERM gets SIGKILL.
    for pid in "${pids[@]}"; do kill -KILL -- -"$pid" 2>/dev/null || true; done
    for pid in "${pids[@]}"; do wait "$pid" 2>/dev/null || true; done
    echo "=> Exiting with code 1."
    exit 1
  fi
  if [ $elapsed -ge $BARRIER_TIMEOUT ]; then
    echo "=> Barrier timeout (${BARRIER_TIMEOUT}s). Releasing start anyway with $ready_count/$N ready."
    echo "   Missing: $(for ((i=0; i<N; i++)); do [ -f "${ready_files[$i]}" ] || echo "process-$i"; done)"
    break
  fi
  sleep 5
  elapsed=$((elapsed + 5))
done

# Cross-machine sync: when running stress-multi.sh on multiple machines in
# parallel, set WAIT_FOR_USER=1 on each. Both will pause after their local
# barrier here. Press Enter on both machines (roughly) simultaneously to
# release the spam phase.
if [ "${WAIT_FOR_USER:-0}" = "1" ]; then
  echo
  echo "================================================================"
  echo "  All $N processes finished setup on THIS machine."
  echo "  Press Enter to release the spam phase."
  echo "  (For multi-machine sync: press Enter on all machines at once.)"
  echo "================================================================"
  read -r _
fi

mark "all procs ready (gas pool gen done)"
echo "=> Releasing start barrier."
# Write wall-clock epoch (ns) so workers in barrier mode can align ticks across
# processes (and across machines, if clocks are NTP-synced). Plain `touch` is
# preserved as a fallback for non-barrier runs since stress.rs only parses the
# content when --barrier-period-ms > 0.
date +%s%N > "$START_FILE"
SPAM_START_EPOCH=$(date +%s)
# Derive spam_end_epoch from DURATION so downstream analysis can slice
# per-iter time-series to the actual spam window (sweep.sh's iter_window
# also covers stress-multi setup + cooldown, which is ~70s of mostly-idle).
SPAM_DURATION_SECS=$(echo "$DURATION" | sed 's/s$//')
SPAM_END_EPOCH=$((SPAM_START_EPOCH + SPAM_DURATION_SECS))
echo "=> Spam phase running (DURATION=$DURATION, INTERVAL=$INTERVAL)..."
if [ "$MEASURE_FROM_SECS" -gt 0 ]; then
  echo "=> Metrics cover the spam window from ${MEASURE_FROM_SECS}s on."
fi
mark "spam start"

echo "=> Waiting for all $N processes to finish (pids: ${pids[*]})"
exit_codes=()
for pid in "${pids[@]}"; do
  wait "$pid"
  exit_codes+=($?)
done
SPAM_END_EPOCH=$(date +%s)
mark "spam end (procs finished)"

# Helper to find the inner timestamped dir for a given process index.
inner_dir() {
  local i=$1
  ls -td "${runs_dirs[$i]}"/*/ 2>/dev/null | head -1
}

echo
echo "=> All processes done. Exit codes: ${exit_codes[*]}"

# Surface any panics / errors immediately so failures don't get buried in
# the per-process logs.
any_failed=0
for ((i=0; i<N; i++)); do
  if [ "${exit_codes[$i]}" -ne 0 ] 2>/dev/null; then
    any_failed=1
  fi
done
if [ "$any_failed" -ne 0 ]; then
  echo
  echo "=> Subprocess failures detected. Last error lines per process:"
  for ((i=0; i<N; i++)); do
    if [ "${exit_codes[$i]}" -ne 0 ] 2>/dev/null; then
      echo "--- process $i (exit=${exit_codes[$i]}, log=${logs[$i]}) ---"
      sed -r 's/\x1b\[[0-9;]*[mGKH]//g' "${logs[$i]}" 2>/dev/null \
        | grep -iE 'panic|error|fail|insufficient' \
        | grep -v 'compile\|warning:\|^warning\|note:' \
        | tail -5
    fi
  done
fi
echo "=> Per-process logs: ${logs[*]}"
echo
echo "=> Per-process run dirs:"
for ((i=0; i<N; i++)); do
  latest=$(inner_dir "$i")
  echo "   process $i → ${latest:-(no dir)}"
done
echo
echo "=> Per-process per-validator gauge peaks:"
for ((i=0; i<N; i++)); do
  echo "--- process $i → ${FN_ARR[$((i % ${#FN_ARR[@]}))]} ---"
  latest=$(inner_dir "$i")
  if [ -n "$latest" ] && [ -f "$latest/summary.txt" ]; then
    grep -A 9 '^\[gauge\] sum(sequencing_certificate_inflight)' "$latest/summary.txt" | head -10
  else
    echo "   (no summary)"
  fi
done

echo
echo "=> Aggregate rejections across all processes:"
total=0
for ((i=0; i<N; i++)); do
  latest=$(inner_dir "$i")
  if [ -n "$latest" ] && [ -f "$latest/summary.txt" ]; then
    n=$(grep -E '^[[:space:]]+TOTAL[[:space:]]+[0-9]+' "$latest/summary.txt" | head -1 | awk '{print $2}')
    echo "   process $i: $n"
    total=$((total + ${n:-0}))
  fi
done
echo "   GRAND TOTAL: $total"

echo
echo "=> TCP errors (EADDRNOTAVAIL) per subprocess:"
tcp_total=0
for ((i=0; i<N; i++)); do
  c=$(grep -c 'AddrNotAvailable\|code: 99' "${logs[$i]}" 2>/dev/null)
  c=${c:-0}
  echo "   process $i (${FN_ARR[$((i % ${#FN_ARR[@]}))]}): $c"
  tcp_total=$((tcp_total + c))
done
echo "   TOTAL: $tcp_total"

echo
echo "=> Peak num_inflight vs sem_cap (max across all process summaries; all see same network metrics):"
# Aggregate across all per-process summaries — some may have empty queries
# (Prometheus timing issues), so we take the max of whatever each captured.
sem_cap=0
peak_inflight=0
for ((i=0; i<N; i++)); do
  latest=$(inner_dir "$i")
  if [ -n "$latest" ] && [ -f "$latest/summary.txt" ]; then
    s=$(awk '/^\[gauge\] sequencing_in_flight_submissions/{f=1; next} /^$/{f=0} f && /peak=/{match($0, /peak=([0-9]+)/, a); print a[1]}' "$latest/summary.txt" | sort -n | tail -1)
    p=$(awk '/^\[gauge\] sum\(sequencing_certificate_inflight\)/{f=1; next} /^$/{f=0} f && /peak=/{match($0, /peak=([0-9]+)/, a); print a[1]}' "$latest/summary.txt" | sort -n | tail -1)
    [ -n "$s" ] && [ "$s" -gt "$sem_cap" ] && sem_cap=$s
    [ -n "$p" ] && [ "$p" -gt "$peak_inflight" ] && peak_inflight=$p
  fi
done
if [ "$sem_cap" -gt 0 ] && [ "$peak_inflight" -gt 0 ]; then
  ratio=$(awk -v p="$peak_inflight" -v s="$sem_cap" 'BEGIN{printf "%.2f", p/s}')
  echo "   sem_cap (max of post-permit peaks): $sem_cap"
  echo "   peak num_inflight (network max):    $peak_inflight"
  echo "   ratio peak/sem_cap:                 ${ratio}×"
else
  echo "   (couldn't compute — no process captured both metrics)"
fi

# Window: spam start → end + a small grace to capture the last scrape.
# With MEASURE_FROM_SECS, it starts that many seconds after the spam start
# instead, without the grace, which would reach back before that point:
# the queries end at SPAM_END_EPOCH.
MEASURE_START_EPOCH=$((SPAM_START_EPOCH + MEASURE_FROM_SECS))
if [ "$MEASURE_FROM_SECS" -gt 0 ]; then
  WINDOW=$(( SPAM_END_EPOCH - MEASURE_START_EPOCH ))
else
  WINDOW=$(( SPAM_END_EPOCH - SPAM_START_EPOCH + 5 ))
fi
if [ "$WINDOW" -lt 10 ]; then WINDOW=10; fi

# Helper: run an instant PromQL query and print the first sample's value
# as a float, or empty string on error / no result.
#
# Pinned at SPAM_END_EPOCH via the `time=` param so every `[WINDOW:1s]`
# subquery reads exactly the spam window (spam_end - WINDOW .. spam_end)
# rather than the default "ending at now" — which would otherwise leak
# ~5-10s of post-spam zero-inflight tail into the aggregates (biases
# inflight_mean down, inflight_stddev up, latency percentiles down).
# Same semantics as the PromQL `@${SPAM_END_EPOCH}` modifier but applied
# at the API layer so every existing query is fixed without rewriting.
prom_scalar() {
  local query="$1"
  if ! command -v curl >/dev/null 2>&1 || ! command -v python3 >/dev/null 2>&1; then
    return
  fi
  curl -sfG --max-time 5 "$PROM_URL/api/v1/query" \
    --data-urlencode "query=$query" \
    --data-urlencode "time=$SPAM_END_EPOCH" 2>/dev/null | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    rs = d.get('data', {}).get('result', [])
    if rs:
        v = float(rs[0]['value'][1])
        print(f'{v:.4f}')
except Exception:
    pass
" 2>/dev/null
}

# Helper: save a query_range time series to a JSON file for later
# cliff-vs-ramp analysis. Step 1s gives second-resolution detail.
prom_range() {
  local query="$1" outfile="$2"
  if ! command -v curl >/dev/null 2>&1; then return; fi
  curl -sfG --max-time 10 "$PROM_URL/api/v1/query_range" \
    --data-urlencode "query=$query" \
    --data-urlencode "start=$SPAM_START_EPOCH" \
    --data-urlencode "end=$SPAM_END_EPOCH" \
    --data-urlencode "step=1s" \
    >"$outfile" 2>/dev/null || rm -f "$outfile"
}

# Pull rejection counts per source from Prometheus over the spam window.
# The graduated check writes 5 distinct labels after the authority.rs split:
#   consensus_graduated_preventive  — shed_pct<100, probabilistic drop (soft zone)
#   consensus_graduated_saturated   — shed_pct=100, current<hard_limit (saturation
#                                     band [saturation_limit, hard_limit) — only
#                                     fires when graduated_load_shedding_saturation_pct
#                                     < 100, otherwise this band is empty)
#   consensus_graduated_reactive    — num_inflight >= max_pending, 100% shed at the
#                                     hard cap (safety fallback)
#   consensus_max_pending_exceeded  — num_inflight >= max_pending detected by the
#                                     binary check after graduated passed (race window)
#   consensus_semaphore_no_permits  — submit_semaphore exhausted (independent limit)
declare -A REJECT
REJECT[consensus_graduated_preventive]=0
REJECT[consensus_graduated_saturated]=0
REJECT[consensus_graduated_reactive]=0
REJECT[consensus_max_pending_exceeded]=0
REJECT[consensus_semaphore_no_permits]=0

if command -v curl >/dev/null 2>&1 && command -v python3 >/dev/null 2>&1; then
  prom_query="sum by (source) (increase(transaction_overload_sources{host=~\"validator.*\"}[${WINDOW}s]))"
  prom_resp=$(curl -sfG --max-time 5 "$PROM_URL/api/v1/query" \
    --data-urlencode "query=$prom_query" \
    --data-urlencode "time=$SPAM_END_EPOCH" 2>/dev/null || echo "")
  if [ -n "$prom_resp" ]; then
    while IFS=$'\t' read -r src val; do
      [ -z "$src" ] && continue
      REJECT[$src]=${val%.*}
    done < <(printf '%s' "$prom_resp" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    for r in d.get('data', {}).get('result', []):
        s = r['metric'].get('source', '?')
        v = float(r['value'][1])
        print(f'{s}\t{v:.0f}')
except Exception:
    pass
")
  fi
fi

# Throughput, queue distribution, latency, rejection-rate metrics over the
# spam window. We aggregate across the whole committee rather than hardcode
# one host because `NUM_TARGET_VALIDATORS=1` picks the target validator
# randomly per stress.rs run — it's not deterministically validator-1. For
# spam-attack metrics (queue depth, rejection rate), `max()` across hosts
# picks the targeted validator's value because the other validators see no
# spam and stay at 0. For network-wide metrics (TPS, latency) we aggregate
# similarly across hosts.

# Useful TPS: total executed transactions (= effects produced) per second.
# Max across validators — each validator executes the same finalised set
# (it's replicated state), so max() picks the one most up-to-date.
USEFUL_TPS=$(prom_scalar "max(increase(total_transaction_effects{host=~\"validator.*\"}[${WINDOW}s])) / ${WINDOW}")
[ -z "$USEFUL_TPS" ] && USEFUL_TPS=0

# Queue depth distribution during the spam window. `sequencing_certificate_inflight`
# is an IntGaugeVec with a `tx_type` label — sum across types per host
# first, then take max across hosts to surface the targeted validator's
# queue depth. Quantiles are then taken over time via a subquery.
QUEUE_P50=$(prom_scalar "quantile_over_time(0.50, max(sum by (host) (sequencing_certificate_inflight{host=~\"validator.*\"}))[${WINDOW}s:1s])")
QUEUE_P75=$(prom_scalar "quantile_over_time(0.75, max(sum by (host) (sequencing_certificate_inflight{host=~\"validator.*\"}))[${WINDOW}s:1s])")
QUEUE_P99=$(prom_scalar "quantile_over_time(0.99, max(sum by (host) (sequencing_certificate_inflight{host=~\"validator.*\"}))[${WINDOW}s:1s])")
[ -z "$QUEUE_P50" ] && QUEUE_P50=0
[ -z "$QUEUE_P75" ] && QUEUE_P75=0
[ -z "$QUEUE_P99" ] && QUEUE_P99=0

# Rejection rate distribution. `sum(rate(...))` aggregates across all
# `source` labels AND across hosts (the targeted validator's series
# dominates; other validators stay at 0). max_rate (rejs/sec peak across
# 5-sec windows) vs mean_rate characterises cliff vs ramp.
REJECT_RATE_MAX=$(prom_scalar "max_over_time((sum(rate(transaction_overload_sources{host=~\"validator.*\"}[5s])))[${WINDOW}s:5s])")
[ -z "$REJECT_RATE_MAX" ] && REJECT_RATE_MAX=0
TOTAL_REJ=$(( REJECT[consensus_graduated_preventive] + REJECT[consensus_graduated_saturated] + REJECT[consensus_graduated_reactive] + REJECT[consensus_max_pending_exceeded] + REJECT[consensus_semaphore_no_permits] ))
REJECT_RATE_MEAN=$(awk -v t="$TOTAL_REJ" -v w="$WINDOW" 'BEGIN{printf "%.2f", t/w}')

# Admission latency. The validator does not yet expose a full submit_tx
# RPC histogram; tx_verification_latency is the closest existing proxy
# (signature-verification step inside submit_tx_impl). Adding a proper
# end-to-end submit_tx histogram in authority_server/metrics.rs is a small
# follow-up that would give a more accurate number here.
ADMIT_LAT_P50=$(prom_scalar "histogram_quantile(0.50, sum by (le) (rate(validator_service_tx_verification_latency_bucket{host=~\"validator.*\"}[${WINDOW}s])))")
ADMIT_LAT_P99=$(prom_scalar "histogram_quantile(0.99, sum by (le) (rate(validator_service_tx_verification_latency_bucket{host=~\"validator.*\"}[${WINDOW}s])))")
[ -z "$ADMIT_LAT_P50" ] && ADMIT_LAT_P50=0
[ -z "$ADMIT_LAT_P99" ] && ADMIT_LAT_P99=0
# Wall-clock time the submit_semaphore permit is held per tx
# (acquire-success → drop). Drives interval sizing for burst sweeps:
# interval ≲ p99 → bursts overlap, interval ≫ p99 → drain between bursts.
# p90/p95/p999 added for richer tail characterisation.
PERMIT_HOLD_P50=$(prom_scalar "histogram_quantile(0.50, sum by (le) (rate(sequencing_submit_permit_hold_duration_bucket{host=~\"validator.*\"}[${WINDOW}s])))")
PERMIT_HOLD_P90=$(prom_scalar "histogram_quantile(0.90, sum by (le) (rate(sequencing_submit_permit_hold_duration_bucket{host=~\"validator.*\"}[${WINDOW}s])))")
PERMIT_HOLD_P95=$(prom_scalar "histogram_quantile(0.95, sum by (le) (rate(sequencing_submit_permit_hold_duration_bucket{host=~\"validator.*\"}[${WINDOW}s])))")
PERMIT_HOLD_P99=$(prom_scalar "histogram_quantile(0.99, sum by (le) (rate(sequencing_submit_permit_hold_duration_bucket{host=~\"validator.*\"}[${WINDOW}s])))")
PERMIT_HOLD_P999=$(prom_scalar "histogram_quantile(0.999, sum by (le) (rate(sequencing_submit_permit_hold_duration_bucket{host=~\"validator.*\"}[${WINDOW}s])))")
[ -z "$PERMIT_HOLD_P50" ] && PERMIT_HOLD_P50=0
[ -z "$PERMIT_HOLD_P90" ] && PERMIT_HOLD_P90=0
[ -z "$PERMIT_HOLD_P95" ] && PERMIT_HOLD_P95=0
[ -z "$PERMIT_HOLD_P99" ] && PERMIT_HOLD_P99=0
[ -z "$PERMIT_HOLD_P999" ] && PERMIT_HOLD_P999=0
# Stage B: time each tx blocked on submit_semaphore.acquire() — non-zero
# only when sem is the binding cap. Together with permit_hold (stage C),
# total in-flight latency ≈ wait + hold.
PERMIT_WAIT_P50=$(prom_scalar "histogram_quantile(0.50, sum by (le) (rate(sequencing_submit_permit_wait_duration_bucket{host=~\"validator.*\"}[${WINDOW}s])))")
PERMIT_WAIT_P90=$(prom_scalar "histogram_quantile(0.90, sum by (le) (rate(sequencing_submit_permit_wait_duration_bucket{host=~\"validator.*\"}[${WINDOW}s])))")
PERMIT_WAIT_P95=$(prom_scalar "histogram_quantile(0.95, sum by (le) (rate(sequencing_submit_permit_wait_duration_bucket{host=~\"validator.*\"}[${WINDOW}s])))")
PERMIT_WAIT_P99=$(prom_scalar "histogram_quantile(0.99, sum by (le) (rate(sequencing_submit_permit_wait_duration_bucket{host=~\"validator.*\"}[${WINDOW}s])))")
PERMIT_WAIT_P999=$(prom_scalar "histogram_quantile(0.999, sum by (le) (rate(sequencing_submit_permit_wait_duration_bucket{host=~\"validator.*\"}[${WINDOW}s])))")
[ -z "$PERMIT_WAIT_P50" ] && PERMIT_WAIT_P50=0
[ -z "$PERMIT_WAIT_P90" ] && PERMIT_WAIT_P90=0
[ -z "$PERMIT_WAIT_P95" ] && PERMIT_WAIT_P95=0
[ -z "$PERMIT_WAIT_P99" ] && PERMIT_WAIT_P99=0
[ -z "$PERMIT_WAIT_P999" ] && PERMIT_WAIT_P999=0
# Stage A: pre-acquire wait — InflightDropGuard::acquire to select! resolution.
# Captures leader-rotation wait + dedup-via-consensus race.
PRE_ACQUIRE_P50=$(prom_scalar "histogram_quantile(0.50, sum by (le) (rate(sequencing_submit_pre_acquire_duration_bucket{host=~\"validator.*\"}[${WINDOW}s])))")
PRE_ACQUIRE_P99=$(prom_scalar "histogram_quantile(0.99, sum by (le) (rate(sequencing_submit_pre_acquire_duration_bucket{host=~\"validator.*\"}[${WINDOW}s])))")
[ -z "$PRE_ACQUIRE_P50" ] && PRE_ACQUIRE_P50=0
[ -z "$PRE_ACQUIRE_P99" ] && PRE_ACQUIRE_P99=0
# Live graduated shed pct (gauge). Set on every admission attempt to the
# computed shed probability from `compute_graduated_load_shedding_percentage`.
# avg = typical shedding aggressiveness over the window. max = worst-case
# (queue peaked into the high-shed region). Both in [0, 100].
SHED_PCT_AVG=$(prom_scalar "avg_over_time((max(consensus_queue_load_shedding_percentage{host=~\"validator.*\"}))[${WINDOW}s:1s])")
SHED_PCT_MAX=$(prom_scalar "max_over_time((max(consensus_queue_load_shedding_percentage{host=~\"validator.*\"}))[${WINDOW}s:1s])")
[ -z "$SHED_PCT_AVG" ] && SHED_PCT_AVG=0
[ -z "$SHED_PCT_MAX" ] && SHED_PCT_MAX=0
# Stability of in-flight depth over the spam window. stddev/mean
# gives the coefficient of variation (CV) at analysis time —
# graduated should keep CV lower than binary since it paces
# admission smoothly instead of slamming + releasing.
INFLIGHT_STDDEV=$(prom_scalar "stddev_over_time((max(sum by (host) (sequencing_certificate_inflight{host=~\"validator.*\"})))[${WINDOW}s:1s])")
INFLIGHT_MEAN=$(prom_scalar "avg_over_time((max(sum by (host) (sequencing_certificate_inflight{host=~\"validator.*\"})))[${WINDOW}s:1s])")
[ -z "$INFLIGHT_STDDEV" ] && INFLIGHT_STDDEV=0
[ -z "$INFLIGHT_MEAN" ] && INFLIGHT_MEAN=0
# Fraction of the spam window in-flight sat above 75% of max_pending.
# Reveals "system was choked for 8 of 15s" runs that look fine on
# mean/p50 alone. Reads max_pending from the validator yaml since
# stress-multi doesn't otherwise know it.
val_max_pending_txs=$(grep -E "^[[:space:]]*max-pending-transactions:" \
  "$SCRIPT_DIR/dev-tools/iota-private-network/configs/validator-common.yaml" 2>/dev/null \
  | awk -F: '{print $2}' | xargs)
sat_thresh=$(( ${val_max_pending_txs:-1000} * 75 / 100 ))
SATURATION_75PCT=$(prom_scalar "avg_over_time(((max(sum by (host) (sequencing_certificate_inflight{host=~\"validator.*\"}))) > bool ${sat_thresh})[${WINDOW}s:1s])")
[ -z "$SATURATION_75PCT" ] && SATURATION_75PCT=0
# End-to-end consensus cert sequencing latency (post-permit-acquire
# through ack). Different from admit_lat_p99 (verification only).
# Tail behavior here is what users feel.
CONSENSUS_LAT_P50=$(prom_scalar "histogram_quantile(0.50, sum by (le) (rate(sequencing_certificate_latency_bucket{host=~\"validator.*\"}[${WINDOW}s])))")
CONSENSUS_LAT_P90=$(prom_scalar "histogram_quantile(0.90, sum by (le) (rate(sequencing_certificate_latency_bucket{host=~\"validator.*\"}[${WINDOW}s])))")
CONSENSUS_LAT_P95=$(prom_scalar "histogram_quantile(0.95, sum by (le) (rate(sequencing_certificate_latency_bucket{host=~\"validator.*\"}[${WINDOW}s])))")
CONSENSUS_LAT_P99=$(prom_scalar "histogram_quantile(0.99, sum by (le) (rate(sequencing_certificate_latency_bucket{host=~\"validator.*\"}[${WINDOW}s])))")
CONSENSUS_LAT_P999=$(prom_scalar "histogram_quantile(0.999, sum by (le) (rate(sequencing_certificate_latency_bucket{host=~\"validator.*\"}[${WINDOW}s])))")
[ -z "$CONSENSUS_LAT_P50" ] && CONSENSUS_LAT_P50=0
[ -z "$CONSENSUS_LAT_P90" ] && CONSENSUS_LAT_P90=0
[ -z "$CONSENSUS_LAT_P95" ] && CONSENSUS_LAT_P95=0
[ -z "$CONSENSUS_LAT_P99" ] && CONSENSUS_LAT_P99=0
[ -z "$CONSENSUS_LAT_P999" ] && CONSENSUS_LAT_P999=0

# Per-container CPU & memory consumption over the spam window — from
# cadvisor (grafana-local/docker-compose.yaml). Direct test of graduated's
# "cheaper per-rejection cost" claim (preventive shed at admission gate vs
# binary's reactive shed after permit/cert work). max() picks the targeted
# validator (validator-1 gets all spam under the single-validator-target
# strategy). Falls back to 0 if cadvisor isn't running.
VALIDATOR_CPU_SECONDS=$(prom_scalar "max(increase(container_cpu_usage_seconds_total{host=~\"validator.*\"}[${WINDOW}s]))")
VALIDATOR_MEM_PEAK=$(prom_scalar "max(max_over_time(container_memory_usage_bytes{host=~\"validator.*\"}[${WINDOW}s]))")
[ -z "$VALIDATOR_CPU_SECONDS" ] && VALIDATOR_CPU_SECONDS=0
[ -z "$VALIDATOR_MEM_PEAK" ] && VALIDATOR_MEM_PEAK=0

# The node's admission metrics. Only the target validator admits client
# transactions, so summing over hosts reads that validator. Empty when the
# node predates these metrics.
#   fill:     consensus queue length at each admission decision, as a
#             percentage of max_pending_transactions, by outcome.
#   stretch:  consecutive decisions with the same outcome; mean length, and
#             p99 of how long such a stretch lasted.
#   wait:     how long a user transaction admitted on the P-COOL path held
#             its slot in the consensus queue, until processed via consensus.
#   settings: the load shedding settings the node was built with.
adm_quantile() {
  prom_scalar "histogram_quantile($1, sum by (le) (rate(${2}_bucket{host=~\"validator.*\"${3:+,$3}}[${WINDOW}s])))"
}
adm_mean() {
  prom_scalar "sum(increase(${1}_sum{host=~\"validator.*\"${2:+,$2}}[${WINDOW}s])) / sum(increase(${1}_count{host=~\"validator.*\"${2:+,$2}}[${WINDOW}s]))"
}
adm_count() {
  prom_scalar "sum(increase(${1}_count{host=~\"validator.*\"${2:+,$2}}[${WINDOW}s]))"
}
FILL_ACC_P50=$(adm_quantile 0.50 consensus_queue_fill_at_admission 'outcome="accepted"')
FILL_ACC_P90=$(adm_quantile 0.90 consensus_queue_fill_at_admission 'outcome="accepted"')
FILL_ACC_P99=$(adm_quantile 0.99 consensus_queue_fill_at_admission 'outcome="accepted"')
FILL_ACC_MEAN=$(adm_mean consensus_queue_fill_at_admission 'outcome="accepted"')
FILL_ACC_COUNT=$(adm_count consensus_queue_fill_at_admission 'outcome="accepted"')
FILL_REJ_P50=$(adm_quantile 0.50 consensus_queue_fill_at_admission 'outcome="rejected"')
FILL_REJ_P99=$(adm_quantile 0.99 consensus_queue_fill_at_admission 'outcome="rejected"')
FILL_REJ_MEAN=$(adm_mean consensus_queue_fill_at_admission 'outcome="rejected"')
FILL_REJ_COUNT=$(adm_count consensus_queue_fill_at_admission 'outcome="rejected"')
STRETCH_LEN_ACC_MEAN=$(adm_mean consensus_queue_admission_stretch_length 'outcome="accepted"')
STRETCH_LEN_REJ_MEAN=$(adm_mean consensus_queue_admission_stretch_length 'outcome="rejected"')
STRETCH_LEN_REJ_P99=$(adm_quantile 0.99 consensus_queue_admission_stretch_length 'outcome="rejected"')
STRETCH_DUR_ACC_P99=$(adm_quantile 0.99 consensus_queue_admission_stretch_duration 'outcome="accepted"')
STRETCH_DUR_REJ_P99=$(adm_quantile 0.99 consensus_queue_admission_stretch_duration 'outcome="rejected"')
WAIT_FILTER='tx_type=~".*user_transaction",processed_method="processed_via_consensus"'
WAIT_ADM_P50=$(adm_quantile 0.50 consensus_queue_wait_since_admission "$WAIT_FILTER")
WAIT_ADM_P90=$(adm_quantile 0.90 consensus_queue_wait_since_admission "$WAIT_FILTER")
WAIT_ADM_P99=$(adm_quantile 0.99 consensus_queue_wait_since_admission "$WAIT_FILTER")
WAIT_ADM_MEAN=$(adm_mean consensus_queue_wait_since_admission "$WAIT_FILTER")
node_setting() {
  prom_scalar "max(consensus_queue_settings{host=~\"validator.*\",setting=\"$1\"})"
}
SET_MAX_PENDING=$(node_setting max_pending_transactions)
SET_SOFT_PCT=$(node_setting soft_limit_pct)
SET_SAT_PCT=$(node_setting saturation_pct)
SET_PERMITS=$(node_setting submit_permits)
SET_SEM_SHEDDING=$(node_setting semaphore_shedding_enabled)

# Consensus over the same window, written to consensus.json for sweep.sh:
# how often it commits and how fast rounds advance, what a commit carries,
# what each validator proposes and why, how leaders were decided, how long
# transactions take to be sequenced, and what the nodes dropped after
# consensus. Gauges that only grow (commit index, rounds) count as their
# rise over the window. Means come from the _sum and _count pairs that
# Starfish exports. Figures every validator sees alike take the max over
# validators; figures about a validator's own blocks are kept per host.
PROM_URL="$PROM_URL" WINDOW="$WINDOW" END_EPOCH="$SPAM_END_EPOCH" python3 - <<'PY' > "$PARENT_DIR/consensus.json" 2>/dev/null || echo '{}' > "$PARENT_DIR/consensus.json"
import json, math, os, urllib.parse, urllib.request

prom = os.environ["PROM_URL"]
window = int(os.environ["WINDOW"])
end = os.environ["END_EPOCH"]
V = '{host=~"validator.*"}'
W = f"[{window}s]"


def query(expr):
    url = f"{prom}/api/v1/query?" + urllib.parse.urlencode({"query": expr, "time": end})
    try:
        with urllib.request.urlopen(url, timeout=5) as resp:
            return json.load(resp).get("data", {}).get("result", [])
    except (OSError, ValueError):
        return []


def number(raw):
    try:
        value = float(raw)
    except (TypeError, ValueError):
        return None
    return round(value, 4) if math.isfinite(value) else None


def scalar(expr):
    rows = query(expr)
    return number(rows[0]["value"][1]) if rows else None


def by(expr, *labels):
    return {
        "/".join(row["metric"].get(label, "?") for label in labels): number(row["value"][1])
        for row in query(expr)
    }


def rise_per_sec(gauge):
    return scalar(f"max(max_over_time({gauge}{V}{W}) - min_over_time({gauge}{V}{W})) / {window}")


def mean_by_host(pair):
    return by(f"sum by (host) (increase({pair}_sum{V}{W})) / sum by (host) (increase({pair}_count{V}{W}))", "host")


def max_mean(pair):
    values = [value for value in mean_by_host(pair).values() if value is not None]
    return max(values) if values else None


def quantiles(hist):
    return {
        f"p{int(q * 100)}": scalar(f"histogram_quantile({q}, sum by (le) (rate({hist}_bucket{V}{W})))")
        for q in (0.5, 0.99)
    }


by_host = {}


def put(field, values):
    for host, value in values.items():
        by_host.setdefault(host, {})[field] = value


put("blocks_per_sec", by(f"sum by (host) (rate(consensus_proposed_blocks{V}{W}))", "host"))
put("block_transactions_mean", mean_by_host("consensus_proposed_block_transactions"))
put("block_bytes_mean", mean_by_host("consensus_proposed_block_size"))
put("proposal_interval_secs_mean", mean_by_host("consensus_block_proposal_interval"))
put("leader_wait_ms_mean", by(f"sum by (host) (increase(consensus_block_proposal_leader_wait_ms{V}{W})) / sum by (host) (increase(consensus_block_proposal_leader_wait_count{V}{W}))", "host"))
put("leader_waits_per_sec", by(f"sum by (host) (rate(consensus_block_proposal_leader_wait_count{V}{W}))", "host"))
for field, metric in (("proposals_by_reason", "consensus_proposed_blocks"),
                      ("skipped_proposals_by_reason", "consensus_core_skipped_proposals")):
    for key, value in by(f"sum by (host, reason) (increase({metric}{V}{W}))", "host", "reason").items():
        host, reason = key.split("/", 1)
        by_host.setdefault(host, {}).setdefault(field, {})[reason] = value

print(json.dumps({
    "window_secs": window,
    "commits_per_sec": rise_per_sec("consensus_last_commit_index"),
    "committed_leader_rounds_per_sec": rise_per_sec("consensus_last_committed_leader_round"),
    "rounds_per_sec": rise_per_sec("consensus_threshold_clock_round"),
    "commit_interval_secs_mean": max_mean("consensus_commit_round_advancement_interval"),
    "commit_timestamp_gap_ms_mean": max_mean("consensus_last_commit_time_diff"),
    "per_commit_mean": {
        "transactions": max_mean("consensus_transactions_per_commit_count"),
        "blocks": max_mean("consensus_blocks_per_commit_count"),
        "non_empty_blocks": max_mean("consensus_non_empty_blocks_per_commit_count"),
        "sub_dags": max_mean("consensus_sub_dags_per_commit_count"),
    },
    "decided_leaders": by(f"max by (commit_type) (sum by (host, commit_type) (increase(consensus_decided_leaders_total{V}{W})))", "commit_type"),
    "committed_user_transactions_per_sec": by(f"max by (authority) (increase(consensus_committed_user_transactions{V}{W})) / {window}", "authority"),
    "transaction_commit_latency_secs": quantiles("consensus_transaction_commit_latency"),
    "block_header_commit_latency_secs": quantiles("consensus_block_header_commit_latency"),
    "dropped_after_consensus": {
        "transactions": scalar(f"max(increase(consensus_handler_load_shedding_dropped_transactions{V}{W}))"),
        "shedding_pct_avg": scalar(f"max(avg_over_time(consensus_handler_load_shedding_percentage{V}{W}))"),
        "shedding_pct_max": scalar(f"max(max_over_time(consensus_handler_load_shedding_percentage{V}{W}))"),
    },
    "commit_execution_lag_max": scalar(f"max(max_over_time(consensus_handler_commit_execution_lag{V}{W}))"),
    "calculated_throughput_avg": scalar(f"max(avg_over_time(consensus_calculated_throughput{V}{W}))"),
    "by_host": by_host,
}))
PY

# One line for summary.txt and the sweep log.
CONSENSUS_LINE=$(python3 - "$PARENT_DIR/consensus.json" <<'PY' 2>/dev/null
import json, sys

try:
    c = json.load(open(sys.argv[1]))
except (OSError, ValueError):
    c = {}


def fmt(value, spec):
    return format(value, spec) if isinstance(value, (int, float)) else "n/a"


block_txs = [h.get("block_transactions_mean") for h in (c.get("by_host") or {}).values()]
block_txs = max((v for v in block_txs if v is not None), default=None)
print(f"consensus: commits_per_sec={fmt(c.get('commits_per_sec'), '.2f')} "
      f"rounds_per_sec={fmt(c.get('rounds_per_sec'), '.2f')} "
      f"txs_per_commit={fmt((c.get('per_commit_mean') or {}).get('transactions'), '.1f')} "
      f"max_block_txs_mean={fmt(block_txs, '.1f')} "
      f"dropped_after_consensus={fmt((c.get('dropped_after_consensus') or {}).get('transactions'), '.0f')}")
PY
)

# Time-series captures for post-hoc analysis (e.g. plotting cliff vs ramp,
# inspecting queue-depth shape during a burst). One JSON per metric, saved
# alongside summary.txt under the run directory. We aggregate across hosts
# so the dumps capture the targeted validator's behaviour without us
# having to know which one was picked.
prom_range "sum by (source) (rate(transaction_overload_sources{host=~\"validator.*\"}[5s]))" \
  "$PARENT_DIR/rejection_rate_by_source.json"
prom_range "max(sum by (host) (sequencing_certificate_inflight{host=~\"validator.*\"}))" \
  "$PARENT_DIR/queue_depth.json"

# Per-pool aggregation. Reads each per-process benchmark_stats.json (written
# by stress.rs to PARENT_DIR/<inner-ts>/benchmark_stats.json via the
# --benchmark-stats-path flag in stress-load-shedding.sh) and sums
# num_success_txes / num_error_txes grouped by pool. With HONEST_PROC_COUNT=0
# the honest pool is empty and the spammer pool equals the whole run.
SPAMMER_SUCCESS=0
SPAMMER_ERROR=0
HONEST_SUCCESS=0
HONEST_ERROR=0
HONEST_CL_SUCCESS=0
HONEST_CL_ERROR=0
for ((i=0; i<N; i++)); do
  stats_file=$(ls "${runs_dirs[$i]}"/*/benchmark_stats.json 2>/dev/null | head -1)
  if [ -z "$stats_file" ] || [ ! -f "$stats_file" ]; then continue; fi
  read -r succ err < <(python3 -c "
import json, sys
try:
    d = json.load(open('$stats_file'))
    print(d.get('num_success_txes', 0), d.get('num_error_txes', 0))
except Exception:
    print(0, 0)
" 2>/dev/null)
  succ=${succ:-0}
  err=${err:-0}
  case "${pool_labels[$i]}" in
    honest_cl)
      HONEST_CL_SUCCESS=$((HONEST_CL_SUCCESS + succ))
      HONEST_CL_ERROR=$((HONEST_CL_ERROR + err))
      ;;
    honest)
      HONEST_SUCCESS=$((HONEST_SUCCESS + succ))
      HONEST_ERROR=$((HONEST_ERROR + err))
      ;;
    *)
      SPAMMER_SUCCESS=$((SPAMMER_SUCCESS + succ))
      SPAMMER_ERROR=$((SPAMMER_ERROR + err))
      ;;
  esac
done
# Derive per-pool TPS and accept-rate over the whole spam window, which the
# success counts from the stats files cover, whatever MEASURE_FROM_SECS is.
SPAM_SECS=$(( SPAM_END_EPOCH - SPAM_START_EPOCH ))
SPAMMER_TPS=$(awk -v s=$SPAMMER_SUCCESS -v w=$SPAM_SECS 'BEGIN{if(w>0) printf "%.2f", s/w; else print 0}')
HONEST_TPS=$(awk -v s=$HONEST_SUCCESS -v w=$SPAM_SECS 'BEGIN{if(w>0) printf "%.2f", s/w; else print 0}')
HONEST_CL_TPS=$(awk -v s=$HONEST_CL_SUCCESS -v w=$SPAM_SECS 'BEGIN{if(w>0) printf "%.2f", s/w; else print 0}')
SPAMMER_TOTAL=$((SPAMMER_SUCCESS + SPAMMER_ERROR))
HONEST_TOTAL=$((HONEST_SUCCESS + HONEST_ERROR))
HONEST_CL_TOTAL=$((HONEST_CL_SUCCESS + HONEST_CL_ERROR))
SPAMMER_ACCEPT_PCT=$(awk -v s=$SPAMMER_SUCCESS -v t=$SPAMMER_TOTAL 'BEGIN{if(t>0) printf "%.2f", 100.0*s/t; else print 0}')
HONEST_ACCEPT_PCT=$(awk -v s=$HONEST_SUCCESS -v t=$HONEST_TOTAL 'BEGIN{if(t>0) printf "%.2f", 100.0*s/t; else print 0}')
HONEST_CL_ACCEPT_PCT=$(awk -v s=$HONEST_CL_SUCCESS -v t=$HONEST_CL_TOTAL 'BEGIN{if(t>0) printf "%.2f", 100.0*s/t; else print 0}')

# Per-pool totals from each process's `send_report` (written with
# SUBMIT_ONCE=true): attempts by outcome, first attempts accepted, whether
# every process held its send rate, and the spikes. "measured" counts the
# steady sends from MEASURE_FROM_SECS on, and how many of them were
# accepted, from the client's counts for each second. Written to pools.json
# for sweep.sh; a pool without send reports gets null.
POOL_LABELS="${pool_labels[*]}" PARENT_DIR="$PARENT_DIR" MEASURE_FROM_SECS="$MEASURE_FROM_SECS" python3 - <<'PY' > "$PARENT_DIR/pools.json" 2>/dev/null || echo '{}' > "$PARENT_DIR/pools.json"
import glob, json, os

parent = os.environ["PARENT_DIR"]
labels = os.environ["POOL_LABELS"].split()
measure_from = int(os.environ.get("MEASURE_FROM_SECS") or 0)
counts = ["sent", "answered", "accepted", "already_executed",
          "rejected_overload_retry_after", "rejected_overload_no_hint",
          "rejected_other", "transport_errors", "no_answer_at_end",
          "first_attempts", "first_attempts_accepted"]
pools = {}
for i, label in enumerate(labels):
    files = glob.glob(f"{parent}/process-{i}/*/benchmark_stats.json")
    if not files:
        continue
    try:
        with open(files[0]) as f:
            stats = json.load(f)
    except (OSError, json.JSONDecodeError):
        continue
    report = stats.get("send_report")
    if not report:
        continue
    pool = pools.setdefault(label, {
        **{name: 0 for name in counts},
        "rejected_other_by_error": {},
        "processes": 0,
        "invalid_processes": 0,
        "invalid_reasons": [],
        "target_per_sec": 0.0,
        "achieved_per_sec": 0.0,
        "per_process": [],
        "spikes": [],
        "measured": {"from_secs": measure_from, "sent": 0, "accepted": 0},
    })
    attempts = report["attempts"]
    for name in counts:
        pool[name] += attempts.get(name) or 0
    for error, n in (attempts.get("rejected_other_by_error") or {}).items():
        pool["rejected_other_by_error"][error] = pool["rejected_other_by_error"].get(error, 0) + n
    pool["processes"] += 1
    if not report.get("valid", False):
        pool["invalid_processes"] += 1
        pool["invalid_reasons"] += [f"process {i}: {r}" for r in report.get("invalid_reasons", [])]
    rate = report.get("send_rate") or {}
    pool["target_per_sec"] += rate.get("target_per_sec") or 0
    pool["achieved_per_sec"] += rate.get("achieved_per_sec") or 0
    measured = {
        "sent": sum((rate.get("sent_per_second") or [])[measure_from:]),
        "accepted": sum((rate.get("accepted_per_second") or [])[measure_from:]),
    }
    pool["measured"]["sent"] += measured["sent"]
    pool["measured"]["accepted"] += measured["accepted"]
    pool["per_process"].append({
        "process": i,
        "valid": report.get("valid"),
        "send_rate": {k: rate.get(k) for k in ("target_per_sec", "achieved_per_sec", "max_deviation_pct")},
        "measured": measured,
        "latency_ms": report.get("latency_ms"),
        "effects": report.get("effects"),
        "payloads_retired": (report.get("payloads") or {}).get("retired"),
        "run_config": stats.get("run_config"),
        "spam_window": stats.get("spam_window"),
    })
    pool["spikes"] += report.get("spikes") or []

for pool in pools.values():
    first = pool["first_attempts"]
    pool["first_attempt_accept_share"] = (
        pool["first_attempts_accepted"] / first if first else None
    )
    # Every send of the fixed rate sender is a first attempt; for the
    # closed loop sender this share also counts resends.
    measured = pool["measured"]
    measured["accept_share"] = (
        measured["accepted"] / measured["sent"] if measured["sent"] else None
    )
print(json.dumps(pools))
PY

# One line per pool in summary.txt; sweep.sh reads pools.json for the rest.
POOL_LINES=$(python3 - "$PARENT_DIR/pools.json" <<'PY' 2>/dev/null
import json, sys
try:
    pools = json.load(open(sys.argv[1]))
except (OSError, json.JSONDecodeError):
    pools = {}
for label, p in sorted(pools.items()):
    share = p.get("first_attempt_accept_share")
    share = f"{100 * share:.2f}" if share is not None else "n/a"
    m = p.get("measured") or {}
    measured = ""
    if m.get("from_secs") and m.get("accept_share") is not None:
        measured = (f" measured_from={m['from_secs']}s measured_sent={m['sent']} "
                    f"measured_accept_pct={100 * m['accept_share']:.2f}")
    print(f"{label}_send_report: sent={p['sent']} accepted={p['accepted']} "
          f"first_attempts={p['first_attempts']} first_accept_pct={share} "
          f"overload_hint={p['rejected_overload_retry_after']} "
          f"overload_no_hint={p['rejected_overload_no_hint']} "
          f"other={p['rejected_other']} transport={p['transport_errors']} "
          f"invalid_processes={p['invalid_processes']}{measured}")
PY
)

# Which validators the client sent to. With SUBMIT_ONCE=true the client
# prints them on one line; the transaction driver path prints its own.
# Strip tracing's ANSI color codes first so the line is greppable.
TARGET_VALIDATOR=$(sed -r 's/\x1B\[[0-9;]*[mGKHF]//g' "$PARENT_DIR/process-0.log" 2>/dev/null \
  | grep -oE "Submit once: each worker sends to one of .*" | tail -1 \
  | sed 's/^Submit once: each worker sends to one of //')
if [ -z "$TARGET_VALIDATOR" ]; then
  TARGET_VALIDATOR=$(grep "Targeting [0-9]\+ of [0-9]\+ validators" "$PARENT_DIR/process-0.log" 2>/dev/null \
    | tail -1 | sed -r 's/\x1B\[[0-9;]*[mGKHF]//g' | grep -oE '\[[^]]*\]' | tail -1 || echo "?")
fi

# Save a top-level summary so this run is self-contained
{
  echo "ts:           $MASTER_TS"
  echo "config:       QPS_TOTAL=$QPS_TOTAL DURATION=$DURATION WORKERS=$WORKERS IN_FLIGHT_RATIO=$IN_FLIGHT_RATIO BURST=$BURST INTERVAL=$INTERVAL GAS_CHUNK_SIZE=$GAS_CHUNK_SIZE NUM_TRANSFER_ACCOUNTS=$NUM_TRANSFER_ACCOUNTS"
  echo "fullnodes:    $FULLNODES"
  echo "target_validator: $TARGET_VALIDATOR"
  echo "exit codes:   ${exit_codes[*]}"
  echo "tcp errors:   $tcp_total"
  if [ "${peak_inflight:-0}" -gt 0 ] && [ "${sem_cap:-0}" -gt 0 ]; then
    echo "sem_cap:      $sem_cap"
    echo "peak inflight:$peak_inflight"
    echo "ratio:        ${ratio:-0}×"
  fi
  echo "reject_grad_preventive: ${REJECT[consensus_graduated_preventive]}"
  echo "reject_grad_saturated:  ${REJECT[consensus_graduated_saturated]}"
  echo "reject_grad_reactive:   ${REJECT[consensus_graduated_reactive]}"
  echo "reject_max_pending:     ${REJECT[consensus_max_pending_exceeded]}"
  echo "reject_semaphore:       ${REJECT[consensus_semaphore_no_permits]}"
  echo "useful_tps:             $USEFUL_TPS"
  echo "queue_depth_p50:        $QUEUE_P50"
  echo "queue_depth_p75:        $QUEUE_P75"
  echo "queue_depth_p99:        $QUEUE_P99"
  echo "reject_rate_max:        $REJECT_RATE_MAX"
  echo "reject_rate_mean:       $REJECT_RATE_MEAN"
  echo "admit_lat_p50:          $ADMIT_LAT_P50"
  echo "admit_lat_p99:          $ADMIT_LAT_P99"
  echo "permit_wait_p50:        $PERMIT_WAIT_P50"
  echo "permit_wait_p90:        $PERMIT_WAIT_P90"
  echo "permit_wait_p95:        $PERMIT_WAIT_P95"
  echo "permit_wait_p99:        $PERMIT_WAIT_P99"
  echo "permit_wait_p999:       $PERMIT_WAIT_P999"
  echo "shed_pct_avg:           $SHED_PCT_AVG"
  echo "shed_pct_max:           $SHED_PCT_MAX"
  echo "pre_acquire_p50:        $PRE_ACQUIRE_P50"
  echo "pre_acquire_p99:        $PRE_ACQUIRE_P99"
  echo "permit_hold_p50:        $PERMIT_HOLD_P50"
  echo "permit_hold_p90:        $PERMIT_HOLD_P90"
  echo "permit_hold_p95:        $PERMIT_HOLD_P95"
  echo "permit_hold_p99:        $PERMIT_HOLD_P99"
  echo "permit_hold_p999:       $PERMIT_HOLD_P999"
  echo "inflight_stddev:        $INFLIGHT_STDDEV"
  echo "inflight_mean:          $INFLIGHT_MEAN"
  echo "saturation_75pct:       $SATURATION_75PCT"
  echo "consensus_lat_p50:      $CONSENSUS_LAT_P50"
  echo "consensus_lat_p90:      $CONSENSUS_LAT_P90"
  echo "consensus_lat_p95:      $CONSENSUS_LAT_P95"
  echo "consensus_lat_p99:      $CONSENSUS_LAT_P99"
  echo "consensus_lat_p999:     $CONSENSUS_LAT_P999"
  echo "validator_cpu_seconds:  $VALIDATOR_CPU_SECONDS"
  echo "validator_mem_peak:     $VALIDATOR_MEM_PEAK"
  echo "fill_accepted_p50:      $FILL_ACC_P50"
  echo "fill_accepted_p90:      $FILL_ACC_P90"
  echo "fill_accepted_p99:      $FILL_ACC_P99"
  echo "fill_accepted_mean:     $FILL_ACC_MEAN"
  echo "fill_accepted_count:    $FILL_ACC_COUNT"
  echo "fill_rejected_p50:      $FILL_REJ_P50"
  echo "fill_rejected_p99:      $FILL_REJ_P99"
  echo "fill_rejected_mean:     $FILL_REJ_MEAN"
  echo "fill_rejected_count:    $FILL_REJ_COUNT"
  echo "stretch_len_accepted_mean: $STRETCH_LEN_ACC_MEAN"
  echo "stretch_len_rejected_mean: $STRETCH_LEN_REJ_MEAN"
  echo "stretch_len_rejected_p99:  $STRETCH_LEN_REJ_P99"
  echo "stretch_dur_accepted_p99:  $STRETCH_DUR_ACC_P99"
  echo "stretch_dur_rejected_p99:  $STRETCH_DUR_REJ_P99"
  echo "wait_admitted_p50:      $WAIT_ADM_P50"
  echo "wait_admitted_p90:      $WAIT_ADM_P90"
  echo "wait_admitted_p99:      $WAIT_ADM_P99"
  echo "wait_admitted_mean:     $WAIT_ADM_MEAN"
  echo "setting_max_pending:    $SET_MAX_PENDING"
  echo "setting_soft_pct:       $SET_SOFT_PCT"
  echo "setting_sat_pct:        $SET_SAT_PCT"
  echo "setting_permits:        $SET_PERMITS"
  echo "setting_sem_shedding:   $SET_SEM_SHEDDING"
  [ -n "$POOL_LINES" ] && echo "$POOL_LINES"
  [ -n "$CONSENSUS_LINE" ] && echo "$CONSENSUS_LINE"
  echo "spam_start_epoch:       $SPAM_START_EPOCH"
  echo "spam_end_epoch:         $SPAM_END_EPOCH"
  echo "spam_duration_secs:     $SPAM_DURATION_SECS"
  echo "measure_from_secs:      $MEASURE_FROM_SECS"
  echo "measure_start_epoch:    $MEASURE_START_EPOCH"
  echo "window_secs:            $WINDOW"
  echo "spammer_proc_count:     $N_SPAMMER"
  echo "honest_proc_count:      $HONEST_PROC_COUNT"
  echo "honest_cl_proc_count:   $HONEST_CL_PROC_COUNT"
  echo "spammer_success:        $SPAMMER_SUCCESS"
  echo "spammer_error:          $SPAMMER_ERROR"
  echo "spammer_tps:            $SPAMMER_TPS"
  echo "spammer_accept_pct:     $SPAMMER_ACCEPT_PCT"
  echo "honest_success:         $HONEST_SUCCESS"
  echo "honest_error:           $HONEST_ERROR"
  echo "honest_tps:             $HONEST_TPS"
  echo "honest_accept_pct:      $HONEST_ACCEPT_PCT"
  echo "honest_cl_success:      $HONEST_CL_SUCCESS"
  echo "honest_cl_error:        $HONEST_CL_ERROR"
  echo "honest_cl_tps:          $HONEST_CL_TPS"
  echo "honest_cl_accept_pct:   $HONEST_CL_ACCEPT_PCT"
} > "$PARENT_DIR/summary.txt"
mark "metrics scrape + summary done"
echo
echo "=> Top-level summary: $PARENT_DIR/summary.txt"
