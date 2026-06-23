#!/usr/bin/env bash
#
# matrix.sh — run the H1 config matrix (workload × submission-path × TARGET_QPS),
# ITERS iterations each, as labeled experiments under results/<LABEL>/.
#
#   3 workloads {owned, shared cnt2, slow 500x500}
# × 2 paths     {fullnode (DIRECT=false), pinned (DIRECT=true, 1 target validator)}
# × 3 qps       {200, 1000, 2000}                                      = 18 configs,
# plus 6 slow-owned configs (slow compute, owned-only / no shared object) = 24.
#
# Each run.sh invocation does ITERS iterations (V1+V2 per iter), so this is
# 24 * ITERS full experiments — HOURS of wall time at ITERS=5. Per-config console
# output goes to logs/<LABEL>.log; redirecting it also makes run.sh non-interactive
# (no monitoring prompt) and strips ANSI colors, so the matrix runs unattended.
#
# Usage:
#   ITERS=5 ./matrix.sh             # run all 18 configs
#   ITERS=3 ./matrix.sh slow        # only labels containing "slow" (substring filter)
#
# A config that fails (or is interrupted) does NOT abort the matrix — it's logged
# and the next config runs. Re-running is safe: run.sh's config gate appends more
# iterations to an existing label (same config) rather than overwriting.
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ITERS="${ITERS:-5}"
FILTER="${1:-}"
LOGDIR="$SCRIPT_DIR/logs"
mkdir -p "$LOGDIR"

# When launched detached (nohup / output not a terminal), send our own console
# output to logs/_matrix.log instead of relying on an outer `> logs/_matrix.log`
# redirect — that redirect is opened by the shell before this script's mkdir runs,
# so it would fail if logs/ didn't exist yet. Now `nohup ./matrix.sh &` just works.
# (Per-config detail still goes to logs/<LABEL>.log.)
if [[ ! -t 1 ]]; then
  exec >"$LOGDIR/_matrix.log" 2>&1
fi

# "LABEL | env assignments passed to run.sh". qps batches: 200, then 1000, then 2000.
configs=(
  "owned-fn-200|WORKLOAD=owned DIRECT=false TARGET_QPS=200"
  "owned-pin-200|WORKLOAD=owned DIRECT=true NUM_TARGET_VALIDATORS=1 TARGET_QPS=200"
  "shared2-fn-200|WORKLOAD=shared NUM_SHARED_COUNTERS=2 DIRECT=false TARGET_QPS=200"
  "shared2-pin-200|WORKLOAD=shared NUM_SHARED_COUNTERS=2 DIRECT=true NUM_TARGET_VALIDATORS=1 TARGET_QPS=200"
  "slow-fn-200|WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 DIRECT=false TARGET_QPS=200"
  "slow-pin-200|WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 DIRECT=true NUM_TARGET_VALIDATORS=1 TARGET_QPS=200"
  "owned-fn-1000|WORKLOAD=owned DIRECT=false TARGET_QPS=1000"
  "owned-pin-1000|WORKLOAD=owned DIRECT=true NUM_TARGET_VALIDATORS=1 TARGET_QPS=1000"
  "shared2-fn-1000|WORKLOAD=shared NUM_SHARED_COUNTERS=2 DIRECT=false TARGET_QPS=1000"
  "shared2-pin-1000|WORKLOAD=shared NUM_SHARED_COUNTERS=2 DIRECT=true NUM_TARGET_VALIDATORS=1 TARGET_QPS=1000"
  "slow-fn-1000|WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 DIRECT=false TARGET_QPS=1000"
  "slow-pin-1000|WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 DIRECT=true NUM_TARGET_VALIDATORS=1 TARGET_QPS=1000"
  "owned-fn-2000|WORKLOAD=owned DIRECT=false TARGET_QPS=2000"
  "owned-pin-2000|WORKLOAD=owned DIRECT=true NUM_TARGET_VALIDATORS=1 TARGET_QPS=2000"
  "shared2-fn-2000|WORKLOAD=shared NUM_SHARED_COUNTERS=2 DIRECT=false TARGET_QPS=2000"
  "shared2-pin-2000|WORKLOAD=shared NUM_SHARED_COUNTERS=2 DIRECT=true NUM_TARGET_VALIDATORS=1 TARGET_QPS=2000"
  "slow-fn-2000|WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 DIRECT=false TARGET_QPS=2000"
  "slow-pin-2000|WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 DIRECT=true NUM_TARGET_VALIDATORS=1 TARGET_QPS=2000"
  # slow-owned: same heavy compute (slow::slow(500,500)) but SLOW_SHARED=false, so
  # the tx is owned-object-only — no shared-object congestion. Isolates attestation
  # dry-run cost on expensive txs, with no congestion-control confound.
  "slow-owned-fn-200|WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false DIRECT=false TARGET_QPS=200"
  "slow-owned-pin-200|WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false DIRECT=true NUM_TARGET_VALIDATORS=1 TARGET_QPS=200"
  "slow-owned-fn-1000|WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false DIRECT=false TARGET_QPS=1000"
  "slow-owned-pin-1000|WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false DIRECT=true NUM_TARGET_VALIDATORS=1 TARGET_QPS=1000"
  "slow-owned-fn-2000|WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false DIRECT=false TARGET_QPS=2000"
  "slow-owned-pin-2000|WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false DIRECT=true NUM_TARGET_VALIDATORS=1 TARGET_QPS=2000"
)

# Cache sudo up front (run.sh uses sudo per iteration) and keep it alive for the
# whole matrix — otherwise creds expire mid-run and sudo blocks on /dev/tty.
sudo -v || {
  echo "matrix.sh: need sudo (run.sh uses it for cleanup/bootstrap)"
  exit 1
}
(while true; do
  sudo -n true
  sleep 60
  kill -0 "$$" 2>/dev/null || exit
done) &
trap 'kill %1 2>/dev/null' EXIT

total=${#configs[@]}
n=0
ok=0
fail=0
start=$(date +%s)
for row in "${configs[@]}"; do
  label="${row%%|*}"
  envs="${row#*|}"
  [[ -n "$FILTER" && "$label" != *"$FILTER"* ]] && continue
  n=$((n + 1))
  log="$LOGDIR/$label.log"
  echo "[$(date +%H:%M:%S)] ($n) $label  ITERS=$ITERS  -> logs/$label.log"
  # shellcheck disable=SC2086  # $envs is intentionally word-split into KEY=VAL args
  if env LABEL="$label" ITERS="$ITERS" $envs "$SCRIPT_DIR/run.sh" >"$log" 2>&1; then
    echo "    ✓ done"
    ok=$((ok + 1))
  else
    rc=$?
    echo "    ✗ FAILED (exit $rc) — tail $log"
    fail=$((fail + 1))
  fi
done

mins=$((($(date +%s) - start) / 60))
echo
echo "matrix complete: $ok ok, $fail failed (of $n) in ${mins}m"
echo "results -> results/<LABEL>/  (summary.md + plots/ per label)"
