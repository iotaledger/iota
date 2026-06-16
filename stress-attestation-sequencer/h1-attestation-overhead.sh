#!/usr/bin/env bash
#
# h1-attestation-overhead.sh — run the H1 experiment (attestation overhead,
# W4: V1 vs V2) end to end and capture the metrics:
#   0. build      pre-build the stress binary (so timestamps bracket only spam)
#   1. cleanup    tear down anything already running (best-effort)
#   2. bootstrap  -b, regenerate genesis with benchmark gas accounts
#   3. Run A — V1 attestation OFF (control), owned-object load, TotalTxCount
#   4. Run B — V2 attestation ON, same load (FULL reset between runs — cleanup +
#                 re-bootstrap, incl. a fresh Prometheus — so Run B cold-starts
#                 exactly like Run A; Run A's timeseries is saved beforehand)
#   then        save each run's window as a raw timeseries JSON (Run A before its
#               reset), aggregate raw histograms across the same-config runs under
#               results/h1/ into summary.md, stop + clean the network, and prompt
#               whether to also stop monitoring (Grafana).
#
# Run as a NORMAL user (cargo must not run as root); `sudo` is used internally
# only for cleanup/bootstrap.
#
# Tunables (env): N, RUN_DURATION, TARGET_QPS, NUM_WORKERS, NUM_CLIENT_THREADS,
#                 NUM_TRANSFER_ACCOUNTS, IN_FLIGHT_RATIO, DIRECT,
#                 NUM_TARGET_VALIDATORS, WORKLOAD (owned|shared|slow),
#                 NUM_SHARED_COUNTERS, SLOW_N, SLOW_SIZE, SLEEP_BETWEEN_RUNS_S,
#                 PRE_SPAM_WAIT_S, PRE_STOP_WAIT_S, PROM, TS_STEP.

set -euo pipefail

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

if [[ ${EUID:-$(id -u)} -eq 0 ]]; then
  echo "${RED}ERROR: run as a normal user, not root (cargo would build as root)." >&2
  echo "       sudo is invoked internally for cleanup/bootstrap.${RESET}" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
GENESIS_DIR="$REPO_ROOT/dev-tools/iota-private-network/configs/genesis"

N="${N:-4}"
RUN_DURATION="${RUN_DURATION:-30s}"
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
# Setup-phase gas coins prepped before spam = TARGET_QPS * IN_FLIGHT_RATIO *
# (NUM_TRANSFER_ACCOUNTS + 1). That product drives warmup time, so keep
# NUM_TRANSFER_ACCOUNTS / IN_FLIGHT_RATIO small — they don't gate throughput at
# this scale (concurrency comes from max_ops = TARGET_QPS * IN_FLIGHT_RATIO).
SLEEP_BETWEEN_RUNS_S="${SLEEP_BETWEEN_RUNS_S:-5}" # idle gap (s) to separate A/B on the timeline
PRE_SPAM_WAIT_S="${PRE_SPAM_WAIT_S:-0}"           # let the network settle this long after it's up, before the spam
PRE_STOP_WAIT_S="${PRE_STOP_WAIT_S:-2}"           # keep Run A's network up this long after scraping, before stopping
PROM="${PROM:-http://localhost:9090}"
TS_STEP="${TS_STEP:-1}" # query_range step (s) for the per-run raw timeseries dump
PRIMARY_GAS_OWNER="0xf479d29837d22943aba6afc401f518a36521b990874eca784886185bd26bf681"
STRESS_BIN="$REPO_ROOT/target/release/stress"
RESULTS_DIR="$SCRIPT_DIR/results/h1/$(date +%Y%m%d-%H%M%S)"
mkdir -p "$RESULTS_DIR"

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
  ;;
*)
  echo "${RED}ERROR: unknown WORKLOAD='$WORKLOAD' (expected: owned | shared | slow)${RESET}" >&2
  exit 1
  ;;
esac

# `shared`/`slow` publish a Move package at runtime (basics / slow), compiled
# from repo sources that depend on the iota-framework — present on the host
# (fullnode path), but NOT inside the iota-tools image. So they can't run on the
# in-docker direct path (and therefore can't be pinned). owned needs no publish.
if [[ "$WORKLOAD" != owned && "$DIRECT" == true ]]; then
  echo "${RED}ERROR: WORKLOAD=$WORKLOAD needs a runtime Move publish that the in-docker" >&2
  echo "       iota-tools image can't do. Use DIRECT=false (fullnode path).${RESET}" >&2
  exit 1
fi

