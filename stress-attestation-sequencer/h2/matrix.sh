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
# transaction count. The limits are geometric rungs from 10k up to 50m — ten
# transactions of the 5,000,000-unit metering ceiling — all at
# TARGET_QPS=1000, so one limit sweeps the cost and one cost point sweeps the
# limit. A limit binds only while it admits less than the rate offers, which
# at ~20 commits/s means up to 50x the point's per-transaction cost; each
# point's ladder stops at its first unbindable rung (or at the 50m ceiling,
# which the most expensive points reach first) and starts one rung below its
# floor, where such a rung exists.
#
# Cost points from the h2 calibration (probe-test.md; size fixed at 100). "drains"
# is 1 / execution time — how fast one object can execute that point, since
# transactions on one mutable shared object run one after another:
#
#   point    slow_n    units/tx   exec ms (WS / EPYC)   drains (WS / EPYC)
#   cu1k          1       1,000     0.23 /  0.55        4400 / 1800 per s
#   cu2k         70       2,000     not measured yet
#   cu5k        120       5,000     not measured yet
#   cu10k       160      10,000     not measured yet
#   cu20k       217      20,000     not measured yet
#   cu50k       267      50,000     not measured yet
#   cu100k      350     100,000     not measured yet
#   cu200k      516     200,000     not measured yet
#   cu500k     1015     500,000     not measured yet
#   cu1m       1848   1,000,000     not measured yet
#   cu2m       3511   2,000,000     not measured yet
#   cu5m       8000   5,000,000     not measured yet (the metering ceiling)
#   cu16k       200      16,000     4.27 / 18.78         234 /   53 per s
#   cu491k     1000     491,000    18.81 / 74.62          53 /   13 per s
#
# units/tx for cu2k..cu5m is the attested cost measured with SLOW_SHARED=true,
# which is the value the scheduler charges. Their execution times are still
# missing: that probe ran with the default per-object limit of 10 units, so
# every shared-input transaction was cancelled instead of executed. cu16k and
# cu491k are kept for reference; no cell uses them.
#
# Transactions per commit each limit admits is the limit divided by the cost
# (both ladders are geometric, so it is 1 at the limit equal to the point's
# own cost — its floor — and 10, matching Run A, at ten times its cost:
# lim10k for cu1k, lim20k for cu2k, ... lim50m for cu5m).
#
# A limit below one transaction's cost admits nothing at all: the scheduler
# needs `start_time + cost <= limit` with a start time of at least 0, so every
# transaction is deferred each commit and cancelled after max_deferral_rounds.
# Each point keeps ONE such rung, the one just below its floor (Run A still
# runs there, and Run B is the shed-everything control); deeper rungs would
# repeat it and were dropped.
#
# The `slow` workload publishes ONE `slow::Obj` and every transaction takes it as a
# mutable input, so all of them contend on the same object.
#
# 80 configs total. Use the substring FILTER to run one cost point or one
# limit at a time.
#
# Every cell runs on 4 validators; N=4 is not in the label since nothing else is
# planned.
#
# Usage:
#   ITERS=5 ./matrix.sh             # run all 80 configs
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
SLOW1="WORKLOAD=slow SLOW_N=1 SLOW_SIZE=100 SLOW_SHARED=true"       # 1,000 units/tx
SLOW70="WORKLOAD=slow SLOW_N=70 SLOW_SIZE=100 SLOW_SHARED=true"     # 2,000 units/tx
SLOW120="WORKLOAD=slow SLOW_N=120 SLOW_SIZE=100 SLOW_SHARED=true"   # 5,000 units/tx
SLOW160="WORKLOAD=slow SLOW_N=160 SLOW_SIZE=100 SLOW_SHARED=true"   # 10,000 units/tx
SLOW217="WORKLOAD=slow SLOW_N=217 SLOW_SIZE=100 SLOW_SHARED=true"   # 20,000 units/tx
SLOW267="WORKLOAD=slow SLOW_N=267 SLOW_SIZE=100 SLOW_SHARED=true"   # 50,000 units/tx
SLOW350="WORKLOAD=slow SLOW_N=350 SLOW_SIZE=100 SLOW_SHARED=true"   # 100,000 units/tx
SLOW516="WORKLOAD=slow SLOW_N=516 SLOW_SIZE=100 SLOW_SHARED=true"   # 200,000 units/tx
SLOW1015="WORKLOAD=slow SLOW_N=1015 SLOW_SIZE=100 SLOW_SHARED=true" # 500,000 units/tx
SLOW1848="WORKLOAD=slow SLOW_N=1848 SLOW_SIZE=100 SLOW_SHARED=true" # 1,000,000 units/tx
SLOW3511="WORKLOAD=slow SLOW_N=3511 SLOW_SIZE=100 SLOW_SHARED=true" # 2,000,000 units/tx
SLOW8000="WORKLOAD=slow SLOW_N=8000 SLOW_SIZE=100 SLOW_SHARED=true" # 5,000,000 units/tx
# Run A's reference in every cell: production's count limit, burst off in both runs.
# The duration is pinned: run.sh's own default is shorter, for ad-hoc test runs.
REF="N=4 RUN_DURATION=60s LIMIT_A=10 OVERSHOOT_A=0 OVERSHOOT_B=0"

