#!/usr/bin/env bash
#
# restart.sh — full restart of the local stress-test stack, in order:
#   1. cleanup.sh    tear down monitoring + network and wipe data (best-effort)
#   2. bootstrap.sh  regenerate genesis / validator configs
#   3. start.sh      bring the network up, verify overrides, start monitoring
#
# Arg routing (single command line; put options before modes):
#   -n N        number of validators           -> bootstrap AND start (kept in sync)
#   -e MS       epoch duration in ms            -> bootstrap only
#   -b          benchmark mode (gas accounts)   -> bootstrap only
#   <modes...>  faucet | backup | indexer | ... -> start (forwarded to run.sh)
#
# start.sh's env knobs (MODE, ATTEST, PCOOL, MAX_*) are inherited automatically.
#
# Needs root (cleanup + bootstrap require it), e.g.:
#   sudo ./restart.sh -b -n 4 faucet
#   sudo MODE=TotalGasBudget ./restart.sh -b -n 4 faucet
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [[ "$OSTYPE" != darwin* && $EUID -ne 0 ]]; then
  echo "ERROR: run with sudo — cleanup and bootstrap require root." >&2
  exit 1
fi

if [[ -t 1 ]]; then MAGENTA=$'\033[0;35m'; RESET=$'\033[0m'; else MAGENTA=''; RESET=''; fi

# Route args: -n to both, -e/-b to bootstrap, remaining positionals to start.
boot_args=()
start_args=()
while getopts "n:e:b" opt; do
  case "$opt" in
    n) boot_args+=(-n "$OPTARG"); start_args+=(-n "$OPTARG") ;;
    e) boot_args+=(-e "$OPTARG") ;;
    b) boot_args+=(-b) ;;
    *) echo "Usage: $0 [-n N] [-e EPOCH_MS] [-b] [modes...]" >&2; exit 1 ;;
  esac
done
shift $((OPTIND - 1))
start_args+=("$@")

echo "${MAGENTA}== restart [1/3] cleanup ==${RESET}"
"$SCRIPT_DIR/cleanup.sh" || true
echo

echo "${MAGENTA}== restart [2/3] bootstrap ${boot_args[*]} ==${RESET}"
"$SCRIPT_DIR/bootstrap.sh" "${boot_args[@]}"
echo

echo "${MAGENTA}== restart [3/3] start ${start_args[*]} ==${RESET}"
"$SCRIPT_DIR/start.sh" "${start_args[@]}"
