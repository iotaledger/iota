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
#   5 compute {0, 50, 100, 200, 500}  (slow::slow(n,n); 0 = no-op floor, ~gas_rounding_step)
# × 3 paths   {f1 fullnode (DIRECT=false), v1 pinned (1 target validator),
#              v48 spread (direct to all 48 validators)}
# × 3 qps     {200, 1000, 2000}                                      = 45 configs.
#
# Labels carry the network size as an -n<N> suffix and pass N to run.sh; the
# current grid runs on a 48-validator network (-n48 / N=48). The same grid can
# be run on another size (e.g. -n4 / N=4) under distinct labels without
# colliding with these results.
#
# Round-robin: each round runs 1 iteration (V1+V2) of every config; ITERS rounds
# total, so each config ends with ITERS iters — interleaved, not config-major. So
# an interrupted run leaves every config with ~equal iters. That's 45 * ITERS full
# experiments — HOURS of wall time at ITERS=5. Per-config console output goes to
# logs/<LABEL>.log (truncated on round 1, appended thereafter); redirecting it also
# makes run.sh non-interactive (no monitoring prompt) and strips ANSI colors, so
# the matrix runs unattended.
#
# Usage:
#   ITERS=5 ./matrix.sh             # run all 45 configs
#   ITERS=3 ./matrix.sh slow100     # only labels containing "slow100" (substring filter)
#
# A config that fails (or is interrupted) does NOT abort the matrix — it's logged
# and the next config runs. Re-running is safe: run.sh's config gate appends more
# iterations to an existing label (same config) rather than overwriting.
#
# Per-config runs skip aggregate/plots (ANALYZE=false): those tools re-read every
# accumulated iteration of a label, so invoking them each round costs
# quadratically over a campaign. One sweep after the last round aggregates +
# plots every label; if the matrix is interrupted before it, run the sweep
# manually (see the bottom of this script).

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ITERS="${ITERS:-5}"
FILTER="${1:-}"
LOGDIR="$SCRIPT_DIR/logs"
mkdir -p "$LOGDIR"

