#!/bin/bash
# Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
#
# Update gRPC protobuf types.
set -e

SCRIPT_PATH=$(realpath "$0")
SCRIPT_DIR=$(dirname "$SCRIPT_PATH")
ROOT="$SCRIPT_DIR/.."

pushd "$ROOT"

function cleanup() {
    popd
}

trap cleanup EXIT

rm -Rf crates/iota-grpc-types/src/proto_generated/
mkdir -p crates/iota-grpc-types/src/proto_generated/
cargo run -p iota-proto-build
