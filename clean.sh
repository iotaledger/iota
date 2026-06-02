#!/usr/bin/env bash
# clean.sh — remove sweep logs and (optionally) JSONL data + runs/.
#
# Logs (run.log, sweep.log): removed without prompt — they grow on every
# run and have no irreplaceable content.
#
# Data (sweep.jsonl + runs/): contains the actual measurement records and
# per-iter logs (including failed-* forensic snapshots), may represent
# hours of compute. Prompts for explicit confirmation unless --force.
#
# Usage:
#   ./clean.sh           interactive (prompts before removing data + runs/)
#   ./clean.sh --force   skip the prompt, remove EVERYTHING including data

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
warn()    { echo "${C_YELLOW}$*${C_RESET}"; }
good()    { echo "${C_GREEN}$*${C_RESET}"; }
bad()     { echo "${C_RED}$*${C_RESET}"; }

FORCE=0
case "${1:-}" in
  --force|-f) FORCE=1 ;;
  --help|-h)
    echo "Usage: $0 [--force]"
    echo "  no args  remove logs unconditionally, prompt for sweep.jsonl + runs/"
    echo "  --force  remove logs + sweep.jsonl + runs/ without prompt"
    exit 0
    ;;
  "") ;;
  *)
    bad "Unknown option: $1"
    echo "See: $0 --help"
    exit 1
    ;;
esac

LOG_FILES=(sweep.log run.log)
DATA_FILE=sweep.jsonl
RUNS_DIR=runs

# Human-readable file size (or "missing").
file_size() {
  if [ -f "$1" ]; then
    du -h "$1" 2>/dev/null | awk '{print $1}'
  else
    echo "missing"
  fi
}

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

# ---------- preview what would be removed ----------
section "logs (will be removed unconditionally)"
any_logs=0
for f in "${LOG_FILES[@]}"; do
  if [ -f "$f" ]; then
    info "  $f  ($(file_size "$f"))"
    any_logs=1
  else
    info "  $f  (missing — skip)"
  fi
done

section "data"
REMOVE_DATA=1
REMOVE_RUNS=1
HAS_ANY=0
if [ -f "$DATA_FILE" ]; then
  records=$(wc -l < "$DATA_FILE" 2>/dev/null || echo "?")
  size=$(file_size "$DATA_FILE")
  warn "  $DATA_FILE  (${C_BOLD}$size${C_RESET}${C_YELLOW}, ${C_BOLD}$records${C_RESET}${C_YELLOW} records)"
  HAS_ANY=1
else
  info "  $DATA_FILE  (missing — nothing to do)"
  REMOVE_DATA=0
fi
if [ -d "$RUNS_DIR" ]; then
  warn "  $RUNS_DIR/  (${C_BOLD}$(dir_size "$RUNS_DIR")${C_RESET}${C_YELLOW}) — per-iter logs, failed-* forensic snapshots, gas-pool cache"
  HAS_ANY=1
else
  info "  $RUNS_DIR/  (missing — nothing to do)"
  REMOVE_RUNS=0
fi

if [ "$HAS_ANY" = "1" ]; then
  if [ "$FORCE" = "1" ]; then
    bad "  --force set: removing without prompt"
  else
    echo
    printf "%b" "${C_BOLD}${C_RED}Remove research data above? This deletes collected JSONL + per-iter run logs.${C_RESET} [y/N] "
    read -r REPLY
    case "${REPLY:-N}" in
      y|Y|yes|YES) ;;
      *)
        REMOVE_DATA=0
        REMOVE_RUNS=0
        good "  Keeping $DATA_FILE and $RUNS_DIR/"
        ;;
    esac
  fi
fi

# ---------- actually remove ----------
section "cleaning"
removed=0
for f in "${LOG_FILES[@]}"; do
  if [ -f "$f" ]; then
    rm -f "$f"
    good "  removed $f"
    removed=$((removed + 1))
  fi
done
if [ -f "$DATA_FILE" ] && [ "$REMOVE_DATA" = "1" ]; then
  rm -f "$DATA_FILE"
  good "  removed $DATA_FILE"
  removed=$((removed + 1))
fi
if [ -d "$RUNS_DIR" ] && [ "$REMOVE_RUNS" = "1" ]; then
  rm -rf "$RUNS_DIR"
  good "  removed $RUNS_DIR/"
  removed=$((removed + 1))
fi

if [ "$removed" = "0" ]; then
  info "  (nothing removed)"
fi
echo
