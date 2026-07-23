#!/usr/bin/env bash
#
# probe.sh — H2 calibration pre-step: characterize ONE slow::slow(n, size) point.
#
# Fires a short, low-rate `slow` spam and reports the per-transaction computation
# units (attested + actual, the values TotalComputationUnits schedules on) and
# the internal execution time (mean ± sem, plus std). Repeated invocations sweep
# the (n, size) space and accumulate results/calibration-<machine>.csv, which then selects
# the W5 cost points and sets the per-object limits for the H2 mode comparison.
#
# Unlike ../h1/run.sh this does NOT do an A/B two-run flow and does NOT wipe or
# re-bootstrap between invocations: it reuses a running network so a sweep is
# fast. It brings the network up (attestation ON, TotalComputationUnits) only if
# nothing is running, and asks at the end whether to tear it down (default: no).
#
# Run as a NORMAL user (cargo must not run as root); sudo is used internally only
# if it has to bootstrap/start the network.
#
# Required env: SLOW_N, SLOW_SIZE.
# Tunables (env): SLOW_SHARED (default false = owned), QPS (default 5),
#                 DURATION (default 20s), DIRECT (default false), N (validators,
#                 default 4), NUM_CLIENT_THREADS, NUM_TRANSFER_ACCOUNTS,
#                 IN_FLIGHT_RATIO, NUM_WORKERS, PROM, TS_STEP, DRAIN_POLL_S,
#                 DRAIN_TIMEOUT_S, WIPE (yes|no; default: prompt interactively,
#                 else no).
#
# Example:
#   SLOW_N=100 SLOW_SIZE=100 ./probe.sh
#   SLOW_N=400 SLOW_SIZE=100 SLOW_SHARED=true QPS=2 ./probe.sh

set -euo pipefail

ulimit -n "$(ulimit -Hn)" || true

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
  echo "       sudo is invoked internally only to bootstrap/start the network." >&2
  echo "       On a root-by-default server, re-run with ALLOW_ROOT=1.${RESET}" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)" # .../stress-attestation-sequencer/h2
TOOLS_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"                  # shared: start/cleanup/bootstrap
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"               # repo root (iota/)
GENESIS_DIR="$REPO_ROOT/dev-tools/iota-private-network/configs/genesis"