# Node-log compression is CPU-bound and single-threaded under gzip; use pigz
# (parallel gzip, same .gz format) when installed.
GZIP_BIN="$(command -v pigz || echo gzip)"

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
  "slow0-owned-f1-qps200-n48     | WORKLOAD=slow SLOW_N=0 SLOW_SIZE=0 SLOW_SHARED=false TARGET_QPS=200  N=48 DIRECT=false"
  "slow0-owned-v1-qps200-n48     | WORKLOAD=slow SLOW_N=0 SLOW_SIZE=0 SLOW_SHARED=false TARGET_QPS=200  N=48 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow0-owned-v48-qps200-n48    | WORKLOAD=slow SLOW_N=0 SLOW_SIZE=0 SLOW_SHARED=false TARGET_QPS=200  N=48 DIRECT=true NUM_TARGET_VALIDATORS=48"
  "slow0-owned-f1-qps1000-n48    | WORKLOAD=slow SLOW_N=0 SLOW_SIZE=0 SLOW_SHARED=false TARGET_QPS=1000 N=48 DIRECT=false"
  "slow0-owned-v1-qps1000-n48    | WORKLOAD=slow SLOW_N=0 SLOW_SIZE=0 SLOW_SHARED=false TARGET_QPS=1000 N=48 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow0-owned-v48-qps1000-n48   | WORKLOAD=slow SLOW_N=0 SLOW_SIZE=0 SLOW_SHARED=false TARGET_QPS=1000 N=48 DIRECT=true NUM_TARGET_VALIDATORS=48"
  "slow0-owned-f1-qps2000-n48    | WORKLOAD=slow SLOW_N=0 SLOW_SIZE=0 SLOW_SHARED=false TARGET_QPS=2000 N=48 DIRECT=false"
  "slow0-owned-v1-qps2000-n48    | WORKLOAD=slow SLOW_N=0 SLOW_SIZE=0 SLOW_SHARED=false TARGET_QPS=2000 N=48 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow0-owned-v48-qps2000-n48   | WORKLOAD=slow SLOW_N=0 SLOW_SIZE=0 SLOW_SHARED=false TARGET_QPS=2000 N=48 DIRECT=true NUM_TARGET_VALIDATORS=48"
  #
  "slow50-owned-f1-qps200-n48    | WORKLOAD=slow SLOW_N=50 SLOW_SIZE=50 SLOW_SHARED=false TARGET_QPS=200  N=48 DIRECT=false"
  "slow50-owned-v1-qps200-n48    | WORKLOAD=slow SLOW_N=50 SLOW_SIZE=50 SLOW_SHARED=false TARGET_QPS=200  N=48 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow50-owned-v48-qps200-n48   | WORKLOAD=slow SLOW_N=50 SLOW_SIZE=50 SLOW_SHARED=false TARGET_QPS=200  N=48 DIRECT=true NUM_TARGET_VALIDATORS=48"
  "slow50-owned-f1-qps1000-n48   | WORKLOAD=slow SLOW_N=50 SLOW_SIZE=50 SLOW_SHARED=false TARGET_QPS=1000 N=48 DIRECT=false"
  "slow50-owned-v1-qps1000-n48   | WORKLOAD=slow SLOW_N=50 SLOW_SIZE=50 SLOW_SHARED=false TARGET_QPS=1000 N=48 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow50-owned-v48-qps1000-n48  | WORKLOAD=slow SLOW_N=50 SLOW_SIZE=50 SLOW_SHARED=false TARGET_QPS=1000 N=48 DIRECT=true NUM_TARGET_VALIDATORS=48"
  "slow50-owned-f1-qps2000-n48   | WORKLOAD=slow SLOW_N=50 SLOW_SIZE=50 SLOW_SHARED=false TARGET_QPS=2000 N=48 DIRECT=false"
  "slow50-owned-v1-qps2000-n48   | WORKLOAD=slow SLOW_N=50 SLOW_SIZE=50 SLOW_SHARED=false TARGET_QPS=2000 N=48 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow50-owned-v48-qps2000-n48  | WORKLOAD=slow SLOW_N=50 SLOW_SIZE=50 SLOW_SHARED=false TARGET_QPS=2000 N=48 DIRECT=true NUM_TARGET_VALIDATORS=48"
  #
  "slow100-owned-f1-qps200-n48   | WORKLOAD=slow SLOW_N=100 SLOW_SIZE=100 SLOW_SHARED=false TARGET_QPS=200  N=48 DIRECT=false"
  "slow100-owned-v1-qps200-n48   | WORKLOAD=slow SLOW_N=100 SLOW_SIZE=100 SLOW_SHARED=false TARGET_QPS=200  N=48 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow100-owned-v48-qps200-n48  | WORKLOAD=slow SLOW_N=100 SLOW_SIZE=100 SLOW_SHARED=false TARGET_QPS=200  N=48 DIRECT=true NUM_TARGET_VALIDATORS=48"
  "slow100-owned-f1-qps1000-n48  | WORKLOAD=slow SLOW_N=100 SLOW_SIZE=100 SLOW_SHARED=false TARGET_QPS=1000 N=48 DIRECT=false"
  "slow100-owned-v1-qps1000-n48  | WORKLOAD=slow SLOW_N=100 SLOW_SIZE=100 SLOW_SHARED=false TARGET_QPS=1000 N=48 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow100-owned-v48-qps1000-n48 | WORKLOAD=slow SLOW_N=100 SLOW_SIZE=100 SLOW_SHARED=false TARGET_QPS=1000 N=48 DIRECT=true NUM_TARGET_VALIDATORS=48"
  "slow100-owned-f1-qps2000-n48  | WORKLOAD=slow SLOW_N=100 SLOW_SIZE=100 SLOW_SHARED=false TARGET_QPS=2000 N=48 DIRECT=false"
  "slow100-owned-v1-qps2000-n48  | WORKLOAD=slow SLOW_N=100 SLOW_SIZE=100 SLOW_SHARED=false TARGET_QPS=2000 N=48 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow100-owned-v48-qps2000-n48 | WORKLOAD=slow SLOW_N=100 SLOW_SIZE=100 SLOW_SHARED=false TARGET_QPS=2000 N=48 DIRECT=true NUM_TARGET_VALIDATORS=48"
  #
  "slow200-owned-f1-qps200-n48   | WORKLOAD=slow SLOW_N=200 SLOW_SIZE=200 SLOW_SHARED=false TARGET_QPS=200  N=48 DIRECT=false"
  "slow200-owned-v1-qps200-n48   | WORKLOAD=slow SLOW_N=200 SLOW_SIZE=200 SLOW_SHARED=false TARGET_QPS=200  N=48 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow200-owned-v48-qps200-n48  | WORKLOAD=slow SLOW_N=200 SLOW_SIZE=200 SLOW_SHARED=false TARGET_QPS=200  N=48 DIRECT=true NUM_TARGET_VALIDATORS=48"
  "slow200-owned-f1-qps1000-n48  | WORKLOAD=slow SLOW_N=200 SLOW_SIZE=200 SLOW_SHARED=false TARGET_QPS=1000 N=48 DIRECT=false"
  "slow200-owned-v1-qps1000-n48  | WORKLOAD=slow SLOW_N=200 SLOW_SIZE=200 SLOW_SHARED=false TARGET_QPS=1000 N=48 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow200-owned-v48-qps1000-n48 | WORKLOAD=slow SLOW_N=200 SLOW_SIZE=200 SLOW_SHARED=false TARGET_QPS=1000 N=48 DIRECT=true NUM_TARGET_VALIDATORS=48"
  "slow200-owned-f1-qps2000-n48  | WORKLOAD=slow SLOW_N=200 SLOW_SIZE=200 SLOW_SHARED=false TARGET_QPS=2000 N=48 DIRECT=false"
  "slow200-owned-v1-qps2000-n48  | WORKLOAD=slow SLOW_N=200 SLOW_SIZE=200 SLOW_SHARED=false TARGET_QPS=2000 N=48 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow200-owned-v48-qps2000-n48 | WORKLOAD=slow SLOW_N=200 SLOW_SIZE=200 SLOW_SHARED=false TARGET_QPS=2000 N=48 DIRECT=true NUM_TARGET_VALIDATORS=48"
  #
  "slow500-owned-f1-qps200-n48   | WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false TARGET_QPS=200  N=48 DIRECT=false"
  "slow500-owned-v1-qps200-n48   | WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false TARGET_QPS=200  N=48 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow500-owned-v48-qps200-n48  | WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false TARGET_QPS=200  N=48 DIRECT=true NUM_TARGET_VALIDATORS=48"
  "slow500-owned-f1-qps1000-n48  | WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false TARGET_QPS=1000 N=48 DIRECT=false"
  "slow500-owned-v1-qps1000-n48  | WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false TARGET_QPS=1000 N=48 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow500-owned-v48-qps1000-n48 | WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false TARGET_QPS=1000 N=48 DIRECT=true NUM_TARGET_VALIDATORS=48"
  "slow500-owned-f1-qps2000-n48  | WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false TARGET_QPS=2000 N=48 DIRECT=false"
  "slow500-owned-v1-qps2000-n48  | WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false TARGET_QPS=2000 N=48 DIRECT=true NUM_TARGET_VALIDATORS=1"
  "slow500-owned-v48-qps2000-n48 | WORKLOAD=slow SLOW_N=500 SLOW_SIZE=500 SLOW_SHARED=false TARGET_QPS=2000 N=48 DIRECT=true NUM_TARGET_VALIDATORS=48"
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

