#!/bin/bash
TARGET_FOLDER="../.."

# fast fail.
set -e

REPO_ROOT="$(git rev-parse --show-toplevel)"

# Source common.sh from the utils directory
source "$REPO_ROOT/scripts/utils/common.sh"

# Parse command line arguments
# Usage:
# --target-folder <path>        - the target folder of the repository
while [ $# -gt 0 ]; do
    # error on unknown arguments
    if [[ $1 != *"--target-folder"* ]]; then
        echo "Unknown argument: $1"
        echo "Usage: $0 [--target-folder <path>]"
        exit 1
    fi

    if [[ $1 == *"--target-folder"* ]]; then
        TARGET_FOLDER=$2
    fi

    shift
done

# Resolve the target folder
TARGET_FOLDER=$(realpath ${TARGET_FOLDER})

function docker_run {
    docker run --rm --name pnpm-cargo-image -v ${TARGET_FOLDER}:/home/node/app:rw --user $(id -u):$(id -g) pnpm-cargo-image sh -c "$1"
}

print_step "Parse the rust toolchain version from 'rust-toolchain.toml'..."
RUST_VERSION=$(grep -oE 'channel = "[^"]+' ./../../rust-toolchain.toml | sed 's/channel = "//')
if [ -z "$RUST_VERSION" ]; then
    print_error "Failed to parse the rust toolchain version"
    exit 1
fi

print_step "Building pnpm-cargo docker image with rust version ${RUST_VERSION}..."
docker build --build-arg RUST_VERSION=${RUST_VERSION} --build-arg USER_ID=$(id -u) -t pnpm-cargo-image -f ./Dockerfile .
check_error "Failed to build pnpm-cargo docker image"

print_step "Changing directory to ${TARGET_FOLDER}"
pushd ${TARGET_FOLDER}

# add cleanup hook to return to original folder
function cleanup {
    popd
}

trap cleanup EXIT

print_step "Generating open rpc schema..."
cargo run --package iota-open-rpc --example generate-json-rpc-spec -- record
check_error "Failed to generate open rpc schema"

echo -e "\e[32mGenerating graphql schema..."
cargo run --package iota-graphql-rpc generate-schema --file ./crates/iota-graphql-rpc/schema.graphql
check_error "Failed to generate graphql schema"