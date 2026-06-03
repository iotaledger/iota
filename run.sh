#!/usr/bin/env bash

set -euo pipefail

# Make sure relative paths to sweep.sh resolve regardless of where the
# user invokes this script from.
cd "$(dirname "$0")"

# Refuse to launch if a previous run_inner.sh is still alive — concurrent
# runs share run.log / sweep.log and fight over docker state, producing
# garbled monitor output and corrupted iter data.
if pgrep -f "run_inner.sh" >/dev/null 2>&1; then
  echo "Error: a run_inner.sh is already running. Stop it with ./kill.sh first." >&2
  pgrep -af "run_inner.sh" | sed 's/^/  /' >&2
  exit 1
fi

# How many ROUNDS through the policy list. Each round runs ONE iter of
# every policy below before moving to the next round, so the JSONL
# accumulates evenly-spaced samples across all policies — diagnose
# issues early instead of waiting N iters of policy A to finish before
# seeing any of policy B. Total wall-clock ≈ ITERS × num_policies × per-iter.
ITERS="${ITERS:-1}"
export ITERS

# Semaphore cap applied to all policies in run_inner.sh.
# Sized so SEM is not the bottleneck for max=2000 policies (SEM/max=0.5 ratio).
# Earlier 400 was a hidden bottleneck for max=20K — masked graduated's value.
SEM_CAP="${SEM_CAP:-1000}"
export SEM_CAP

# Saturation pct for GRADUATED policies (the 100%-shed threshold).
# Binary policies always use sat=100 (structural — no soft zone).
# Machine-specific tuning: smaller race window → can push sat higher.
#   SAT_PCT=95   (WS, race window ~30 → 50-unit cushion holds)
#   SAT_PCT=90   (EPYC, race window ~75-95 → 100-unit cushion for absolute discipline)
SAT_PCT="${SAT_PCT:-95}"
export SAT_PCT

# AIMD closed-loop congestion control on the spammer. Models a responsive
# sender — required for RED-style graduated shedding benefits to materialize
# (desync, early backoff). Disable only to test non-responsive load.
AIMD="${AIMD:-true}"
export AIMD

# AIMD starting cwnd. Default 8 in Rust; we use 4 so workers ramp up to
# the feedback equilibrium without overshooting in the warm-up phase.
AIMD_INITIAL="${AIMD_INITIAL:-4}"
export AIMD_INITIAL

# Per-worker spammer in-flight cap (Rust default 200 too high).
# 40 × 16 workers × 25 procs = 16K total spammer in-flight — calibrated
# to push validator queue to ~13-16K mean at max=20K, hits cap frequently.
OPEN_LOOP_MAX_INFLIGHT_PER_WORKER="${OPEN_LOOP_MAX_INFLIGHT_PER_WORKER:-40}"
export OPEN_LOOP_MAX_INFLIGHT_PER_WORKER

# Examples:
#   ITERS=20 ./run.sh
#   ITERS=20 SEM_CAP=500 SAT_PCT=90 ./run.sh   (different sem / sat)
#   ITERS=20 OPEN_LOOP=true AIMD=false ./run.sh    (open-loop, non-responsive)

echo "config: ITERS=$ITERS  SEM_CAP=$SEM_CAP  SAT_PCT=$SAT_PCT  AIMD=$AIMD AIMD_INITIAL=$AIMD_INITIAL  OPEN_LOOP_CAP=$OPEN_LOOP_MAX_INFLIGHT_PER_WORKER"

nohup ./run_inner.sh >> run.log 2>&1 &

disown
echo "PID: $!"
