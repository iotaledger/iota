#!/usr/bin/env bash
#
# run.sh — run the H1 experiment (attestation overhead, W4: V1 vs V2) ITERS times
# and pool the iterations. REQUIRES a LABEL env var naming the experiment; all its
# iterations accumulate under results/<LABEL>/iter-NNN/, gated by a config.json so
# a label can never mix configs (see exp_dir.py). Each iteration:
#   0. build      pre-build the stress binary (once, before the loop)
#   1. cleanup    tear down anything already running (best-effort)
#   2. bootstrap  -b, regenerate genesis with benchmark gas accounts
#   3. Run A — V1 attestation OFF (control), TotalTxCount
#   4. Run B — V2 attestation ON, same load (network reset between A and B —
#                 cleanup wipes the node DBs but keeps Run A's genesis, so Run B
#                 cold-starts like Run A; unattended runs wipe the Prometheus TSDB (at each
#                 iteration start AND between A and B) for cleanly separated runs;
#                 interactively nothing is wiped, so all runs coexist in Grafana;
#                 Run A's JSON is saved before the reset)
#   5. capture    save each run's window as a raw timeseries JSON + node logs,
#                 then stop the network.
# After all ITERS iterations: aggregate.py pools results/<LABEL>/ into summary.md
# and plot.py renders per-panel figures across ALL iterations into
# results/<LABEL>/plots/ — both skipped when ANALYZE=false (matrix.sh sets it:
# they re-read every accumulated iteration of the label, so per-round invocation
# grows quadratically over a campaign; the matrix sweeps every label once at the
# end instead). Finally (once) prompts whether to also stop monitoring (Grafana).
#
# Run as a NORMAL user (cargo must not run as root); `sudo` is used internally
# only for cleanup/bootstrap.
#
# Required env: LABEL (experiment name -> results/<LABEL>/).
# Tunables (env): ITERS (default 1), N, RUN_DURATION, TARGET_QPS, NUM_WORKERS,
#                 NUM_CLIENT_THREADS, NUM_TRANSFER_ACCOUNTS, IN_FLIGHT_RATIO,
#                 DIRECT, NUM_TARGET_VALIDATORS, WORKLOAD (owned|shared|slow),
#                 NUM_SHARED_COUNTERS, SLOW_N, SLOW_SIZE, MAX_DEFERRAL_ROUNDS,
#                 MAX_ACCUMULATED_TXN_COST, MAX_CONGESTION_OVERSHOOT,
#                 SLEEP_BETWEEN_RUNS_S, PRE_SPAM_WAIT_S, PRE_STOP_WAIT_S, PROM,
#                 TS_STEP, EPOCH_DURATION_MS, ANALYZE.

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

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)" # .../stress-attestation-sequencer/h1
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
DIRECT="${DIRECT:-false}"                           # true => submit direct-to-validator (in-docker); false => via fullnode
NUM_TARGET_VALIDATORS="${NUM_TARGET_VALIDATORS:-}"  # DIRECT only: pin submission/attestation to first N validators (empty => all)
WORKLOAD="${WORKLOAD:-owned}"                       # owned (transfer) | shared (shared-counter) | slow (slow::bimodal)
NUM_SHARED_COUNTERS="${NUM_SHARED_COUNTERS:-}"      # WORKLOAD=shared: fewer => more congestion (empty => benchmark default ~qps/2)
SLOW_N="${SLOW_N:-}"                                # WORKLOAD=slow: slow::slow(n,size) — n vectors (empty => default 100)
SLOW_SIZE="${SLOW_SIZE:-}"                          # WORKLOAD=slow: each vector size in bytes (empty => default 100)
SLOW_SHARED="${SLOW_SHARED:-}"                      # WORKLOAD=slow: true (default) attaches a shared object (congestion); false => owned-only pure compute
# Congestion-control protocol overrides (empty => protocol default). Applied by
# start.sh to the network for BOTH runs; recorded in each run's config below.
MAX_DEFERRAL_ROUNDS="${MAX_DEFERRAL_ROUNDS:-}"           # rounds a tx may stay deferred before it is CANCELLED (default 10)
MAX_ACCUMULATED_TXN_COST="${MAX_ACCUMULATED_TXN_COST:-}" # base per-object per-commit budget (TotalTxCount => tx count; default 10)
MAX_CONGESTION_OVERSHOOT="${MAX_CONGESTION_OVERSHOOT:-}" # burst allowed over the base budget per commit (default 100)
# Setup-phase gas coins prepped before spam = TARGET_QPS * IN_FLIGHT_RATIO *
# (NUM_TRANSFER_ACCOUNTS + 1). That product drives warmup time, so keep
# NUM_TRANSFER_ACCOUNTS / IN_FLIGHT_RATIO small — they don't gate throughput at
# this scale (concurrency comes from max_ops = TARGET_QPS * IN_FLIGHT_RATIO).
SLEEP_BETWEEN_RUNS_S="${SLEEP_BETWEEN_RUNS_S:-5}" # idle gap (s) to separate A/B on the timeline
PRE_SPAM_WAIT_S="${PRE_SPAM_WAIT_S:-0}"           # let the network settle this long after it's up, before the spam
PRE_STOP_WAIT_S="${PRE_STOP_WAIT_S:-2}"           # keep Run A's network up this long after scraping, before stopping
PROM="${PROM:-http://localhost:9090}"
TS_STEP="${TS_STEP:-1}" # query_range step (s) for the per-run raw timeseries dump
ITERS="${ITERS:-1}"     # how many times to run the whole experiment; each adds one iter-NNN to results/<LABEL>/
ANALYZE="${ANALYZE:-true}" # false => skip the post-run aggregate.py/plot.py (matrix.sh sweeps once at campaign end)
PRIMARY_GAS_OWNER="0xf479d29837d22943aba6afc401f518a36521b990874eca784886185bd26bf681"
# iota-benchmark moved to the sibling repo `network-benchmark` (one level up
# from the iota repo root). Override the repo dir with BENCH_REPO, or point
# STRESS_BIN_PATH directly at a prebuilt stress binary.
BENCH_REPO="${BENCH_REPO:-$REPO_ROOT/../network-benchmark}"
STRESS_BIN="${STRESS_BIN_PATH:-$BENCH_REPO/target/release/stress}"

