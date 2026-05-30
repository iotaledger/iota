#!/usr/bin/env bash
# cap-policy-sweep.sh — measures the queue-overshoot and useful-throughput
# behaviour of the validator's pre-consensus load-shedding policy under
# sustained spam pressure, swept across `graduated-load-shedding-soft-limit-pct`
# values. Used to answer "do we need graduated shedding, or is binary
# at a lower cap equivalent?"
#
# At start_pct=100 the policy is binary (reactive only — 100% drop at
# `max_pending_transactions`). At start_pct<100 it's graduated: a soft
# zone between (start_pct × max_pending) and max_pending where
# submissions are probabilistically dropped, plus the same reactive cap
# at max_pending.
#
# This script does NOT include an honest pool (see fairness-sweep.sh
# for that). It's a pure spammer-only stress that measures:
#   - peak_inflight / max_pending  (overshoot ratio, lower = safer)
#   - useful_tps                   (throughput, higher = better)
#
# Usage:
#   ./cap-policy-sweep.sh                       # use current yaml pct
#   START_PCT=50 ./cap-policy-sweep.sh          # patch yaml to 50 first
#   ITERS=60 START_PCT=50 ./cap-policy-sweep.sh # 60 iters at pct=50
#   for p in 100 50 25 10; do
#     START_PCT=$p ITERS=60 ./cap-policy-sweep.sh
#   done                                         # full pct sweep
#
# Output:
#   cap-policy-sweep.jsonl  — one JSON record per iter (appends, nested)
#   cap-policy-sweep.log    — full sweep log
#
# pandas analysis:
#   df = pd.read_json("cap-policy-sweep.jsonl", lines=True)
#   df = pd.json_normalize(df.to_dict(orient="records"))
#
# Note: if you see AddrNotAvailable / TCP port exhaustion errors in the
# subprocess logs (rare since the TransactionDriver switch — long-lived
# gRPC channels replace per-call short-lived sockets), run
# `sudo ./tune-sysctl.sh` once per boot session.
set -uo pipefail

# CRITICAL: the validator-side white-flag override needs these env vars at
# `docker compose up` time. They must be set before run.sh starts validators.
export IOTA_PROTOCOL_CONFIG_OVERRIDE_ENABLE=1
export IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_WHITE_FLAG_FLOW=true

ITERS="${ITERS:-20}"

# Validator-side load-shedding policy knobs. Each is OPTIONAL — if set,
# the script patches the corresponding field in validator-common.yaml
# before the sweep. If unset, whatever the yaml currently has is used.
#
# START_PCT       → graduated-load-shedding-soft-limit-pct (0-100)
# SAT_PCT         → graduated-load-shedding-saturation-pct (0-100, must be
#                   >= START_PCT). Where the curve reaches 100% shedding.
#                   Default 100 = saturate at max_pending (legacy). Lower
#                   gives preventive headroom: at SAT_PCT=90 the curve
#                   reaches 100% at 90% of max_pending, preventing overshoot.
# MAX_PENDING     → max-pending-transactions (any positive int)
# SEMAPHORE_CAP   → max-pending-local-submissions (any positive int)
# SEM_SHEDDING    → semaphore-shedding-enabled (true|false). When false,
#                   the upfront "no permits available" reject is disabled
#                   while submit_semaphore still bounds submit_inner
#                   concurrency. Default leaves yaml as-is (= true).
#
# Typical sweep at fixed max_pending=1000, sem=2000:
#   for p in 100 50 25 10; do
#     MAX_PENDING=1000 SEMAPHORE_CAP=2000 START_PCT=$p \
#       ITERS=60 ./cap-policy-sweep.sh
#   done
START_PCT="${START_PCT:-}"
SAT_PCT="${SAT_PCT:-}"
MAX_PENDING="${MAX_PENDING:-}"
SEMAPHORE_CAP="${SEMAPHORE_CAP:-}"
SEM_SHEDDING="${SEM_SHEDDING:-}"

# Spammer-only — no honest pool here (see fairness-sweep.sh for that).
# Default NUM_PROCS=24 matches burst-sweep.sh / fairness-sweep.sh's
# spammer count so cap-policy results are directly comparable to the
# fairness results.
NUM_PROCS="${NUM_PROCS:-24}"