RULE="$(printf '%80s' '' | tr ' ' '*')"
banner() {
  echo
  echo "${MAGENTA}${RULE}${RESET}"
  echo "${MAGENTA}$*${RESET}"
  echo "${MAGENTA}${RULE}${RESET}"
}

# Block until the fullnode JSON-RPC at :9000 accepts connections (start.sh
# verifies validators, not the fullnode, so it can lag behind).
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

# Reset the network between runs so Run B's startup path is IDENTICAL to the
# initial setup before Run A — that full symmetry is what keeps the pre-spam
# warmup the same. We tear EVERYTHING down (incl. the monitoring stack) and
# re-bootstrap a brand-new genesis (current timestamp) with an empty DB, exactly
# like Run A's [1/5] cleanup + [2/5] bootstrap. Leaving Prometheus up across the
# reset appears to give Run B a longer warmup, so cleanup.sh brings it down and
# start.sh brings a fresh one back up for Run B.
#
# Run A is already scraped to run-a-v1.timeseries.json before this point, so
# tearing Prometheus down here only drops Run A's live Grafana history (the
# aggregated summary.md, built from the saved timeseries, is unaffected).
reset_network() {
  echo "${YELLOW}Tearing everything down (incl. Prometheus) and re-bootstrapping a fresh genesis for Run B...${RESET}"
  echo "  - cleanup/bootstrap output -> $RESULTS_DIR/bootstrap.log"
  sudo "$SCRIPT_DIR/cleanup.sh" >>"$RESULTS_DIR/bootstrap.log" 2>&1 || true
  sudo "$SCRIPT_DIR/bootstrap.sh" -b -n "$N" >>"$RESULTS_DIR/bootstrap.log" 2>&1
}

# Dump the raw timeseries (Prometheus query_range) over the run window to a JSON
# file. We store the underlying series verbatim — cumulative histogram buckets
# (+ _count/_sum) and raw counters/gauges — with NO rate()/histogram_quantile()/
# aggregation baked in. Everything (any rate window, any quantile, per-validator
# breakdowns, and correct cross-run aggregation by pooling raw histograms) can be
# reconstructed from this offline. Each entry is the raw query_range result: one
# series per full label set (le, host, name, ping, ...), values = [[ts,"v"],...].
# $1 label  $2 start_epoch  $3 end_epoch  $4 out.timeseries.json
dump_timeseries() {
  local label="$1" start="$2" end="$3" out="$4"
  echo "${BLUE}Dumping raw timeseries (step=${TS_STEP}s) -> $out${RESET}"
  PROM="$PROM" \
    CFG_target_qps="$TARGET_QPS" CFG_num_workers="$NUM_WORKERS" \
    CFG_in_flight_ratio="$IN_FLIGHT_RATIO" CFG_num_client_threads="$NUM_CLIENT_THREADS" \
    CFG_num_transfer_accounts="$NUM_TRANSFER_ACCOUNTS" CFG_run_duration="$RUN_DURATION" \
    CFG_direct="$DIRECT" CFG_num_target_validators="${NUM_TARGET_VALIDATORS:-all}" CFG_n="$N" \
    CFG_workload="$WORKLOAD" CFG_num_shared_counters="${NUM_SHARED_COUNTERS:-default}" \
    CFG_slow_n="${SLOW_N:-default}" CFG_slow_size="${SLOW_SIZE:-default}" \
    python3 - "$label" "$start" "$end" "$TS_STEP" "$out" <<'PY'
import json, os, sys, urllib.parse, urllib.request
label, start, end, step, out = sys.argv[1:6]
prom = os.environ["PROM"]
# Per-run config (so the aggregator can flag pooling across mismatched configs).
config = {k[4:]: v for k, v in os.environ.items() if k.startswith("CFG_")}

# name -> raw PromQL selector. Histograms keep their le+host labels; counters
# stay cumulative (compute rate() offline); CPU is scoped to the node containers
# (validators + fullnodes) to bound cardinality.
metrics = {}
for base in (
    "validator_attestation_latency",            # pre-consensus dry-run (V2)
    "validator_transaction_execution_latency",  # validator-internal pipeline
    "authority_state_internal_execution_latency",  # pure VM execution
    "transaction_driver_settlement_finality_latency",  # client-side (fullnode)
    "transaction_driver_submit_transaction_latency",   # client-side (fullnode)
):
    metrics[f"{base}_bucket"] = f"{base}_bucket"
    metrics[f"{base}_count"] = f"{base}_count"
    metrics[f"{base}_sum"] = f"{base}_sum"
# raw counters / gauges
metrics["transactions_included_in_checkpoint"] = "transactions_included_in_checkpoint"
metrics["validator_attestations_total"] = "validator_attestations_total"
metrics["container_cpu_usage_seconds_total"] = (
    'container_cpu_usage_seconds_total{name=~"validator-.*|fullnode-.*"}'
)

series = {}
for name, q in metrics.items():
    url = prom + "/api/v1/query_range?" + urllib.parse.urlencode(
        {"query": q, "start": start, "end": end, "step": step})
    try:
        with urllib.request.urlopen(url, timeout=60) as r:
            series[name] = json.load(r).get("data", {}).get("result", [])
    except Exception as e:
        series[name] = {"error": str(e)}

with open(out, "w") as f:
    json.dump({"label": label, "start_epoch": int(start), "end_epoch": int(end),
               "step_seconds": int(step), "config": config, "series": series}, f, indent=2)
print("  - wrote", out)
PY
}

