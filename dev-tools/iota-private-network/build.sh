#!/bin/bash
set -e

DAG_VIZ=false
while getopts "d" opt; do
  case "$opt" in
    d) DAG_VIZ=true ;;
    *) echo "Usage: $0 [-d]"; exit 1 ;;
  esac
done

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

if $DAG_VIZ; then
  echo "Building DAG visualizer images..."

  pushd "$SCRIPT_DIR/../dag-visualizer/server"
  ./build.sh
  popd

  pushd "$SCRIPT_DIR/../dag-visualizer/frontend"
  ./build.sh
  popd

  docker compose -f docker-compose.yaml -f docker-compose.dag-viz.yaml build validator-1
fi