# Spammer pool config (canonical settings from burst-sweep.sh).
QPS_TOTAL="${QPS_TOTAL:-40000}"
DURATION="${DURATION:-15s}"
WORKERS="${WORKERS:-16}"
IN_FLIGHT_RATIO="${IN_FLIGHT_RATIO:-20}"
BURST_SIZE="${BURST_SIZE:-1800}"
# OPEN_LOOP=true makes the spammer pool fire submissions at target_qps
# regardless of in-flight count. Removes the closed-loop per-worker
# round-trip ceiling that pins per-proc submission rate well below
# nominal QPS under heavy validator contention. Use for cap-safety and
# goodput experiments where sustained validator-gate pressure matters
# more than per-tx correctness.
OPEN_LOOP="${OPEN_LOOP:-true}"
BARRIER_PERIOD_MS="${BARRIER_PERIOD_MS:-500}"
GAS_CHUNK_SIZE="${GAS_CHUNK_SIZE:-500}"
NUM_VALIDATORS_TO_TARGET="${NUM_VALIDATORS_TO_TARGET:-1}"

OUT_JSONL="cap-policy-sweep.jsonl"
OUT_LOG="cap-policy-sweep.log"
PRIVNET=/home/roman/IOTA/iotaledger/iota/dev-tools/iota-private-network
REPO=/home/roman/IOTA/iotaledger/iota
YAML_CFG="$PRIVNET/configs/validator-common.yaml"

# Derived
DURATION_SECS=$(echo "$DURATION" | sed 's/s$//')
N_SPAMMER=$NUM_PROCS
QPS_PER_SPAMMER=$((QPS_TOTAL / N_SPAMMER))
# Spammer "offered" includes the burst at t=0 plus QPS-paced submissions
# over DURATION_SECS. Ignores retries (which inflate stress.rs's view
# but not what the client originally intended to submit).
SPAMMER_OFFERED=$((BURST_SIZE * N_SPAMMER + QPS_PER_SPAMMER * DURATION_SECS * N_SPAMMER))

# Host + git context captured ONCE at sweep start; embedded into every
# JSONL record so post-hoc analysis can group / filter by host or commit.
HOST_NAME="$(hostname)"
HOST_NPROC="$(nproc 2>/dev/null || echo 0)"
HOST_KERNEL="$(uname -r)"
HOST_MEM_GIB="$(awk '/MemTotal/ {printf "%.0f", $2/1024/1024}' /proc/meminfo 2>/dev/null || echo 0)"
IOTA_GIT_COMMIT="$(git -C "$REPO" rev-parse --short HEAD 2>/dev/null || echo unknown)"

# JSONL output is append-only; no header required (each line is self-describing).

exec >> "$OUT_LOG" 2>&1

echo "================ cap-policy-sweep $(date -u) ================"
echo "config: NUM_PROCS=$NUM_PROCS (spammer-only, no honest pool)"
echo "        spammer: N=$N_SPAMMER QPS_per=$QPS_PER_SPAMMER BURST=$BURST_SIZE BAR=${BARRIER_PERIOD_MS}ms"
echo "        offered per iter: spammer=$SPAMMER_OFFERED"
echo

# Patch a `key: int_value` line in validator-common.yaml. Used to set
# load-shedding knobs (graduated-soft-limit-pct, max-pending-transactions,
# max-pending-local-submissions) before the per-iter network reset
# regenerates each validator's config from this overlay yaml.
#
# Args: $1 = yaml key, $2 = new integer value
# If $2 is empty, no-op (leave the yaml as-is). Verifies the patch
# actually landed by re-reading the yaml — sed silently does nothing
# if the key is misspelled, and that's a foot-gun that wastes hours.
patch_yaml_int() {
  local key="$1" value="$2"
  [ -z "$value" ] && return 0
  if ! [[ "$value" =~ ^[0-9]+$ ]]; then
    echo "Error: $key must be a positive integer, got '$value'" >&2
    exit 1
  fi
  sed -i -E "s/^([[:space:]]*${key}:[[:space:]]*).*/\1${value}/" "$YAML_CFG"
  local actual=$(grep -E "^[[:space:]]*${key}:" "$YAML_CFG" | awk -F: '{print $2}' | xargs)
  if [ "$actual" != "$value" ]; then
    echo "Error: yaml patch did not stick for $key (asked for $value, found '$actual')" >&2
    exit 1
  fi
  echo "=> Patched $YAML_CFG: ${key} = $value"
}

