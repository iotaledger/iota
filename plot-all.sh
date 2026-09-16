#!/usr/bin/env bash
# Plot every regime JSONL under sweeps/latest/data/, each into its own
# subdir under sweeps/latest/plots/. Single-regime invocation: use
# `sweeps/.venv/bin/python sweeps/plot.py [path/to/sweep.jsonl]` directly.
set -uo pipefail
cd "$(dirname "$0")"

PYTHON="sweeps/.venv/bin/python"
PLOT="sweeps/plot.py"

shopt -s nullglob
files=(sweeps/latest/data/*.jsonl)
if [ "${#files[@]}" -eq 0 ]; then
  echo "no JSONL files in sweeps/latest/data/" >&2
  exit 1
fi

echo "=> plotting ${#files[@]} regime(s) from sweeps/latest/data/"
for f in "${files[@]}"; do
  echo ""
  echo "--- $f ---"
  "$PYTHON" "$PLOT" "$f"
done