# The aggregated V1-vs-V2 summary is built across ALL runs by h1-aggregate.py
# (it pools raw histogram buckets — correct cross-run percentile aggregation).
# See the [5/5] step below.

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
  local stress_log="${ts_out%.timeseries.json}.stress.log"
  start=$(date +%s)
  if [[ "$DIRECT" == true ]]; then
    # Direct-to-validator, in-docker (validators only reachable on the docker
    # network). Same workload knobs, passed through to the runner.
    RUN_DURATION="$RUN_DURATION" TARGET_QPS="$TARGET_QPS" NUM_WORKERS="$NUM_WORKERS" \
      NUM_CLIENT_THREADS="$NUM_CLIENT_THREADS" NUM_TRANSFER_ACCOUNTS="$NUM_TRANSFER_ACCOUNTS" \
      IN_FLIGHT_RATIO="$IN_FLIGHT_RATIO" PRIMARY_GAS_OWNER="$PRIMARY_GAS_OWNER" \
      USE_FULLNODE_FOR_EXECUTION=false NUM_TARGET_VALIDATORS="$NUM_TARGET_VALIDATORS" \
      WORKLOAD="$WORKLOAD" \
      "$SCRIPT_DIR/run-stress-docker.sh" 2>"$stress_log"
  else
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
  echo "stderr -> $stress_log"
  echo

  dump_timeseries "$label" "$start" "$end" "$ts_out"
  echo
}

# Cache sudo credentials up front so prompts don't interrupt mid-run.
sudo -v

banner "== H1 [0/5] build stress binary =="
(cd "$REPO_ROOT" && cargo build --release -p iota-benchmark --bin stress)

banner "== H1 [1/5] cleanup (in case something is running) =="
# cleanup/bootstrap are verbose (docker compose + genesis tooling); send their
# output to bootstrap.log to keep the console readable.
sudo "$SCRIPT_DIR/cleanup.sh" >>"$RESULTS_DIR/bootstrap.log" 2>&1 || true
# Start this invocation with an EMPTY Prometheus TSDB. The data volume is
# persistent (so it survives the between-runs teardown and both runs stay
# visible), but here — once, at the very start — we drop it with `down -v` so
# stale series from previous invocations don't linger. reset_network between
# Run A and Run B deliberately does NOT pass -v, so Run A's data is kept.
(cd "$REPO_ROOT/dev-tools/grafana-local" && docker compose down -v --remove-orphans) >/dev/null 2>&1 || true
echo "cleanup/bootstrap output -> $RESULTS_DIR/bootstrap.log"

banner "== H1 [2/5] bootstrap (-b, $N validators) =="
sudo "$SCRIPT_DIR/bootstrap.sh" -b -n "$N" >>"$RESULTS_DIR/bootstrap.log" 2>&1
echo "cleanup/bootstrap output -> $RESULTS_DIR/bootstrap.log"

banner "== H1 [3/5] Run A — V1 (attestation OFF, control) =="
MODE=TotalTxCount ATTEST=false "$SCRIPT_DIR/start.sh" -n "$N" faucet
run_stress "Run A — V1 (attestation off)" "$RESULTS_DIR/run-a-v1.timeseries.json"

# Run A is scraped; let the network run a moment so the post-run tail is
# captured, then fully reset (incl. Prometheus) and re-bootstrap a fresh genesis
# for Run B, idling so the runs are cleanly separated (targets go DOWN in the
# gap). Run A's metrics are already saved to run-a-v1.timeseries.json, so
# dropping Prometheus here is fine.
echo "${YELLOW}Letting the network run ${PRE_STOP_WAIT_S}s after the scrape before resetting...${RESET}"
sleep "$PRE_STOP_WAIT_S"
reset_network
echo

