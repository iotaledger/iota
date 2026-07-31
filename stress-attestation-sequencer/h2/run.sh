#!/usr/bin/env bash
#
# run.sh — run the H2 mode comparison (TotalTxCount vs TotalComputationUnits)
# ITERS times. LABEL is required and names results/<LABEL>/; every iteration of a
# label accumulates under it, gated by config.json (see ../exp_dir.py). Each
# iteration:
#   1. cleanup    tear down anything already running
#   2. bootstrap  -b, regenerate genesis with benchmark gas accounts
#   3. Run A — MODE_A (default TotalTxCount), LIMIT_A / OVERSHOOT_A
#   4. Run B — MODE_B (default TotalComputationUnits), LIMIT_B / OVERSHOOT_B,
#              same load, after a network reset that keeps Run A's genesis
#   5. capture    per-run timeseries JSON + client report + node logs, then stop
#
# Adapted from ../h1/run.sh, which runs the same flow with attestation off/on.
#
# Attestation is ON in both runs, so the mode is the only difference; without it
# TotalComputationUnits falls back to gas_budget / gas_price (5,000,000 units).
#
# LIMIT_A (transactions per object per commit) and LIMIT_B (computation units per
# object per commit) are independent inputs; neither is computed from the other.
#
# Run as a NORMAL user; sudo is used internally for cleanup/bootstrap.
#
# Required env: LABEL and LIMIT_B.
# Tunables (env): ITERS (default 1), N, RUN_DURATION, TARGET_QPS, NUM_WORKERS,
#                 NUM_CLIENT_THREADS, NUM_TRANSFER_ACCOUNTS, IN_FLIGHT_RATIO,
#                 DIRECT, NUM_TARGET_VALIDATORS,
#                 WORKLOAD (owned|shared|slow),
#                 NUM_SHARED_COUNTERS, SLOW_N, SLOW_SIZE, SLOW_SHARED,
#                 MODE_A, LIMIT_A, OVERSHOOT_A, MODE_B, OVERSHOOT_B,
#                 MAX_DEFERRAL_ROUNDS, ATTEST, PRE_SPAM_DELAY_SECS,
#                 SLEEP_BETWEEN_RUNS_S, PRE_SPAM_WAIT_S, PRE_STOP_WAIT_S, PROM,
#                 TS_STEP, EPOCH_DURATION_MS, NODE_LOG.

set -euo pipefail

# Raise the open-file soft limit to the hard max for this script and every child
# (the host stress client, docker). The closed-loop client holds
# TARGET_QPS * IN_FLIGHT_RATIO concurrent fullnode connections (+ retries); the
# default soft limit (often 1024) is exhausted well below that, surfacing as
# "Too many open files" (EMFILE) transport errors that drop txs and pollute the
# throughput/latency numbers. (run-stress-docker.sh sets --ulimit separately.)
ulimit -n "$(ulimit -Hn)" || true

# ANSI palette (auto-disabled when stdout is not a terminal), matching the H1 script.
if [[ -t 1 ]]; then
  RED=$'\033[0;31m'
  GREEN=$'\033[0;32m'
  YELLOW=$'\033[0;33m'
  BLUE=$'\033[0;34m'
  MAGENTA=$'\033[0;35m'
  CYAN=$'\033[0;36m'
  RESET=$'\033[0m'
else
  RED=''
  GREEN=''
  YELLOW=''
  BLUE=''
  MAGENTA=''
  CYAN=''
  RESET=''
fi

if [[ ${EUID:-$(id -u)} -eq 0 && "${ALLOW_ROOT:-}" != "1" ]]; then
  echo "${RED}ERROR: run as a normal user, not root (cargo would build as root)." >&2
  echo "       sudo is invoked internally for cleanup/bootstrap." >&2
  echo "       On a root-by-default server, re-run with ALLOW_ROOT=1 (cargo will" >&2
  echo "       build as root and the internal sudo calls become pass-throughs).${RESET}" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)" # .../stress-attestation-sequencer/h2
TOOLS_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"                  # shared scripts: start/cleanup/bootstrap/restart
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"               # repo root (iota/)
GENESIS_DIR="$REPO_ROOT/dev-tools/iota-private-network/configs/genesis"

