#!/usr/bin/env bash
#
# probe_sweep.sh — run probe.sh over the H2 calibration grid, accumulating
# results/probe/calibration-<machine>.csv. Tears down any leftover network first
# (a previous failed run may have leaked one, still churning its backlog),
# brings a fresh one up on the first probe, REUSES it for all points, and
# tears everything down at the end regardless of point failures (WIPE=no
# keeps it up for debugging).
#
# Grid: a geometric ladder of the product n*size (computation units are strongly
# superlinear in the product, so a log ladder samples evenly in log-CU). Kept at
# size=100, varying n to hit each product rung. Plus a split-invariance check:
# equal product (40000) at different n/size splits, to confirm CU depends only on
# the product (validating it as the single W5 axis).
#
# The groth16 grid (W8) is separate: it steps the number of calls to one
# 0x2::groth16 native function per transaction, and probe.sh writes its rows to
# results/probe/groth16-<machine>.csv with its groth16 defaults (1 per second
# for 100 s).
#
# Usage:
#   ./probe_sweep.sh              # everything: ladder + split + cost points
#   ./probe_sweep.sh ladder       # ladder only
#   ./probe_sweep.sh split        # split-invariance check only
#   ./probe_sweep.sh cu           # the mode comparison's cost points
#   ./probe_sweep.sh groth16      # the groth16 grid, not part of "all"
#
# Tunables inherited by probe.sh: QPS, DURATION, DIRECT, N, PROM; for groth16
# also GROTH16_CURVE (bn254 | bls12381, default bn254) and GROTH16_FUNCTION
# (verify | prepare, default verify).

set -uo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WHICH="${1:-all}"
LOGDIR="$SCRIPT_DIR/logs"
RETRIES="${RETRIES:-3}" # extra attempts per failed point
GROTH16_CURVE="${GROTH16_CURVE:-bn254}"
GROTH16_FUNCTION="${GROTH16_FUNCTION:-verify}"
mkdir -p "$LOGDIR"

# "n size" pairs. Ladder: product in {100,200,500,...,2M} at size=100. The top
# rungs hit the CU ceiling: the gas meter is created with
# min(gas_budget, max_gas_computation_bucket * gas_price), both 5M here, so no
# transaction is metered above 5,000,000 CU. Once the work costs more than the
# budget the transaction fails with InsufficientGas and is charged the whole
# budget, so every point past the ceiling reports exactly 5,000,000. Measured
# on both machines: product >= 800k. The 1M/1.2M/1.5M/2M rungs extend the
# plateau; they are ceiling-characterization points, not usable workloads.
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
  "8500 100"  # 850k  (at the ceiling; 800k reaches it first, see cu5m)
  "10000 100" # 1M    (caps at 5M)
  "12000 100" # 1.2M  (caps at 5M)
  "15000 100" # 1.5M  (caps at 5M)
  "20000 100" # 2M    (caps at 5M)
)
# Split-invariance check: all product 40000, different n/size splits.
split=(
  "100 400"
  "200 200"
  "400 100"
)
# The twelve cost points the mode comparison runs (matrix.sh's cost table),
# each n chosen so the attested cost lands on a round target. They are not
# ladder rungs: the ladder steps the product geometrically and these fall
# between its rungs, which is why they were probed one at a time. Measures
# what the scheduler will charge for each of the grid's cost points; the
# mode comparison attaches a mutable shared input to the same call, but that
# input is unused, so it changes only whether congestion control applies.
cu=(
  "1 100"    # cu1k       1,000 units
  "70 100"   # cu2k       2,000
  "120 100"  # cu5k       5,000
  "160 100"  # cu10k     10,000
  "217 100"  # cu20k     20,000
  "267 100"  # cu50k     50,000
  "350 100"  # cu100k   100,000
  "516 100"  # cu200k   200,000
  "1015 100" # cu500k   500,000
  "1848 100" # cu1m   1,000,000
  "3511 100" # cu2m   2,000,000
  "8000 100" # cu5m   5,000,000 (the metering ceiling)
)
# groth16 (W8): calls to the native function per transaction. For BN254 verify,
# at about 125 CUs a call, 1 and 7 both round up to 1,000 CUs, 8 is the first
# step to 2,000, and 700 (88,000 CUs) is the most at the flat price: from the
# 701st native call in a transaction, gas model v2 charges each call's amount
# as instructions, so 701, 710 and 750 show that jump. Past about 810 calls a
# transaction reaches the 5,000,000 metering ceiling. The other three
# workloads charge less per call, so their rounding steps fall elsewhere.
groth16=(1 7 8 50 100 200 400 700 701 710 750)

points=()
case "$WHICH" in
# The grids overlap at "1 100" (a ladder rung and cu1k), so drop repeats —
# probing the same point twice only adds a duplicate CSV row.
all)
  declare -A _seen=()
  for _p in "${ladder[@]}" "${split[@]}" "${cu[@]}"; do
    [[ -n "${_seen[$_p]:-}" ]] && continue
    _seen[$_p]=1
    points+=("$_p")
  done
  ;;
