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

# Semaphore cap applied to all policies in run_inner.sh. Production-shape
# default is 500; override per workload via env. Same on WS and EPYC.
SEM_CAP="${SEM_CAP:-500}"
export SEM_CAP

# Saturation pct for GRADUATED policies (the 100%-shed threshold).
# Binary policies always use sat=100 (structural — no soft zone).
# Machine-specific tuning: smaller race window → can push sat higher.
#   SAT_PCT=95   (WS, race window ~30 → 50-unit cushion holds)
#   SAT_PCT=90   (EPYC, race window ~75-95 → 100-unit cushion for absolute discipline)
SAT_PCT="${SAT_PCT:-95}"
export SAT_PCT

# Examples:
#   ITERS=20 ./run.sh                      (WS default: sem=750, sat=95)
#   ITERS=20 SEM_CAP=500 SAT_PCT=90 ./run.sh  (EPYC: amplification sweet spot, conservative sat)
#   ITERS=20 SEM_CAP=500 ./run.sh          (EPYC with sat=95 — direct cross-machine comparison)

echo "config: ITERS=$ITERS  SEM_CAP=$SEM_CAP  SAT_PCT=$SAT_PCT (graduated only; binary uses 100)"

nohup ./run_inner.sh >> run.log 2>&1 &

disown
echo "PID: $!"