echo "${YELLOW}Idle gap (${SLEEP_BETWEEN_RUNS_S}s) to separate Run A/B on the timeline...${RESET}"
sleep "$SLEEP_BETWEEN_RUNS_S"

banner "== H1 [4/5] Run B — V2 (attestation ON) — fresh genesis, empty DB =="
# start.sh boots the validators from the freshly re-bootstrapped genesis with an
# empty data dir and brings up a fresh monitoring stack (reset_network tore the
# old one down), so Run B cold-starts exactly like Run A — only attestation
# differs. Run A's metrics live in run-a-v1.timeseries.json (already saved).
MODE=TotalTxCount "$SCRIPT_DIR/start.sh" -n "$N" faucet
run_stress "Run B — V2 (attestation on)" "$RESULTS_DIR/run-b-v2.timeseries.json"

# Symmetric with the end of Run A: let the network run the same moment after the
# scrape so Run B's post-run tail lands on the dashboard too, before teardown.
echo "${YELLOW}Letting the network run ${PRE_STOP_WAIT_S}s after the scrape before tearing down...${RESET}"
sleep "$PRE_STOP_WAIT_S"

banner "== H1 [5/5] both runs complete — stopping network =="
H1_ROOT="$(dirname "$RESULTS_DIR")" # .../results/h1

# Keep only same-config runs under results/h1/ before aggregating: any PRIOR run
# whose config differs from THIS run's is moved to results/h1/archive/ (not
# deleted, and excluded from the aggregator's one-level glob). The pool is then
# homogeneous, so percentiles aren't combined across mismatched configs.
python3 - "$H1_ROOT" "$RESULTS_DIR" <<'PY'
import glob, json, os, shutil, sys
root, current = sys.argv[1], sys.argv[2]
def cfg(d):
    try:
        return json.load(open(os.path.join(d, "run-a-v1.timeseries.json"))).get("config")
    except Exception:
        return None
ref = cfg(current)
if ref is not None:
    archive = os.path.join(root, "archive")
    for d in sorted(glob.glob(os.path.join(root, "*"))):
        if (not os.path.isdir(d) or os.path.basename(d) == "archive"
                or os.path.abspath(d) == os.path.abspath(current)):
            continue
        if cfg(d) != ref:  # different config (or no recorded config) -> archive
            os.makedirs(archive, exist_ok=True)
            print(f"archiving mismatched-config run {os.path.basename(d)} -> archive/")
            shutil.move(d, os.path.join(archive, os.path.basename(d)))
PY

# Build the summary by aggregating raw timeseries across the same-config runs
# remaining under results/h1/ (pooled histograms). One run => matches a per-run
# summary; more => reduced noise.
echo "${BLUE}This run's raw data: $RESULTS_DIR${RESET}"
echo "  - run-a-v1.timeseries.json, run-b-v2.timeseries.json"
echo "${BLUE}Aggregated summary (same-config runs under results/h1/): $H1_ROOT/summary.md${RESET}"
python3 "$SCRIPT_DIR/h1-aggregate.py" "$H1_ROOT" "$H1_ROOT/summary.md"
echo

# Always stop + clean the network (down + wipe data) via the privnet's OWN
# cleanup, which leaves the monitoring stack up so both runs stay visible in
# Grafana. (cd in first: it runs `docker compose down` against the cwd.)
echo "${YELLOW}Stopping and cleaning the network...${RESET}"
(cd "$REPO_ROOT/dev-tools/iota-private-network" && sudo ./cleanup.sh) >>"$RESULTS_DIR/bootstrap.log" 2>&1
echo "  - Network stopped and cleaned. Monitoring is still up — both runs visible:"
echo "${CYAN}    - Grafana: http://localhost:3000/d/attestation-sequencer-stress${RESET}"
echo

# Monitoring teardown is opt-in: `down -v` also removes the prometheus-data
# volume, fully clearing both runs' series.
read -r -p "${YELLOW}Also stop and CLEAR monitoring (Prometheus data)? [y/N] ${RESET}" ans
if [[ "$ans" == "y" || "$ans" == "Y" ]]; then
  (cd "$REPO_ROOT/dev-tools/grafana-local" && docker compose down -v --remove-orphans) >>"$RESULTS_DIR/bootstrap.log" 2>&1 || true
  echo "${YELLOW}  - Monitoring stopped and Prometheus data cleared.${RESET}"
else
  echo "${YELLOW}  - Monitoring left running. Stop + clear it later with:${RESET}"
  echo "${YELLOW}    (cd $REPO_ROOT/dev-tools/grafana-local && docker compose down -v)${RESET}"
fi
