#!/usr/bin/env bash
#
# matrix.sh — sweep Run B's per-object computation-unit limit against a fixed
# TotalTxCount reference, ITERS iterations each, as labeled experiments under
# results/<LABEL>/.
#
# Adapted from ../h1/matrix.sh.
#
# LIMIT_B (computation units per object per commit) is the swept axis; Run A stays
# at production's LIMIT_A=10 transactions in every cell as the reference. The two
# limits are independent inputs in different units.
#
# The burst is off everywhere (OVERSHOOT_A=0, OVERSHOOT_B=0), so each run is
# described by one number and no debt is carried between commits.
#
# SLOW_N sets the cost per transaction, which converts a unit limit into a
# transaction count. The grid is the same four limits (10k, 20k, 50k and 100k
# units) at every cost point, all at TARGET_QPS=1000, so one limit sweeps the
# cost and one cost point sweeps the limit. A limit binds only while it admits
# less than the rate offers.
#
# Cost points from the h2 calibration (probe-test.md; size fixed at 100). "drains"
# is 1 / execution time — how fast one object can execute that point, since
# transactions on one mutable shared object run one after another:
#
#   point    slow_n   units/tx   exec ms (WS / EPYC)   drains (WS / EPYC)
#   cu1k          1      1,000     0.23 /  0.55        4400 / 1800 per s
#   cu2k         70      2,000     not measured yet
#   cu5k        120      5,000     not measured yet
#   cu10k       160     10,000     not measured yet
#   cu20k       217     20,000     not measured yet
#   cu16k       200     16,000     4.27 / 18.78         234 /   53 per s
#   cu491k     1000    491,000    18.81 / 74.62          53 /   13 per s
#
# units/tx for cu2k..cu20k is the attested cost measured with SLOW_SHARED=true,
# which is the value the scheduler charges. Their execution times are still
# missing: that probe ran with the default per-object limit of 10 units, so
# every shared-input transaction was cancelled instead of executed. cu16k and
# cu491k are kept for reference; no cell uses them.
#
# Transactions per commit each limit admits, which is the limit divided by the
# cost. Run A admits 10 in every cell, so the rung that matches it moves down
# the table as the cost rises:
#
#   point   units/tx   lim10k   lim20k   lim50k   lim100k
#   cu1k       1,000       10       20       50       100
#   cu2k       2,000        5       10       25        50
#   cu5k       5,000        2        4       10        20
#   cu10k     10,000        1        2        5        10
#   cu20k     20,000        0        1        2         5
#
# cu20k at lim10k is the zero above: the limit is below one transaction's cost,
# so that cell admits nothing and every transaction is cancelled.
#
# A limit below one transaction's cost admits nothing at all: the scheduler needs
# `start_time + cost <= limit` with a start time of at least 0, so every transaction
# is deferred each commit and cancelled after max_deferral_rounds. That is why the
# limits per point all start at or above that point's own cost.
#
# The `slow` workload publishes ONE `slow::Obj` and every transaction takes it as a
# mutable input, so all of them contend on the same object.
#
# 20 configs total. Use the substring FILTER to run one cost point or one
# limit at a time.
#
# Every cell runs on 4 validators; N=4 is not in the label since nothing else is
# planned.
#
# Usage:
#   ITERS=5 ./matrix.sh             # run all 20 configs
#   ITERS=5 ./matrix.sh cu10k       # one cost point, its whole limit ladder
#   ITERS=5 ./matrix.sh lim100k     # one limit, every cost point that uses it
#
# A failed config does not abort the matrix. Re-running appends iterations to an
# existing label rather than overwriting (run.sh's config gate).

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ITERS="${ITERS:-5}"
FILTER="${1:-}"
LOGDIR="$SCRIPT_DIR/logs"
mkdir -p "$LOGDIR"

# Node-log compression is CPU-bound and single-threaded under gzip; use pigz
# (parallel gzip, same .gz format) when installed.
GZIP_BIN="$(command -v pigz || echo gzip)"

# Detached runs (nohup, output not a terminal) send this script's console output to
# logs/_matrix.log; per-config detail still goes to logs/<LABEL>.log.
if [[ ! -t 1 ]]; then
  exec >"$LOGDIR/_matrix.log" 2>&1
fi

