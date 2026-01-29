#!/bin/bash
# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# fast fail.
set -e

REPO_ROOT="$(git rev-parse --show-toplevel)"

# Source common.sh from the utils directory
source "$REPO_ROOT/scripts/utils/common.sh"

GIT_REVISION="$(git describe --always --abbrev=12 --dirty --exclude '*')"
BUILD_DATE="$(date -u +'%Y-%m-%d')"
PROFILE="release"
TARGET_FOLDER="target/$PROFILE"
# If the build profile is dev, set the target folder to debug
if [ "$PROFILE" = "dev" ]; then
    TARGET_FOLDER="target/debug"
fi
IMAGE_TAG=""

# Parse command line arguments
# Usage:
# --image-tag <image_tag> - the name and tag of the image
while [ "$#" -gt 0 ]; do
    case "$1" in
        --image-tag=*) 
            IMAGE_TAG="${1#*=}"
            shift
            ;;
        --image-tag) 
            IMAGE_TAG="$2"
            shift 2
            ;;
        *) 
            print_error "Unknown argument: $1"
            print_step "Usage: $0 --image-tag <image_tag>"
            exit 1
            ;;
    esac
done

# check if the image tag is set
if [ -z "$IMAGE_TAG" ]; then
    print_error "Image tag is not set"
    print_step "Usage: $0 --image-tag <image_tag>"
    exit 1
fi

DOCKERFILE="$REPO_ROOT/docker/$(basename "${IMAGE_TAG%%:*}")/Dockerfile"

print_step "Parse the rust toolchain version from 'rust-toolchain.toml'..."
RUST_VERSION=$(grep -oE 'channel = "[^"]+' ${REPO_ROOT}/rust-toolchain.toml | sed 's/channel = "//')
if [ -z "$RUST_VERSION" ]; then
    print_error "Failed to parse the rust toolchain version"
    exit 1
fi
RUST_IMAGE_VERSION=${RUST_VERSION}-trixie

echo
echo "Building \"$IMAGE_TAG\" docker image"
echo "Dockerfile:                 $DOCKERFILE"
echo "docker context:             $REPO_ROOT"
echo "profile:                    $PROFILE"
echo "builder rust image version: $RUST_IMAGE_VERSION"
echo "cargo build features:       $CARGO_BUILD_FEATURES"
echo "build date:                 $BUILD_DATE"
echo "git revision:               $GIT_REVISION"
echo

# Check if we should use cache mounts
if [ "${DOCKER_BUILDKIT:-0}" = "1" ]; then
	print_step "Cache mounts enabled - creating temporary Dockerfile with cache support"
	DOCKERFILE_TMP="${DOCKERFILE}.cache"

	# Add BuildKit syntax and inject cache mounts before cargo build
	{
		echo "# syntax=docker/dockerfile:1"
		sed 's/^RUN cargo build --profile \${PROFILE}/RUN --mount=type=cache,target=\/usr\/local\/cargo\/registry \\\
    --mount=type=cache,target=\/usr\/local\/cargo\/git \\\
    --mount=type=cache,target=\/iota\/target,sharing=locked \\\
    cargo build --profile ${PROFILE}/' "$DOCKERFILE"
	} > "$DOCKERFILE_TMP"
	
	DOCKERFILE="$DOCKERFILE_TMP"
	
	# Ensure cleanup on exit
	trap "rm -f $DOCKERFILE_TMP" EXIT
fi

docker build -f "$DOCKERFILE" "$REPO_ROOT" \
	-t ${IMAGE_TAG} \
	--build-arg RUST_IMAGE_VERSION="${RUST_IMAGE_VERSION}" \
	--build-arg PROFILE="$PROFILE" \
	--build-arg TARGET_FOLDER="$TARGET_FOLDER" \
	--build-arg CARGO_BUILD_FEATURES="$CARGO_BUILD_FEATURES" \
	--build-arg BUILD_DATE="$BUILD_DATE" \
	--build-arg GIT_REVISION="$GIT_REVISION" \
	--target runtime \
	"$@"