# Apply patches in order. graduated-load-shedding-soft-limit-pct gets
# extra validation (must be 0-100).
if [ -n "$START_PCT" ]; then
  if [ "$START_PCT" -gt 100 ] 2>/dev/null; then
    echo "Error: START_PCT must be in [0, 100], got '$START_PCT'" >&2
    exit 1
  fi
fi
patch_yaml_bool() {
  local key="$1" value="$2"
  [ -z "$value" ] && return 0
  if [ "$value" != "true" ] && [ "$value" != "false" ]; then
    echo "Error: $key must be 'true' or 'false', got '$value'" >&2
    exit 1
  fi
  sed -i -E "s/^([[:space:]]*${key}:[[:space:]]*).*/\1${value}/" "$YAML_CFG"
  local actual=$(grep -E "^[[:space:]]*${key}:" "$YAML_CFG" | awk -F: '{print $2}' | xargs)
  if [ "$actual" != "$value" ]; then
    echo "Error: yaml patch did not stick for $key (asked for $value, found '$actual')" >&2
    exit 1
  fi
  echo "=> Patched $YAML_CFG: ${key} = $value"
}

patch_yaml_int "max-pending-transactions"               "$MAX_PENDING"
patch_yaml_int "max-pending-local-submissions"          "$SEMAPHORE_CAP"
patch_yaml_int "graduated-load-shedding-soft-limit-pct" "$START_PCT"
patch_yaml_int "graduated-load-shedding-saturation-pct" "$SAT_PCT"
patch_yaml_bool "semaphore-shedding-enabled"            "$SEM_SHEDDING"

# Bump per-process file descriptor limit to the hard cap. Each stress
# subprocess opens many gRPC connections; the default soft limit of 1024
# is too tight at NUM_PROCS=96. Hard limit is whatever the system permits;
# raising the soft up to it doesn't require root.
HARD_NOFILE=$(ulimit -Hn)
if [ "$(ulimit -n)" -lt "$HARD_NOFILE" ]; then
  ulimit -n "$HARD_NOFILE" 2>/dev/null \
    || echo "  (warning: could not raise file descriptor soft limit to $HARD_NOFILE)"
fi
echo "  ulimit -n  ✓ soft=$(ulimit -n) hard=$HARD_NOFILE"

# -------- Pre-flight (matches burst-sweep.sh) ---------
PREFLIGHT_OK=1
echo "=== pre-flight checks ==="

if sudo -n true 2>/dev/null; then
  echo "  sudo cache  ✓"
else
  echo "  sudo cache  ✗ — run \`sudo -v\` first"
  PREFLIGHT_OK=0
fi

if [ "$EUID" -eq 0 ]; then
  echo "  target/ own ✓ skipped (running as root)"
else
  FOREIGN=$(find "$REPO/target" ! -uid "$(id -u)" 2>/dev/null | head -1)
  if [ -z "$FOREIGN" ]; then
    echo "  target/ own ✓ user-owned"
  else
    echo "  target/ own ✗ found foreign-owned file: $FOREIGN"
    echo "    fix: sudo chown -R \$USER:\$USER $REPO/target"
    PREFLIGHT_OK=0
  fi
fi

if grep -q '^127\.0\.0\.11.*validator-1' /etc/hosts; then
  echo "  /etc/hosts  ✓ validator-1..4 aliased"
else
  echo "  /etc/hosts  ✗ missing validator-N → 127.0.0.{11..14} entries"
  PREFLIGHT_OK=0
fi

if [ "$PREFLIGHT_OK" -eq 0 ]; then
  echo
  echo "=== ABORTING: pre-flight checks failed — fix the issues above and re-run ==="
  exit 1
fi