# --- Experiment label + config-gated results directory --------------------
# LABEL names the EXPERIMENT (one config). Every run.sh iteration for the same
# LABEL accumulates under results/<LABEL>/iter-NNN/, and aggregate.py / plot.py
# pool them into mean/median + variance bands. config.json — written once when
# the label is created — is the contract: a later run with the SAME label but
# DIFFERENT inputs is REJECTED, so a pool can never go mixed (this replaces the
# old archive/ hack). Same-config re-runs just append the next iteration.
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
    CFG_workload="$WORKLOAD" CFG_direct="$DIRECT" CFG_target_qps="$TARGET_QPS" \
      CFG_num_target_validators="${NUM_TARGET_VALIDATORS:-all}" CFG_n="$N" \
      CFG_in_flight_ratio="$IN_FLIGHT_RATIO" CFG_num_workers="$NUM_WORKERS" \
      CFG_num_client_threads="$NUM_CLIENT_THREADS" CFG_num_transfer_accounts="$NUM_TRANSFER_ACCOUNTS" \
      CFG_run_duration="$RUN_DURATION" CFG_num_shared_counters="${NUM_SHARED_COUNTERS:-default}" \
      CFG_slow_n="${SLOW_N:-default}" CFG_slow_size="${SLOW_SIZE:-default}" CFG_slow_shared="${SLOW_SHARED:-default}" \
      CFG_max_deferral_rounds="${MAX_DEFERRAL_ROUNDS:-default}" \
      CFG_max_accumulated_txn_cost="${MAX_ACCUMULATED_TXN_COST:-default}" \
      CFG_max_congestion_overshoot="${MAX_CONGESTION_OVERSHOOT:-default}" \
      python3 "$SCRIPT_DIR/exp_dir.py" "$EXP_DIR"
  )" || exit 1
  RESULTS_DIR="$EXP_DIR/$iter"
  mkdir -p "$RESULTS_DIR"
}

# Map WORKLOAD to the stress workload-weight flags. `shared`/`slow` are
# shared-object workloads whose attestation dry-run is more expensive than a
# plain transfer — the point of varying this.
case "$WORKLOAD" in
owned) WORKLOAD_ARGS=(--transfer-object 100 --shared-counter 0) ;;
shared)
  # Default counter count is ~qps*(1-hotness/100) ≈ qps/2 — too many to congest.
  # Set NUM_SHARED_COUNTERS small (e.g. 1) to actually trigger congestion control.
  WORKLOAD_ARGS=(--transfer-object 0 --shared-counter 100)
  [[ -n "$NUM_SHARED_COUNTERS" ]] && WORKLOAD_ARGS+=(--num-shared-counters "$NUM_SHARED_COUNTERS")
  ;;
