#!/bin/bash
# Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# Determine script's location to resolve the relative path correctly
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE}")" >/dev/null && pwd -P)
cd "$SCRIPT_DIR"

./../utils/build-script.sh --image-tag "iotaledger/iota-rest-kv"