# -------- Initial bring-up: iota network first, then grafana ---------
echo
echo "=== bringing up network (validators + fullnode-1 + faucet) ==="
cd "$PRIVNET"
if [ ! -f "$PRIVNET/configs/genesis/genesis.blob" ]; then
  echo "  genesis.blob missing — running bootstrap.sh -b -n 4"
  sudo ./bootstrap.sh -b -n 4 2>&1 | tail -3
fi
./run.sh -n 4 faucet 2>&1 | tail -2
sleep 5

echo
echo "=== bringing up Prometheus + Grafana ==="
cd "$REPO/dev-tools/grafana-local"
docker compose up -d 2>&1 | tail -3

echo -n "  waiting for prometheus..."
PROM_READY=0
for attempt in $(seq 1 30); do
  if curl -sf --max-time 2 'http://localhost:9090/api/v1/query?query=up' >/dev/null 2>&1; then
    PROM_READY=1
    echo " ready after ${attempt}s"
    break
  fi
  sleep 1
  echo -n "."
done
if [ "$PROM_READY" -eq 0 ]; then
  echo
  echo "  prometheus  ✗ still unreachable after 30s — aborting"
  exit 1
fi
cd "$REPO"

echo
echo "=== all systems up — starting cap-policy sweep (ITERS=$ITERS) ==="
echo

