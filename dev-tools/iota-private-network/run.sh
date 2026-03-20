#!/bin/bash
# Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# Default validator count
NUM_VALIDATORS=4
DAG_VIZ=false
while getopts "n:d" opt; do
  case "$opt" in
    n) NUM_VALIDATORS="$OPTARG" ;;
    d) DAG_VIZ=true ;;
    *) echo "Usage: $0 [-n num_validators] [-d]"; exit 1 ;;
  esac
done
shift $((OPTIND -1))

COMPOSE_FILES="-f docker-compose.yaml"
if $DAG_VIZ; then
  COMPOSE_FILES="$COMPOSE_FILES -f docker-compose.dag-viz.yaml"
  echo "DAG visualizer enabled (-d flag)"
fi

function start_services() {
  services="$1"
  validators=""
  for ((i=1; i<=NUM_VALIDATORS; i++)); do
    validators="$validators validator-$i"
  done
  if $DAG_VIZ; then
    services="$services dag-visualizer-server dag-visualizer-frontend dag-viz-traefik"
  fi
  docker compose $COMPOSE_FILES up -d $validators $services
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