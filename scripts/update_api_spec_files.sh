#!/bin/bash
TARGET_FOLDER=".."

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

print_step "Generating graphql schema..."
cargo run --package iota-graphql-rpc generate-schema --file ./crates/iota-graphql-rpc/schema.graphql
check_error "Failed to generate graphql schema"