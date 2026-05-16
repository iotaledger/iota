#!/usr/bin/env bash
# Run multiple stress.rs processes in parallel, each pinned to one fullnode.
# Works around iota-benchmark stress.rs's "random proxy chosen once per workload"
# behavior in bench_driver.rs:356.
#
# Outputs go to runs/multi-<utc-ts>/ — one parent dir per invocation, never overwritten.

set -uo pipefail

QPS_TOTAL="${QPS_TOTAL:-10000}"
DURATION="${DURATION:-300s}"
WORKERS="${WORKERS:-12}"
IN_FLIGHT_RATIO="${IN_FLIGHT_RATIO:-10}"
BURST_SIZE="${BURST_SIZE:-1}"
BARRIER_PERIOD_MS="${BARRIER_PERIOD_MS:-0}"
GAS_CHUNK_SIZE="${GAS_CHUNK_SIZE:-500}"
NUM_TRANSFER_ACCOUNTS="${NUM_TRANSFER_ACCOUNTS:-2}"
# Persistent gas-pool cache dir (survives across runs). Each subprocess gets
# its own cache file under this dir, keyed by primary_gas_owner index. To
# disable the cache, set GAS_POOL_CACHE_DIR="disable" (or any non-path that
# doesn't exist and can't be created).
GAS_POOL_CACHE_DIR="${GAS_POOL_CACHE_DIR:-$HOME/.stress-gas-pool}"
FULLNODES="${FULLNODES:-http://127.0.0.1:9000}"

IFS=',' read -ra FN_ARR <<< "$FULLNODES"
# Number of stress subprocesses. Defaults to the number of fullnode URLs (one
# proc primarily targets one fullnode), but can be overridden — e.g.
# NUM_PROCS=8 FULLNODES=<4 urls> ... gives 8 procs sharing 4 fullnodes (2:1),
# which preserves client-side concurrency (more independent RPC clients, more
# burst races at the gate) even with a slimmer validator network.
N="${NUM_PROCS:-${#FN_ARR[@]}}"
QPS_PER=$((QPS_TOTAL / N))

# Offset into the GAS_OWNERS array. Set GAS_OWNERS_OFFSET=4 on the second
# machine in a 2-machine setup so it uses gas owners 4-7 (avoiding contention
# with machine 1 using 0-3).
GAS_OWNERS_OFFSET="${GAS_OWNERS_OFFSET:-0}"

# Benchmark gas-owner addresses from bootstrap.sh — one per subprocess.
# 24 owners → NUM_PROCS up to 24.
GAS_OWNERS=(
  "0xf479d29837d22943aba6afc401f518a36521b990874eca784886185bd26bf681"
  "0xcd2617da70a7b430103ad101ae570db412156521851fb18cc1acbdd59720c2c1"
  "0x0b75ee76891aeab997785963407e6add2ed6a7cd3a414e11dad8b7204d0d3f4b"
  "0x05febd29e0f349b6fbfbed1f279481517f162c5653c5c98173cc1aa79d4d2fdd"
  "0x9d7e87c7519b31853f3c3cc471cff15e4dd910c55588acb0c3b8237847e04134"
  "0x4380e818ee8f001795742ad87be2dcd6eebf0624884a8871557f3be997aca35b"
  "0x9fa4e43c1bebd3d787b44022e7e4c96ff5977456099842cf464d707dde02328d"
  "0x6ab84f85ea9c40b3eb6841b66c7baaf63b271c3f198a5b2260b74d007c8c71f5"
  "0x5bbc757c117d79fe27533af0c3d112b85299385e8935be0f167b5184222afd12"
  "0xccc9e62219640cd2c0f48ef49daf8c9755e74c03f649a3ecd2d576b9b03a135f"
  "0x88d4be6e3236eee110de5c592c90615ddbe6999688ad91c766609f03ad64f6e4"
  "0x0aad9e9dadacdee74a393efeec28ed60090750a7f6d63343ff1cadf280144402"
  "0xb2dc6c370b5f475c281c6a7b15f5974acf7fbd331ace6b3f38c9c42c48ba6031"
  "0xde7b523dafefc8695cc117e168324bfd01b60be7dda1d173177f699feafde734"
  "0x450451005bf6cf8ec46424ba399e020528dcc3cd3f945edd9551ffb3cd309d56"
  "0x293210fa0f212e3bdb988a2b413b0579a0f74ca6cf6ca46fa73c801552ea2e52"
  "0xa45fe7c6081c17266a26f7e2cd3da78d5363a88d3d0358a2365f291ff3a52208"
  "0x0fff519f4245be4ceb3015f0fa863d97c54f733386e2f72ee294160157b5017f"
  "0x11c8aa01f82e52aa8200f8c9b4662937bf4ad7a67b066a610a38db38d8343055"
  "0xf0ff2adeb165d77035bc76f656200929d4675bc83e72815fae3895fc1a6efa1a"
  "0x0d488eda069891f1c4025edd9ed1dfc5af77ad317713366fc13f17515a2503b2"
  "0x9b0f7453fe23ee5908edf08fa16b708077ad8ba6c85e9f812bec43871d7aabb3"
  "0xbdf67fb29e2fb7052a063fbf3d7ee4491171fb9c7dff8c832c87a38eb011ef2d"
  "0x7fbb8146800a060450e0ae653c34d147e5301b1b2bbd056980da5e4fa72b19e8"
)
if [ $((GAS_OWNERS_OFFSET + N)) -gt "${#GAS_OWNERS[@]}" ]; then
  echo "Error: GAS_OWNERS_OFFSET=$GAS_OWNERS_OFFSET + N=$N exceeds ${#GAS_OWNERS[@]} defined gas owners" >&2
  exit 1
