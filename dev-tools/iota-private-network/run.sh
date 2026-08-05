#!/bin/bash

# Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0


# Default validator count
NUM_VALIDATORS=4
FORCE=false
while getopts "n:fh" opt; do
  case "$opt" in
    n) NUM_VALIDATORS="$OPTARG" ;;
    f) FORCE=true ;;
    h|*) echo "Usage: $0 [-n num_validators] [-f] <modes...>"; exit 1 ;;
  esac
done
shift $((OPTIND -1))

# Refuse to bring up validators over another active run's network — same
# `iota-network` and container names, so recreates / port conflicts ensue.
# Bypass via IOTA_EXPERIMENT_LOCK_HELD or an explicit -f.
LOCK_PATH="/tmp/iota-experiments.lock"

# Lock state of $LOCK_PATH: 0 = free, 3 = held, 4 = cannot tell. Shared flock
# on a read-only FD avoids a false "locked" under fs.protected_regular; the
# python3 fcntl probe covers macOS, which lacks flock(1).
function experiment_lock_state() {
  if command -v flock >/dev/null 2>&1; then
    (flock -n -s 9) 9<"$LOCK_PATH" 2>/dev/null && return 0
    return 3
  fi
  if command -v python3 >/dev/null 2>&1; then
    python3 -c 'import fcntl, sys
try:
    fcntl.flock(open(sys.argv[1]), fcntl.LOCK_SH | fcntl.LOCK_NB)
except OSError:
    sys.exit(3)' "$LOCK_PATH" 2>/dev/null
    case "$?" in
      0) return 0 ;;
      3) return 3 ;;
    esac
  fi
  return 4
}

if [ "$FORCE" != "true" ] \
   && [ "${IOTA_EXPERIMENT_LOCK_HELD:-0}" != "1" ] \
   && [ -f "$LOCK_PATH" ]; then
  lock_state=0
  experiment_lock_state || lock_state=$?
  if [ "$lock_state" -eq 3 ]; then
    holder=$(cat "$LOCK_PATH" 2>/dev/null || true)
    echo "ERROR: another experiment run is active: ${holder:-(holder unknown)}" >&2
    echo "       Wait for it to finish, or pass -f to override." >&2
    exit 1
  elif [ "$lock_state" -ne 0 ]; then
    echo "ERROR: $LOCK_PATH exists but cannot be verified (need flock or a working python3)." >&2
    echo "       Install one, or pass -f if no experiment run is active." >&2
    exit 1
  fi
fi

function start_services() {
  services="$1"
  validators=""
  for ((i=1; i<=NUM_VALIDATORS; i++)); do
    validators="$validators validator-$i"
  done
  docker compose up -d $validators $services
}

modes=(
  [faucet]="fullnode-1 faucet-1"
  [backup]="fullnode-2"
  [indexer]="fullnode-3 indexer-1 postgres_primary"
  [indexer-cluster]="fullnode-3 indexer-1 postgres_primary fullnode-4 indexer-2 postgres_replica"
)

services_to_start=""
for mode in "$@"; do
  case $mode in
    all)
      services_to_start="fullnode-1 fullnode-2 fullnode-3 fullnode-4 indexer-1 indexer-2 postgres_primary postgres_replica"
      ;;
    faucet)
      services_to_start="$services_to_start fullnode-1 faucet-1"
      ;;
    backup)
      services_to_start="$services_to_start fullnode-2"
      ;;
    indexer)
      services_to_start="$services_to_start fullnode-3 indexer-1 postgres_primary"
      ;;
    indexer-cluster)
      services_to_start="$services_to_start fullnode-3 indexer-1 postgres_primary fullnode-4 indexer-2 postgres_replica"
      ;;
  esac
done

start_services "$services_to_start"