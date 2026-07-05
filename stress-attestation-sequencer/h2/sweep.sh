#!/usr/bin/env bash
#
# sweep.sh — run probe.sh over the H2 calibration grid, accumulating
# results/calibration.csv. Brings the network up on the first probe and REUSES
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
#   ./sweep.sh              # full ladder + split check
#   ./sweep.sh ladder       # ladder only
#   ./sweep.sh split        # split-invariance check only
#
# Tunables inherited by probe.sh: QPS, DURATION, SLOW_SHARED, DIRECT, N, PROM.

set -uo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WHICH="${1:-all}"
LOGDIR="$SCRIPT_DIR/logs"
mkdir -p "$LOGDIR"

# "n size" pairs. Ladder: product in {100,200,500,...,500k} at size=100.
ladder=(
  "1 100"    # product 100
  "2 100"    # 200
  "5 100"    # 500
  "10 100"   # 1k
  "20 100"   # 2k
  "50 100"   # 5k
  "100 100"  # 10k
  "200 100"  # 20k
  "500 100"  # 50k
  "1000 100" # 100k
  "2000 100" # 200k
  "5000 100" # 500k
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
  echo "sweep.sh: need sudo (first probe may bootstrap/start the network)"
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
  # Keep the network up for every point except the last (WIPE=no); tear down at
  # the end. Override by exporting WIPE before calling sweep.sh.
  wipe="${WIPE:-no}"
  [[ $i -eq $total && -z "${WIPE:-}" ]] && wipe=no # default: leave up even at end
  echo "[$(date +%H:%M:%S)] ($i/$total) probe $label -> logs/$label.log"
  SLOW_N="$n" SLOW_SIZE="$size" WIPE="$wipe" "$SCRIPT_DIR/probe.sh" >"$LOGDIR/$label.log" 2>&1 &&
    echo "    ✓ done" || echo "    ✗ FAILED — tail logs/$label.log"
done

echo
echo "sweep complete -> results/calibration.csv"