# -------- Iteration loop ---------
for i in $(seq 1 $ITERS); do
  echo
  echo "=================================================="
  echo "[cap-policy iter=$i/$ITERS  pct=${START_PCT:-(current yaml)}]  $(date -u +%H:%M:%S)"
  echo "=================================================="

  # Kill any leftover stress.rs processes from a previous iter that may
  # still be holding metric ports (8081 + i). The metric server panics
  # with "Address already in use" if a stale binary hasn't released its
  # port yet — stress-multi.sh waits on the bash wrapper PID, but
  # setsid puts the stress.rs grandchild in its own process group, so
  # `wait` may return before the grandchild fully cleans up.
  pkill -9 -f "target/release/stress " 2>/dev/null || true
  sleep 1

  # Reset both stacks per iter. Grafana must come down BEFORE the iota
  # network: grafana-local containers attach to `iota-network` (for
  # validator scraping), so the network can't be removed cleanly while
  # they're up. All Prometheus queries happen at the end of each iter
  # window inside stress-multi.sh — there is no cross-iter analysis,
  # so bouncing the grafana stack per iter costs nothing and gives
  # each iter a fresh Prometheus DB.
  (cd "$REPO/dev-tools/grafana-local" && docker compose down 2>&1 | tail -1) || true
  # `docker compose down` returns before the OS fully releases host
  # ports bound by the removed containers (e.g. tempo's 3200, grafana's
  # 3000, prom's 9090). The next `compose up` then races and panics
  # with "address already in use". 2s is enough on every test we've
  # seen so far.
  sleep 2
  cd "$PRIVNET"
  docker compose down -v 2>&1 | tail -1 || true
  sudo ./bootstrap.sh -b -n 4 2>&1 | tail -3
  ./run.sh -n 4 faucet 2>&1 | tail -1
  rm -f "$REPO"/runs/.stress-gas-pool/owner-*.json
  (cd "$REPO/dev-tools/grafana-local" && docker compose up -d 2>&1 | tail -1)
  # Wait for Prometheus to be ready before the spam window starts.
  for attempt in $(seq 1 30); do
    if curl -sf --max-time 2 'http://localhost:9090/api/v1/query?query=up' >/dev/null 2>&1; then
      break
    fi
    sleep 1
  done
  sleep 20

  cd "$REPO"

  # Launch stress-multi.sh. HONEST_PROC_COUNT=0 → spammer-only run.
  NUM_PROCS="$NUM_PROCS" \
  HONEST_PROC_COUNT=0 \
  NUM_VALIDATORS_TO_TARGET="$NUM_VALIDATORS_TO_TARGET" \
  QPS_TOTAL="$QPS_TOTAL" \
  DURATION="$DURATION" \
  WORKERS="$WORKERS" \
  IN_FLIGHT_RATIO="$IN_FLIGHT_RATIO" \
  BURST_SIZE="$BURST_SIZE" \
  OPEN_LOOP="$OPEN_LOOP" \
  BARRIER_PERIOD_MS="$BARRIER_PERIOD_MS" \
  GAS_CHUNK_SIZE="$GAS_CHUNK_SIZE" \
  ./stress-multi.sh 2>&1 | tail -50 | tee "$REPO/runs/cap-policy-iter.log"

  # Parse per-iter summary + emit one JSONL record.
  latest=$(ls -td "$REPO"/runs/multi-*/ | head -1)

  # Read validator config from the yaml at THIS moment so each record
  # captures what was actually deployed (in case the yaml was patched
  # between sweeps or by hand).
  read_yaml_int() {
    grep -E "^[[:space:]]*$1:" "$YAML_CFG" 2>/dev/null | awk -F: '{print $2}' | xargs
  }
  val_max_pending=$(read_yaml_int "max-pending-transactions")
  val_sem_cap=$(read_yaml_int "max-pending-local-submissions")
  val_start_pct=$(read_yaml_int "graduated-load-shedding-soft-limit-pct")
  val_sat_pct=$(read_yaml_int "graduated-load-shedding-saturation-pct")
  : "${val_sat_pct:=100}"
  val_sem_shedding=$(read_yaml_int "semaphore-shedding-enabled")
  # Default to "true" if the yaml line is somehow missing (matches Rust default).
  : "${val_sem_shedding:=true}"

  if [ -f "$latest/summary.txt" ]; then
    peak=$(grep '^peak inflight:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    ratio=$(grep '^ratio:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs | sed 's/×//')
    exits=$(grep '^exit codes:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    r_prev=$(grep '^reject_grad_preventive:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    r_grad_react=$(grep '^reject_grad_reactive:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    r_grad_sat=$(grep '^reject_grad_saturated:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    r_max=$(grep '^reject_max_pending:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    r_sem=$(grep '^reject_semaphore:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    useful_tps=$(grep '^useful_tps:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    admit_p50=$(grep '^admit_lat_p50:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    admit_p99=$(grep '^admit_lat_p99:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    permit_hold_p50=$(grep '^permit_hold_p50:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    permit_hold_p99=$(grep '^permit_hold_p99:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    permit_wait_p50=$(grep '^permit_wait_p50:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    permit_wait_p99=$(grep '^permit_wait_p99:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    pre_acquire_p50=$(grep '^pre_acquire_p50:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    pre_acquire_p99=$(grep '^pre_acquire_p99:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    shed_pct_avg=$(grep '^shed_pct_avg:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    shed_pct_max=$(grep '^shed_pct_max:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    inflight_stddev=$(grep '^inflight_stddev:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    inflight_mean=$(grep '^inflight_mean:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    saturation_75pct=$(grep '^saturation_75pct:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    consensus_lat_p50=$(grep '^consensus_lat_p50:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    consensus_lat_p99=$(grep '^consensus_lat_p99:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    spammer_success=$(grep '^spammer_success:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)

    : "${peak:=0}"; : "${ratio:=0}"; : "${r_prev:=0}"; : "${r_grad_react:=0}"
    : "${r_max:=0}"; : "${r_sem:=0}"; : "${r_grad_sat:=0}"; : "${useful_tps:=0}"; : "${admit_p50:=0}"; : "${admit_p99:=0}"
    : "${permit_hold_p50:=0}"; : "${permit_hold_p99:=0}"
    : "${permit_wait_p50:=0}"; : "${permit_wait_p99:=0}"
    : "${pre_acquire_p50:=0}"; : "${pre_acquire_p99:=0}"
    : "${shed_pct_avg:=0}"; : "${shed_pct_max:=0}"
    : "${inflight_stddev:=0}"; : "${inflight_mean:=0}"; : "${saturation_75pct:=0}"
    : "${consensus_lat_p50:=0}"; : "${consensus_lat_p99:=0}"
    : "${spammer_success:=0}"

    ok=$(echo "$exits" | awk '{for(j=1;j<=NF;j++) if($j!="0"){print 0; exit} print 1}')
    spammer_fp=$(awk -v s="$spammer_success" -v o="$SPAMMER_OFFERED" \
      'BEGIN{if(o>0) printf "%.4f", 100.0*s/o; else print 0}')
    iso=$(basename "$latest" | sed 's/multi-//')
    failed=0
  else
    iso=$(basename "$latest" 2>/dev/null | sed 's/multi-//' || echo "?")
    peak=0; ratio=0; r_prev=0; r_grad_react=0; r_grad_sat=0; r_max=0; r_sem=0
    useful_tps=0; admit_p50=0; admit_p99=0; permit_hold_p50=0; permit_hold_p99=0
    permit_wait_p50=0; permit_wait_p99=0
    pre_acquire_p50=0; pre_acquire_p99=0
    shed_pct_avg=0; shed_pct_max=0
    inflight_stddev=0; inflight_mean=0; saturation_75pct=0; consensus_lat_p50=0; consensus_lat_p99=0
    spammer_success=0; spammer_fp=0; ok=0; exits=""
    failed=1
  fi

  # Emit one JSONL record. Values passed via env to avoid shell-quoting
  # hell inside the python one-liner.
  ISO="$iso" ITER="$i" FAILED="$failed" \
  HOST_NAME="$HOST_NAME" HOST_NPROC="$HOST_NPROC" HOST_KERNEL="$HOST_KERNEL" HOST_MEM_GIB="$HOST_MEM_GIB" \
  IOTA_GIT_COMMIT="$IOTA_GIT_COMMIT" \
  SPAM_NUM_PROCS="$N_SPAMMER" SPAM_QPS_PER="$QPS_PER_SPAMMER" SPAM_QPS_TOTAL="$QPS_TOTAL" \
  SPAM_BURST="$BURST_SIZE" SPAM_BAR_MS="$BARRIER_PERIOD_MS" SPAM_DURATION="$DURATION" SPAM_DURATION_SECS="$DURATION_SECS" \
  SPAM_WORKERS="$WORKERS" SPAM_IFR="$IN_FLIGHT_RATIO" SPAM_OPEN_LOOP="$OPEN_LOOP" \
  SPAM_GAS_CHUNK="$GAS_CHUNK_SIZE" SPAM_NUM_VALIDATORS="$NUM_VALIDATORS_TO_TARGET" \
  SPAM_OFFERED="$SPAMMER_OFFERED" \
  VAL_MAX_PENDING="$val_max_pending" VAL_SEM_CAP="$val_sem_cap" VAL_START_PCT="$val_start_pct" \
  VAL_SAT_PCT="$val_sat_pct" \
  VAL_SEM_SHEDDING="$val_sem_shedding" \
  R_PEAK="$peak" R_RATIO="$ratio" R_USEFUL_TPS="$useful_tps" \
  R_ADMIT_LAT_P50="$admit_p50" R_ADMIT_LAT_P99="$admit_p99" \
  R_PERMIT_HOLD_P50="$permit_hold_p50" R_PERMIT_HOLD_P99="$permit_hold_p99" \
  R_PERMIT_WAIT_P50="$permit_wait_p50" R_PERMIT_WAIT_P99="$permit_wait_p99" \
  R_PRE_ACQUIRE_P50="$pre_acquire_p50" R_PRE_ACQUIRE_P99="$pre_acquire_p99" \
  R_SHED_PCT_AVG="$shed_pct_avg" R_SHED_PCT_MAX="$shed_pct_max" \
  R_INFLIGHT_STDDEV="$inflight_stddev" R_INFLIGHT_MEAN="$inflight_mean" \
  R_SATURATION_75PCT="$saturation_75pct" \
  R_CONSENSUS_LAT_P50="$consensus_lat_p50" R_CONSENSUS_LAT_P99="$consensus_lat_p99" \
  R_SPAMMER_SUCCESS="$spammer_success" R_SPAMMER_FP="$spammer_fp" \
  R_REJ_PREV="$r_prev" R_REJ_REACT="$r_grad_react" R_REJ_SAT="$r_grad_sat" R_REJ_MAX="$r_max" R_REJ_SEM="$r_sem" \
  R_EXIT_CODES="$exits" R_OK="$ok" \
  python3 -c '
import json, os
def s(k, d=""): return os.environ.get(k, d)
def i(k):
    v=os.environ.get(k,"")
    try: return int(v)
    except: return None
def f(k):
    v=os.environ.get(k,"")
    try: return float(v)
    except: return None
def b(k): return os.environ.get(k,"").lower()=="true"
def ints(k):
    return [int(x) for x in s(k).split() if x.lstrip("-").isdigit()]

rec = {
  "iso_time": s("ISO"),
  "iter": i("ITER"),
  "failed": i("FAILED")==1,
  "host": {
    "hostname": s("HOST_NAME"),
    "nproc": i("HOST_NPROC"),
    "kernel": s("HOST_KERNEL"),
    "total_mem_gib": i("HOST_MEM_GIB"),
  },
  "git": {"iota_commit": s("IOTA_GIT_COMMIT")},
  "spammer": {
    "num_procs": i("SPAM_NUM_PROCS"),
    "qps_per_proc": i("SPAM_QPS_PER"),
    "qps_total": i("SPAM_QPS_TOTAL"),
    "burst_size": i("SPAM_BURST"),
    "barrier_period_ms": i("SPAM_BAR_MS"),
    "duration": s("SPAM_DURATION"),
    "duration_secs": i("SPAM_DURATION_SECS"),
    "workers": i("SPAM_WORKERS"),
    "in_flight_ratio": i("SPAM_IFR"),
    "open_loop": b("SPAM_OPEN_LOOP"),
    "gas_chunk_size": i("SPAM_GAS_CHUNK"),
    "num_validators_to_target": i("SPAM_NUM_VALIDATORS"),
    "offered": i("SPAM_OFFERED"),
  },
  "validator": {
    "max_pending_transactions": i("VAL_MAX_PENDING"),
    "max_pending_local_submissions": i("VAL_SEM_CAP"),
    "graduated_load_shedding_soft_limit_pct": i("VAL_START_PCT"),
    "graduated_load_shedding_saturation_pct": i("VAL_SAT_PCT"),
    "semaphore_shedding_enabled": b("VAL_SEM_SHEDDING"),
  },
  "results": {
    "peak_inflight": i("R_PEAK"),
    "ratio_peak_over_sem": f("R_RATIO"),
    "useful_tps": f("R_USEFUL_TPS"),
    "admit_lat_p50": f("R_ADMIT_LAT_P50"),
    "admit_lat_p99": f("R_ADMIT_LAT_P99"),
    "permit_hold_p50": f("R_PERMIT_HOLD_P50"),
    "permit_hold_p99": f("R_PERMIT_HOLD_P99"),
    "permit_wait_p50": f("R_PERMIT_WAIT_P50"),
    "permit_wait_p99": f("R_PERMIT_WAIT_P99"),
    "pre_acquire_p50": f("R_PRE_ACQUIRE_P50"),
    "pre_acquire_p99": f("R_PRE_ACQUIRE_P99"),
    "shed_pct_avg": f("R_SHED_PCT_AVG"),
    "shed_pct_max": f("R_SHED_PCT_MAX"),
    "inflight_stddev": f("R_INFLIGHT_STDDEV"),
    "inflight_mean": f("R_INFLIGHT_MEAN"),
    "saturation_75pct": f("R_SATURATION_75PCT"),
    "consensus_lat_p50": f("R_CONSENSUS_LAT_P50"),
    "consensus_lat_p99": f("R_CONSENSUS_LAT_P99"),
    "spammer_success": i("R_SPAMMER_SUCCESS"),
    "spammer_first_pass_pct": f("R_SPAMMER_FP"),
    "reject_grad_preventive": i("R_REJ_PREV"),
    "reject_grad_saturated": i("R_REJ_SAT"),
    "reject_grad_reactive": i("R_REJ_REACT"),
    "reject_max_pending": i("R_REJ_MAX"),
    "reject_semaphore": i("R_REJ_SEM"),
    "exit_codes": ints("R_EXIT_CODES"),
    "exit_codes_ok": i("R_OK")==1,
  },
}
# Derived convenience field: ratio_peak_over_max_pending — the
# experimentally meaningful safety metric for this study. Computed here
# so analysis code does not have to re-derive it.
peak = rec["results"]["peak_inflight"]
maxp = rec["validator"]["max_pending_transactions"]
rec["results"]["ratio_peak_over_max_pending"] = (
    round(peak / maxp, 4) if (peak is not None and maxp) else None
)
print(json.dumps(rec))
' >> "$OUT_JSONL"

  if [ "$failed" -eq 1 ]; then
    echo ">>> RESULT: iter=$i pct=$val_start_pct FAILED"
  else
    echo ">>> RESULT: iter=$i pct=$val_start_pct sat=$val_sat_pct max=$val_max_pending sem=$val_sem_cap sem_shed=$val_sem_shedding"
    echo "    spammer: offered=$SPAMMER_OFFERED success=$spammer_success first_pass=${spammer_fp}%"
    echo "    peak=$peak  ratio=${ratio}×  tps=$useful_tps  rej[prev=$r_prev,sat=$r_grad_sat,react=$r_grad_react,max=$r_max,sem=$r_sem]  hold[p50=$permit_hold_p50,p99=$permit_hold_p99]  wait[p50=$permit_wait_p50,p99=$permit_wait_p99]  pre_acq[p50=$pre_acquire_p50,p99=$pre_acquire_p99]  shed[avg=$shed_pct_avg,max=$shed_pct_max]"
  fi

  # Per-iter cleanup: keep last 2 multi-* dirs, drop older ones.
  ls -dt "$REPO"/runs/multi-* 2>/dev/null | tail -n +3 | xargs -r rm -rf
done

echo
echo "================ DONE $(date -u) ================"
echo "Results: $OUT_JSONL"

# Headline summary grouped by (max_pending, sem_cap, start_pct). Reads
# ALL records in the JSONL (across sweeps and across hosts), so the
# table accumulates as you re-run more arms.
#
# Headline metrics:
#   peak_inflight             (raw queue depth — useful but absolute)
#   ratio_peak_over_max_pending (safety — lower is better)
#   useful_tps                (throughput — higher is better)
echo
echo "=== Per-policy summary from $OUT_JSONL ==="
P="$OUT_JSONL" python3 -c '
import json, os, sys
from collections import defaultdict
from statistics import median, mean

path = os.environ["P"]
groups = defaultdict(list)
with open(path) as f:
  for line in f:
    line = line.strip()
    if not line: continue
    try: r = json.loads(line)
    except: continue
    if r["results"].get("failed", False) or r.get("failed", False):
      continue
    peak = r["results"].get("peak_inflight")
    if peak is None or peak <= 0: continue
    v = r["validator"]
    key = (
      r["host"]["hostname"],
      v.get("max_pending_transactions", "?"),
      v.get("max_pending_local_submissions", "?"),
      v.get("graduated_load_shedding_soft_limit_pct", "?"),
      v.get("graduated_load_shedding_saturation_pct", "?"),
    )
    groups[key].append({
      "peak": peak,
      "ratio_cap": r["results"].get("ratio_peak_over_max_pending"),
      "tps": r["results"].get("useful_tps") or 0,
    })

print(f"  {'host':<14} {'max_pend':>9} {'sem':>5} {'pct':>4} {'sat':>4} {'n':>3} {'peak_med':>9} {'peak_max':>9} {'ratio_med':>10} {'ratio_max':>10} {'tps_med':>8} {'tps_max':>8}")
print("  " + "-" * 115)
for key in sorted(groups):
  rows = groups[key]
  peaks = [r["peak"] for r in rows]
  ratios = [r["ratio_cap"] for r in rows if r["ratio_cap"] is not None]
  tps = [r["tps"] for r in rows]
  print(f"  {key[0]:<14} {str(key[1]):>9} {str(key[2]):>5} {str(key[3]):>4} {str(key[4]):>4} {len(rows):>3} {median(peaks):>9.0f} {max(peaks):>9} {median(ratios):>10.3f} {max(ratios):>10.3f} {median(tps):>8.1f} {max(tps):>8.1f}")
'

# -------- Teardown ---------
echo
echo "=== tearing down stacks ==="
echo "  stopping grafana + prometheus..."
(cd "$REPO/dev-tools/grafana-local" && docker compose down 2>&1 | tail -3) || true
echo "  stopping iota private network..."
(cd "$PRIVNET" && docker compose down -v 2>&1 | tail -3) || true
echo "  done."
