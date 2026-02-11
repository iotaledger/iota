#!/bin/bash
# Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
#
# Update gRPC protobuf types.
SCRIPT_PATH=$(realpath "$0")
SCRIPT_DIR=$(dirname "$SCRIPT_PATH")
ROOT="$SCRIPT_DIR/.."

pushd "$ROOT"

function cleanup() {
    popd
}

trap cleanup EXIT

rm -Rf crates/iota-grpc-types/src/proto/generated/
mkdir -p crates/iota-grpc-types/src/proto/generated/
cargo run -p iota-proto-build
exit_code=$?

if [ $exit_code -eq 2 ]; then
    echo "Warning: Generated protobuf files have uncommitted changes."
    echo "The generation completed successfully, but you should commit the changes."
    exit 0  # Treat as success since generation worked
elif [ $exit_code -ne 0 ]; then
    echo "Error: Failed to generate protobuf files (exit code: $exit_code)"
    exit $exit_code
fi
