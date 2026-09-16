#!/usr/bin/env bash
# clean.sh — remove sweep logs only.
#
# Logs (sweeps/latest/logs/*): grow on every run, removed without prompt.
# Per-iter forensic dirs (multi-<ts>/, failed-<ts>/) are included since
# they're log-class artifacts.
#
# Data (sweeps/latest/data/*.jsonl) and plots (sweeps/latest/plots/*) are
# the actual research output and are NEVER touched by this script. Archive
# them by renaming sweeps/latest → sweeps/v3/ (etc.) when you want to start
# fresh.
#
# Usage:
#   ./clean.sh

set -uo pipefail
cd "$(dirname "$0")"

# Detect TTY once at the top (must not be inside $() — see monitor.sh).
if [ -t 1 ]; then IS_TTY=1; else IS_TTY=0; fi

# Colors (auto-disable when stdout is not a TTY).
if [ "$IS_TTY" = "1" ]; then
  C_BOLD=$'\033[1m';  C_DIM=$'\033[2m'
  C_RED=$'\033[31m';  C_GREEN=$'\033[32m'
  C_YELLOW=$'\033[33m'; C_CYAN=$'\033[36m'
  C_RESET=$'\033[0m'
else
  C_BOLD=""; C_DIM=""; C_RED=""; C_GREEN=""
  C_YELLOW=""; C_CYAN=""; C_RESET=""
fi
section() { echo; echo "${C_BOLD}${C_CYAN}=== $* ===${C_RESET}"; }
info()    { echo "${C_DIM}$*${C_RESET}"; }
good()    { echo "${C_GREEN}$*${C_RESET}"; }
bad()     { echo "${C_RED}$*${C_RESET}"; }

case "${1:-}" in
  --help|-h)
    echo "Usage: $0"
    echo "  Removes sweeps/latest/logs/* (logs + multi-<ts>/ + failed-<ts>/)."
    echo "  Never touches data/ or plots/."
    exit 0
    ;;
  "") ;;
  *)
    bad "Unknown option: $1 — see $0 --help"
    exit 1
    ;;
esac

LOGS_DIR="sweeps/latest/logs"

# Human-readable dir size + top-level entry count (or "missing").
dir_size() {
  if [ -d "$1" ]; then
    sz=$(du -sh "$1" 2>/dev/null | awk '{print $1}')
    cnt=$(find "$1" -mindepth 1 -maxdepth 1 2>/dev/null | wc -l)
    echo "${sz}, ${cnt} entries"
  else
    echo "missing"
  fi
}

section "removing $LOGS_DIR/"
if [ -d "$LOGS_DIR" ] && [ -n "$(ls -A "$LOGS_DIR" 2>/dev/null)" ]; then
  info "  before: $(dir_size "$LOGS_DIR")"
  rm -rf "$LOGS_DIR"/* "$LOGS_DIR"/.[!.]* 2>/dev/null || true
  good "  removed all contents of $LOGS_DIR/"
else
  info "  (nothing to remove)"
fi

# Confirm data/plots untouched.
section "data + plots (untouched)"
for d in sweeps/latest/data sweeps/latest/plots; do
  if [ -d "$d" ]; then
    info "  $d  ($(dir_size "$d"))"
  fi
done
echo