rel() { case "$1" in "$REPO_ROOT"/*) printf './%s' "${1#"$REPO_ROOT"/}" ;; *) printf '%s' "$1" ;; esac }

: "${SLOW_N:?set SLOW_N (slow::slow n — number of vectors)}"
: "${SLOW_SIZE:?set SLOW_SIZE (slow::slow size — bytes per vector)}"
[[ "$SLOW_N" =~ ^[0-9]+$ && "$SLOW_SIZE" =~ ^[0-9]+$ ]] || {
  echo "${RED}ERROR: SLOW_N/SLOW_SIZE must be non-negative integers.${RESET}" >&2
  exit 1
}

SLOW_SHARED="${SLOW_SHARED:-false}"
QPS="${QPS:-5}"
DURATION="${DURATION:-20s}"
DIRECT="${DIRECT:-false}"
N="${N:-4}"
NUM_CLIENT_THREADS="${NUM_CLIENT_THREADS:-12}"
NUM_TRANSFER_ACCOUNTS="${NUM_TRANSFER_ACCOUNTS:-4}"
IN_FLIGHT_RATIO="${IN_FLIGHT_RATIO:-2}"
NUM_WORKERS="${NUM_WORKERS:-24}"
NUM_TARGET_VALIDATORS="${NUM_TARGET_VALIDATORS:-}"
PROM="${PROM:-http://localhost:9090}"
TS_STEP="${TS_STEP:-1}"
# Window-closing drain: execution lags the client, so instead of a fixed sleep
# the probe polls the pooled executed-tx counter until it stops advancing (see
# wait_for_drain). Machine-independent: fast boxes drain in seconds, slow ones
# get however long they need, capped by DRAIN_TIMEOUT_S.
DRAIN_POLL_S="${DRAIN_POLL_S:-2}" # >= the Prometheus scrape interval (1s)
DRAIN_TIMEOUT_S="${DRAIN_TIMEOUT_S:-120}"
PRODUCT=$((SLOW_N * SLOW_SIZE))
PRIMARY_GAS_OWNER="0xf479d29837d22943aba6afc401f518a36521b990874eca784886185bd26bf681"
BENCH_REPO="${BENCH_REPO:-$REPO_ROOT/../network-benchmark}"
STRESS_BIN="${STRESS_BIN_PATH:-$BENCH_REPO/target/release/stress}"

# Machine-specific CSV so a WS and an EPYC sweep don't collide and the analysis
# scripts can tell them apart: calibration-<cpu-slug>.csv, slug from the CPU model
# (e.g. ryzen-9-9950x3d, epyc-9454p). Override with MACHINE=<slug> if needed.
cpu_slug() {
  local m
  m=$(sed -n 's/^model name[[:space:]]*: //p' /proc/cpuinfo | head -1)
  m=${m#AMD }
  m=${m#Intel(R) }
  m=$(printf '%s' "$m" | sed -E 's/ [0-9]+-Core Processor$//; s/ Processor$//; s/\(R\)//g; s/\(TM\)//g')
  printf '%s' "$m" | tr '[:upper:] ' '[:lower:]-' | sed -E 's/[^a-z0-9-]//g; s/-+/-/g; s/^-|-$//g'
}
MACHINE="${MACHINE:-$(cpu_slug)}"
MACHINE="${MACHINE:-unknown}"
CSV_OUT="$SCRIPT_DIR/results/calibration-$MACHINE.csv"

# slow::slow(n, size) workload weights (same mapping as ../h1/run.sh).
WORKLOAD_ARGS=(--transfer-object 0 --slow 100 --slow-n "$SLOW_N" --slow-size "$SLOW_SIZE" --slow-shared "$SLOW_SHARED")

if [[ "$DIRECT" == true ]]; then
  echo "${YELLOW}NOTE: DIRECT=true publishes the slow Move package in-container; this needs the" >&2
  echo "      stress image rebuilt with the Move sources baked in (network-benchmark docker/stress).${RESET}" >&2
fi

RULE="$(printf '%80s' '' | tr ' ' '*')"
banner() {
  echo
  echo "${MAGENTA}${RULE}${RESET}"
  echo "${MAGENTA}$*${RESET}"
  echo "${MAGENTA}${RULE}${RESET}"
}

network_is_up() {
  curl -s -o /dev/null --max-time 2 -X POST -H 'Content-Type: application/json' \
    --data '{"jsonrpc":"2.0","id":1,"method":"iota_getChainIdentifier","params":[]}' \
    http://127.0.0.1:9000
}

wait_for_fullnode() {
  echo "${YELLOW}Waiting for fullnode RPC at 127.0.0.1:9000 ...${RESET}"
  for _ in $(seq 1 60); do
    if network_is_up; then
      echo "  - Fullnode RPC is up."
      return 0
    fi
    sleep 2
  done
  echo "${RED} -- ERROR: fullnode RPC at 127.0.0.1:9000 not ready in time.${RESET}" >&2
  exit 1
}

# Pooled executed-tx counter across all scraped nodes; empty until Prometheus
# has scraped at least one node exposing it.
exec_count() {
  curl -sG --max-time 5 "$PROM/api/v1/query" \
    --data-urlencode 'query=sum(authority_state_internal_execution_latency_count)' |
    python3 -c 'import json,sys
r = json.load(sys.stdin).get("data", {}).get("result", [])
print(r[0]["value"][1] if r else "")' 2>/dev/null
}

# The measurement window must not open before Prometheus has scraped the
# validators: on a cold start the first points otherwise see partial or absent
# series and come out with missing CU values.
wait_for_first_scrape() {
  echo "${YELLOW}Waiting for Prometheus samples from the network...${RESET}"
  for _ in $(seq 1 60); do
    if [[ -n "$(exec_count)" ]]; then
      echo "  - Prometheus is scraping the network."
      return 0
    fi
    sleep 2
  done
  echo "${RED} -- ERROR: no execution-counter samples in Prometheus at $PROM.${RESET}" >&2
  exit 1
}

# Execution lags the client: stress exits when its transactions are finalized
# client-side, while validators are still draining the execution tail. Close
# the window only once the pooled executed-tx counter has stopped advancing for
# two consecutive polls — i.e. the tail has executed AND been scraped — so the
# window is complete and nothing bleeds into the next point, on any hardware.
wait_for_drain() {
  local prev cur stable=0 waited=0
  prev=$(exec_count)
  while ((waited < DRAIN_TIMEOUT_S)); do
    sleep "$DRAIN_POLL_S"
    waited=$((waited + DRAIN_POLL_S))
    cur=$(exec_count)
    if [[ -n "$cur" && "$cur" == "$prev" ]]; then
      stable=$((stable + 1))
      if ((stable >= 2)); then
        echo "  - Execution drained (counter flat for $((2 * DRAIN_POLL_S))s after ${waited}s)."
        return 0
      fi
    else
      stable=0
    fi
    prev="$cur"
  done
  echo "${YELLOW}  - WARNING: execution still advancing after ${DRAIN_TIMEOUT_S}s; closing the window anyway.${RESET}" >&2
}

# Bring the network up ONCE, only if nothing is running. Attestation ON is
# required to populate attested_computation_units; the mode is irrelevant for
# this owned/low-rate probe but we keep TotalComputationUnits (start.sh default).
ensure_network() {
  if network_is_up; then
    echo "${GREEN}Reusing the running network (fullnode RPC responded).${RESET}"
    return 0
  fi
  echo "${YELLOW}No network detected — bringing up a fresh one (attestation ON, TotalComputationUnits).${RESET}"
  sudo -v
  # Cold start mirrors ../h1/run.sh: always cleanup + bootstrap -b so genesis.blob
  # and benchmark.keystore are regenerated together. Trusting a surviving blob
  # whose keystore was cleaned up makes stress fail with "Cannot find key for
  # address". Only runs when nothing is up, so a sweep still reuses one network.
  banner "== cleanup (in case something is half-up) =="
  sudo "$TOOLS_DIR/cleanup.sh" || true
  banner "== bootstrap (-b, $N validators) =="
  sudo "$TOOLS_DIR/bootstrap.sh" -b -n "$N"
  banner "== start network (attestation ON) =="
  ATTEST=true MODE=TotalComputationUnits "$TOOLS_DIR/start.sh" -n "$N" faucet
  wait_for_fullnode
}

# Build the stress binary from BENCH_REPO (same as ../h1/run.sh) — always, so a
# stale binary built earlier on another branch can't slip through. cargo is
# incremental, so it's a fast no-op when nothing changed.
banner "== build stress binary =="
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
  echo "${RED}stress binary not found/executable at $STRESS_BIN (build network-benchmark, or set STRESS_BIN_PATH)${RESET}" >&2
  exit 1
}

ensure_network

banner ">>> probe: slow(n=$SLOW_N, size=$SLOW_SIZE) product=$PRODUCT shared=$SLOW_SHARED qps=$QPS dur=$DURATION path=$([[ "$DIRECT" == true ]] && echo direct || echo fullnode)"
wait_for_fullnode
wait_for_first_scrape
STRESS_LOG="$SCRIPT_DIR/results/probe-last-stress.log"
mkdir -p "$SCRIPT_DIR/results"

start=$(date +%s)
if [[ "$DIRECT" == true ]]; then
  RUN_DURATION="$DURATION" TARGET_QPS="$QPS" NUM_WORKERS="$NUM_WORKERS" \
    NUM_CLIENT_THREADS="$NUM_CLIENT_THREADS" NUM_TRANSFER_ACCOUNTS="$NUM_TRANSFER_ACCOUNTS" \
    IN_FLIGHT_RATIO="$IN_FLIGHT_RATIO" PRIMARY_GAS_OWNER="$PRIMARY_GAS_OWNER" \
    USE_FULLNODE_FOR_EXECUTION=false NUM_TARGET_VALIDATORS="$NUM_TARGET_VALIDATORS" \
    WORKLOAD=slow SLOW_N="$SLOW_N" SLOW_SIZE="$SLOW_SIZE" SLOW_SHARED="$SLOW_SHARED" \
    "$TOOLS_DIR/run-stress-docker.sh" 2>"$STRESS_LOG"
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
    --run-duration "$DURATION" \
    bench --target-qps "$QPS" \
    --in-flight-ratio "$IN_FLIGHT_RATIO" \
    --num-workers "$NUM_WORKERS" \
    "${WORKLOAD_ARGS[@]}") 2>"$STRESS_LOG"
fi
echo "  - stress stderr -> $(rel "$STRESS_LOG")"

wait_for_drain
end=$(date +%s)

banner "== measure =="
PROM="$PROM" \
  CFG_slow_n="$SLOW_N" CFG_slow_size="$SLOW_SIZE" CFG_product="$PRODUCT" \
  CFG_shared="$SLOW_SHARED" CFG_qps="$QPS" CFG_duration="$DURATION" \
  python3 "$SCRIPT_DIR/probe_scrape.py" "$start" "$end" "$TS_STEP" "$CSV_OUT"

# End-of-run wipe: default NO so the next probe reuses the network. WIPE=yes
# forces teardown; interactively we prompt (default no); non-interactively we keep.
do_wipe=""
case "${WIPE:-}" in
yes | y | Y) do_wipe=1 ;;
no | n | N) do_wipe="" ;;
*)
  if [[ -t 0 && -t 1 ]]; then
    read -r -p "${YELLOW}Tear down the network + monitoring now? [y/N] ${RESET}" ans
    [[ "$ans" == y || "$ans" == Y ]] && do_wipe=1
  fi
  ;;
esac
if [[ -n "$do_wipe" ]]; then
  echo "${YELLOW}Tearing down network + monitoring...${RESET}"
  sudo "$TOOLS_DIR/cleanup.sh" || true
else
  echo "${GREEN}Network left up (reuse for the next probe). Tear down later with: sudo $(rel "$TOOLS_DIR/cleanup.sh")${RESET}"
fi
