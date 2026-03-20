#!/bin/bash
# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
#
# Build the DAG visualizer mock stack.
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

exec docker compose -f "$SCRIPT_DIR/docker-compose.mock.yaml" build "$@"
