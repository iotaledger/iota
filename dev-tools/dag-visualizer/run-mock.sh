#!/bin/bash
# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
#
# Launch the DAG visualizer with the mock backend (no real validator needed).
#
# Configuration via environment variables (all optional):
#   VALIDATORS=100       Number of validators (default: 10)
#   MISS_RATE=0.1        Block miss probability 0..1 (default: 0.05)
#   SKIP_RATE=0.3        Leader skip probability 0..1 (default: 0.15)
#   ROUND_INTERVAL_MS=300 Milliseconds between rounds (default: 500)
#   EQUIVOCATION_RATE=0.05 Equivocation probability per round (default: 0.02)
#   STALE_RATE=0.1       Stale block probability 0..1 (default: 0.08)
#   SLOW_ROUND_RATE=0.1  Slow round probability 0..1 (default: 0.08)
#   EPOCH=5              Current epoch number (default: 1)
#   TRAEFIK_PORT=8080    Host port for the UI (default: 80)
#
# Examples:
#   ./run-mock.sh
#   VALIDATORS=100 MISS_RATE=0.1 ./run-mock.sh
#   VALIDATORS=50 SKIP_RATE=0.3 TRAEFIK_PORT=8080 ./run-mock.sh
#
# Pass any extra arguments to docker compose (e.g. -d for detached, --build):
#   ./run-mock.sh -d
#   ./run-mock.sh --build

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

export VALIDATORS="${VALIDATORS:-10}"
export MISS_RATE="${MISS_RATE:-0.05}"
export SKIP_RATE="${SKIP_RATE:-0.15}"
export ROUND_INTERVAL_MS="${ROUND_INTERVAL_MS:-500}"
export EQUIVOCATION_RATE="${EQUIVOCATION_RATE:-0.02}"
export STALE_RATE="${STALE_RATE:-0.08}"
export SLOW_ROUND_RATE="${SLOW_ROUND_RATE:-0.08}"
export EPOCH="${EPOCH:-1}"
export TRAEFIK_PORT="${TRAEFIK_PORT:-80}"

echo "Starting DAG visualizer with mock backend"
echo "  validators=$VALIDATORS  miss_rate=$MISS_RATE  skip_rate=$SKIP_RATE"
echo "  round_interval=${ROUND_INTERVAL_MS}ms  equivocation_rate=$EQUIVOCATION_RATE"
echo "  stale_rate=$STALE_RATE  slow_round_rate=$SLOW_ROUND_RATE  epoch=$EPOCH"
echo "  UI at http://localhost:${TRAEFIK_PORT}"

exec docker compose -f "$SCRIPT_DIR/docker-compose.mock.yaml" up "$@"