slow)
  # slow::slow(n, size) per tx — bigger n/size => more compute => costlier
  # attestation dry-run. Empty => the workload's defaults (n=100, size=100).
  WORKLOAD_ARGS=(--transfer-object 0 --slow 100)
  [[ -n "$SLOW_N" ]] && WORKLOAD_ARGS+=(--slow-n "$SLOW_N")
  [[ -n "$SLOW_SIZE" ]] && WORKLOAD_ARGS+=(--slow-size "$SLOW_SIZE")
  [[ -n "$SLOW_SHARED" ]] && WORKLOAD_ARGS+=(--slow-shared "$SLOW_SHARED")
  ;;
*)
  echo "${RED}ERROR: unknown WORKLOAD='$WORKLOAD' (expected: owned | shared | slow)${RESET}" >&2
  exit 1
  ;;
esac

# `shared`/`slow` publish a Move package at runtime (basics / slow), compiled
# from sources that depend on the iota-framework. On the host (fullnode path)
# those sources are the network-benchmark repo. In DIRECT mode they must be baked
# into the stress image (network-benchmark docker/stress/Dockerfile) — so rebuild
# that image after changing those, or the in-docker publish will fail.
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

# Wipe the Prometheus TSDB (down -v) — but ONLY when output is redirected to a
# file (an unattended / matrix run). Interactively (stdout is a terminal) it's a
# no-op, so every run stays visible in Grafana for live debugging. Called at the
# start of each iteration and between Run A and Run B; for unattended runs that
# bounds the volume's growth and keeps each run's window on a fresh TSDB (no
# cross-run carryforward). Where it does NOT wipe (interactive), Run B reuses Run
# A's series labels and its fresh process resets the counters — but that A->B
# carryforward is stripped from the saved JSONs by dump_timeseries' reset-aware
# trim, so the per-run data stays correct regardless.
wipe_monitoring() {
  [[ -t 1 ]] && return 0 # interactive: keep the TSDB so Grafana shows all runs
  (cd "$REPO_ROOT/dev-tools/grafana-local" && docker compose down -v --remove-orphans) \
    >/dev/null 2>&1 || true
}

# Reset the network between runs so Run B's startup path matches the initial
# setup before Run A — that symmetry is what keeps the pre-spam warmup the
# same. We tear EVERYTHING down (incl. the monitoring stack: leaving Prometheus
# up across the reset appears to give Run B a longer warmup, so cleanup.sh
# brings the monitoring container down WITHOUT -v and start.sh brings it back
# up for Run B). cleanup.sh wipes the node DBs (data/) but keeps
# configs/genesis, so Run B cold-starts from the SAME genesis blob as Run A —
# byte-identical startup work (better symmetry than a re-bootstrap, whose
# genesis differs by timestamp) without a second multi-minute genesis ceremony
# per iteration. The long EPOCH_DURATION_MS epoch keeps Run B's later start
# within epoch 0 far from any reconfiguration.
#
# wipe_monitoring here clears the TSDB between Run A and Run B — but ONLY when
# output is redirected (unattended run): Run B then starts on a fresh TSDB with no
# A->B carryforward, so the live dashboard separates the runs cleanly too. INTER-
# ACTIVELY it's a no-op, so Run A and Run B coexist in one Grafana view for side-
# by-side debugging (the A->B carryforward that leaves is stripped from the saved
# JSONs by dump_timeseries, so they stay correct either way).
reset_network() {
  echo "${YELLOW}Tearing everything down for Run B (reusing Run A's genesis, fresh DBs)...${RESET}"
  echo "  - cleanup   -> $(rel "$RESULTS_DIR/cleanup.log")"
  sudo "$TOOLS_DIR/cleanup.sh" >>"$RESULTS_DIR/cleanup.log" 2>&1 || true
  wipe_monitoring
}