# Shorten a path for console output: print it relative to the repo root with a
# ./ prefix (falls back to the absolute path if it lies outside the repo). Used
# only for display — the real (absolute) paths are still used for I/O.
rel() { case "$1" in "$REPO_ROOT"/*) printf './%s' "${1#"$REPO_ROOT"/}" ;; *) printf '%s' "$1" ;; esac }

N="${N:-4}"
# Epoch length baked into the bootstrapped genesis. Long (1h) on purpose: the
# A->B reset reuses Run A's genesis, so Run B starts several minutes into
# epoch 0 — a long epoch keeps reconfiguration structurally out of both run
# windows.
EPOCH_DURATION_MS="${EPOCH_DURATION_MS:-3600000}"
RUN_DURATION="${RUN_DURATION:-60s}"
TARGET_QPS="${TARGET_QPS:-2000}"
NUM_WORKERS="${NUM_WORKERS:-24}"
NUM_CLIENT_THREADS="${NUM_CLIENT_THREADS:-12}"      # tokio threads driving the client (raise for higher qps)
NUM_TRANSFER_ACCOUNTS="${NUM_TRANSFER_ACCOUNTS:-4}" # pure multiplier on setup-phase gas-coin count
IN_FLIGHT_RATIO="${IN_FLIGHT_RATIO:-2}"             # max in-flight = IN_FLIGHT_RATIO * TARGET_QPS
# Via the fullnode by default: one mutable shared object caps throughput well below
# what the fullnode can push, and that path keeps the client's latency metrics in
# Prometheus. DIRECT=true runs the client in docker instead.
DIRECT="${DIRECT:-false}"                          # true => submit direct-to-validator (in-docker); false => via fullnode
NUM_TARGET_VALIDATORS="${NUM_TARGET_VALIDATORS:-}" # DIRECT only: pin submission/attestation to first N validators (empty => all)
WORKLOAD="${WORKLOAD:-slow}"                       # slow (slow::slow / slow::bimodal on a shared object) | shared (shared-counter) | owned (transfer; no congestion control)
NUM_SHARED_COUNTERS="${NUM_SHARED_COUNTERS:-}"     # WORKLOAD=shared: fewer => more congestion (empty => benchmark default ~qps/2)
SLOW_N="${SLOW_N:-}"                               # WORKLOAD=slow: slow::slow(n,size) — n vectors (empty + empty SLOW_SIZE => bimodal)
SLOW_SIZE="${SLOW_SIZE:-}"                         # WORKLOAD=slow: each vector size in bytes
SLOW_SHARED="${SLOW_SHARED:-true}"                 # WORKLOAD=slow: true attaches the mutable shared input congestion control needs; false => owned-only pure compute
# Per shared object per commit: LIMIT_* is the base budget, OVERSHOOT_* the burst a
# single commit may exceed it by, carried as debt and repaid from later commits.
#
# OVERSHOOT defaults to 0 in both runs, so nothing exceeds the base limit and no
# debt is carried — each run is described by one number.
#
# With the burst off the base limit must fit ONE transaction: the scheduler needs
# start_time + cost <= limit with start_time >= 0, so a smaller limit defers the
# transaction every commit until MAX_DEFERRAL_ROUNDS cancels it. probe-test.md
# lists what each slow point costs.
MODE_A="${MODE_A:-TotalTxCount}"
LIMIT_A="${LIMIT_A:-10}" # transactions per object per commit (production's value)
OVERSHOOT_A="${OVERSHOOT_A:-0}"
MODE_B="${MODE_B:-TotalComputationUnits}"
LIMIT_B="${LIMIT_B:?set LIMIT_B=<computation units per object per commit> (required)}"
OVERSHOOT_B="${OVERSHOOT_B:-0}"
MAX_DEFERRAL_ROUNDS="${MAX_DEFERRAL_ROUNDS:-}" # rounds a tx may stay deferred before it is CANCELLED (empty => protocol default 10); both arms
ATTEST="${ATTEST:-true}"                       # validator attestation; both arms (see the header)
# Setup-phase gas coins prepped before spam = TARGET_QPS * IN_FLIGHT_RATIO *
# (NUM_TRANSFER_ACCOUNTS + 1). That product drives warmup time, so keep
# NUM_TRANSFER_ACCOUNTS / IN_FLIGHT_RATIO small — they don't gate throughput at
# this scale (concurrency comes from max_ops = TARGET_QPS * IN_FLIGHT_RATIO).
PRE_SPAM_DELAY_SECS="${PRE_SPAM_DELAY_SECS:-2}"   # client-side quiet gap between gas-coin setup and spamming
SLEEP_BETWEEN_RUNS_S="${SLEEP_BETWEEN_RUNS_S:-5}" # idle gap (s) to separate A/B on the timeline
PRE_SPAM_WAIT_S="${PRE_SPAM_WAIT_S:-0}"           # let the network settle this long after it's up, before the spam
PRE_STOP_WAIT_S="${PRE_STOP_WAIT_S:-5}"           # keep the network up this long after scraping, before stopping it
PROM="${PROM:-http://localhost:9090}"
TS_STEP="${TS_STEP:-1}" # query_range step (s) for the per-run raw timeseries dump
ITERS="${ITERS:-1}"     # how many times to run the whole experiment; each adds one iter-NNN to results/<LABEL>/
PRIMARY_GAS_OWNER="0xf479d29837d22943aba6afc401f518a36521b990874eca784886185bd26bf681"
# iota-benchmark moved to the sibling repo `network-benchmark` (one level up
# from the iota repo root). Override the repo dir with BENCH_REPO, or point
# STRESS_BIN_PATH directly at a prebuilt stress binary. Only the DIRECT=false
# path uses it; DIRECT=true runs the prebuilt stress image instead.
BENCH_REPO="${BENCH_REPO:-$REPO_ROOT/../network-benchmark}"
STRESS_BIN="${STRESS_BIN_PATH:-$BENCH_REPO/target/release/stress}"

# --- Validate the limits ------------------------------------------------------
for _v in LIMIT_A OVERSHOOT_A LIMIT_B OVERSHOOT_B; do
  [[ "${!_v}" =~ ^[0-9]+$ ]] || {
    echo "${RED}ERROR: $_v must be a non-negative integer (got '${!_v}').${RESET}" >&2
    exit 1
  }
done

# --- Experiment label + config-gated results directory --------------------
# LABEL names the EXPERIMENT (one config). Every run.sh iteration for the same
# LABEL accumulates under results/<LABEL>/iter-NNN/. config.json — written once when
# the label is created — is the contract: a later run with the SAME label but
# DIFFERENT inputs is REJECTED, so a pool can never go mixed. Same-config re-runs
# just append the next iteration.
: "${LABEL:?set LABEL=<experiment-name> — names results/<LABEL>/ (required)}"
if [[ ! "$LABEL" =~ ^[A-Za-z0-9._-]+$ ]]; then
  echo "${RED}ERROR: LABEL='$LABEL' must match [A-Za-z0-9._-]+ (it is used as a dir name).${RESET}" >&2
  exit 1
fi
EXP_DIR="$SCRIPT_DIR/results/$LABEL"

# Allocate the next config-gated iteration dir. exp_dir.py writes/validates
# config.json and prints the next iter-NNN; on a config mismatch it prints a diff
# to stderr and exits non-zero (aborting the whole run). Sets the global
# RESULTS_DIR used by the rest of the iteration.
allocate_iter() {
  local iter
  iter="$(
    env CFG_workload="$WORKLOAD" CFG_direct="$DIRECT" CFG_target_qps="$TARGET_QPS" \
      CFG_num_target_validators="${NUM_TARGET_VALIDATORS:-all}" CFG_n="$N" \
      CFG_in_flight_ratio="$IN_FLIGHT_RATIO" CFG_num_workers="$NUM_WORKERS" \
      CFG_num_client_threads="$NUM_CLIENT_THREADS" CFG_num_transfer_accounts="$NUM_TRANSFER_ACCOUNTS" \
      CFG_run_duration="$RUN_DURATION" CFG_num_shared_counters="${NUM_SHARED_COUNTERS:-default}" \
      CFG_slow_n="${SLOW_N:-default}" CFG_slow_size="${SLOW_SIZE:-default}" CFG_slow_shared="$SLOW_SHARED" \
      CFG_mode_a="$MODE_A" CFG_limit_a="$LIMIT_A" CFG_overshoot_a="$OVERSHOOT_A" \
      CFG_mode_b="$MODE_B" CFG_limit_b="$LIMIT_B" CFG_overshoot_b="$OVERSHOOT_B" \
      CFG_attest="$ATTEST" \
      CFG_max_deferral_rounds="${MAX_DEFERRAL_ROUNDS:-default}" \
      python3 "$TOOLS_DIR/exp_dir.py" "$EXP_DIR"
  )" || exit 1
  RESULTS_DIR="$EXP_DIR/$iter"
  mkdir -p "$RESULTS_DIR"
}

# Map WORKLOAD to the stress `bench` workload flags. Only SHARED-object workloads
# exercise per-object congestion control, so H2 uses `slow` (W5: the
# per-transaction computation cost is set by n/size) and `shared` (W1: every
# transaction costs the same, the control). `owned` is kept only for a run with no
# congestion control at all.
case "$WORKLOAD" in
owned) WORKLOAD_ARGS=(--transfer-object 100 --shared-counter 0) ;;
shared)
  # Default counter count is ~qps*(1-hotness/100) ≈ qps/2 — too many to congest.
  # Set NUM_SHARED_COUNTERS small (e.g. 1) to actually trigger congestion control.
  WORKLOAD_ARGS=(--transfer-object 0 --shared-counter 100)
  [[ -n "$NUM_SHARED_COUNTERS" ]] && WORKLOAD_ARGS+=(--num-shared-counters "$NUM_SHARED_COUNTERS")
  ;;
slow)
  # slow::slow(n, size) per transaction; bigger n/size costs more computation units
  # (probe-test.md lists the points). With both knobs empty the workload runs
  # slow::bimodal instead, alternating heavy and light every 10s.
  # The workload publishes ONE slow::Obj and every payload takes it as a mutable
  # input, so all of them contend on the same object.
  WORKLOAD_ARGS=(--transfer-object 0 --slow 100)
  [[ -n "$SLOW_N" ]] && WORKLOAD_ARGS+=(--slow-n "$SLOW_N")
  [[ -n "$SLOW_SIZE" ]] && WORKLOAD_ARGS+=(--slow-size "$SLOW_SIZE")
  WORKLOAD_ARGS+=(--slow-shared "$SLOW_SHARED")
  ;;
*)
  echo "${RED}ERROR: unknown WORKLOAD='$WORKLOAD' (expected: owned | shared | slow)${RESET}" >&2
  exit 1
  ;;
esac

if [[ "$WORKLOAD" == owned || ("$WORKLOAD" == slow && "$SLOW_SHARED" != true) ]]; then
  echo "${YELLOW}NOTE: this workload takes no MUTABLE shared object, so per-object congestion" >&2
  echo "      control never accumulates cost and both arms behave identically — there is" >&2
  echo "      nothing for the mode comparison to measure.${RESET}" >&2
fi

# `shared`/`slow` publish a Move package at runtime (basics / slow), compiled from
# sources that depend on the iota-framework. On the host (fullnode path) those
# sources are the network-benchmark repo. In DIRECT mode they must be baked into
# the stress image (network-benchmark docker/stress/Dockerfile) — so rebuild that
# image after changing those, or the in-docker publish will fail.
if [[ "$WORKLOAD" != owned && "$DIRECT" == true ]]; then
  echo "${YELLOW}NOTE: WORKLOAD=$WORKLOAD publishes a Move package in-container; this needs the" >&2
  echo "      stress image rebuilt with the Move sources baked in (network-benchmark docker/stress).${RESET}" >&2
fi

RULE="$(printf '%80s' '' | tr ' ' '*')"
banner() {
  echo
  echo "${MAGENTA}${RULE}${RESET}"
  echo "${MAGENTA}$*${RESET}"
  echo "${MAGENTA}${RULE}${RESET}"
}

# Block until the fullnode JSON-RPC at :9000 accepts connections (start.sh
# verifies validators, not the fullnode, so it can lag behind). Up to 10
# minutes: on a large network (N=48) the fullnode processes a much bigger
# genesis while every validator warms up on the same machine.
wait_for_fullnode() {
  echo "${YELLOW}Waiting for fullnode RPC at 127.0.0.1:9000 ...${RESET}"
  for _ in $(seq 1 60); do
    if curl -s -o /dev/null --max-time 2 \
      -X POST -H 'Content-Type: application/json' \
      --data '{"jsonrpc":"2.0","id":1,"method":"iota_getChainIdentifier","params":[]}' \
      http://127.0.0.1:9000; then
      echo "  - Fullnode RPC is up."
      return 0
    fi
    sleep 2
  done
  echo "${RED} -- ERROR: fullnode RPC at 127.0.0.1:9000 not ready in time.${RESET}" >&2
  exit 1
}

# Wipe the Prometheus TSDB, but only when output is redirected (an unattended or
# matrix run); interactively it is a no-op so every run stays visible in Grafana.
# Called at each iteration start and between Run A and Run B.
wipe_monitoring() {
  [[ -t 1 ]] && return 0 # interactive: keep the TSDB so Grafana shows all runs
  (cd "$REPO_ROOT/dev-tools/grafana-local" && docker compose down -v --remove-orphans) \
    >/dev/null 2>&1 || true
}

# Reset between runs so Run B starts like Run A: cleanup.sh wipes the node DBs but
# keeps configs/genesis, so Run B cold-starts from the same genesis blob, and the
# long EPOCH_DURATION_MS keeps it inside epoch 0. The monitoring stack goes down
# with it and start.sh brings it back up for Run B.
reset_network() {
  echo "${YELLOW}Tearing everything down for Run B (reusing Run A's genesis, fresh DBs)...${RESET}"
  echo "  - cleanup   -> $(rel "$RESULTS_DIR/cleanup.log")"
  sudo "$TOOLS_DIR/cleanup.sh" >>"$RESULTS_DIR/cleanup.log" 2>&1 || true
  wipe_monitoring
}

# Dump the run window to JSON: raw histogram buckets, counters and gauges, with no
# rate() or quantile baked in, so any of that can be computed offline.
# $1 label  $2 start_epoch  $3 end_epoch  $4 out-timeseries.json
dump_timeseries() {
  local label="$1" start="$2" end="$3" out="$4"
  echo "${BLUE}Dumping raw timeseries (step=${TS_STEP}s)...${RESET}"
  echo "  - timeseries -> $(rel "$out")"

  PROM="$PROM" \
    CFG_target_qps="$TARGET_QPS" CFG_num_workers="$NUM_WORKERS" \
    CFG_in_flight_ratio="$IN_FLIGHT_RATIO" CFG_num_client_threads="$NUM_CLIENT_THREADS" \
    CFG_num_transfer_accounts="$NUM_TRANSFER_ACCOUNTS" CFG_run_duration="$RUN_DURATION" \
    CFG_direct="$DIRECT" CFG_num_target_validators="${NUM_TARGET_VALIDATORS:-all}" CFG_n="$N" \
    CFG_workload="$WORKLOAD" CFG_num_shared_counters="${NUM_SHARED_COUNTERS:-default}" \
    CFG_slow_n="${SLOW_N:-default}" CFG_slow_size="${SLOW_SIZE:-default}" CFG_slow_shared="$SLOW_SHARED" \
    CFG_mode_a="$MODE_A" CFG_limit_a="$LIMIT_A" CFG_overshoot_a="$OVERSHOOT_A" \
    CFG_mode_b="$MODE_B" CFG_limit_b="$LIMIT_B" CFG_overshoot_b="$OVERSHOOT_B" \
    CFG_attest="$ATTEST" \
    CFG_max_deferral_rounds="${MAX_DEFERRAL_ROUNDS:-default}" \
    python3 "$TOOLS_DIR/dump_timeseries.py" "$label" "$start" "$end" "$TS_STEP" "$out"
}

# Submission path: DIRECT=false goes via the fullnode, whose client-side latency
# metrics are scraped; DIRECT=true runs the client in docker straight to the
# validators, where they are not.
#
# The window opens where the client prints PROBE_SPAM_START_UNIX, after its
# gas-coin setup drains — those setup transactions would otherwise inflate
# throughput and pull the mean attested cost toward the 1,000-unit floor.
# $1 label  $2 out.json
run_stress() {
  local label="$1" ts_out="$2" start end
  banner ">>> stress: $label (path: $([[ "$DIRECT" == true ]] && echo direct-to-validator || echo via-fullnode))"
  wait_for_fullnode
  # Let the network settle before measuring, so the window has a clean idle
  # baseline and the validators are stable after (re)start.
  echo "${YELLOW}Letting the network settle ${PRE_SPAM_WAIT_S}s before the spam...${RESET}"
  sleep "$PRE_SPAM_WAIT_S"
  echo
  # The stress client logs thousands of (retried, benign) transport errors to
  # stderr; send stderr to a per-run log so the console stays clean. The final
  # "Benchmark Report" is on stdout, so it still shows — and is teed to a per-run
  # file, since on the direct path it is the only client-side view of throughput
  # and latency.
  local stress_log="${ts_out%-timeseries.json}-stress.log"
  local report_log="${ts_out%-timeseries.json}-stress-report.log"
  start=$(date +%s)
  if [[ "$DIRECT" == true ]]; then
    # Direct-to-validator, in-docker (validators only reachable on the docker
    # network). Same workload knobs, passed through to the runner.
    RUN_DURATION="$RUN_DURATION" TARGET_QPS="$TARGET_QPS" NUM_WORKERS="$NUM_WORKERS" \
      NUM_CLIENT_THREADS="$NUM_CLIENT_THREADS" NUM_TRANSFER_ACCOUNTS="$NUM_TRANSFER_ACCOUNTS" \
      IN_FLIGHT_RATIO="$IN_FLIGHT_RATIO" PRIMARY_GAS_OWNER="$PRIMARY_GAS_OWNER" \
      USE_FULLNODE_FOR_EXECUTION=false NUM_TARGET_VALIDATORS="$NUM_TARGET_VALIDATORS" \
      PRE_SPAM_DELAY_SECS="$PRE_SPAM_DELAY_SECS" \
      WORKLOAD="$WORKLOAD" \
      NUM_SHARED_COUNTERS="$NUM_SHARED_COUNTERS" \
      SLOW_N="$SLOW_N" SLOW_SIZE="$SLOW_SIZE" SLOW_SHARED="$SLOW_SHARED" \
      "$TOOLS_DIR/run-stress-docker.sh" 2>"$stress_log" | tee "$report_log"
  else
    echo "${BLUE}Running stress via ${STRESS_BIN} executable...${RESET}"
    (cd "$REPO_ROOT" && "$STRESS_BIN" \
      --local false \
      --fullnode-rpc-addresses http://127.0.0.1:9000 \
      --use-fullnode-for-execution true \
      --use-fullnode-for-reconfig true \
      --genesis-blob-path "$GENESIS_DIR/genesis.blob" \
      --keystore-path "$GENESIS_DIR/benchmark.keystore" \
      --primary-gas-owner-id "$PRIMARY_GAS_OWNER" \
      --num-client-threads "$NUM_CLIENT_THREADS" \
      --num-transfer-accounts "$NUM_TRANSFER_ACCOUNTS" \
      --run-duration "$RUN_DURATION" \
      --pre-spam-delay-secs "$PRE_SPAM_DELAY_SECS" \
      bench --target-qps "$TARGET_QPS" \
      --in-flight-ratio "$IN_FLIGHT_RATIO" \
      --num-workers "$NUM_WORKERS" \
      "${WORKLOAD_ARGS[@]}") 2>"$stress_log" | tee "$report_log"
  fi
  end=$(date +%s)
  echo "  - stderr -> $(rel "$stress_log")"
  echo "  - client benchmark report -> $(rel "$report_log")"
  echo

  # Anchor the window at the spam start the client printed, 1s earlier so the
  # baseline sits inside the pre-spam quiet gap (Prometheus samples on a 1s grid,
  # so a baseline exactly at the marker can already include the first workload
  # transactions). No marker (older stress image) => measure the whole
  # invocation, gas-coin setup included.
  local spam_start window_start
  spam_start="$(sed -n 's/.*PROBE_SPAM_START_UNIX=\([0-9.][0-9.]*\).*/\1/p' "$stress_log" | tail -1)"
  if [[ -n "$spam_start" ]]; then
    if ((PRE_SPAM_DELAY_SECS >= 2)); then
      window_start="$(awk "BEGIN{printf \"%.0f\", $spam_start - 1}")"
    else
      window_start="${spam_start%.*}"
    fi
    echo "  - spam started at $spam_start; window from $window_start (setup excluded)."
  else
    window_start="$start"
    echo "${YELLOW}  - no PROBE_SPAM_START_UNIX marker; measuring the whole invocation (gas-coin" >&2
    echo "    setup included, which biases throughput and cost). Rebuild the stress image.${RESET}" >&2
  fi

  dump_timeseries "$label" "$window_start" "$end" "$ts_out"
  echo
}

