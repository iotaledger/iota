#!/bin/bash
# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# Get the directory where build-script.sh is located
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

# Source common.sh from the utils directory
source "$SCRIPT_DIR/common.sh"

# fast fail.
set -e

# Get the current working directory where the script was called
CURRENT_WORKING_DIR="$(pwd)"

REPO_ROOT="$(git rev-parse --show-toplevel)"
DOCKERFILE="$CURRENT_WORKING_DIR/Dockerfile"
GIT_REVISION="$(git describe --always --abbrev=12 --dirty --exclude '*')"
BUILD_DATE="$(date -u +'%Y-%m-%d')"
PROFILE="release"
CONTAINER_NAME=""

# Parse command line arguments
# Usage:
# --container-name <container_name> - the name of the container
while [ "$#" -gt 0 ]; do
    case "$1" in
        --container-name=*) 
            CONTAINER_NAME="${1#*=}"
            shift
            ;;
        --container-name) 
            CONTAINER_NAME="$2"
            shift 2
            ;;
        *) 
            print_error "Unknown argument: $1"
            print_step "Usage: $0 --container-name <container_name>"
            exit 1
            ;;
    esac
done

# check if the container name is set
if [ -z "$CONTAINER_NAME" ]; then
    print_error "Container name is not set"
    print_step "Usage: $0 --container-name <container_name>"
    exit 1
fi

print_step "Parse the rust toolchain version from 'rust-toolchain.toml'..."
RUST_VERSION=$(grep -oE 'channel = "[^"]+' ${REPO_ROOT}/rust-toolchain.toml | sed 's/channel = "//')
if [ -z "$RUST_VERSION" ]; then
    print_error "Failed to parse the rust toolchain version"
    exit 1
fi
RUST_IMAGE_VERSION=${RUST_VERSION}-bookworm

echo
echo "Building \"$CONTAINER_NAME\" docker image"
echo "Dockerfile:                 $DOCKERFILE"
echo "docker context:             $REPO_ROOT"
echo "profile: 					  $PROFILE"
echo "builder rust image version: $RUST_IMAGE_VERSION"
echo "cargo build features:       $CARGO_BUILD_FEATURES"
echo "build date:                 $BUILD_DATE"
echo "git revision:               $GIT_REVISION"
echo

docker build -f "$DOCKERFILE" "$REPO_ROOT" \
	-t ${CONTAINER_NAME} \
	--build-arg RUST_IMAGE_VERSION="${RUST_IMAGE_VERSION}" \
	--build-arg PROFILE="$PROFILE" \
	--build-arg CARGO_BUILD_FEATURES="$CARGO_BUILD_FEATURES" \
	--build-arg BUILD_DATE="$BUILD_DATE" \
	--build-arg GIT_REVISION="$GIT_REVISION" \
	--target runtime \
	"$@"