# Dump the raw timeseries (Prometheus query_range) over the run window to a JSON
# file. We store the underlying series verbatim — cumulative histogram buckets
# (+ _count/_sum) and raw counters/gauges — with NO rate()/histogram_quantile()/
# aggregation baked in. Everything (any rate window, any quantile, per-validator
# breakdowns, and correct cross-run aggregation by pooling raw histograms) can be
# reconstructed from this offline. Each entry is the raw query_range result: one
# series per full label set (le, host, name, ping, ...), values = [[ts,"v"],...].
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
    CFG_slow_n="${SLOW_N:-default}" CFG_slow_size="${SLOW_SIZE:-default}" CFG_slow_shared="${SLOW_SHARED:-default}" \
    CFG_max_deferral_rounds="${MAX_DEFERRAL_ROUNDS:-default}" \
    CFG_max_accumulated_txn_cost="${MAX_ACCUMULATED_TXN_COST:-default}" \
    CFG_max_congestion_overshoot="${MAX_CONGESTION_OVERSHOOT:-default}" \
    python3 "$SCRIPT_DIR/dump_timeseries.py" "$label" "$start" "$end" "$TS_STEP" "$out"
}

# The aggregated V1-vs-V2 summary is built across ALL iterations of this label by
# aggregate.py (it pools raw histogram buckets — correct cross-run percentile
# aggregation). See the post-loop aggregate step in main below.

# Identical owned-object (transfer) load for both runs. TotalTxCount + owned
# objects => no shared-object sequencing, so the V1<->V2 delta is pure
# attestation overhead.
#
# Submission path (DIRECT env):
#   DIRECT=false (default) — host binary submits via the fullnode (:9000); the
#     fullnode's TD/QD drives. Client-side TD metrics are emitted by the
#     fullnode (a Prometheus target), so settlement/submit latency is scraped.
#   DIRECT=true — the in-docker runner submits DIRECTLY to validators (the TD
#     runs inside the stress container, which is NOT a Prometheus target). So
#     validator-side metrics (attestation, execution, CPU) still scrape, but the
#     client-side settlement/submit latency will be null for these runs.
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
  # "Benchmark Report" is on stdout, so it still shows.
  local stress_log="${ts_out%-timeseries.json}-stress.log"
  start=$(date +%s)
  if [[ "$DIRECT" == true ]]; then
    # Direct-to-validator, in-docker (validators only reachable on the docker
    # network). Same workload knobs, passed through to the runner.
    RUN_DURATION="$RUN_DURATION" TARGET_QPS="$TARGET_QPS" NUM_WORKERS="$NUM_WORKERS" \
      NUM_CLIENT_THREADS="$NUM_CLIENT_THREADS" NUM_TRANSFER_ACCOUNTS="$NUM_TRANSFER_ACCOUNTS" \
      IN_FLIGHT_RATIO="$IN_FLIGHT_RATIO" PRIMARY_GAS_OWNER="$PRIMARY_GAS_OWNER" \
      USE_FULLNODE_FOR_EXECUTION=false NUM_TARGET_VALIDATORS="$NUM_TARGET_VALIDATORS" \
      WORKLOAD="$WORKLOAD" \
      "$TOOLS_DIR/run-stress-docker.sh" 2>"$stress_log"
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
      bench --target-qps "$TARGET_QPS" \
      --in-flight-ratio "$IN_FLIGHT_RATIO" \
      --num-workers "$NUM_WORKERS" \
      "${WORKLOAD_ARGS[@]}") 2>"$stress_log"
  fi
  end=$(date +%s)
  echo "  - stderr -> $(rel "$stress_log")"
  echo

  dump_timeseries "$label" "$start" "$end" "$ts_out"
  echo
}

# Capture per-node restart/OOM state + full logs + a WARN/ERROR crash digest into
# $1, BEFORE any teardown removes the containers. Called for BOTH Run A (before the
# reset) and Run B (before final cleanup) so the V1-vs-V2 fork/crash comparison is
# symmetric. RestartCount/OOMKilled from `docker inspect` reveal whether a node
# crashed during the run; the digest keeps only real WARN/ERROR panics/forks (the
# level filter drops benign DEBUG "processing aborted (retriable)" TD spam).
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

