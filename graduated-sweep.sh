#!/usr/bin/env bash
# Sweep the graduated-load-shedding-soft-limit-pct yaml field across a
# series of values, running the full burst-sweep at each one. Output is
# one CSV + one log per pct value so the arms are easy to compare.
#
# Usage:
#   ./graduated-sweep.sh                # default: 100 50 25 10 2
#   ./graduated-sweep.sh 25 10 2        # custom list
#
# Already-completed pcts (those with burst-sweep-pct${pct}.csv on disk)
# are SKIPPED automatically — re-running with the default list is safe
# and resumes wherever you left off.
#
# Pre-requisites:
#   - burst-sweep.sh in same dir, with ITERS set to your target N (e.g. 30)
#   - validator-common.yaml has a `graduated-load-shedding-soft-limit-pct:`
#     line (the sed pattern targets this exact key)
#   - sudo -v has been run recently (cache lasts ~15 min by default;
#     this script re-primes it before each pct so it survives a 5h sweep,
#     but the first prime needs your password)
#
# Output:
#   burst-sweep-pct${pct}.csv  — one per pct value
#   burst-sweep-pct${pct}.log  — one per pct value
#
# Note: each pct run takes ~75 min at N=30, so the default 4-value sweep
# is ~5 hours total. Start it under `nohup` or in a tmux pane.

set -uo pipefail   # NOT set -e: we want to continue across pct values
                   # even if one burst-sweep fails partway through.

PCTS=("$@")
if [ ${#PCTS[@]} -eq 0 ]; then
  PCTS=(100 50 25 10 2)
fi

YAML="dev-tools/iota-private-network/configs/validator-common.yaml"
SWEEP_SCRIPT="./burst-sweep.sh"

# Sanity checks
[ -f "$YAML" ]          || { echo "ERROR: yaml not found: $YAML"; exit 1; }
[ -x "$SWEEP_SCRIPT" ]  || { echo "ERROR: not executable: $SWEEP_SCRIPT"; exit 1; }
grep -q 'graduated-load-shedding-soft-limit-pct:' "$YAML" \
  || { echo "ERROR: yaml has no graduated-load-shedding-soft-limit-pct field"; exit 1; }

echo "=========================================================="
echo "=== graduated soft_limit_pct sweep                    ==="
echo "===   values: ${PCTS[*]}"
echo "===   started: $(date -u)"
echo "=========================================================="
echo

# Preserve any existing burst-sweep outputs so we don't clobber them
TS=$(date -u +%Y%m%dT%H%M%SZ)
[ -f burst-sweep.csv ] && mv burst-sweep.csv "burst-sweep-pre-${TS}.csv" \
  && echo "  archived existing burst-sweep.csv → burst-sweep-pre-${TS}.csv"
[ -f burst-sweep.log ] && mv burst-sweep.log "burst-sweep-pre-${TS}.log"

# Loop
for pct in "${PCTS[@]}"; do
  # Validate input — must be int in [0, 100]
  if ! [[ "$pct" =~ ^[0-9]+$ ]] || [ "$pct" -gt 100 ]; then
    echo "  SKIPPING invalid pct=$pct (must be 0..100)"
    continue
  fi

  echo
  echo "=========================================================="
  echo "=== pct=$pct                            $(date -u +%H:%M:%S) ==="
  echo "=========================================================="

  # Skip if we already have results for this pct — useful when re-running
  # the sweep with the same arg list and you don't want to overwrite the
  # existing data. The final summary at the bottom still reads the file.
  EXISTING="burst-sweep-pct${pct}.csv"
  if [ -f "$EXISTING" ]; then
    n=$(awk -F, 'NR>1 && $6 != "FAIL" && $6+0 > 0' "$EXISTING" | wc -l)
    echo "  SKIPPING — $EXISTING already exists ($n valid runs)."
    echo "  (delete it to force a re-run for this pct)"
    continue
  fi

  # Patch yaml — preserve leading whitespace, swap value only
  sed -i -E "s/^([[:space:]]*graduated-load-shedding-soft-limit-pct:[[:space:]]*).*/\1${pct}/" "$YAML"
  # Verify patch landed
  ACTUAL=$(grep -E 'graduated-load-shedding-soft-limit-pct:' "$YAML" | awk -F: '{print $2}' | xargs)
  if [ "$ACTUAL" != "$pct" ]; then
    echo "  ERROR: yaml patch failed (expected $pct, got '$ACTUAL') — skipping this pct"
    continue
  fi
  echo "  yaml patched: graduated-load-shedding-soft-limit-pct = $pct"
  grep -E 'max-pending|graduated' "$YAML" | sed 's/^/    /'

  # Re-prime sudo cache (long sweep can outlive the default 15-min cache)
  sudo -v 2>/dev/null || echo "  WARN: sudo -v failed — bootstrap.sh inside burst-sweep will prompt"

  # Kick off the sweep. burst-sweep.sh redirects stdout/stderr to its
  # own burst-sweep.log via `exec >> ...`, so we get no live output —
  # but we know it finished when this returns.
  echo "  running burst-sweep.sh (logs to burst-sweep.log) ..."
  START=$(date +%s)
  "$SWEEP_SCRIPT" || echo "  WARN: burst-sweep.sh exited non-zero (continuing to next pct)"
  ELAPSED=$(( $(date +%s) - START ))
  echo "  finished after ${ELAPSED}s ($(( ELAPSED / 60 ))m $(( ELAPSED % 60 ))s)"

  # Archive outputs under the pct-specific name
  if [ -f burst-sweep.csv ]; then
    mv burst-sweep.csv "burst-sweep-pct${pct}.csv"
    echo "  → burst-sweep-pct${pct}.csv"
  else
    echo "  WARN: burst-sweep.csv missing — no results saved for pct=$pct"
  fi
  [ -f burst-sweep.log ] && mv burst-sweep.log "burst-sweep-pct${pct}.log"
done

echo
echo "=========================================================="
echo "=== ALL DONE — summary across pct values              ==="
echo "===   finished: $(date -u)"
echo "=========================================================="

printf "  %-6s %-6s %-9s %-9s %-9s %-9s %-12s\n" \
  "pct" "n" "min" "median" "mean" "max" "≥50× rate"
echo "  ---------------------------------------------------------------"
for pct in "${PCTS[@]}"; do
  csv="burst-sweep-pct${pct}.csv"
  [ -f "$csv" ] || { printf "  pct=%-3s (no file)\n" "$pct"; continue; }
  awk -F, -v pct="$pct" 'NR>1 && $6 != "FAIL" && $6+0 > 0 {
    sum += $6; n++
    if ($6+0 > max) max = $6+0
    if (min == "" || $6+0 < min) min = $6+0
    arr[n] = $6+0
    if ($6+0 >= 50) hi++
  } END {
    if (n == 0) { printf "  %-6s n=0 (no valid runs)\n", pct; next }
    asort(arr)
    med = arr[int((n+1)/2)]
    rate = (hi/n) * 100
    printf "  %-6s %-6d %-9.2f %-9.2f %-9.2f %-9.2f %.0f%%  (%d/%d)\n", \
      pct, n, min, med, sum/n, max, rate, hi+0, n
  }' "$csv"
done
