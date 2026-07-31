#!/usr/bin/env bash
#
# matrix.sh — run the H2 congestion-mode matrix (Run A TotalTxCount vs Run B
# TotalComputationUnits), ITERS iterations each, as labeled experiments under
# results/<LABEL>/.
#
# Every cell runs both modes on the same load with attestation ON in both, so the
# mode is the only thing that differs. Both runs have to be able to admit the same
# amount of work, which means different numeric limits: Run A gets a transaction
# count per object per commit (LIMIT_A base, OVERSHOOT_A burst) and Run B gets that
# count multiplied by the workload's attested computation units per transaction
# (run.sh computes it from CU_PER_TX).
#
# The burst is off in every cell (OVERSHOOT_A=0, and OVERSHOOT_B follows from it),
# so each arm is described by one number: the base limit per object per commit.
# Nothing exceeds it and no debt is carried between commits, which keeps the two
# arms comparable — with a burst, the debt would be carried in transactions on one
# side and in computation units on the other. Once a base limit is settled, re-run
# it with OVERSHOOT_A=10*LIMIT_A to see what the burst adds.
#
# LIMIT_A is spelled out per cell, because the value chosen decides what the cell
# measures. 10 is production's base limit and the reference, but 10 transactions per
# object per commit is likely well below what these four validators can execute, so
# the limit rather than the mode would cap throughput. The two lightest points
# therefore also run at 100 (`lim100` in the label): if throughput moves with the
# limit, the limit was binding; if it does not, execution was.
#
# Every config runs ONE fixed cost, so all the transactions in a run are identical
# and both modes admit the same work once the limits match. A run whose transactions
# differ in cost — where a count limit and a cost limit would admit different
# amounts of work — is out of the grid for now (see README.md).
#
# Both workloads are SHARED-object ones — only those exercise per-object
# congestion control. The `slow` workload publishes ONE `slow::Obj` shared object
# and every transaction takes it as a mutable input, so all of them contend on the
# same object; the workload has no setting for more objects. Transactions on one
# mutable shared object also execute one after another, so the rates are picked per
# cost point (roughly what that point can execute, then a few times more) rather
# than using the same rates everywhere: at 2000 qps the heaviest point would only
# build a backlog the network cannot drain. Those same low rates are why every cell
# submits through the fullnode (run.sh's default) — nothing here comes close to
# saturating it.
#
# The cost points come from the h2 calibration (probe-test.md; size fixed at 100,
# units are the same on any machine), listed with the execution time per transaction
# that decides how fast a single object can drain:
#
#   label    slow_n   product   units/tx   exec ms (WS / EPYC)   rates (qps)
#   cu1k          1       100      1,000      0.23 / 0.55        200 1000 2000
#   cu4k        100    10,000      4,000      2.30 / 9.28        100 500
#   cu16k       200    20,000     16,000      4.27 / 18.78       50 250
#   cu130k      400    40,000    130,000      7.96 / 35.04       25 100
#   cu491k     1000   100,000    491,000     18.81 / 74.62       10 50
#
# cu1k and cu4k run their upper rates twice, once at each base limit, and those four
# cells carry `lim100` in the label.
#
# Those units were measured on OWNED slow transactions; the ones here carry a
# mutable shared input as well, so confirm each point with
# `SLOW_N=<n> SLOW_SIZE=100 SLOW_SHARED=true ./probe.sh` before a long campaign. A
# wrong CU_PER_TX gives the two runs different capacity, and then the cell no longer
# measures the mode.
#
# The plan's W1 (`--shared-counter` on one counter) is not in the grid: with
# NUM_SHARED_COUNTERS=1 every transaction increments the same counter at a cost that
# also lands on the 1,000-unit floor, which is what cu1k already runs. run.sh still
# takes WORKLOAD=shared, so it is there as a second workload to cross-check against
# if the `slow` numbers look surprising.
#
# 15 configs total. Use the substring FILTER to run one point at a time — the full
# grid at ITERS=5 is days of wall time.
#
# Labels carry the network size as an -n4 suffix and pass N=4 to run.sh: 4
# validators are enough, since every validator schedules the same committed order
# through the same per-object limits. The same grid can be run on another size
# under distinct labels without colliding with these results.
#
# Round-robin: each round runs 1 iteration (A+B) of every config; ITERS rounds
# total, so each config ends with ITERS iters — interleaved, not config-major. So
# an interrupted run leaves every config with ~equal iters. That's 15 * ITERS full
# experiments — DAYS of wall time at ITERS=5 for the whole grid, so filter to one
# group unless you mean it. Per-config console output goes to
# logs/<LABEL>.log (truncated on round 1, appended thereafter); redirecting it also
# makes run.sh non-interactive (no monitoring prompt) and strips ANSI colors, so
# the matrix runs unattended.
#
# Usage:
#   ITERS=5 ./matrix.sh             # run all 15 configs
#   ITERS=3 ./matrix.sh lim100      # only the 4 higher-limit configs
#   ITERS=3 ./matrix.sh cu4k-       # one cost point, every rate and limit (4 configs)
#
# A config that fails (or is interrupted) does NOT abort the matrix — it's logged
# and the next config runs. Re-running is safe: run.sh's config gate appends more
# iterations to an existing label (same config) rather than overwriting.
#
# Unlike ../h1/matrix.sh there is no aggregate/plot sweep at the end: H2 has no
# aggregation step yet (see README.md), so a campaign leaves the raw per-run JSONs
# plus the per-config logs.

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

