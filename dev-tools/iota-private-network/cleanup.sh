#!/bin/bash
# Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

if [[ "$OSTYPE" != "darwin"* && "$EUID" -ne 0 ]]; then
  echo "Please run as root or with sudo"
  exit
fi

DAG_VIZ=false
while getopts "d" opt; do
  case "$opt" in
    d) DAG_VIZ=true ;;
    *) echo "Usage: $0 [-d]"; exit 1 ;;
  esac
done

COMPOSE_FILES="-f docker-compose.yaml"
if $DAG_VIZ; then
  COMPOSE_FILES="$COMPOSE_FILES -f docker-compose.dag-viz.yaml"
fi

docker compose $COMPOSE_FILES down --remove-orphans
rm -rf data