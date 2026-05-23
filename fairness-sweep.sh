#!/usr/bin/env bash
# fairness-sweep.sh — anti-spam fairness experiment (Experiment 1 from
# graduated-benefits.md). Runs a spammer pool + a low-rate honest pool
# concurrently against the same validator via stress-multi.sh's
# HONEST_PROC_COUNT mode. Records per-pool first-pass accept rate so the
# binary FCFS-vs-graduated hash-based comparison is directly visible.
#
# Usage:
#   ./fairness-sweep.sh                          # use current yaml pct
#   START_PCT=50 ./fairness-sweep.sh             # patch yaml to 50 first
#   ITERS=30 START_PCT=50 ./fairness-sweep.sh    # 30 iters at pct=50
#   for p in 100 50 25 10 2; do
#     START_PCT=$p ITERS=20 ./fairness-sweep.sh
#   done                                          # full pct sweep
#
# Output:
#   fairness-sweep.csv  — one row per iter (appends)
#   fairness-sweep.log  — full sweep log
#
# Each row records both pools' offered transactions (derived from
# QPS_per_proc × DURATION × proc_count) and the first-pass accept rate
# (success / offered). Stress.rs's own num_success_txes is post-retry,
# so accept rates close to 100% via the stress-multi.sh summary are
# misleading. The first-pass rate computed here is the fairness metric
# that distinguishes binary FCFS (spammer-dominated) from graduated
# hash-based (per-tx equal probability).
set -uo pipefail

# CRITICAL: the validator-side white-flag override needs these env vars at
# `docker compose up` time. They must be set before run.sh starts validators.
export IOTA_PROTOCOL_CONFIG_OVERRIDE_ENABLE=1
export IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_WHITE_FLAG_FLOW=true

ITERS="${ITERS:-10}"
START_PCT="${START_PCT:-}"

# Pool config. Honest defaults: 1 process at 50 QPS, no burst, no
# periodic barrier — a low-rate steady-state submitter that should be
# the "victim" under binary FCFS unfairness.
#
# Default NUM_PROCS=25 = 24 spammer + 1 honest. This matches
# burst-sweep.sh's 24-spammer baseline so spam pressure is identical
# across the two experiments (otherwise the honest pool would steal
# one spam slot and we'd be comparing apples to oranges).
NUM_PROCS="${NUM_PROCS:-25}"
HONEST_PROC_COUNT="${HONEST_PROC_COUNT:-1}"
HONEST_QPS_PER_PROC="${HONEST_QPS_PER_PROC:-50}"
HONEST_BURST_SIZE="${HONEST_BURST_SIZE:-1}"
HONEST_BARRIER_PERIOD_MS="${HONEST_BARRIER_PERIOD_MS:-0}"
HONEST_IFR="${HONEST_IFR:-4}"
HONEST_WORKERS="${HONEST_WORKERS:-4}"

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
# more than per-tx correctness. Honest pool is always closed-loop.
OPEN_LOOP="${OPEN_LOOP:-false}"
BARRIER_PERIOD_MS="${BARRIER_PERIOD_MS:-500}"
GAS_CHUNK_SIZE="${GAS_CHUNK_SIZE:-500}"
NUM_VALIDATORS_TO_TARGET="${NUM_VALIDATORS_TO_TARGET:-1}"

OUT_CSV="fairness-sweep.csv"
OUT_LOG="fairness-sweep.log"
PRIVNET=/home/roman/IOTA/iotaledger/iota/dev-tools/iota-private-network
REPO=/home/roman/IOTA/iotaledger/iota
YAML_CFG="$PRIVNET/configs/validator-common.yaml"

# Derived
DURATION_SECS=$(echo "$DURATION" | sed 's/s$//')
N_SPAMMER=$((NUM_PROCS - HONEST_PROC_COUNT))
QPS_PER_SPAMMER=$((QPS_TOTAL / N_SPAMMER))
# Spammer "offered" includes the burst at t=0 plus QPS-paced submissions
# over DURATION_SECS. Honest is QPS-paced from start. Both ignore retries
# (which inflate stress.rs's view but not what the client originally
# intended to submit).
SPAMMER_OFFERED=$((BURST_SIZE * N_SPAMMER + QPS_PER_SPAMMER * DURATION_SECS * N_SPAMMER))
HONEST_OFFERED=$((HONEST_QPS_PER_PROC * DURATION_SECS * HONEST_PROC_COUNT))

# CSV header (only if new file)
[ -f "$OUT_CSV" ] || echo "iso_time,iter,start_pct,duration_secs,spammer_proc_count,spammer_qps_per_proc,spammer_burst,spammer_offered,spammer_success,spammer_first_pass_pct,honest_proc_count,honest_qps_per_proc,honest_offered,honest_success,honest_first_pass_pct,peak_inflight,ratio,reject_grad_preventive,reject_grad_reactive,reject_max_pending,reject_semaphore,useful_tps,admit_lat_p99,exit_codes_ok" > "$OUT_CSV"