fi

# Build stress.rs once before forking subprocesses. Each subprocess uses
# `cargo run --release` which will auto-rebuild if needed, but doing it serially
# here avoids 8 parallel cargo invocations racing on the build directory.
echo "=> Pre-building iota-benchmark (cargo build --release -p iota-benchmark)..."
cargo build --release -p iota-benchmark --bin stress || {
  echo "Error: build failed" >&2
  exit 1
}

# Master timestamp + parent dir under runs/ so each invocation is preserved.
MASTER_TS=$(date -u +"%Y-%m-%dT%H-%M-%SZ")
PARENT_DIR="runs/multi-${MASTER_TS}"
mkdir -p "$PARENT_DIR"

# Barrier files: each subprocess writes its READY_FILE after setup, then waits
# for START_FILE to appear. We touch START_FILE once all are ready, so all
# spam windows begin simultaneously.
BARRIER_DIR="$PARENT_DIR/barrier"
mkdir -p "$BARRIER_DIR"
START_FILE="$BARRIER_DIR/go"

echo "=> Launching $N stress.rs processes, each at QPS=$QPS_PER (total=$QPS_TOTAL)"
echo "=> Parent dir: $PARENT_DIR"
echo "=> Barrier: $BARRIER_DIR (start file: $START_FILE)"

pids=()
logs=()
runs_dirs=()
ready_files=()

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
  echo "   process $i → $fn  (log: $log, runs: $proc_runs/, cache: ${proc_cache:-disabled})"
  QPS="$QPS_PER" \
  DURATION="$DURATION" \
  WORKERS="$WORKERS" \
  IN_FLIGHT_RATIO="$IN_FLIGHT_RATIO" \
  BURST_SIZE="$BURST_SIZE" \
  BARRIER_PERIOD_MS="$BARRIER_PERIOD_MS" \
  GAS_CHUNK_SIZE="$GAS_CHUNK_SIZE" \
  GAS_POOL_CACHE_PATH="$proc_cache" \
  NUM_TRANSFER_ACCOUNTS="$NUM_TRANSFER_ACCOUNTS" \
  NUM_VALIDATORS_TO_TARGET="${NUM_VALIDATORS_TO_TARGET:-0}" \
  FULLNODE_RPC="$fn" \
  FULLNODE_RPC_ALL="$FULLNODES" \
  USE_FULLNODE_FOR_EXECUTION="${USE_FULLNODE_FOR_EXECUTION:-false}" \
  USE_FULLNODE_FOR_RECONFIG="${USE_FULLNODE_FOR_RECONFIG:-false}" \
  CLIENT_METRIC_PORT="$((8081 + i))" \
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

echo "=> Releasing start barrier."
# Write wall-clock epoch (ns) so workers in barrier mode can align ticks across
# processes (and across machines, if clocks are NTP-synced). Plain `touch` is
# preserved as a fallback for non-barrier runs since stress.rs only parses the
# content when --barrier-period-ms > 0.
date +%s%N > "$START_FILE"
echo "=> Spam phase running (DURATION=$DURATION, BARRIER_PERIOD_MS=$BARRIER_PERIOD_MS)..."

echo "=> Waiting for all $N processes to finish (pids: ${pids[*]})"
exit_codes=()
for pid in "${pids[@]}"; do
  wait "$pid"
  exit_codes+=($?)
done

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

# Save a top-level summary so this run is self-contained
{
  echo "ts:           $MASTER_TS"
  echo "config:       QPS_TOTAL=$QPS_TOTAL DURATION=$DURATION WORKERS=$WORKERS IN_FLIGHT_RATIO=$IN_FLIGHT_RATIO BURST_SIZE=$BURST_SIZE BARRIER_PERIOD_MS=$BARRIER_PERIOD_MS GAS_CHUNK_SIZE=$GAS_CHUNK_SIZE NUM_TRANSFER_ACCOUNTS=$NUM_TRANSFER_ACCOUNTS"
  echo "fullnodes:    $FULLNODES"
  echo "exit codes:   ${exit_codes[*]}"
  echo "tcp errors:   $tcp_total"
  if [ -n "${peak_inflight:-}" ] && [ -n "${sem_cap:-}" ]; then
    echo "sem_cap:      $sem_cap"
    echo "peak inflight:$peak_inflight"
    echo "ratio:        ${ratio}×"
  fi
} > "$PARENT_DIR/summary.txt"
echo
echo "=> Top-level summary: $PARENT_DIR/summary.txt"
