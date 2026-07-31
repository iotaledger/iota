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
# The grid has two knobs. TARGET_QPS sets how many transactions are available
# per commit, swept 250/500/1000/2000; a limit only binds when demand exceeds what
# it admits, so each cell pairs a limit with a rate high enough to saturate it.
# SLOW_N sets the cost per transaction, which converts a unit limit into a
# transaction count.
#
# Cost points from the h2 calibration (probe-test.md; size fixed at 100). "drains"
# is 1 / execution time — how fast one object can execute that point, since
# transactions on one mutable shared object run one after another:
#
#   point    slow_n   units/tx   exec ms (WS / EPYC)   drains (WS / EPYC)
#   cu1k          1      1,000     0.23 /  0.55        4400 / 1800 per s
#   cu16k       200     16,000     4.27 / 18.78         234 /   53 per s
#   cu491k     1000    491,000    18.81 / 74.62          53 /   13 per s
#
# The limits per point, as units and as the transactions per commit they admit:
#
#   point    limit    tx/commit   note
#   cu1k     5k               5   tighter than Run A
#   cu1k     10k             10   same as Run A
#   cu1k     100k           100   needs 2000 qps to saturate
#   cu16k    100k             6   tighter than Run A
#   cu16k    160k            10   same as Run A
#   cu16k    500k            31   more than either machine drains
#   cu491k   491k             1   the floor: one transaction per commit
#   cu491k   1m               2
#   cu491k   4910k           10   same as Run A
#   cu491k   50m            100   far more than either machine drains
#
# A limit below one transaction's cost admits nothing at all: the scheduler needs
# `start_time + cost <= limit` with a start time of at least 0, so every transaction
# is deferred each commit and cancelled after max_deferral_rounds. That is why the
# limits per point all start at or above that point's own cost.
#
# The `slow` workload publishes ONE `slow::Obj` and every transaction takes it as a
# mutable input, so all of them contend on the same object.
#
# 19 configs total. Use the substring FILTER to run one cost point, one limit or one
# rate at a time.
#
# Every cell runs on 4 validators; N=4 is not in the label since nothing else is
# planned.
#
# Usage:
#   ITERS=5 ./matrix.sh             # run all 19 configs
#   ITERS=5 ./matrix.sh cu491k      # one cost point, every limit and rate
#   ITERS=5 ./matrix.sh lim4910k    # one limit, its whole rate ladder
#   ITERS=5 ./matrix.sh qps2000     # one rate, every cost point and limit
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
SLOW1="WORKLOAD=slow SLOW_N=1 SLOW_SIZE=100 SLOW_SHARED=true"       # 1,000 units/tx
SLOW200="WORKLOAD=slow SLOW_N=200 SLOW_SIZE=100 SLOW_SHARED=true"   # 16,000 units/tx
SLOW1000="WORKLOAD=slow SLOW_N=1000 SLOW_SIZE=100 SLOW_SHARED=true" # 491,000 units/tx
# Run A's reference in every cell: production's count limit, burst off in both runs.
REF="N=4 LIMIT_A=10 OVERSHOOT_A=0 OVERSHOOT_B=0"

# "LABEL | env assignments passed to run.sh"
configs=(
  # ---- cu1k, 1,000 units/tx. lim10k admits 10/commit, the same as Run A, and gets
  #      the full rate ladder. lim5k halves that; lim100k needs the top rate to bind.
  "cu1k-lim10k-qps250    | $SLOW1 $REF LIMIT_B=10000  TARGET_QPS=250"
  "cu1k-lim10k-qps500    | $SLOW1 $REF LIMIT_B=10000  TARGET_QPS=500"
  "cu1k-lim10k-qps1000   | $SLOW1 $REF LIMIT_B=10000  TARGET_QPS=1000"
  "cu1k-lim10k-qps2000   | $SLOW1 $REF LIMIT_B=10000  TARGET_QPS=2000"
  "cu1k-lim5k-qps250     | $SLOW1 $REF LIMIT_B=5000   TARGET_QPS=250"
  "cu1k-lim100k-qps2000  | $SLOW1 $REF LIMIT_B=100000 TARGET_QPS=2000"
  #
  # ---- cu16k, 16,000 units/tx. lim160k is the same as Run A; lim100k is tighter,
  #      lim500k admits more than either machine can execute.
  "cu16k-lim160k-qps250  | $SLOW200 $REF LIMIT_B=160000 TARGET_QPS=250"
  "cu16k-lim160k-qps500  | $SLOW200 $REF LIMIT_B=160000 TARGET_QPS=500"
  "cu16k-lim160k-qps1000 | $SLOW200 $REF LIMIT_B=160000 TARGET_QPS=1000"
  "cu16k-lim160k-qps2000 | $SLOW200 $REF LIMIT_B=160000 TARGET_QPS=2000"
  "cu16k-lim100k-qps250  | $SLOW200 $REF LIMIT_B=100000 TARGET_QPS=250"
  "cu16k-lim500k-qps1000 | $SLOW200 $REF LIMIT_B=500000 TARGET_QPS=1000"
  #
  # ---- cu491k, 491,000 units/tx. lim4910k is the same as Run A; lim491k is the
  #      floor of one transaction per commit, and lim50m is far past what the
  #      network can execute.
  "cu491k-lim4910k-qps250  | $SLOW1000 $REF LIMIT_B=4910000  TARGET_QPS=250"
  "cu491k-lim4910k-qps500  | $SLOW1000 $REF LIMIT_B=4910000  TARGET_QPS=500"
  "cu491k-lim4910k-qps1000 | $SLOW1000 $REF LIMIT_B=4910000  TARGET_QPS=1000"
  "cu491k-lim4910k-qps2000 | $SLOW1000 $REF LIMIT_B=4910000  TARGET_QPS=2000"
  "cu491k-lim491k-qps250   | $SLOW1000 $REF LIMIT_B=491000   TARGET_QPS=250"
  "cu491k-lim1m-qps250     | $SLOW1000 $REF LIMIT_B=1000000  TARGET_QPS=250"
  "cu491k-lim50m-qps2000   | $SLOW1000 $REF LIMIT_B=50000000 TARGET_QPS=2000"
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
