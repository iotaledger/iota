#!/bin/bash

# Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

if [ ! -d "./data" ]; then
  echo "Please run './bootstrap.sh' first"
  exit
fi

function start_services() {
  services="$1"
  docker compose up -d validator-1 validator-2 validator-3 validator-4 $services
}

declare -A modes
modes=(
  [faucet]="fullnode-1 faucet-1"
  [backup]="fullnode-2"
  [indexer]="fullnode-3 indexer-1 postgres_primary"
  [indexer-cluster]="fullnode-3 indexer-1 postgres_primary fullnode-4 indexer-2 postgres_replica"
)

services_to_start=""

if [ $# -eq 0 ]; then
  services_to_start="fullnode-1 fullnode-2 fullnode-3 fullnode-4 indexer-1 indexer-2 postgres_primary postgres_replica"
else
  for mode in "$@"; do
    if [[ $mode == "all" ]]; then
      services_to_start="fullnode-1 fullnode-2 fullnode-3 fullnode-4 indexer-1 indexer-2 postgres_primary postgres_replica"
      break
    else
      services_to_start="$services_to_start ${modes[$mode]}"
    fi
  done
fi

start_services "$services_to_start"