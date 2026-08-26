#!/usr/bin/env bash
# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
#
# End-to-end calibration collection and fit, resumable. Intended for a
# remote machine under tmux/nohup: every stage skips work already on disk,
# so re-running after an interruption continues where it stopped.
#
#   run_all.sh OUT_DIR [--write-duration SECS] [--skip-cold] [--skip-write]
#
# Stages: build → Stage 1 sweeps → mixed workload → cold reads (page cache
# dropped when root/sudo is available) → optional sustained write run →
# fit → validation score. Logs to OUT_DIR/run_all.log.
set -euo pipefail
out="${1:?usage: run_all.sh OUT_DIR [--write-duration SECS] [--skip-cold] [--skip-write]}"; shift
write_duration=0; skip_cold=0; skip_write=1
while [ $# -gt 0 ]; do
  case "$1" in
    --write-duration) write_duration="$2"; skip_write=0; shift 2;;
    --skip-cold) skip_cold=1; shift;;
    --skip-write) skip_write=1; shift;;
    *) echo "unknown arg $1" >&2; exit 2;;
  esac
done
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
mkdir -p "$out"
log="$out/run_all.log"
exec > >(tee -a "$log") 2>&1
echo "=== run_all start $(date -u +%FT%TZ) on $(hostname) ==="

stage() { echo; echo "--- $1 ($(date -u +%T)) ---"; }

stage "machine state"
bash "$here/machine_prep.sh" "$out"

stage "build (release)"
(cd "$repo" && cargo build --release -p iota-single-node-benchmark)

stage "Stage 1 sweeps"
python3 "$here/sweep.py" --out "$out/sweeps"

stage "mixed workload"
python3 "$here/validate.py" collect --out "$out/mixed"

if [ "$skip_cold" -eq 0 ]; then
  stage "cold reads"
  purge=""
  if [ "$(id -u)" -eq 0 ]; then purge="sync && echo 3 > /proc/sys/vm/drop_caches"
  elif sudo -n true 2>/dev/null; then purge="sync && echo 3 | sudo -n tee /proc/sys/vm/drop_caches >/dev/null"
  else echo "WARNING: no root/sudo — cold reads will be page-cache-warm lower bounds"; fi
  if [ -n "$purge" ]; then python3 "$here/cold_read.py" --out "$out/cold" --purge-cmd "$purge"
  else python3 "$here/cold_read.py" --out "$out/cold"; fi
fi

if [ "$skip_write" -eq 0 ]; then
  stage "sustained write run (${write_duration}s)"
  [ -d "$out/write/db" ] && echo "write run already present, skipping" || \
    python3 "$here/write_side.py" --out "$out/write" --duration "$write_duration"
fi

stage "fit"
python3 "$here/fit.py" --data "$out/sweeps" "$out/mixed" --out "$out/calibration-artifact.json"

stage "validation score (single-workload sweeps, held-out split is inside fit)"
python3 "$here/validate.py" score --artifact "$out/calibration-artifact.json" \
  --data "$out/sweeps" --report "$out/score-sweeps.json" || true
python3 "$here/validate.py" score --artifact "$out/calibration-artifact.json" \
  --data "$out/mixed" --report "$out/score-mixed.json" || true

echo; echo "=== run_all done $(date -u +%FT%TZ) — results in $out ==="
