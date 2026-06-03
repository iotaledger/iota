#!/usr/bin/env bash
# Overnight experiment runner — graduated load-shedding Pareto study.
# Runs three experiment groups back-to-back, backing up sweep.jsonl after
# each one so results aren't overwritten by the next run.
#
# Total estimated runtime: ~12-14h at ITERS=10 across 6 runs.
# Drop ITERS lower (e.g., 5) on lines below if you need it shorter.

set -euo pipefail
cd "$(dirname "$0")"

mkdir -p results

# Refresh sudo cache up front. The runner doesn't itself sudo, but
# bootstrap/teardown inside run_inner.sh may need it.
sudo -v

# Keep sudo refreshed in the background so overnight runs don't stall on
# a password prompt mid-experiment. Dies when this script exits.
( while true; do sudo -n true; sleep 60; done ) &
SUDO_KEEPALIVE_PID=$!
trap "kill $SUDO_KEEPALIVE_PID 2>/dev/null || true" EXIT

# Wait until no run_inner.sh process is alive.
wait_for_run() {
  while pgrep -f "run_inner.sh" >/dev/null 2>&1; do
    sleep 30
  done
}

# Launch a run, wait for it to finish, then back up sweep.jsonl.
run_experiment() {
  local name="$1"; shift
  local ts
  ts="$(date '+%Y-%m-%d %H:%M:%S')"
  echo "================================================================="
  echo "[$ts] starting: $name"
  echo "       env: $*"
  echo "================================================================="

  wait_for_run                # safety: don't stack on top of another run
  env "$@" ./run.sh           # ./run.sh disowns run_inner.sh and returns
  sleep 10                    # give run_inner.sh time to start
  wait_for_run                # block until the launched run finishes

  cp sweep.jsonl "results/${name}.jsonl"
  ts="$(date '+%Y-%m-%d %H:%M:%S')"
  echo "[$ts] done: results/${name}.jsonl ($(wc -l < "results/${name}.jsonl") iters)"
  echo ""
}

# Config 1 — Pareto frontier
run_experiment "config1-pareto" \
  QPS_TOTAL=2000 BURST_SIZE=10 BARRIER_PERIOD_MS=1000 ITERS=10

# Config 2 — Burstiness sweep (4 runs)
run_experiment "config2-burst3"   \
  QPS_TOTAL=2000 BURST_SIZE=3   BARRIER_PERIOD_MS=1000 ITERS=10
run_experiment "config2-burst10"  \
  QPS_TOTAL=2000 BURST_SIZE=10  BARRIER_PERIOD_MS=1000 ITERS=10
run_experiment "config2-burst30"  \
  QPS_TOTAL=2000 BURST_SIZE=30  BARRIER_PERIOD_MS=1000 ITERS=10
run_experiment "config2-burst100" \
  QPS_TOTAL=2000 BURST_SIZE=100 BARRIER_PERIOD_MS=1000 ITERS=10

# Config 3 — Sustained smooth
run_experiment "config3-smooth" \
  QPS_TOTAL=3000 BURST_SIZE=1 BARRIER_PERIOD_MS=0 ITERS=10

echo "================================================================="
echo "all experiments complete. outputs:"
ls -la results/*.jsonl
echo "================================================================="
