#!/usr/bin/env bash
#
# matrix.sh — sweep Run B's per-object computation-unit limit against a fixed
# TotalTxCount reference, ITERS iterations each, as labeled experiments under
# results/matrix/<LABEL>/.
#
# Adapted from ../h1/matrix.sh.
#
# LIMIT_B (computation units per object per commit) is the swept axis; Run A stays
# at production's LIMIT_A=10 transactions in every config as the reference. The
# two limits are independent inputs in different units.
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
# Cost points from the h2 calibration (probe-test.md; size fixed at 100).
# units/tx is the attested cost, the value the scheduler charges. It is a
# property of the workload: the probe measures the same figure on both
# machines, to the digit.
#
# "drains" is how fast one object sustains that point on EPYC. It comes from
# this grid, not from the probe: transactions on one mutable shared object run
# one after another, so in a config whose success rate settles below what its
# limit admits while cancellations stay quiet, execution is the only remaining
# constraint and the success rate IS the drain rate. Per-transaction execution
# time would be the other way to get it, but that is a per-machine, per-load
# quantity — the probe measures the same point 2.0x to 5.7x apart on the two
# machines, at 5 QPS with nothing contending — so the rate measured under the
# grid's own load is the one that applies here.
#
#   point    slow_n    units/tx   drains on EPYC (tx/s)
#   cu1k          1       1,000   arrival-limited, no plateau
#   cu2k         70       2,000   179
#   cu5k        120       5,000   118
#   cu10k       160      10,000    94
#   cu20k       217      20,000    73
#   cu50k       267      50,000    62
#   cu100k      350     100,000    52
#   cu200k      516     200,000    39
#   cu500k     1015     500,000    24
#   cu1m       1848   1,000,000    15
#   cu2m       3511   2,000,000     8.5
#   cu5m       8000   5,000,000     3.7
#
# cu1k has no drain figure because its whole ladder is arrival-limited: the
# client never offers enough to saturate one object at 1,000 units a
# transaction. cu5m sits at the metering ceiling — 5,000,000 is the gas budget
# in computation units, so those transactions fail with InsufficientGas and are
# charged the whole budget (see probe-test.md). Its work is truncated, which is
# worth remembering when reading its throughput.
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
# The cu configs all give every transaction the SAME cost, which makes the two
# modes equivalent — a unit limit and a count limit then admit the same work,
# so those configs are the control. The mix configs at the end give transactions
# in one commit DIFFERENT costs, which is the only way the modes can differ.
#
# 85 configs total. Use the substring FILTER to run one cost point, one limit,
# or the mixed-cost configs (FILTER=mix) at a time.
#
# Every config runs on 4 validators; N=4 is not in the label since nothing else
# is planned.
#
# Usage:
#   ITERS=5 ./matrix.sh             # run all 85 configs
#   ITERS=5 ./matrix.sh cu10k       # one cost point, its whole limit ladder
#   ITERS=5 ./matrix.sh lim100k     # one limit, every cost point that uses it
#   ITERS=1 ./matrix.sh mix         # the mixed-cost configs only
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
# Mixed cost: SLOW_MIX draws n per TRANSACTION from a weighted list, so one
# commit carries transactions of different cost. That is the only setting in
# which the two modes can differ at all — with one fixed cost a unit limit
# admits limit/cost transactions, exactly what a count limit of limit/cost
# admits, and the 12 matched configs above measure that (Run B within 1.6% of
# Run A). Under a spread a count limit admits a fixed NUMBER and lets the
# admitted work swing, while a unit limit admits a fixed amount of WORK and
# lets the number swing. Named by mean cost; each level costs the calibrated
# cost of its own n: 1 -> 1,000, 160 -> 10,000, 350 -> 100,000,
# 1015 -> 500,000 units.
MIX1900="WORKLOAD=slow SLOW_MIX=1:9,160:1 SLOW_SIZE=100 SLOW_SHARED=true"   # 1,900 mean units/tx
MIX3700="WORKLOAD=slow SLOW_MIX=1:7,160:3 SLOW_SIZE=100 SLOW_SHARED=true"   # 3,700 mean units/tx
MIX10900="WORKLOAD=slow SLOW_MIX=1:9,350:1 SLOW_SIZE=100 SLOW_SHARED=true"  # 10,900 mean units/tx
MIX20800="WORKLOAD=slow SLOW_MIX=1:4,350:1 SLOW_SIZE=100 SLOW_SHARED=true"  # 20,800 mean units/tx
MIX50900="WORKLOAD=slow SLOW_MIX=1:9,1015:1 SLOW_SIZE=100 SLOW_SHARED=true" # 50,900 mean units/tx
# Run A's reference in every config: production's count limit, burst off in both
# runs. The duration is pinned: run.sh's own default is shorter, for ad-hoc test
# runs.
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
  #
  # ---- mixed cost. LIMIT_B is 10x the mean cost in every config, so Run B's
  #      budget matches what Run A's count limit admits on average and the
  #      spread is the only difference between the arms. Each config's
  #      fixed-cost control is the cu config of the same mean cost above,
  #      already run to 10 iterations.
  #
  #      Two constraints pin the weights to 10-40%. Below 10% the matched
  #      limit falls BELOW one expensive transaction, which Run B could then
  #      never schedule at all; above 40% the mean cost is high enough that
  #      fewer than 10 transactions arrive per commit, so neither limit binds
  #      and the arms are identical.
  "mix1900-w10-lim19k-qps1000   | $MIX1900 $REF LIMIT_B=19000  TARGET_QPS=1000"
  "mix3700-w30-lim37k-qps1000   | $MIX3700 $REF LIMIT_B=37000  TARGET_QPS=1000"
  "mix10900-w10-lim109k-qps1000 | $MIX10900 $REF LIMIT_B=109000 TARGET_QPS=1000"
  "mix20800-w20-lim208k-qps1000 | $MIX20800 $REF LIMIT_B=208000 TARGET_QPS=1000"
  "mix50900-w10-lim509k-qps1000 | $MIX50900 $REF LIMIT_B=509000 TARGET_QPS=1000"
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
    sudo find "$SCRIPT_DIR/results/matrix/$label" -path '*node-logs/*.log' \
      ! -name '_state.log' ! -name '_crash.log' -exec "$GZIP_BIN" -f {} + 2>/dev/null
  done
done

mins=$((($(date +%s) - start) / 60))
echo
echo "matrix complete: $ok ok, $fail failed (of $n) in ${mins}m"
echo "results -> results/matrix/<LABEL>/iter-NNN/  (run-a/run-b timeseries JSON + client reports)"
echo "aggregate: python3 aggregate.py results/matrix   (writes results/matrix/summary.md)"
