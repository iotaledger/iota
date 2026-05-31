#!/usr/bin/env bash

set -euo pipefail

# Make sure relative paths to sweep.sh resolve regardless of where the
# user invokes this script from.
cd "$(dirname "$0")"

# How many ROUNDS through the policy list. Each round runs ONE iter of
# every policy below before moving to the next round, so the JSONL
# accumulates evenly-spaced samples across all policies — diagnose
# issues early instead of waiting N iters of policy A to finish before
# seeing any of policy B. Total wall-clock ≈ ITERS × num_policies × per-iter.
ITERS="${ITERS:-1}"
export ITERS

# Semaphore cap applied to all policies in run_inner.sh. Machine-specific
# (different "amplification sweet spot" per hardware): WS=750, EPYC=500.
SEM_CAP="${SEM_CAP:-750}"
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
