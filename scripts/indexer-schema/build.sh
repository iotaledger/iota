#!/bin/bash
set -x
set -e

if ! command -v git &> /dev/null; then
    echo "git not installed" >&2
    exit 1
fi

REPO_ROOT=$(git rev-parse --show-toplevel)

echo "Parse the rust toolchain version from 'rust-toolchain.toml'..."
RUST_TOOLCHAIN_VERSION=$(grep -oE 'channel = "[^"]+' ${REPO_ROOT}/rust-toolchain.toml | sed 's/channel = "//')
if [ -z "$RUST_TOOLCHAIN_VERSION" ]; then
    echo "Failed to parse the rust toolchain version"
    exit 1
fi

docker build --build-arg RUST_TOOLCHAIN_VERSION=${RUST_TOOLCHAIN_VERSION} -t postgres-rust-diesel ${REPO_ROOT}/scripts/indexer-schema
