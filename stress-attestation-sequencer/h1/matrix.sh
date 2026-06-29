#!/usr/bin/env bash
#
# matrix.sh — run the H1 attestation-overhead matrix (V1 vs V2), ITERS iterations
# each, as labeled experiments under results/<LABEL>/.
#
# Workload is slow-owned only: pure owned-object compute (no shared object, so no
# congestion/cancellation to confound the V1-vs-V2 delta). Each tx calls
# slow::slow(n, size) with n == size, sweeping per-tx compute — which is exactly
# what the V2 pre-consensus attestation dry-run cost scales with.
#
#   4 compute {0, 50, 100, 500}  (slow::slow(n,n); 0 = no-op floor, ~gas_rounding_step)
# × 2 paths   {fullnode (DIRECT=false), pinned (DIRECT=true, 1 target validator)}
# × 3 qps     {200, 1000, 2000}                                      = 24 configs.
#
# Each run.sh invocation does ITERS iterations (V1+V2 per iter), so this is
# 24 * ITERS full experiments — HOURS of wall time at ITERS=5. Per-config console
# output goes to logs/<LABEL>.log; redirecting it also makes run.sh non-interactive
# (no monitoring prompt) and strips ANSI colors, so the matrix runs unattended.
#
# Usage:
#   ITERS=5 ./matrix.sh             # run all 24 configs
#   ITERS=3 ./matrix.sh slow100     # only labels containing "slow100" (substring filter)
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

# "LABEL | env assignments passed to run.sh"
configs=(
  "slow0-owned-f-qps200  | WORKLOAD=slow SLOW_N=0 SLOW_SIZE=0 SLOW_SHARED=false TARGET_QPS=200  DIRECT=false"
  "slow0-owned-v-qps200  | WORKLOAD=slow SLOW_N=0 SLOW_SIZE=0 SLOW_SHARED=false TARGET_QPS=200  DIRECT=true  NUM_TARGET_VALIDATORS=1"
  "slow0-owned-f-qps1000 | WORKLOAD=slow SLOW_N=0 SLOW_SIZE=0 SLOW_SHARED=false TARGET_QPS=1000 DIRECT=false"
  "slow0-owned-v-qps1000 | WORKLOAD=slow SLOW_N=0 SLOW_SIZE=0 SLOW_SHARED=false TARGET_QPS=1000 DIRECT=true  NUM_TARGET_VALIDATORS=1"
  "slow0-owned-f-qps2000 | WORKLOAD=slow SLOW_N=0 SLOW_SIZE=0 SLOW_SHARED=false TARGET_QPS=2000 DIRECT=false"
  "slow0-owned-v-qps2000 | WORKLOAD=slow SLOW_N=0 SLOW_SIZE=0 SLOW_SHARED=false TARGET_QPS=2000 DIRECT=true  NUM_TARGET_VALIDATORS=1"
  #
  "slow50-owned-f-qps200  | WORKLOAD=slow SLOW_N=50 SLOW_SIZE=50 SLOW_SHARED=false TARGET_QPS=200  DIRECT=false"
  "slow50-owned-v-qps200  | WORKLOAD=slow SLOW_N=50 SLOW_SIZE=50 SLOW_SHARED=false TARGET_QPS=200  DIRECT=true  NUM_TARGET_VALIDATORS=1"
  "slow50-owned-f-qps1000 | WORKLOAD=slow SLOW_N=50 SLOW_SIZE=50 SLOW_SHARED=false TARGET_QPS=1000 DIRECT=false"
  "slow50-owned-v-qps1000 | WORKLOAD=slow SLOW_N=50 SLOW_SIZE=50 SLOW_SHARED=false TARGET_QPS=1000 DIRECT=true  NUM_TARGET_VALIDATORS=1"
  "slow50-owned-f-qps2000 | WORKLOAD=slow SLOW_N=50 SLOW_SIZE=50 SLOW_SHARED=false TARGET_QPS=2000 DIRECT=false"
  "slow50-owned-v-qps2000 | WORKLOAD=slow SLOW_N=50 SLOW_SIZE=50 SLOW_SHARED=false TARGET_QPS=2000 DIRECT=true  NUM_TARGET_VALIDATORS=1"
  #
  "slow100-owned-f-qps200  | WORKLOAD=slow SLOW_N=100 SLOW_SIZE=100 SLOW_SHARED=false TARGET_QPS=200  DIRECT=false"
  "slow100-owned-v-qps200  | WORKLOAD=slow SLOW_N=100 SLOW_SIZE=100 SLOW_SHARED=false TARGET_QPS=200  DIRECT=true  NUM_TARGET_VALIDATORS=1"
  "slow100-owned-f-qps1000 | WORKLOAD=slow SLOW_N=100 SLOW_SIZE=100 SLOW_SHARED=false TARGET_QPS=1000 DIRECT=false"
  "slow100-owned-v-qps1000 | WORKLOAD=slow SLOW_N=100 SLOW_SIZE=100 SLOW_SHARED=false TARGET_QPS=1000 DIRECT=true  NUM_TARGET_VALIDATORS=1"
  "slow100-owned-f-qps2000 | WORKLOAD=slow SLOW_N=100 SLOW_SIZE=100 SLOW_SHARED=false TARGET_QPS=2000 DIRECT=false"
  "slow100-owned-v-qps2000 | WORKLOAD=slow SLOW_N=100 SLOW_SIZE=100 SLOW_SHARED=false TARGET_QPS=2000 DIRECT=true  NUM_TARGET_VALIDATORS=1"
  #
  "slow500-owned-f-qps200  | WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false TARGET_QPS=200  DIRECT=false"
  "slow500-owned-v-qps200  | WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false TARGET_QPS=200  DIRECT=true  NUM_TARGET_VALIDATORS=1"
  "slow500-owned-f-qps1000 | WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false TARGET_QPS=1000 DIRECT=false"
  "slow500-owned-v-qps1000 | WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false TARGET_QPS=1000 DIRECT=true  NUM_TARGET_VALIDATORS=1"
  "slow500-owned-f-qps2000 | WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false TARGET_QPS=2000 DIRECT=false"
  "slow500-owned-v-qps2000 | WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false TARGET_QPS=2000 DIRECT=true  NUM_TARGET_VALIDATORS=1"
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
  label="${label// /}" # strip alignment padding around |
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
