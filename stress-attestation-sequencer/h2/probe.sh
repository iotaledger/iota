#!/usr/bin/env bash
#
# probe.sh — H2 calibration pre-step: characterize ONE slow::slow(n, size) point.
#
# Fires a short, low-rate `slow` spam and reports the per-transaction computation
# units (attested — metered during the attestation dry-run, the value
# TotalComputationUnits schedules on — plus actual, metered at post-consensus
# execution; for owned-object txs the two should match) and the internal
# execution time (mean ± sem, plus std). Repeated invocations sweep the
# (n, size) space and accumulate results/calibration-<machine>.csv, from which
# the (n, size) settings and per-object limits for the H2 mode comparison get
# picked. The probe runs the owned slow variant (W4 in ../stress-plan.md); the
# mode comparison runs the shared variant (W5).
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
#                 DURATION (default 20s), DIRECT (default true = in-docker
#                 client submitting directly to validators, like ../h1; false =
#                 host binary via the fullnode), N (validators,
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
# Default vehicle: the stress client runs IN-DOCKER on the private network
# (run-stress-docker.sh, same as ../h1) and submits straight to the validators'
# submit_tx — the attested P-COOL path — with no fullnode hop in the response
# chain. DIRECT=false falls back to the host-built binary via the fullnode.
DIRECT="${DIRECT:-true}"
N="${N:-4}"
NUM_CLIENT_THREADS="${NUM_CLIENT_THREADS:-12}"
NUM_TRANSFER_ACCOUNTS="${NUM_TRANSFER_ACCOUNTS:-4}"
# Payload slots per worker = its qps share x this ratio. This is ALSO the
# client's only back-pressure: a worker stops submitting once every slot waits
# on a response. Keep it at 2 — raising it to 5 removed the self-throttling
# and let retries stack ~25 concurrent ceiling-cost txs onto an already-slow
# network, which snowballed (delivered 55 -> 30 -> 0 across attempts on the
# WS). Under-delivered points are cheaper to retry than to prevent.
IN_FLIGHT_RATIO="${IN_FLIGHT_RATIO:-2}"
NUM_WORKERS="${NUM_WORKERS:-24}"
NUM_TARGET_VALIDATORS="${NUM_TARGET_VALIDATORS:-}"
# Seconds the client waits between warmup/setup and spamming (stress
# --pre-spam-delay-secs). The measurement window is anchored at the exact spam
# start emitted by the client, so this only widens the safety margin between
# setup draining and spamming. The probe opts into 2s; the flag itself defaults
# to 0 everywhere else (backward compatible). Requires a stress image/binary
# with the flag.
PRE_SPAM_DELAY_SECS="${PRE_SPAM_DELAY_SECS:-2}"
PROM="${PROM:-http://localhost:9090}"
TS_STEP="${TS_STEP:-1}"
# Window-closing drain: execution lags the client, so instead of a fixed sleep
# the probe polls the pooled executed-tx counter until it stops advancing (see
# wait_for_drain). Machine-independent: fast boxes drain in seconds, slow ones
# get however long they need, capped by DRAIN_TIMEOUT_S.
DRAIN_POLL_S="${DRAIN_POLL_S:-2}" # >= the Prometheus scrape interval (1s)
DRAIN_TIMEOUT_S="${DRAIN_TIMEOUT_S:-120}"
# Inter-point checkpoint drain: after a point's measurement the user-tx drain
# above guarantees our txs executed, but the checkpoint BUILDER can still be
# sealing a backlog. On a reused network that backlog bleeds into the next
# sweep point's checkpoint-lag reading (a cheap point right after a ceiling
# point reads seconds of lag that aren't its own). So after measuring, wait
# with the network idle until freshly built checkpoints are back near the idle
# baseline — recent checkpoint_creation_latency below CKPT_DRAIN_THRESHOLD_S.
CKPT_DRAIN="${CKPT_DRAIN:-yes}" # set to no to skip
CKPT_DRAIN_POLL_S="${CKPT_DRAIN_POLL_S:-3}"
CKPT_DRAIN_TIMEOUT_S="${CKPT_DRAIN_TIMEOUT_S:-180}"
CKPT_DRAIN_THRESHOLD_S="${CKPT_DRAIN_THRESHOLD_S:-0.5}" # recent mean lag ⇒ caught up
PRODUCT=$((SLOW_N * SLOW_SIZE))
PRIMARY_GAS_OWNER="0xf479d29837d22943aba6afc401f518a36521b990874eca784886185bd26bf681"
BENCH_REPO="${BENCH_REPO:-$REPO_ROOT/../network-benchmark}"
STRESS_BIN="${STRESS_BIN_PATH:-$BENCH_REPO/target/release/stress}"

