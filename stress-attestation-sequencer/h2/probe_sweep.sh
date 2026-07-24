#!/usr/bin/env bash
#
# probe_sweep.sh — run probe.sh over the H2 calibration grid, accumulating
# results/calibration-<machine>.csv. Brings the network up on the first probe and REUSES
# it for the rest (probe.sh never wipes between points), so the whole sweep runs
# on one network. Tears everything down only at the very end (WIPE=yes on the
# last probe).
#
# Grid: a geometric ladder of the product n*size (computation units are strongly
# superlinear in the product, so a log ladder samples evenly in log-CU). Kept at
# size=100, varying n to hit each product rung. Plus a split-invariance check:
# equal product (40000) at different n/size splits, to confirm CU depends only on
# the product (validating it as the single W5 axis).
#
# Usage:
#   ./probe_sweep.sh              # full ladder + split check
#   ./probe_sweep.sh ladder       # ladder only
#   ./probe_sweep.sh split        # split-invariance check only
#
# Tunables inherited by probe.sh: QPS, DURATION, SLOW_SHARED, DIRECT, N, PROM.

set -uo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WHICH="${1:-all}"
LOGDIR="$SCRIPT_DIR/logs"
mkdir -p "$LOGDIR"

# "n size" pairs. Ladder: product in {100,200,500,...,2M} at size=100. The top
# rungs hit the CU ceiling: the VM computation budget is capped at
# max_gas_computation_bucket (5M CU) — min(gas_budget, 5M * gas_price) — so metered
# CU plateaus at ~4.85M just below it and the tx aborts out-of-gas. Measured:
# product >= ~850k all cap at 4.85M. The 1.2M/1.5M/2M rungs extend the plateau;
# they are ceiling-characterization points, not usable workloads.
ladder=(
  "1 100"     # product 100
  "2 100"     # 200
  "5 100"     # 500
  "10 100"    # 1k
  "20 100"    # 2k
  "50 100"    # 5k
  "100 100"   # 10k
  "200 100"   # 20k
  "500 100"   # 50k
  "1000 100"  # 100k
  "2000 100"  # 200k
  "5000 100"  # 500k
  "7000 100"  # 700k  (~4.0M CU)
  "8500 100"  # 850k  (~4.85M CU, first point at the ceiling)
  "10000 100" # 1M    (caps at ~4.85M)
  "12000 100" # 1.2M  (caps at ~4.85M)
  "15000 100" # 1.5M  (caps at ~4.85M)
  "20000 100" # 2M    (caps at ~4.85M)
)
# Split-invariance check: all product 40000, different n/size splits.
split=(
  "100 400"
  "200 200"
  "400 100"
)

points=()
case "$WHICH" in
all) points=("${ladder[@]}" "${split[@]}") ;;
ladder) points=("${ladder[@]}") ;;
split) points=("${split[@]}") ;;
*)
  echo "usage: $0 [all|ladder|split]" >&2
  exit 1
  ;;
esac

# Cache sudo up front (the first probe may bootstrap/start the network) and keep
# it alive for the whole sweep.
sudo -v || {
  echo "probe_sweep.sh: need sudo (first probe may bootstrap/start the network)"
  exit 1
}
(while true; do
  sudo -n true
  sleep 60
  kill -0 "$$" 2>/dev/null || exit
done) &
trap 'kill %1 2>/dev/null' EXIT

total=${#points[@]}
i=0
for p in "${points[@]}"; do
  read -r n size <<<"$p"
  i=$((i + 1))
  label="slow-n${n}-s${size}"
  # Keep the network up between points (WIPE=no); tear down after the last
  # one. Override by exporting WIPE before calling probe_sweep.sh.
  wipe="${WIPE:-no}"
  [[ $i -eq $total && -z "${WIPE:-}" ]] && wipe=yes # default: tear down at the end
  echo "[$(date +%H:%M:%S)] ($i/$total) probe $label -> logs/$label.log"
  if SLOW_N="$n" SLOW_SIZE="$size" WIPE="$wipe" "$SCRIPT_DIR/probe.sh" >"$LOGDIR/$label.log" 2>&1; then
    echo "    ✓ done"
  else
    # Transient submit-path stalls fail the odd point (the scrape guard keeps
    # bad rows out of the CSV); one immediate retry usually lands it.
    echo "    ✗ failed — retrying once"
    if SLOW_N="$n" SLOW_SIZE="$size" WIPE="$wipe" "$SCRIPT_DIR/probe.sh" >>"$LOGDIR/$label.log" 2>&1; then
      echo "    ✓ done (retry)"
    else
      echo "    ✗ FAILED twice — tail logs/$label.log"
    fi
  fi
done

echo
echo "sweep complete -> results/calibration-<machine>.csv"

# Recompute the markdown tables and redraw the figures from the fresh CSV.
# Tables are pure stdlib; figures need a matplotlib venv (reuse ../h1/.venv).
echo
echo "regenerating tables + figures..."
python3 "$SCRIPT_DIR/make_calibration_table.py" || echo "  (table regen failed)"
VENV_PY="$SCRIPT_DIR/../h1/.venv/bin/python"
if [[ -x "$VENV_PY" ]]; then
  "$VENV_PY" "$SCRIPT_DIR/plot_calibration.py" || echo "  (figure regen failed)"
else
  echo "  (skipping figures: no matplotlib venv at $VENV_PY)"
fi