exec >> "$OUT_LOG" 2>&1

echo "================ fairness-sweep $(date -u) ================"
echo "config: NUM_PROCS=$NUM_PROCS HONEST_PROC_COUNT=$HONEST_PROC_COUNT"
echo "        spammer: N=$N_SPAMMER QPS_per=$QPS_PER_SPAMMER BURST=$BURST_SIZE BAR=${BARRIER_PERIOD_MS}ms"
echo "        honest:  N=$HONEST_PROC_COUNT QPS_per=$HONEST_QPS_PER_PROC BURST=$HONEST_BURST_SIZE BAR=${HONEST_BARRIER_PERIOD_MS}ms"
echo "        offered per iter: spammer=$SPAMMER_OFFERED  honest=$HONEST_OFFERED"
echo

# Optional yaml patch (same mechanism as burst-sweep.sh).
if [ -n "$START_PCT" ]; then
  if ! [[ "$START_PCT" =~ ^[0-9]+$ ]] || [ "$START_PCT" -gt 100 ]; then
    echo "Error: START_PCT must be an integer in [0, 100], got '$START_PCT'" >&2
    exit 1
  fi
  sed -i -E "s/^([[:space:]]*graduated-load-shedding-soft-limit-pct:[[:space:]]*).*/\1${START_PCT}/" "$YAML_CFG"
  ACTUAL_PCT=$(grep -E '^[[:space:]]*graduated-load-shedding-soft-limit-pct:' \
    "$YAML_CFG" | awk -F: '{print $2}' | xargs)
  if [ "$ACTUAL_PCT" != "$START_PCT" ]; then
    echo "Error: yaml patch did not stick (asked for $START_PCT, found '$ACTUAL_PCT')" >&2
    exit 1
  fi
  echo "=> Patched $YAML_CFG: graduated-load-shedding-soft-limit-pct = $START_PCT"
fi

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
echo "=== all systems up — starting fairness sweep (ITERS=$ITERS) ==="
echo