# One experiment iteration: cleanup -> bootstrap -> Run A (V1) -> reset ->
# Run B (V2) -> capture node logs -> stop network, all into a fresh config-gated
# RESULTS_DIR. Called ITERS times by main below.
run_one_iteration() {
  allocate_iter
  echo "experiment outputs -> $(rel "$RESULTS_DIR")"

  banner "== H1 [1/5] cleanup (in case something is running) =="
  # cleanup/bootstrap are verbose (docker compose + genesis tooling); send their
  # output to cleanup.log / bootstrap.log to keep the console readable. After the
  # teardown, when output is redirected (unattended run), wipe_monitoring wipes the
  # Prometheus volume so this ITERATION starts on a fresh TSDB — no series carry
  # over from a previous iteration/invocation, bounding the volume's growth. (It's
  # a no-op interactively, so nothing is cleared and all runs stay in Grafana.)
  sudo "$TOOLS_DIR/cleanup.sh" >>"$RESULTS_DIR/cleanup.log" 2>&1 || true
  wipe_monitoring
  echo "cleanup output -> $(rel "$RESULTS_DIR/cleanup.log")"

  banner "== H1 [2/5] bootstrap (-b, $N validators) =="
  sudo "$TOOLS_DIR/bootstrap.sh" -b -n "$N" -e "$EPOCH_DURATION_MS" >>"$RESULTS_DIR/bootstrap.log" 2>&1
  echo "bootstrap output -> $(rel "$RESULTS_DIR/bootstrap.log")"

  banner "== H1 [3/5] Run A — V1 (attestation OFF) =="
  MODE=TotalTxCount ATTEST=false \
    MAX_DEFERRAL_ROUNDS="$MAX_DEFERRAL_ROUNDS" MAX_ACCUMULATED_TXN_COST="$MAX_ACCUMULATED_TXN_COST" \
    MAX_CONGESTION_OVERSHOOT="$MAX_CONGESTION_OVERSHOOT" \
    "$TOOLS_DIR/start.sh" -n "$N" faucet
  run_stress "Run A — V1 (attestation off)" "$RESULTS_DIR/run-a-v1-timeseries.json"

  # Run A is scraped; let the network run a moment so the post-run tail is
  # captured, then fully reset (same genesis, fresh DBs) for Run B, idling so
  # the runs are cleanly separated. Run A's metrics are already saved.
  echo "${YELLOW}Letting the network run ${PRE_STOP_WAIT_S}s after the scrape before resetting...${RESET}"
  sleep "$PRE_STOP_WAIT_S"
  # Capture Run A's (V1) node state BEFORE the reset destroys these containers — so
  # the attestation-OFF flow's fork/crash status is on record (parallels Run B).
  echo "${BLUE}Capturing Run A (V1) node logs + state -> $(rel "$RESULTS_DIR/run-a-node-logs")/${RESET}"
  capture_node_state "$RESULTS_DIR/run-a-node-logs"
  reset_network
  echo

  echo "${YELLOW}Idle gap (${SLEEP_BETWEEN_RUNS_S}s) to separate Run A/B on the timeline...${RESET}"
  sleep "$SLEEP_BETWEEN_RUNS_S"

  banner "== H1 [4/5] Run B — V2 (attestation ON) =="
  # start.sh boots the validators from Run A's genesis with an empty data dir,
  # so Run B cold-starts exactly like Run A — only attestation differs. Run A's
  # metrics live in run-a-v1-timeseries.json (already saved).
  MODE=TotalTxCount \
    MAX_DEFERRAL_ROUNDS="$MAX_DEFERRAL_ROUNDS" MAX_ACCUMULATED_TXN_COST="$MAX_ACCUMULATED_TXN_COST" \
    MAX_CONGESTION_OVERSHOOT="$MAX_CONGESTION_OVERSHOOT" \
    "$TOOLS_DIR/start.sh" -n "$N" faucet
  run_stress "Run B — V2 (attestation on)" "$RESULTS_DIR/run-b-v2-timeseries.json"

  # Symmetric with the end of Run A: let the network run the same moment after the
  # scrape so Run B's post-run tail lands on the dashboard too, before teardown.
  echo "${YELLOW}Letting the network run ${PRE_STOP_WAIT_S}s after the scrape before tearing down...${RESET}"
  sleep "$PRE_STOP_WAIT_S"

  banner "== H1 [5/5] iteration complete — capturing logs + stopping network =="
  echo "${BLUE}This iteration's raw data: $(rel "$RESULTS_DIR")${RESET}"
  echo "  - run-a-v1-timeseries.json, run-b-v2-timeseries.json"

  # Capture Run B's (V2) per-node logs + restart/OOM state + crash digest BEFORE the
  # final cleanup (`docker compose down`) removes the containers. Run A's equivalent
  # is already in run-a-node-logs/ (captured before the reset).
  node_logs="$RESULTS_DIR/node-logs" # Run B (V2, attestation ON)
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
# main — build the stress binary once, then run ITERS config-gated iterations of
# the experiment, aggregate them all, and (once) offer to tear monitoring down.
# ===========================================================================
sudo -v # cache sudo creds up front so prompts don't interrupt mid-run

banner "== H1 [0/5] build stress binary =="
# Guard: the stress binary must come from the intended feature branch. If
# BENCH_REPO is on a different branch, warn and force-checkout the expected one;
# abort if that checkout fails (dirty tree / missing branch) so we never build or
# run on the wrong branch. Override the target with EXPECT_BENCH_BRANCH.
EXPECT_BENCH_BRANCH="${EXPECT_BENCH_BRANCH:-protocol-research/feat/transaction-attestation-feature-test}"
if [[ -z "${STRESS_BIN_PATH:-}" && -d "$BENCH_REPO/.git" ]]; then
  cur_branch="$(git -C "$BENCH_REPO" rev-parse --abbrev-ref HEAD 2>/dev/null || echo '?')"
  if [[ "$cur_branch" != "$EXPECT_BENCH_BRANCH" ]]; then
    echo "${YELLOW}network-benchmark on '$cur_branch' — switching to '$EXPECT_BENCH_BRANCH'...${RESET}" >&2
    git -C "$BENCH_REPO" checkout "$EXPECT_BENCH_BRANCH" || {
      echo "${RED}ERROR: could not checkout '$EXPECT_BENCH_BRANCH' in $BENCH_REPO" >&2
      echo "       (uncommitted changes, or branch missing?) — resolve and re-run.${RESET}" >&2
      exit 1
    }
  fi
  built_branch="$(git -C "$BENCH_REPO" rev-parse --abbrev-ref HEAD)"
  echo "network-benchmark on $built_branch (HEAD $(git -C "$BENCH_REPO" rev-parse --short HEAD))"
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

for ((_iter = 1; _iter <= ITERS; _iter++)); do
  banner "########## experiment '$LABEL': iteration $_iter of $ITERS ##########"
  run_one_iteration
done

if [[ "$ANALYZE" == true ]]; then
  # Aggregate ALL iterations of this label into one summary (not just this run's
  # $ITERS). The config gate guarantees they share a config, so pooling their raw
  # histograms is valid — more iterations => less noise. (Replaces the old per-run
  # archive/ de-mixing.)
  n_iters=$(ls -1d "$EXP_DIR"/iter-*/ 2>/dev/null | wc -l | tr -d ' ')
  banner "== aggregate experiment '$LABEL' (all $n_iters iteration(s)) =="
  python3 "$SCRIPT_DIR/aggregate.py" "$EXP_DIR" "$EXP_DIR/summary.md"
  echo "  - summary -> $(rel "$EXP_DIR/summary.md")"

  # Render Grafana-style figures across ALL iterations of this label (every iter-NNN
  # under results/<LABEL>/, not just this run) — V1 vs V2, mean/median + variance
  # band — into results/<LABEL>/plots/. Uses the local venv (matplotlib/numpy);
  # non-fatal and skipped with a hint if the venv isn't set up.
  VENV_PY="$SCRIPT_DIR/.venv/bin/python"
  if [[ -x "$VENV_PY" ]]; then
    echo "  - plots   -> $(rel "$EXP_DIR/plots")/"
    "$VENV_PY" "$SCRIPT_DIR/plot.py" --label "$LABEL" ||
      echo "${RED}    - plot.py failed (non-fatal); data + summary are intact.${RESET}"
  else
    echo "${YELLOW}Skipping plots: venv not found. Set it up once, then re-plot anytime:${RESET}"
    echo "${YELLOW}  python3 -m venv $SCRIPT_DIR/.venv && $SCRIPT_DIR/.venv/bin/pip install matplotlib numpy${RESET}"
    echo "${YELLOW}  $VENV_PY $SCRIPT_DIR/plot.py --label $LABEL${RESET}"
  fi
else
  banner "== ANALYZE=false — skipping aggregate + plots (matrix sweeps all labels at the end) =="
fi

echo
echo "${CYAN}Grafana: http://localhost:3000/d/attestation-sequencer-stress${RESET}"

# Monitoring teardown happens ONCE, after every iteration. `down -v` also removes
# the prometheus-data volume, clearing every run's series. Interactively (both
# stdin AND stdout a terminal) we PROMPT and default to keeping it, so you can
# keep inspecting the runs in Grafana. When output is redirected (unattended /
# matrix run) we just CLEAR it — no prompt to block on, and nobody's watching.
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
