#!/bin/bash
# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
#
# Check whether the version of framework in the repo is compatible
# with the version on chain, as reported by the currently active
# environment, using the binary in environment variable $IOTA.

set -e

IOTA=${IOTA:-iota}
REPO=$(git rev-parse --show-toplevel)

for PACKAGE in "$REPO"/crates/iota-framework/packages/*; do
    $IOTA client verify-source "$PACKAGE"
done