# The repeated env groups, named once so each config row shows what varies.
SLOW1="WORKLOAD=slow SLOW_N=1 SLOW_SIZE=100 SLOW_SHARED=true CU_PER_TX=1000"
SLOW100="WORKLOAD=slow SLOW_N=100 SLOW_SIZE=100 SLOW_SHARED=true CU_PER_TX=4000"
SLOW200="WORKLOAD=slow SLOW_N=200 SLOW_SIZE=100 SLOW_SHARED=true CU_PER_TX=16000"
SLOW400="WORKLOAD=slow SLOW_N=400 SLOW_SIZE=100 SLOW_SHARED=true CU_PER_TX=130000"
SLOW1000="WORKLOAD=slow SLOW_N=1000 SLOW_SIZE=100 SLOW_SHARED=true CU_PER_TX=491000"
# Base limit per object per commit, burst off. 10 is production's base limit.
BASE10="LIMIT_A=10 OVERSHOOT_A=0"
BASE100="LIMIT_A=100 OVERSHOOT_A=0"

# "LABEL | env assignments passed to run.sh"
configs=(
  # One fixed cost per run; with the limits matched, both modes admit the same work.
  "cu1k-qps200-n4     | $SLOW1 $BASE10 TARGET_QPS=200  N=4"
  "cu1k-qps1000-n4    | $SLOW1 $BASE10 TARGET_QPS=1000 N=4"
  "cu1k-qps2000-n4    | $SLOW1 $BASE10 TARGET_QPS=2000 N=4"
  "cu1k-lim100-qps1000-n4 | $SLOW1 $BASE100 TARGET_QPS=1000 N=4"
  "cu1k-lim100-qps2000-n4 | $SLOW1 $BASE100 TARGET_QPS=2000 N=4"
  #
  "cu4k-qps100-n4     | $SLOW100 $BASE10 TARGET_QPS=100 N=4"
  "cu4k-qps500-n4     | $SLOW100 $BASE10 TARGET_QPS=500 N=4"
  "cu4k-lim100-qps100-n4 | $SLOW100 $BASE100 TARGET_QPS=100 N=4"
  "cu4k-lim100-qps500-n4 | $SLOW100 $BASE100 TARGET_QPS=500 N=4"
  #
  "cu16k-qps50-n4     | $SLOW200 $BASE10 TARGET_QPS=50  N=4"
  "cu16k-qps250-n4    | $SLOW200 $BASE10 TARGET_QPS=250 N=4"
  #
  "cu130k-qps25-n4    | $SLOW400 $BASE10 TARGET_QPS=25  N=4"
  "cu130k-qps100-n4   | $SLOW400 $BASE10 TARGET_QPS=100 N=4"
  #
  "cu491k-qps10-n4    | $SLOW1000 $BASE10 TARGET_QPS=10 N=4"
  "cu491k-qps50-n4    | $SLOW1000 $BASE10 TARGET_QPS=50 N=4"
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
    if env LABEL="$label" ITERS=1 $envs "$SCRIPT_DIR/run.sh" >>"$log" 2>&1; then
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

mins=$((($(date +%s) - start) / 60))
echo
echo "matrix complete: $ok ok, $fail failed (of $n) in ${mins}m"
echo "results -> results/<LABEL>/iter-NNN/  (run-a/run-b timeseries JSON + client reports)"
echo "no aggregation step yet — see README.md"
