#!/bin/sh
# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# fast fail.
set -e

DIR="$( cd "$( dirname "$0" )" && pwd )"
REPO_ROOT="$(git rev-parse --show-toplevel)"
DOCKERFILE="$DIR/Dockerfile"
GIT_REVISION="$(git describe --always --abbrev=12 --dirty --exclude '*')"
BUILD_DATE="$(date -u +'%Y-%m-%d')"

echo
echo "Building iota-services docker image"
echo "Dockerfile: \t$DOCKERFILE"
echo "docker context: $REPO_ROOT"
echo "build date: \t$BUILD_DATE"
echo "git revision: \t$GIT_REVISION"
echo

# TODO: Remove "--ssh default" when iota-sim is public https://github.com/iotaledger/iota/issues/2149
docker build --ssh default -f "$DOCKERFILE" "$REPO_ROOT" \
	--build-arg GIT_REVISION="$GIT_REVISION" \
	--build-arg BUILD_DATE="$BUILD_DATE" \
	"$@"