# "LABEL | env assignments passed to run.sh"
configs=(
  # ---- cu1k, 1,000 units/tx. 10 to 100 transactions per commit. lim10k
  #      matches Run A; lim50k admits exactly the 1000 qps offered; lim100k
  #      is the unconstrained reference — higher rungs can never bind and
  #      were dropped.
  "cu1k-lim10k-qps1000    | $SLOW1 $REF LIMIT_B=10000    TARGET_QPS=1000"
  "cu1k-lim20k-qps1000    | $SLOW1 $REF LIMIT_B=20000    TARGET_QPS=1000"
  "cu1k-lim50k-qps1000    | $SLOW1 $REF LIMIT_B=50000    TARGET_QPS=1000"
  "cu1k-lim100k-qps1000   | $SLOW1 $REF LIMIT_B=100000   TARGET_QPS=1000"
  #
  # ---- cu2k, 2,000 units/tx (slow_n=70). Each rung admits half of cu1k's:
  #      lim20k matches Run A; lim100k admits exactly the 1000 qps offered;
  #      lim200k is the unconstrained reference — higher rungs were dropped.
  "cu2k-lim10k-qps1000    | $SLOW70 $REF LIMIT_B=10000    TARGET_QPS=1000"
  "cu2k-lim20k-qps1000    | $SLOW70 $REF LIMIT_B=20000    TARGET_QPS=1000"
  "cu2k-lim50k-qps1000    | $SLOW70 $REF LIMIT_B=50000    TARGET_QPS=1000"
  "cu2k-lim100k-qps1000   | $SLOW70 $REF LIMIT_B=100000   TARGET_QPS=1000"
  "cu2k-lim200k-qps1000   | $SLOW70 $REF LIMIT_B=200000   TARGET_QPS=1000"
  #
  # ---- cu5k, 5,000 units/tx (slow_n=120). lim50k matches Run A. The
  #      exactly-offered point (250k) is not a rung: lim200k is the last that
  #      can bind, lim500k the unconstrained reference — higher rungs were
  #      dropped.
  "cu5k-lim10k-qps1000    | $SLOW120 $REF LIMIT_B=10000    TARGET_QPS=1000"
  "cu5k-lim20k-qps1000    | $SLOW120 $REF LIMIT_B=20000    TARGET_QPS=1000"
  "cu5k-lim50k-qps1000    | $SLOW120 $REF LIMIT_B=50000    TARGET_QPS=1000"
  "cu5k-lim100k-qps1000   | $SLOW120 $REF LIMIT_B=100000   TARGET_QPS=1000"
  "cu5k-lim200k-qps1000   | $SLOW120 $REF LIMIT_B=200000   TARGET_QPS=1000"
  "cu5k-lim500k-qps1000   | $SLOW120 $REF LIMIT_B=500000   TARGET_QPS=1000"
  #
  # ---- cu10k, 10,000 units/tx (slow_n=160). lim10k is the floor of one per
  #      commit; lim100k matches Run A.
  "cu10k-lim10k-qps1000   | $SLOW160 $REF LIMIT_B=10000    TARGET_QPS=1000"
  "cu10k-lim20k-qps1000   | $SLOW160 $REF LIMIT_B=20000    TARGET_QPS=1000"
  "cu10k-lim50k-qps1000   | $SLOW160 $REF LIMIT_B=50000    TARGET_QPS=1000"
  "cu10k-lim100k-qps1000  | $SLOW160 $REF LIMIT_B=100000   TARGET_QPS=1000"
  "cu10k-lim200k-qps1000  | $SLOW160 $REF LIMIT_B=200000   TARGET_QPS=1000"
  "cu10k-lim500k-qps1000  | $SLOW160 $REF LIMIT_B=500000   TARGET_QPS=1000"
  "cu10k-lim1m-qps1000    | $SLOW160 $REF LIMIT_B=1000000  TARGET_QPS=1000"
  #
  # ---- cu20k, 20,000 units/tx (slow_n=217). lim10k admits nothing;
  #      lim200k matches Run A.
  "cu20k-lim10k-qps1000   | $SLOW217 $REF LIMIT_B=10000    TARGET_QPS=1000"
  "cu20k-lim20k-qps1000   | $SLOW217 $REF LIMIT_B=20000    TARGET_QPS=1000"
  "cu20k-lim50k-qps1000   | $SLOW217 $REF LIMIT_B=50000    TARGET_QPS=1000"
  "cu20k-lim100k-qps1000  | $SLOW217 $REF LIMIT_B=100000   TARGET_QPS=1000"
  "cu20k-lim200k-qps1000  | $SLOW217 $REF LIMIT_B=200000   TARGET_QPS=1000"
  "cu20k-lim500k-qps1000  | $SLOW217 $REF LIMIT_B=500000   TARGET_QPS=1000"
  "cu20k-lim1m-qps1000    | $SLOW217 $REF LIMIT_B=1000000  TARGET_QPS=1000"
  "cu20k-lim2m-qps1000    | $SLOW217 $REF LIMIT_B=2000000  TARGET_QPS=1000"
  #
  # ---- cu50k, 50,000 units/tx (slow_n=267). lim20k admits nothing;
  #      lim50k is the floor; lim500k matches Run A.
  "cu50k-lim20k-qps1000   | $SLOW267 $REF LIMIT_B=20000    TARGET_QPS=1000"
  "cu50k-lim50k-qps1000   | $SLOW267 $REF LIMIT_B=50000    TARGET_QPS=1000"
  "cu50k-lim100k-qps1000  | $SLOW267 $REF LIMIT_B=100000   TARGET_QPS=1000"
  "cu50k-lim200k-qps1000  | $SLOW267 $REF LIMIT_B=200000   TARGET_QPS=1000"
  "cu50k-lim500k-qps1000  | $SLOW267 $REF LIMIT_B=500000   TARGET_QPS=1000"
  "cu50k-lim1m-qps1000    | $SLOW267 $REF LIMIT_B=1000000  TARGET_QPS=1000"
  "cu50k-lim2m-qps1000    | $SLOW267 $REF LIMIT_B=2000000  TARGET_QPS=1000"
  "cu50k-lim5m-qps1000    | $SLOW267 $REF LIMIT_B=5000000  TARGET_QPS=1000"
  #
  # ---- cu100k, 100,000 units/tx (slow_n=350). Below lim100k (the floor)
  #      nothing admits; lim1m matches Run A.
  "cu100k-lim50k-qps1000  | $SLOW350 $REF LIMIT_B=50000    TARGET_QPS=1000"
  "cu100k-lim100k-qps1000 | $SLOW350 $REF LIMIT_B=100000   TARGET_QPS=1000"
  "cu100k-lim200k-qps1000 | $SLOW350 $REF LIMIT_B=200000   TARGET_QPS=1000"
  "cu100k-lim500k-qps1000 | $SLOW350 $REF LIMIT_B=500000   TARGET_QPS=1000"
  "cu100k-lim1m-qps1000   | $SLOW350 $REF LIMIT_B=1000000  TARGET_QPS=1000"
  "cu100k-lim2m-qps1000   | $SLOW350 $REF LIMIT_B=2000000  TARGET_QPS=1000"
  "cu100k-lim5m-qps1000   | $SLOW350 $REF LIMIT_B=5000000  TARGET_QPS=1000"
  "cu100k-lim10m-qps1000  | $SLOW350 $REF LIMIT_B=10000000 TARGET_QPS=1000"
  #
  # ---- cu200k, 200,000 units/tx (slow_n=516). Floor lim200k; lim2m matches
  #      Run A.
  "cu200k-lim100k-qps1000 | $SLOW516 $REF LIMIT_B=100000   TARGET_QPS=1000"
  "cu200k-lim200k-qps1000 | $SLOW516 $REF LIMIT_B=200000   TARGET_QPS=1000"
  "cu200k-lim500k-qps1000 | $SLOW516 $REF LIMIT_B=500000   TARGET_QPS=1000"
  "cu200k-lim1m-qps1000   | $SLOW516 $REF LIMIT_B=1000000  TARGET_QPS=1000"
  "cu200k-lim2m-qps1000   | $SLOW516 $REF LIMIT_B=2000000  TARGET_QPS=1000"
  "cu200k-lim5m-qps1000   | $SLOW516 $REF LIMIT_B=5000000  TARGET_QPS=1000"
  "cu200k-lim10m-qps1000  | $SLOW516 $REF LIMIT_B=10000000 TARGET_QPS=1000"
  "cu200k-lim20m-qps1000  | $SLOW516 $REF LIMIT_B=20000000 TARGET_QPS=1000"
  #
  # ---- cu500k, 500,000 units/tx (slow_n=1015). Floor lim500k; lim5m matches
  #      Run A.
  "cu500k-lim200k-qps1000 | $SLOW1015 $REF LIMIT_B=200000   TARGET_QPS=1000"
  "cu500k-lim500k-qps1000 | $SLOW1015 $REF LIMIT_B=500000   TARGET_QPS=1000"
  "cu500k-lim1m-qps1000   | $SLOW1015 $REF LIMIT_B=1000000  TARGET_QPS=1000"
  "cu500k-lim2m-qps1000   | $SLOW1015 $REF LIMIT_B=2000000  TARGET_QPS=1000"
  "cu500k-lim5m-qps1000   | $SLOW1015 $REF LIMIT_B=5000000  TARGET_QPS=1000"
  "cu500k-lim10m-qps1000  | $SLOW1015 $REF LIMIT_B=10000000 TARGET_QPS=1000"
  "cu500k-lim20m-qps1000  | $SLOW1015 $REF LIMIT_B=20000000 TARGET_QPS=1000"
  "cu500k-lim50m-qps1000  | $SLOW1015 $REF LIMIT_B=50000000 TARGET_QPS=1000"
  #
  # ---- cu1m, 1,000,000 units/tx (slow_n=1848). Floor lim1m; lim10m matches
  #      Run A.
  "cu1m-lim500k-qps1000   | $SLOW1848 $REF LIMIT_B=500000   TARGET_QPS=1000"
  "cu1m-lim1m-qps1000     | $SLOW1848 $REF LIMIT_B=1000000  TARGET_QPS=1000"
  "cu1m-lim2m-qps1000     | $SLOW1848 $REF LIMIT_B=2000000  TARGET_QPS=1000"
  "cu1m-lim5m-qps1000     | $SLOW1848 $REF LIMIT_B=5000000  TARGET_QPS=1000"
  "cu1m-lim10m-qps1000    | $SLOW1848 $REF LIMIT_B=10000000 TARGET_QPS=1000"
  "cu1m-lim20m-qps1000    | $SLOW1848 $REF LIMIT_B=20000000 TARGET_QPS=1000"
  "cu1m-lim50m-qps1000    | $SLOW1848 $REF LIMIT_B=50000000 TARGET_QPS=1000"
  #
  # ---- cu2m, 2,000,000 units/tx (slow_n=3511). Floor lim2m; lim20m matches
  #      Run A.
  "cu2m-lim1m-qps1000     | $SLOW3511 $REF LIMIT_B=1000000  TARGET_QPS=1000"
  "cu2m-lim2m-qps1000     | $SLOW3511 $REF LIMIT_B=2000000  TARGET_QPS=1000"
  "cu2m-lim5m-qps1000     | $SLOW3511 $REF LIMIT_B=5000000  TARGET_QPS=1000"
  "cu2m-lim10m-qps1000    | $SLOW3511 $REF LIMIT_B=10000000 TARGET_QPS=1000"
  "cu2m-lim20m-qps1000    | $SLOW3511 $REF LIMIT_B=20000000 TARGET_QPS=1000"
  "cu2m-lim50m-qps1000    | $SLOW3511 $REF LIMIT_B=50000000 TARGET_QPS=1000"
  #
  # ---- cu5m, 5,000,000 units/tx (slow_n=8000, the metering ceiling). Floor
  #      lim5m; lim50m matches Run A — the ladder's top rung.
  "cu5m-lim2m-qps1000     | $SLOW8000 $REF LIMIT_B=2000000  TARGET_QPS=1000"
  "cu5m-lim5m-qps1000     | $SLOW8000 $REF LIMIT_B=5000000  TARGET_QPS=1000"
  "cu5m-lim10m-qps1000    | $SLOW8000 $REF LIMIT_B=10000000 TARGET_QPS=1000"
  "cu5m-lim20m-qps1000    | $SLOW8000 $REF LIMIT_B=20000000 TARGET_QPS=1000"
  "cu5m-lim50m-qps1000    | $SLOW8000 $REF LIMIT_B=50000000 TARGET_QPS=1000"
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
echo "aggregate: python3 aggregate.py results   (writes results/summary.md)"
