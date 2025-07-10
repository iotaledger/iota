#!/bin/bash

# Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

nval=4  # default number of validators

if [ ! -d "./data" ]; then
  echo "Please run './bootstrap.sh' first"
  exit
fi

while [[ $# -gt 0 ]]; do
  case "$1" in
    -nval)
      shift
      nval=$1
      shift
      ;;
    *)
      break
      ;;
  esac
done

function start_services() {
  services="$1"
  validators=""
  for ((i=1; i<=nval; i++)); do
    validators="$validators validator-$i"
  done
  docker compose up -d $validators $services
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