# The repeated env groups, named once so each config row shows what varies.
SLOW1="WORKLOAD=slow SLOW_N=1 SLOW_SIZE=100 SLOW_SHARED=true"     # 1,000 units/tx
SLOW70="WORKLOAD=slow SLOW_N=70 SLOW_SIZE=100 SLOW_SHARED=true"   # 2,000 units/tx
SLOW120="WORKLOAD=slow SLOW_N=120 SLOW_SIZE=100 SLOW_SHARED=true" # 5,000 units/tx
SLOW160="WORKLOAD=slow SLOW_N=160 SLOW_SIZE=100 SLOW_SHARED=true" # 10,000 units/tx
SLOW217="WORKLOAD=slow SLOW_N=217 SLOW_SIZE=100 SLOW_SHARED=true" # 20,000 units/tx
# Run A's reference in every cell: production's count limit, burst off in both runs.
# The duration is pinned: run.sh's own default is shorter, for ad-hoc test runs.
REF="N=4 RUN_DURATION=60s LIMIT_A=10 OVERSHOOT_A=0 OVERSHOOT_B=0"

# "LABEL | env assignments passed to run.sh"
configs=(
  # ---- cu1k, 1,000 units/tx. A limit ladder at one rate (1000 qps), 10 to 100
  #      transactions per commit. lim10k matches Run A; lim100k admits more than
  #      the rate offers, so the ladder brackets where the limit stops capping
  #      throughput.
  "cu1k-lim10k-qps1000    | $SLOW1 $REF LIMIT_B=10000   TARGET_QPS=1000"
  "cu1k-lim20k-qps1000    | $SLOW1 $REF LIMIT_B=20000   TARGET_QPS=1000"
  "cu1k-lim50k-qps1000    | $SLOW1 $REF LIMIT_B=50000   TARGET_QPS=1000"
  "cu1k-lim100k-qps1000   | $SLOW1 $REF LIMIT_B=100000  TARGET_QPS=1000"
  #
  # ---- cu2k, 2,000 units/tx (slow_n=70). Same limits, so each admits half as
  #      many transactions per commit as at cu1k.
  "cu2k-lim10k-qps1000    | $SLOW70 $REF LIMIT_B=10000   TARGET_QPS=1000"
  "cu2k-lim20k-qps1000    | $SLOW70 $REF LIMIT_B=20000   TARGET_QPS=1000"
  "cu2k-lim50k-qps1000    | $SLOW70 $REF LIMIT_B=50000   TARGET_QPS=1000"
  "cu2k-lim100k-qps1000   | $SLOW70 $REF LIMIT_B=100000  TARGET_QPS=1000"
  #
  # ---- cu5k, 5,000 units/tx (slow_n=120).
  "cu5k-lim10k-qps1000    | $SLOW120 $REF LIMIT_B=10000   TARGET_QPS=1000"
  "cu5k-lim20k-qps1000    | $SLOW120 $REF LIMIT_B=20000   TARGET_QPS=1000"
  "cu5k-lim50k-qps1000    | $SLOW120 $REF LIMIT_B=50000   TARGET_QPS=1000"
  "cu5k-lim100k-qps1000   | $SLOW120 $REF LIMIT_B=100000  TARGET_QPS=1000"
  #
  # ---- cu10k, 10,000 units/tx (slow_n=160). lim10k is the floor here: one
  #      transaction per commit.
  "cu10k-lim10k-qps1000   | $SLOW160 $REF LIMIT_B=10000   TARGET_QPS=1000"
  "cu10k-lim20k-qps1000   | $SLOW160 $REF LIMIT_B=20000   TARGET_QPS=1000"
  "cu10k-lim50k-qps1000   | $SLOW160 $REF LIMIT_B=50000   TARGET_QPS=1000"
  "cu10k-lim100k-qps1000  | $SLOW160 $REF LIMIT_B=100000  TARGET_QPS=1000"
  #
  # ---- cu20k, 20,000 units/tx (slow_n=217). lim10k is BELOW one transaction's
  #      cost, so that cell admits nothing at all (see the note above).
  "cu20k-lim10k-qps1000   | $SLOW217 $REF LIMIT_B=10000   TARGET_QPS=1000"
  "cu20k-lim20k-qps1000   | $SLOW217 $REF LIMIT_B=20000   TARGET_QPS=1000"
  "cu20k-lim50k-qps1000   | $SLOW217 $REF LIMIT_B=50000   TARGET_QPS=1000"
  "cu20k-lim100k-qps1000  | $SLOW217 $REF LIMIT_B=100000  TARGET_QPS=1000"
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
# Round-robin: each round runs ONE iteration of every config, so an interrupted
# matrix leaves every config with about equal iterations.
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