ladder) points=("${ladder[@]}") ;;
split) points=("${split[@]}") ;;
cu) points=("${cu[@]}") ;;
groth16) points=("${groth16[@]}") ;;
*)
  echo "usage: $0 [all|ladder|split|cu|groth16]" >&2
  exit 1
  ;;
esac

# Run probe.sh for one point of the chosen grid.
probe_point() {
  if [[ "$WHICH" == groth16 ]]; then
    WORKLOAD=groth16 GROTH16_CALLS="$1" GROTH16_CURVE="$GROTH16_CURVE" \
      GROTH16_FUNCTION="$GROTH16_FUNCTION" WIPE=no "$SCRIPT_DIR/probe.sh"
  else
    local n size
    read -r n size <<<"$1"
    SLOW_N="$n" SLOW_SIZE="$size" WIPE=no "$SCRIPT_DIR/probe.sh"
  fi
}

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

TOOLS_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Every sweep starts from a clean network. A previous run that ended in failing
# points leaves its network up (probe.sh exits before its wipe step) still
# churning undelivered ceiling-cost transactions; reusing it fails the early
# points of the next sweep.
echo "[$(date +%H:%M:%S)] tearing down any leftover network (fresh start)"
sudo "$TOOLS_DIR/cleanup.sh" >"$LOGDIR/sweep-cleanup.log" 2>&1 || true

total=${#points[@]}
i=0
for p in "${points[@]}"; do
  i=$((i + 1))
  if [[ "$WHICH" == groth16 ]]; then
    label="groth16-$GROTH16_CURVE-$GROTH16_FUNCTION-k$p"
  else
    read -r n size <<<"$p"
    label="slow-n${n}-s${size}"
  fi
  echo "[$(date +%H:%M:%S)] ($i/$total) probe $label -> logs/$label.log"
  # Transient submit-path stalls fail the odd point (the scrape guard keeps
  # bad rows out of the CSV); immediate retries usually land it. All attempts
  # append to the same point log.
  ok=""
  for attempt in $(seq 1 $((1 + RETRIES))); do
    if ((attempt > 1)); then
      echo "    ✗ failed — retry $((attempt - 1))/$RETRIES"
      probe_point "$p" >>"$LOGDIR/$label.log" 2>&1 && ok=1
    else
      probe_point "$p" >"$LOGDIR/$label.log" 2>&1 && ok=1
    fi
    [[ -n "$ok" ]] && break
  done
  if [[ -n "$ok" ]]; then
    if ((attempt == 1)); then echo "    ✓ done"; else echo "    ✓ done (attempt $attempt)"; fi
  else
    echo "    ✗ FAILED after $((1 + RETRIES)) attempts — tail logs/$label.log"
  fi
done

echo
# Tear down unconditionally: tying teardown to the LAST point's probe meant a
# failing last point leaked the network (probe.sh exits before its wipe step),
# poisoning the next sweep. WIPE=no keeps it up for debugging.
if [[ "${WIPE:-yes}" != no && "${WIPE:-yes}" != n ]]; then
  echo "[$(date +%H:%M:%S)] tearing down network + monitoring"
  sudo "$TOOLS_DIR/cleanup.sh" >>"$LOGDIR/sweep-cleanup.log" 2>&1 || true
fi

echo
# The CSV name carries probe.sh's CPU slug; report the file it actually wrote.
if [[ "$WHICH" == groth16 ]]; then
  csv_prefix=groth16
else
  csv_prefix=calibration
fi
csv="$(ls -t "$SCRIPT_DIR/results/probe"/"$csv_prefix"-*.csv 2>/dev/null | head -1)"
if [[ -n "$csv" ]]; then
  echo "sweep complete -> ${csv#"$SCRIPT_DIR"/}"
else
  echo "sweep complete — no calibration CSV written (every point failed?)"
fi

# Recompute the markdown tables and redraw the figures from the fresh CSV.
# Tables are pure stdlib; figures need a matplotlib venv (reuse ../h1/.venv).
# REGEN=no skips this — the regen globs every results/probe/calibration-*.csv
# and overwrites the shared calibration-tables.md + figures, so a throwaway
# sweep (distinct MACHINE slug) should skip it to leave the real ones untouched.
if [[ "${REGEN:-yes}" == no || "${REGEN:-yes}" == n ]]; then
  echo
  echo "REGEN=no — skipping table/figure regen (CSV written, shared artifacts untouched)"
elif [[ "$WHICH" == groth16 ]]; then
  # The tables and figures below are built from calibration-*.csv only, which a
  # groth16 sweep does not change.
  echo
  echo "groth16 sweep — slow tables and figures unchanged, nothing to regenerate"
else
  echo
  echo "regenerating tables + figures..."
  python3 "$SCRIPT_DIR/make_calibration_table.py" || echo "  (table regen failed)"
  VENV_PY="$SCRIPT_DIR/../h1/.venv/bin/python"
  if [[ -x "$VENV_PY" ]]; then
    "$VENV_PY" "$SCRIPT_DIR/plot_calibration.py" || echo "  (figure regen failed)"
  else
    echo "  (skipping figures: no matplotlib venv at $VENV_PY)"
  fi
fi
