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

# Refuse to bring up validators on top of another active experiment's
# network — different docker compose project, same `iota-network` and
# container names; recreates / port conflicts ensue. Same env-var /
# explicit-flag bypass shape as cleanup.sh.
LOCK_PATH="/tmp/iota-experiments.lock"
# Read-only FD + shared flock: see cleanup.sh for the rationale (avoids
# false positives under fs.protected_regular when the lock file was left
# behind by a dead orchestrator from another user).
if [ "$FORCE" != "true" ] \
   && [ "${IOTA_EXPERIMENT_LOCK_HELD:-0}" != "1" ] \
   && [ -f "$LOCK_PATH" ] \
   && ! (flock -n -s 9) 9<"$LOCK_PATH" 2>/dev/null; then
  holder=$(cat "$LOCK_PATH" 2>/dev/null || true)
  echo "ERROR: another experiment run is active: ${holder:-(holder unknown)}" >&2
  echo "       Wait for it to finish, or pass -f to override." >&2
  exit 1
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