# Count filter-matching configs up front so the progress display knows the total.
nconf=0
for row in "${configs[@]}"; do
  l="${row%%|*}"
  l="${l// /}"
  [[ -n "$FILTER" && "$l" != *"$FILTER"* ]] && continue
  nconf=$((nconf + 1))
done
total=$((nconf * ITERS))
n=0
ok=0
fail=0
start=$(date +%s)
# Round-robin: each round runs ONE iteration of every config; ITERS rounds total,
# so each config ends with ITERS iters but they are interleaved. An interrupted
# matrix then leaves every config with ~equal iterations, instead of the first
# configs fully done and the last ones with none. (run.sh's config gate appends
# each round's iter to results/<LABEL>/.)
for ((round = 1; round <= ITERS; round++)); do
  echo "########## round $round of $ITERS ##########"
  for row in "${configs[@]}"; do
    label="${row%%|*}"
    label="${label// /}" # strip alignment padding around |
    envs="${row#*|}"
    [[ -n "$FILTER" && "$label" != *"$FILTER"* ]] && continue
    n=$((n + 1))
    log="$LOGDIR/$label.log"
    # Fresh per-config log on this invocation's first round, then append rounds 2..N.
    [[ $round -eq 1 ]] && : >"$log"
    echo "[$(date +%H:%M:%S)] ($n/$total) round $round  $label  -> logs/$label.log"
    echo "===== round $round =====" >>"$log"
    # shellcheck disable=SC2086  # $envs is intentionally word-split into KEY=VAL args
    if env LABEL="$label" ITERS=1 ANALYZE=false $envs "$SCRIPT_DIR/run.sh" >>"$log" 2>&1; then
      echo "    ✓ done"
      ok=$((ok + 1))
    else
      rc=$?
      echo "    ✗ FAILED (exit $rc) — tail $log"
      fail=$((fail + 1))
    fi
    # Compress the node logs this iteration captured (gzip ≈10:1) so a long
    # campaign does not fill the disk. _state.log/_crash.log stay uncompressed —
    # the crash scan reads them; the analysis tooling never reads node logs.
    sudo find "$SCRIPT_DIR/results/$label" -path '*node-logs/*.log' \
      ! -name '_state.log' ! -name '_crash.log' -exec "$GZIP_BIN" -f {} + 2>/dev/null
  done
