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

# Semaphore cap applied to all policies. 400 matches the IOTA production
# default — keeps SEM behavior aligned with what real validators run.
SEM_CAP="${SEM_CAP:-400}"
export SEM_CAP

# Saturation pct for GRADUATED policies (the 100%-shed threshold).
# Default 100: graduated's soft zone runs all the way to max_pending,
# with no separate saturated zone — pure RED-style probabilistic ramp.
# Override to e.g. 95 if you want a saturated-rejection band between the
# soft ramp and the hard cap.
SAT_PCT="${SAT_PCT:-100}"
export SAT_PCT

# AIMD knobs (closed-loop congestion control on the spammer) are now
# OPT-IN. Default behavior matches default-TD client (no AIMD). Pass
# `AIMD=true` explicitly to enable for cooperative-load experiments.
# AIMD is also automatically disabled when OPEN_LOOP=true.

# Per-worker spammer in-flight cap (Rust default 200 too high).
# 40 × 16 workers × 25 procs = 16K total spammer in-flight — calibrated
# to push validator queue to ~13-16K mean at max=20K, hits cap frequently.
OPEN_LOOP_MAX_INFLIGHT_PER_WORKER="${OPEN_LOOP_MAX_INFLIGHT_PER_WORKER:-40}"
export OPEN_LOOP_MAX_INFLIGHT_PER_WORKER

# Examples:
#   ITERS=20 ./run.sh
#   ITERS=20 SEM_CAP=500 SAT_PCT=90 ./run.sh   (different sem / sat)
#   ITERS=20 OPEN_LOOP=true AIMD=false ./run.sh    (open-loop, non-responsive)

echo "config: ITERS=$ITERS  SEM_CAP=$SEM_CAP  SAT_PCT=$SAT_PCT  OPEN_LOOP_CAP=$OPEN_LOOP_MAX_INFLIGHT_PER_WORKER  AIMD=${AIMD:-false}  OPEN_LOOP=${OPEN_LOOP:-false}"

nohup ./run_inner.sh >> run.log 2>&1 &

disown
echo "PID: $!"
