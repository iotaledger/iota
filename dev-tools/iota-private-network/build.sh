#!/bin/bash
set -e

# Determine script's location to resolve the relative path correctly
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null && pwd -P)"

# Go to ../../docker/iota-node with pushd and build the image
pushd "$SCRIPT_DIR/../../docker/iota-node"
./build.sh
popd

# Go to ../../docker/iota-indexer with pushd and build the image
pushd "$SCRIPT_DIR/../../docker/iota-indexer"
./build.sh
popd

# Go to ../../docker/iota-tools with pushd and build the image
pushd "$SCRIPT_DIR/../../docker/iota-tools"
./build.sh
popd