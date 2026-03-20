#!/bin/bash
# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
#
# Stop the DAG visualizer mock stack.
#
# Pass any extra arguments to docker compose (e.g. -v to remove volumes):
#   ./stop-mock.sh
#   ./stop-mock.sh -v

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

exec docker compose -f "$SCRIPT_DIR/docker-compose.mock.yaml" down "$@"
