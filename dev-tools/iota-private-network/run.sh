#!/bin/bash

# Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0


# Default validator count and consensus
NUM_VALIDATORS=4
PROTOCOL="starfish"
while getopts "n:p:" opt; do
  case "$opt" in
    n) NUM_VALIDATORS="$OPTARG" ;;
    p) PROTOCOL="$OPTARG" ;;
    *) echo "Usage: $0 [-n num_validators]"; exit 1 ;;
  esac
done
shift $((OPTIND -1))
PRIVNET_DIR="$(realpath "$(dirname "$0")" || echo "$ROOT/dev-tools/iota-private-network")"

# Set and unset environment variables
set_env_var() {
  # Usage: set_env_var KEY VALUE FILE
  local key="$1" value="$2" file="$3"
  mkdir -p "$(dirname "$file")"
  if [ -f "$file" ]; then
    if grep -q "^${key}=" "$file"; then
      # portable in-place replace without sed -i
      tmpfile="$(mktemp)"
      awk -v k="$key" -v v="$value" 'BEGIN{changed=0} {if ($0 ~ "^"k"=") {print k"="v; changed=1} else {print}} END{if (!changed) print k"="v}' "$file" > "$tmpfile" && mv "$tmpfile" "$file"
    else
      echo "${key}=${value}" >> "$file"
    fi
  else
    echo "${key}=${value}" > "$file"
  fi
}
unset_env_var() {
  # Usage: unset_env_var KEY FILE
  local key="$1" file="$2"
  [ -f "$file" ] || return 0
  tmpfile="$(mktemp)"
  awk -v k="$key" 'BEGIN{removed=0} $0 !~ "^"k"=" {print} $0 ~ "^"k"=" {removed=1} END{}' "$file" > "$tmpfile" && mv "$tmpfile" "$file"
}

# Manage CONSENSUS_PROTOCOL in .env: set if provided, otherwise remove it
ENV_FILE="$PRIVNET_DIR/.env"
set_env_var CONSENSUS_PROTOCOL "$PROTOCOL" "$ENV_FILE"
echo "Set CONSENSUS_PROTOCOL=$PROTOCOL in $ENV_FILE"



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