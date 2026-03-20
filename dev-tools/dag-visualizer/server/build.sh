#!/bin/bash
# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

# Determine script's location to resolve the relative path correctly
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null && pwd -P)"
DOCKER_BUILDKIT=1 "$SCRIPT_DIR/../../../docker/utils/build-script.sh" --image-tag "iotaledger/dag-visualizer-server" --dockerfile-dir "dev-tools/dag-visualizer/server"