# Capture per-node restart/OOM state, full logs and a WARN/ERROR crash digest into
# $1, before teardown removes the containers. Called for both runs.
capture_node_state() {
  local out="$1" c i _nodes
  mkdir -p "$out"
  mapfile -t _nodes < <(docker ps --format '{{.Names}}' | grep -E '^(validator|fullnode)-[0-9]+$' | sort)
  : >"$out/_state.log"
  if ((${#_nodes[@]})); then
    # One inspect call covers every container; the log dumps then run
    # concurrently — they are independent and I/O-bound, and run sequentially
    # the ~50 of them dominate capture time on a large network. Batches of 16
    # keep the docker daemon from serving all dumps at once.
    docker inspect "${_nodes[@]}" --format \
      '{{.Name}} status={{.State.Status}} restarts={{.RestartCount}} oom={{.State.OOMKilled}} exit={{.State.ExitCode}}' \
      >>"$out/_state.log" 2>&1 || true
    i=0
    for c in "${_nodes[@]}"; do
      docker logs "$c" >"$out/$c.log" 2>&1 &
      i=$((i + 1))
      if ((i % 16 == 0)); then wait; fi
    done
    wait
  fi
  grep -rniE "panic|fatal|stack backtrace|out of memory|abort" "$out"/*.log 2>/dev/null |
    grep -vE ':[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9:.]+Z +(TRACE|DEBUG|INFO) ' \
      >"$out/_crash.log" || true
}

# One experiment iteration: cleanup -> bootstrap -> Run A (mode A) -> reset ->
# Run B (mode B) -> capture node logs -> stop network, all into a fresh
# config-gated RESULTS_DIR. Called ITERS times by main below.
run_one_iteration() {
  allocate_iter
  echo "experiment outputs -> $(rel "$RESULTS_DIR")"

  banner "== H2 [1/5] cleanup (in case something is running) =="
  # cleanup/bootstrap are verbose (docker compose + genesis tooling); send their
  # output to cleanup.log / bootstrap.log to keep the console readable. After the
  # teardown, when output is redirected (unattended run), wipe_monitoring wipes the
  # Prometheus volume so this ITERATION starts on a fresh TSDB — no series carry
  # over from a previous iteration/invocation, bounding the volume's growth. (It's
  # a no-op interactively, so nothing is cleared and all runs stay in Grafana.)
  sudo "$TOOLS_DIR/cleanup.sh" >>"$RESULTS_DIR/cleanup.log" 2>&1 || true
  wipe_monitoring
  echo "cleanup output -> $(rel "$RESULTS_DIR/cleanup.log")"

  banner "== H2 [2/5] bootstrap (-b, $N validators) =="
  sudo "$TOOLS_DIR/bootstrap.sh" -b -n "$N" -e "$EPOCH_DURATION_MS" >>"$RESULTS_DIR/bootstrap.log" 2>&1
  echo "bootstrap output -> $(rel "$RESULTS_DIR/bootstrap.log")"

  banner "== H2 [3/5] Run A — $MODE_A (limit $LIMIT_A, overshoot $OVERSHOOT_A) =="
  MODE="$MODE_A" ATTEST="$ATTEST" \
    MAX_ACCUMULATED_TXN_COST="$LIMIT_A" MAX_CONGESTION_OVERSHOOT="$OVERSHOOT_A" \
    MAX_DEFERRAL_ROUNDS="$MAX_DEFERRAL_ROUNDS" \
    "$TOOLS_DIR/start.sh" -n "$N" faucet
  run_stress "Run A — $MODE_A (limit $LIMIT_A, overshoot $OVERSHOOT_A)" \
    "$RESULTS_DIR/run-a-timeseries.json"

  # Run A is scraped; let the network run a moment so the post-run tail is
  # captured, then fully reset (same genesis, fresh DBs) for Run B, idling so
  # the runs are cleanly separated. Run A's metrics are already saved.
  echo "${YELLOW}Letting the network run ${PRE_STOP_WAIT_S}s after the scrape before resetting...${RESET}"
  sleep "$PRE_STOP_WAIT_S"
  # Capture Run A's node state BEFORE the reset destroys these containers — so
  # mode A's fork/crash status is on record (parallels Run B).
  echo "${BLUE}Capturing Run A node logs + state -> $(rel "$RESULTS_DIR/run-a-node-logs")/${RESET}"
  capture_node_state "$RESULTS_DIR/run-a-node-logs"
  reset_network
  echo

  echo "${YELLOW}Idle gap (${SLEEP_BETWEEN_RUNS_S}s) to separate Run A/B on the timeline...${RESET}"
  sleep "$SLEEP_BETWEEN_RUNS_S"

  banner "== H2 [4/5] Run B — $MODE_B (limit $LIMIT_B, overshoot $OVERSHOOT_B) =="
  # start.sh boots the validators from Run A's genesis with an empty data dir,
  # so Run B cold-starts exactly like Run A — only the congestion-control mode
  # and its limits differ. Run A's metrics live in run-a-timeseries.json
  # (already saved).
  MODE="$MODE_B" ATTEST="$ATTEST" \
    MAX_ACCUMULATED_TXN_COST="$LIMIT_B" MAX_CONGESTION_OVERSHOOT="$OVERSHOOT_B" \
    MAX_DEFERRAL_ROUNDS="$MAX_DEFERRAL_ROUNDS" \
    "$TOOLS_DIR/start.sh" -n "$N" faucet
  run_stress "Run B — $MODE_B (limit $LIMIT_B, overshoot $OVERSHOOT_B)" \
    "$RESULTS_DIR/run-b-timeseries.json"

  # Symmetric with the end of Run A: let the network run the same moment after the
  # scrape so Run B's post-run tail lands on the dashboard too, before teardown.
  echo "${YELLOW}Letting the network run ${PRE_STOP_WAIT_S}s after the scrape before tearing down...${RESET}"
  sleep "$PRE_STOP_WAIT_S"

  banner "== H2 [5/5] iteration complete — capturing logs + stopping network =="
  echo "${BLUE}This iteration's raw data: $(rel "$RESULTS_DIR")${RESET}"
  echo "  - run-a-timeseries.json, run-b-timeseries.json"

  # Capture Run B's per-node logs + restart/OOM state + crash digest BEFORE the
  # final cleanup (`docker compose down`) removes the containers. Run A's equivalent
  # is already in run-a-node-logs/ (captured before the reset).
  node_logs="$RESULTS_DIR/run-b-node-logs"
  echo "${BLUE}Capturing node logs + state -> $(rel "$node_logs")/${RESET}"
  capture_node_state "$node_logs"
  echo "  - state logs -> $(rel "$node_logs/_state.log")"
  echo "  - crash logs -> $(rel "$node_logs/_crash.log")"

  # Always stop + clean the network (down + wipe data) via the privnet's OWN
  # cleanup, which leaves the monitoring stack up so both runs stay visible in
  # Grafana. (cd in first: it runs `docker compose down` against the cwd.)
  echo "${YELLOW}Stopping and cleaning the network...${RESET}"
  (cd "$REPO_ROOT/dev-tools/iota-private-network" && sudo ./cleanup.sh) >>"$RESULTS_DIR/cleanup.log" 2>&1
  echo "  - Network stopped and cleaned. Monitoring stays up (runs visible in Grafana)."
}

# ===========================================================================
# main — check the client (build it for the fullnode path), then run ITERS
# config-gated iterations of the experiment, and (once) offer to tear monitoring
# down.
# ===========================================================================
sudo -v # cache sudo creds up front so prompts don't interrupt mid-run

banner "== H2 [0/5] client + config =="
echo "  workload : $WORKLOAD$([[ "$WORKLOAD" == slow ]] && echo " (slow(${SLOW_N:-bimodal}, ${SLOW_SIZE:-bimodal}), shared=$SLOW_SHARED)")"
echo "  Run A    : $MODE_A, limit $LIMIT_A transactions, overshoot $OVERSHOOT_A"
echo "  Run B    : $MODE_B, limit $LIMIT_B units, overshoot $OVERSHOOT_B"
echo "  load     : $TARGET_QPS qps for $RUN_DURATION, attestation=$ATTEST, $N validators, direct=$DIRECT"
if [[ "$DIRECT" == true ]]; then
  # The in-docker client comes from the prebuilt stress image, not from cargo.
  echo "${YELLOW}NOTE: DIRECT=true runs the stress image in-network; it must be built from the" >&2
  echo "      CURRENT network-benchmark branch (docker/stress/build.sh) — a stale image" >&2
  echo "      means a stale client (wrong protocol version pin, missing flags).${RESET}" >&2
else
  # Guard: the stress binary must come from the intended feature branch. Unlike
  # ../h1/run.sh this only CHECKS the branch — the network-benchmark checkout is
  # managed by hand, so this never switches it. Override with EXPECT_BENCH_BRANCH.
  EXPECT_BENCH_BRANCH="${EXPECT_BENCH_BRANCH:-protocol-research/feat/transaction-attestation-feature-test}"
  if [[ -z "${STRESS_BIN_PATH:-}" && -d "$BENCH_REPO/.git" ]]; then
    cur_branch="$(git -C "$BENCH_REPO" rev-parse --abbrev-ref HEAD 2>/dev/null || echo '?')"
    if [[ "$cur_branch" != "$EXPECT_BENCH_BRANCH" ]]; then
      echo "${RED}ERROR: network-benchmark is on '$cur_branch', expected '$EXPECT_BENCH_BRANCH'." >&2
      echo "       Check it out yourself, or set EXPECT_BENCH_BRANCH / STRESS_BIN_PATH.${RESET}" >&2
      exit 1
    fi
    echo "network-benchmark on $cur_branch (HEAD $(git -C "$BENCH_REPO" rev-parse --short HEAD))"
  fi
  if [[ -n "${STRESS_BIN_PATH:-}" ]]; then
    echo "Using prebuilt stress binary: $STRESS_BIN"
  else
    (cd "$BENCH_REPO" && cargo build --release --bin stress)
  fi
  [[ -x "$STRESS_BIN" ]] || {
    echo "stress binary not found/executable at $STRESS_BIN (build network-benchmark, or set STRESS_BIN_PATH)" >&2
    exit 1
  }
fi

for ((_iter = 1; _iter <= ITERS; _iter++)); do
  banner "########## experiment '$LABEL': iteration $_iter of $ITERS ##########"
  run_one_iteration
done

banner "== raw data collected — no aggregation step yet =="
n_iters=$(ls -1d "$EXP_DIR"/iter-*/ 2>/dev/null | wc -l | tr -d ' ')
echo "experiment '$LABEL': $n_iters iteration(s) under $(rel "$EXP_DIR")"
echo "Each iteration holds run-a/run-b timeseries JSONs + client benchmark reports."
echo "Cross-run aggregation and plots are not written yet — see README.md."

echo
echo "${CYAN}Grafana: http://localhost:3000/d/attestation-sequencer-stress${RESET}"

# Monitoring teardown, once after all iterations. `down -v` clears every run's
# series, so interactively it asks and defaults to keeping it.
if [[ -t 0 && -t 1 ]]; then
  read -r -p "${YELLOW}Also stop and CLEAR monitoring (Prometheus data)? [y/N] ${RESET}" ans
else
  ans=y
fi
if [[ "$ans" == "y" || "$ans" == "Y" ]]; then
  (cd "$REPO_ROOT/dev-tools/grafana-local" && docker compose down -v --remove-orphans) >/dev/null 2>&1 || true
  echo "${YELLOW}  - Monitoring stopped and Prometheus data cleared.${RESET}"
else
  echo "${YELLOW}  - Monitoring left running. Stop + clear it later with:${RESET}"
  echo "${YELLOW}    (cd $REPO_ROOT/dev-tools/grafana-local && docker compose down -v)${RESET}"
fi
