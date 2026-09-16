#!/usr/bin/env bash
# kill.sh — terminate everything launched by run.sh.
#
# Kills (in order):
#   1. The run.sh orchestrator (run_inner.sh, so it stops spawning new sweep.sh).
#   2. The currently-running sweep.sh.
#   3. stress-multi.sh and its 25+ stress.rs child processes.
#   4. Tears down grafana + iota-private-network docker stacks.
#
# Uses SIGINT first, escalates to SIGTERM, then SIGKILL. Anything still
# alive after that is reported.
#
# Usage: ./kill.sh

set -uo pipefail
cd "$(dirname "$0")"

# Detect TTY once at the top. Must be checked here (not in $() subshells).
if [ -t 1 ]; then IS_TTY=1; else IS_TTY=0; fi

# Colors (auto-disable when stdout is not a TTY).
if [ "$IS_TTY" = "1" ]; then
  C_BOLD=$'\033[1m';  C_DIM=$'\033[2m'
  C_RED=$'\033[31m';  C_GREEN=$'\033[32m'
  C_YELLOW=$'\033[33m'; C_BLUE=$'\033[34m'
  C_CYAN=$'\033[36m'; C_RESET=$'\033[0m'
else
  C_BOLD=""; C_DIM=""; C_RED=""; C_GREEN=""
  C_YELLOW=""; C_BLUE=""; C_CYAN=""; C_RESET=""
fi
section() { echo; echo "${C_BOLD}${C_CYAN}=== $* ===${C_RESET}"; }
info()    { echo "${C_DIM}$*${C_RESET}"; }
warn()    { echo "${C_YELLOW}$*${C_RESET}"; }
good()    { echo "${C_GREEN}$*${C_RESET}"; }
bad()     { echo "${C_RED}$*${C_RESET}"; }

# Process patterns we want to clean up. run_inner.sh is the orchestrator
# launched by run.sh via nohup; sweep.sh / stress-multi.sh are its
# children; target/release/stress are stress-multi.sh's children.
PATTERNS=(
  "run_inner.sh"
  "sweep.sh"
  "stress-multi.sh"
  "target/release/stress "
)

show_running() {
  for p in "${PATTERNS[@]}"; do
    pgrep -af "$p" 2>/dev/null
  done | sort -u
}

kill_with() {
  local sig="$1"
  for p in "${PATTERNS[@]}"; do
    pkill "$sig" -f "$p" 2>/dev/null || true
  done
}

section "before kill: matching processes"
running="$(show_running)"
if [ -z "$running" ]; then
  good "  (none)"
else
  echo "$running" | sed 's/^/  /'
fi

section "sending SIGINT to orchestrator + scripts"
kill_with -INT
info "  waiting 5s for graceful exit..."
sleep 5

section "escalating to SIGTERM"
kill_with -TERM
sleep 2

section "final SIGKILL on stragglers"
kill_with -9
sleep 1

section "docker teardown"
info "  stopping grafana + prometheus..."
(cd dev-tools/grafana-local && docker compose down 2>&1 | tail -3) || true
info "  stopping iota private network..."
(cd dev-tools/iota-private-network && docker compose down -v 2>&1 | tail -3) || true

section "after kill: matching processes"
remaining="$(show_running)"
if [ -z "$remaining" ]; then
  good "  (all clear)"
  exit 0
else
  echo "$remaining" | sed 's/^/  /'
  bad "WARNING: some processes still running. Inspect and kill manually if needed:"
  bad "  pgrep -af 'run_inner.sh|sweep.sh|stress-multi.sh|target/release/stress '"
  exit 1
fi