# -------- Iteration loop ---------
for i in $(seq 1 $ITERS); do
  echo
  echo "=================================================="
  echo "[fairness iter=$i/$ITERS  pct=${START_PCT:-(current yaml)}]  $(date -u +%H:%M:%S)"
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

  # Launch stress-multi.sh with both pools.
  NUM_PROCS="$NUM_PROCS" \
  HONEST_PROC_COUNT="$HONEST_PROC_COUNT" \
  HONEST_QPS_PER_PROC="$HONEST_QPS_PER_PROC" \
  HONEST_BURST_SIZE="$HONEST_BURST_SIZE" \
  HONEST_BARRIER_PERIOD_MS="$HONEST_BARRIER_PERIOD_MS" \
  HONEST_IFR="$HONEST_IFR" \
  HONEST_WORKERS="$HONEST_WORKERS" \
  NUM_VALIDATORS_TO_TARGET="$NUM_VALIDATORS_TO_TARGET" \
  QPS_TOTAL="$QPS_TOTAL" \
  DURATION="$DURATION" \
  WORKERS="$WORKERS" \
  IN_FLIGHT_RATIO="$IN_FLIGHT_RATIO" \
  BURST_SIZE="$BURST_SIZE" \
  OPEN_LOOP="$OPEN_LOOP" \
  BARRIER_PERIOD_MS="$BARRIER_PERIOD_MS" \
  GAS_CHUNK_SIZE="$GAS_CHUNK_SIZE" \
  ./stress-multi.sh 2>&1 | tail -50 | tee "$REPO/runs/fairness-iter.log"

  # Parse per-iter summary.
  latest=$(ls -td "$REPO"/runs/multi-*/ | head -1)
  if [ -f "$latest/summary.txt" ]; then
    peak=$(grep '^peak inflight:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    ratio=$(grep '^ratio:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs | sed 's/×//')
    exits=$(grep '^exit codes:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    r_prev=$(grep '^reject_grad_preventive:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    r_grad_react=$(grep '^reject_grad_reactive:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    r_max=$(grep '^reject_max_pending:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    r_sem=$(grep '^reject_semaphore:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    useful_tps=$(grep '^useful_tps:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    admit_p99=$(grep '^admit_lat_p99:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    spammer_success=$(grep '^spammer_success:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    honest_success=$(grep '^honest_success:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)

    # Read current pct from yaml (records what was actually deployed for
    # this row, in case it differs from START_PCT for any reason).
    current_pct=$(grep -E '^[[:space:]]*graduated-load-shedding-soft-limit-pct:' \
      "$YAML_CFG" 2>/dev/null | awk -F: '{print $2}' | xargs)

    # Defaults
    : "${peak:=0}"; : "${ratio:=0}"; : "${r_prev:=0}"; : "${r_grad_react:=0}"
    : "${r_max:=0}"; : "${r_sem:=0}"; : "${useful_tps:=0}"; : "${admit_p99:=0}"
    : "${spammer_success:=0}"; : "${honest_success:=0}"; : "${current_pct:=?}"

    ok=$(echo "$exits" | awk '{for(j=1;j<=NF;j++) if($j!="0"){print 0; exit} print 1}')

    # First-pass accept rate = success / offered. Offered counts were
    # pre-computed at the top of this script. NaN-safe via awk.
    spammer_fp=$(awk -v s="$spammer_success" -v o="$SPAMMER_OFFERED" \
      'BEGIN{if(o>0) printf "%.4f", 100.0*s/o; else print 0}')
    honest_fp=$(awk -v s="$honest_success" -v o="$HONEST_OFFERED" \
      'BEGIN{if(o>0) printf "%.4f", 100.0*s/o; else print 0}')

    iso=$(basename "$latest" | sed 's/multi-//')
    echo "$iso,$i,$current_pct,$DURATION_SECS,$N_SPAMMER,$QPS_PER_SPAMMER,$BURST_SIZE,$SPAMMER_OFFERED,$spammer_success,$spammer_fp,$HONEST_PROC_COUNT,$HONEST_QPS_PER_PROC,$HONEST_OFFERED,$honest_success,$honest_fp,$peak,$ratio,$r_prev,$r_grad_react,$r_max,$r_sem,$useful_tps,$admit_p99,$ok" >> "$OUT_CSV"

    echo ">>> RESULT: iter=$i pct=$current_pct"
    echo "    spammer: offered=$SPAMMER_OFFERED success=$spammer_success first_pass=${spammer_fp}%"
    echo "    honest:  offered=$HONEST_OFFERED success=$honest_success first_pass=${honest_fp}%"
    echo "    peak=$peak  ratio=${ratio}×  rej[prev=$r_prev,grad_reactive=$r_grad_react,max=$r_max,sem=$r_sem]"
  else
    iso=$(basename "$latest" 2>/dev/null | sed 's/multi-//' || echo "?")
    current_pct=$(grep -E '^[[:space:]]*graduated-load-shedding-soft-limit-pct:' \
      "$YAML_CFG" 2>/dev/null | awk -F: '{print $2}' | xargs)
    : "${current_pct:=?}"
    echo "$iso,$i,$current_pct,$DURATION_SECS,$N_SPAMMER,$QPS_PER_SPAMMER,$BURST_SIZE,$SPAMMER_OFFERED,FAIL,FAIL,$HONEST_PROC_COUNT,$HONEST_QPS_PER_PROC,$HONEST_OFFERED,FAIL,FAIL,FAIL,FAIL,FAIL,FAIL,FAIL,FAIL,FAIL,FAIL,0" >> "$OUT_CSV"
    echo ">>> RESULT: iter=$i pct=$current_pct FAILED"
  fi

  # Per-iter cleanup: keep last 2 multi-* dirs, drop older ones.
  ls -dt "$REPO"/runs/multi-* 2>/dev/null | tail -n +3 | xargs -r rm -rf
done

echo
echo "================ DONE $(date -u) ================"
echo "Results: $OUT_CSV"

# Per-pct quick stats across iters at the deployed pct value.
echo
echo "=== Per-pct fairness stats from $OUT_CSV ==="
awk -F, 'NR>1 && $9 != "FAIL" {
  pct = $3
  n[pct]++
  spam_fp_sum[pct] += $10
  honest_fp_sum[pct] += $15
  if ($10 > spam_fp_max[pct]) spam_fp_max[pct] = $10
  if ($15 > honest_fp_max[pct]) honest_fp_max[pct] = $15
} END {
  printf "  %-6s %-4s %-12s %-12s %-12s %-12s\n", "pct", "n", "spam_fp_avg", "spam_fp_max", "honest_fp_avg", "honest_fp_max"
  echo_sep = "  --------------------------------------------------------------"
  print echo_sep
  for (p in n) {
    printf "  %-6s %-4d %-12.4f %-12.4f %-12.4f %-12.4f\n", \
      p, n[p], spam_fp_sum[p]/n[p], spam_fp_max[p], honest_fp_sum[p]/n[p], honest_fp_max[p]
  }
}' "$OUT_CSV" | sort

# -------- Teardown ---------
echo
echo "=== tearing down stacks ==="
echo "  stopping grafana + prometheus..."
(cd "$REPO/dev-tools/grafana-local" && docker compose down 2>&1 | tail -3) || true
echo "  stopping iota private network..."
(cd "$PRIVNET" && docker compose down -v 2>&1 | tail -3) || true
echo "  done."
