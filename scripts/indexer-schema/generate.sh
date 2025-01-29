#!/bin/bash
# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
#
# Update iota-indexer's generated src/schema.rs based on the schema after
# running all its migrations on a clean database.
set -x
set -e

if ! command -v git &> /dev/null; then
    echo "git not installed" >&2
    exit 1
fi

REPO=$(git rev-parse --show-toplevel)

# Set up the Docker container with PostgreSQL and Diesel CLI
POSTGRES_USER=postgres
POSTGRES_PASSWORD=postgrespw
CONTAINER_NAME=postgres-rust-diesel

# Ensure the required image is built
if ! docker image inspect ${CONTAINER_NAME} &> /dev/null; then
    ${REPO}/scripts/indexer-schema/build.sh
fi

function cleanup {
  # Cleanup: Stop and remove the container
  docker stop ${CONTAINER_NAME}
}
trap cleanup EXIT

docker run --rm -d \
    --name ${CONTAINER_NAME} \
    -e POSTGRES_USER=${POSTGRES_USER} \
    -e POSTGRES_PASSWORD=${POSTGRES_PASSWORD} \
    -e RUST_TOOLCHAIN_VERSION=${RUST_TOOLCHAIN_VERSION} \
    -v "${REPO}:/workspace" \
    -w /workspace \
    ${CONTAINER_NAME}

# Wait for Postgres to be ready
RETRIES=0
while ! docker exec ${CONTAINER_NAME} pg_isready -p 5432 --username ${POSTGRES_USER}; do
  if [ $RETRIES -gt 30 ]; then
    echo "Postgres failed to start" >&2
    docker stop ${CONTAINER_NAME}
    exit 1
  fi
  sleep 1
  RETRIES=$((RETRIES + 1))
done

# Run migrations and generate the schema.rs file
docker exec ${CONTAINER_NAME} diesel migration run \
  --database-url "postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@localhost:5432" \
  --migration-dir "/workspace/crates/iota-indexer/migrations/pg"

docker exec ${CONTAINER_NAME} diesel print-schema \
  --database-url "postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@localhost:5432" \
  --patch-file "/workspace/crates/iota-indexer/src/schema.patch" \
  --except-tables "^objects_version_|_partition_" \
  > "${REPO}/crates/iota-indexer/src/schema.rs"

# Applying the patch may destroy the formatting, fix it
rustfmt +nightly "${REPO}/crates/iota-indexer/src/schema.rs"