# Machine-specific CSV so a WS and an EPYC sweep don't collide and the analysis
# scripts can distinguish them: calibration-<cpu-slug>.csv, slug from the CPU model
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
  echo "${YELLOW}NOTE: DIRECT=true runs the stress image in-network; it must be built from the" >&2
  echo "      CURRENT network-benchmark branch (docker/stress/build.sh) — a stale image" >&2
  echo "      means a stale client (wrong protocol version pin, missing flags).${RESET}" >&2
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

# Pooled executed USER-tx counter across the validators (fullnode excluded, to
# match what probe_scrape.py measures); empty until Prometheus has scraped at
# least one validator. Must be the user-only histogram: the all-tx counter
# advances forever on background system transactions (commit prologues etc.),
# so it never goes quiet.
exec_count() {
  curl -sG --max-time 5 "$PROM/api/v1/query" \
    --data-urlencode 'query=sum(authority_state_internal_execution_latency_user_count{job=~"Validator_.*"})' |
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

# Instantaneous pooled scalar for a PromQL expression (validators), or empty.
prom_scalar() {
  curl -sG --max-time 5 "$PROM/api/v1/query" --data-urlencode "query=$1" |
    python3 -c 'import json,sys
r = json.load(sys.stdin).get("data", {}).get("result", [])
print(r[0]["value"][1] if r else "")' 2>/dev/null
}

# Wait (network idle) for the checkpoint builder to drain its backlog, so the
# next sweep point starts caught up. While behind, the builder seals old
# checkpoints whose txs took long, so the mean lag of checkpoints built in the
# last interval stays high; once caught up, new (light/system) checkpoints build
# promptly and it drops to the idle baseline. Poll that recent mean until it is
# below CKPT_DRAIN_THRESHOLD_S for two consecutive intervals.
wait_for_checkpoint_drain() {
  [[ "$CKPT_DRAIN" == no || "$CKPT_DRAIN" == n ]] && return 0
  local q_sum='sum(checkpoint_creation_latency_sum{job=~"Validator_.*"})'
  local q_cnt='sum(checkpoint_creation_latency_count{job=~"Validator_.*"})'
  local prev_s prev_c cur_s cur_c recent stable=0 waited=0
  echo "${YELLOW}Draining checkpoint backlog (builder catching up)...${RESET}"
  prev_s=$(prom_scalar "$q_sum")
  prev_c=$(prom_scalar "$q_cnt")
  while ((waited < CKPT_DRAIN_TIMEOUT_S)); do
    sleep "$CKPT_DRAIN_POLL_S"
    waited=$((waited + CKPT_DRAIN_POLL_S))
    cur_s=$(prom_scalar "$q_sum")
    cur_c=$(prom_scalar "$q_cnt")
    [[ -z "$cur_s" || -z "$cur_c" || -z "$prev_s" || -z "$prev_c" ]] && {
      prev_s=$cur_s
      prev_c=$cur_c
      continue
    }
    # Mean lag of checkpoints built in this interval; skip if none were built.
    recent=$(awk -v s0="$prev_s" -v s1="$cur_s" -v c0="$prev_c" -v c1="$cur_c" \
      'BEGIN { dc = c1 - c0; if (dc > 0) printf "%.3f", (s1 - s0) / dc; else print "nan" }')
    prev_s=$cur_s
    prev_c=$cur_c
    [[ "$recent" == nan ]] && continue
    if awk -v r="$recent" -v t="$CKPT_DRAIN_THRESHOLD_S" 'BEGIN { exit !(r < t) }'; then
      stable=$((stable + 1))
      ((stable >= 2)) && {
        echo "  - Checkpoint builder caught up (recent lag ${recent}s < ${CKPT_DRAIN_THRESHOLD_S}s after ${waited}s)."
        return 0
      }
    else
      stable=0
    fi
  done
  echo "${YELLOW}  - WARNING: checkpoint backlog still draining after ${CKPT_DRAIN_TIMEOUT_S}s (recent lag ~${recent:-?}s).${RESET}" >&2
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

# Host-binary fallback only: build the stress binary from BENCH_REPO (same as
# ../h1/run.sh) so a stale binary built earlier on another branch can't slip
# through. The default in-docker path uses the iotaledger/stress image instead —
# rebuild that with network-benchmark's docker/stress/build.sh when the client
# changes.
if [[ "$DIRECT" != true ]]; then
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
fi

ensure_network

banner ">>> probe: slow(n=$SLOW_N, size=$SLOW_SIZE) product=$PRODUCT shared=$SLOW_SHARED qps=$QPS dur=$DURATION path=$([[ "$DIRECT" == true ]] && echo direct-docker || echo fullnode-host)"
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
    PRE_SPAM_DELAY_SECS="$PRE_SPAM_DELAY_SECS" \
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
    --pre-spam-delay-secs "$PRE_SPAM_DELAY_SECS" \
    bench --target-qps "$QPS" \
    --in-flight-ratio "$IN_FLIGHT_RATIO" \
    --num-workers "$NUM_WORKERS" \
    "${WORKLOAD_ARGS[@]}") 2>"$STRESS_LOG"
fi
echo "  - stress stderr -> $(rel "$STRESS_LOG")"
submit_end=$(date +%s)

wait_for_drain
end=$(date +%s)

# Exclude the warmup. The gas-coin setup transactions run during client init,
# before the timed benchmark; anchor the measurement window at the exact spam
# start the client prints (PROBE_SPAM_START_UNIX), so a histogram delta from
# there subtracts the setup txs out (they sit in the pre-baseline cumulative
# counts), leaving the mean over the identical workload transactions only.
# Fall back to (submission end − DURATION) if the marker is absent (older stress
# image without the flag).
spam_start="$(sed -n 's/.*PROBE_SPAM_START_UNIX=\([0-9.][0-9.]*\).*/\1/p' "$STRESS_LOG" | tail -1)"
if [[ -n "$spam_start" ]]; then
  # Anchor 1s BEFORE the marker, inside the pre-spam quiet gap (needs a delay of
  # >= 2s). Prometheus samples on a 1s grid, so a baseline exactly at the marker
  # lets fast workload txs that execute in the first sub-second leak into the
  # baseline and drop the point just under 400 samples; a baseline in the quiet
  # gap sees setup done and no workload yet, so all 100 workload txs land in the
  # delta (setup stays excluded — it is in the baseline). No gap => use the marker.
  if ((PRE_SPAM_DELAY_SECS >= 2)); then
    window_start="$(awk "BEGIN{printf \"%.3f\", $spam_start - 1}")"
  else
    window_start="$spam_start"
  fi
  echo "  - spam started at $spam_start; window from $window_start (warmup excluded)."
else
  dur_secs="${DURATION%s}"
  if [[ "$dur_secs" =~ ^[0-9]+$ ]]; then
    window_start=$((submit_end - dur_secs))
    echo "${YELLOW}  - no PROBE_SPAM_START_UNIX marker; estimating spam start as submit_end − ${dur_secs}s (rebuild the stress image for the exact marker).${RESET}" >&2
  else
    echo "${YELLOW}  - no marker and DURATION='$DURATION' not in Ns form; measuring the whole window (setup included).${RESET}" >&2
    window_start="$start"
  fi
fi

banner "== measure =="
PROM="$PROM" \
  CFG_slow_n="$SLOW_N" CFG_slow_size="$SLOW_SIZE" CFG_product="$PRODUCT" \
  CFG_shared="$SLOW_SHARED" CFG_qps="$QPS" CFG_duration="$DURATION" \
  python3 "$SCRIPT_DIR/probe_scrape.py" "$window_start" "$end" "$TS_STEP" "$CSV_OUT"

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
  # Keeping the network for the next point — let the checkpoint builder catch up
  # first so this point's backlog doesn't contaminate the next point's lag.
  wait_for_checkpoint_drain
  echo "${GREEN}Network left up (reuse for the next probe). Tear down later with: sudo $(rel "$TOOLS_DIR/cleanup.sh")${RESET}"
fi
