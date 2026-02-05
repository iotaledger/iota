#!/bin/bash
# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# Determine script's location to resolve the relative path correctly
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null && pwd -P)"
# The name of last directory in script's path is used for tag
BASE_NAME="$(basename "$SCRIPT_DIR")"

# Set RUSTFLAGS for debug info
export RUSTFLAGS="-C debuginfo=2"

DOCKER_BUILDKIT=1 "$SCRIPT_DIR/../utils/build-script.sh" --image-tag "iotaledger/$BASE_NAME"
