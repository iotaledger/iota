#!/bin/bash
# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
#
# Automatically update all snapshots. This is needed when the framework is changed or when protocol config is changed.

set -x
set -e

SCRIPT_PATH=$(realpath "$0")
SCRIPT_DIR=$(dirname "$SCRIPT_PATH")
ROOT="$SCRIPT_DIR/.."

cd "$ROOT/crates/iota-protocol-config" && cargo insta test --review
cd "$ROOT/crates/iota-swarm-config" && cargo insta test --review
cd "$ROOT/crates/iota-open-rpc" && cargo run --example generate-json-rpc-spec -- record
