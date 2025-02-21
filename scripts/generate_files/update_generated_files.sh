#!/bin/bash
TARGET_FOLDER="../.."
SKIP_SPEC_GENERATION=false
SKIP_TS_GENERATION=false
CHECK_BUILDS=false

# fast fail.
set -e

REPO_ROOT="$(git rev-parse --show-toplevel)"

# Source common.sh from the utils directory
source "$REPO_ROOT/scripts/utils/common.sh"

# Parse command line arguments
# Usage:
# --target-folder <path>        - the target folder of the repository
# --skip-spec-generation        - skip the generation of the open rpc and graphql schema
# --skip-ts-generation          - skip the generation of the typescript files
# --check-builds                - run a build check after the generation of the files
while [ $# -gt 0 ]; do
    # error on unknown arguments
    if [[ $1 != *"--target-folder"* && $1 != *"--skip-spec-generation"* && $1 != *"--skip-ts-generation"* && $1 != *"--check-builds"* ]]; then
        echo "Unknown argument: $1"
        echo "Usage: $0 [--target-folder <path>] [--skip-spec-generation] [--skip-ts-generation] [--check-builds]"
        exit 1
    fi

    if [[ $1 == *"--target-folder"* ]]; then
        TARGET_FOLDER=$2
    fi

    if [[ $1 == *"--skip-spec-generation"* ]]; then
        SKIP_SPEC_GENERATION=true
    fi

    if [[ $1 == *"--skip-ts-generation"* ]]; then
        SKIP_TS_GENERATION=true
    fi

    if [[ $1 == *"--check-builds"* ]]; then
        CHECK_BUILDS=true
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

# if the spec generation is not skipped, generate the spec
if [ "$SKIP_SPEC_GENERATION" = false ]; then
    print_step "Generating open rpc schema..."
    cargo run --package iota-open-rpc --example generate-json-rpc-spec -- record
    check_error "Failed to generate open rpc schema"

    echo -e "\e[32mGenerating graphql schema..."
    cargo run --package iota-graphql-rpc generate-schema --file ./crates/iota-graphql-rpc/schema.graphql
    check_error "Failed to generate graphql schema"
fi

if [ "$SKIP_TS_GENERATION" = true ]; then
    # skipping the generation of the typescript files
    exit 0
fi

print_step "Installing typescript sdk dependencies..."
docker_run "cd sdk/typescript && pnpm i"
check_error "Failed to install typescript sdk dependencies"

print_step "Installing graphql-transport dependencies..."
docker_run "cd sdk/graphql-transport && pnpm i"
check_error "Failed to install graphql-transport dependencies"

print_step "Updating open rpc client types..."
docker_run "cd sdk/typescript && pnpm update-open-rpc-client-types"
check_error "Failed to update open rpc client types"

print_step "Create graphql schema in 'client/types/generated.ts'..."
docker_run "cd sdk/typescript && pnpm update-graphql-schemas"
check_error "Failed to create graphql schema"

print_step "Generating graphql-transport typescript types..."
docker_run "cd sdk/graphql-transport && pnpm codegen"
check_error "Failed to generate graphql-transport typescript types"

if [ "$CHECK_BUILDS" = true ]; then
    print_step "Run a typescript sdk build to check if the generated files are correct..."
    docker_run "cd sdk/typescript && pnpm run build"
    check_error "Failed to build typescript sdk"

    print_step "Run a graphql-transport build to check if the generated files are correct..."
    docker_run "cd sdk/graphql-transport && pnpm run build"
    check_error "Failed to build graphql-transport"

    print_step "Run a complete turbo build to catch possible issues in other SDK modules or apps..."
    docker_run "pnpm turbo build"
    check_error "Failed to build the complete project"    
fi