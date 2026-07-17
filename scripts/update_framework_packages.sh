#!/bin/bash
# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
#
# Rebuild iota-framework compiled packages and update the framework
# bytecode snapshot for the current (working) protocol version.
#
# Run this after making changes to Move modules in
# crates/iota-framework/packages/.

set -x
set -e

SCRIPT_PATH=$(realpath "$0")
SCRIPT_DIR=$(dirname "$SCRIPT_PATH")
ROOT="$SCRIPT_DIR/.."

function cleanup {
    popd > /dev/null 2>&1 || true
}
trap cleanup EXIT

# Step 1: Rebuild compiled packages (packages_compiled/, published_api.txt, docs)
pushd "$ROOT/crates/iota-framework"
UPDATE=1 cargo insta test
popd

# Step 2: Check if git is dirty before proceeding to update the snapshot
if [[ -n $(git status --porcelain) ]]; then
  echo "Git repository is dirty. Please commit or stash your changes before updating the snapshot."
  exit 1
fi

# Step 3: Update bytecode snapshot for the latest protocol version
pushd "$ROOT"
cargo run --release --bin iota-framework-snapshot
popd

# Step 4: Verify compatibility with all previous bytecode snapshots
cargo test --package iota-framework-snapshot --test compatibility_tests