done

# Aggregate + plot every label ONCE, now that all rounds are in (per-round
# analysis was skipped via ANALYZE=false above). If the matrix was interrupted
# before reaching this sweep, run it manually per label:
#   python3 aggregate.py results/<LABEL> results/<LABEL>/summary.md
#   .venv/bin/python plot.py --label <LABEL>
echo
echo "########## aggregate + plots (all labels) ##########"
VENV_PY="$SCRIPT_DIR/.venv/bin/python"
[[ -x "$VENV_PY" ]] ||
  echo "venv not found ($VENV_PY) — plots will be skipped (pip install matplotlib numpy)"
for row in "${configs[@]}"; do
  label="${row%%|*}"
  label="${label// /}"
  [[ -n "$FILTER" && "$label" != *"$FILTER"* ]] && continue
  [[ -d "$SCRIPT_DIR/results/$label" ]] || continue
  echo "[$(date +%H:%M:%S)] $label"
  python3 "$SCRIPT_DIR/aggregate.py" "$SCRIPT_DIR/results/$label" "$SCRIPT_DIR/results/$label/summary.md" ||
    echo "    ✗ aggregate.py failed"
  if [[ -x "$VENV_PY" ]]; then
    "$VENV_PY" "$SCRIPT_DIR/plot.py" --label "$label" ||
      echo "    ✗ plot.py failed"
  fi
done

mins=$((($(date +%s) - start) / 60))
echo
echo "matrix complete: $ok ok, $fail failed (of $n) in ${mins}m"
echo "results -> results/<LABEL>/  (summary.md + plots/ per label)"
