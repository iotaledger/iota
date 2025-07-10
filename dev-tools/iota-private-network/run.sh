#!/bin/bash

# Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

nval=4  # default
if [[ $1 =~ ^[0-9]+$ ]]; then
  nval=$1
  shift
fi

function start_services() {
  services="$1"
  validators=""
  for ((i=1; i<=nval; i++)); do
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