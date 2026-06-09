#!/bin/bash
# Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# fast fail.
set -e

REPO_ROOT="$(git rev-parse --show-toplevel)"

# Source common.sh from the utils directory
source "$REPO_ROOT/scripts/utils/common.sh"

# Change to the root directory of the repository
pushd "../"

function cleanup {
    print_step "Stopping the iota node..."
    kill -9 $IOTA_PID 2> /dev/null

    if [ $CHANGED_TO_LIGHT_CLIENT_DIR -eq 1 ]; then
        popd
    fi

    popd
}

trap cleanup EXIT

print_step "Build the iota-localnet binary in release mode..."
cargo build --release --bin iota-localnet
check_error "Failed to build the iota-localnet binary in release mode"

print_step "Build the iota-light-client 'generate_chk_snapshots' crate in debug mode..."
cargo build --bin generate_chk_snapshots
check_error "Failed to build the iota-light-client 'generate_chk_snapshots' crate in debug mode"

print_step "Start the iota localnet in the background..."
cargo run --release --bin iota-localnet -- start --force-regenesis --epoch-duration-ms 5000 --with-faucet > /dev/null 2>&1 &

# Capture the PID of the iota node to stop it later
IOTA_PID=$!

print_step "Wait 40s for the localnet to start and advance some epochs..."
sleep 40

# Change to the directory of the iota-light-client crate
pushd "crates/iota-light-client/"
CHANGED_TO_LIGHT_CLIENT_DIR=1

print_step "Clear the old checkpoints file..."
cat << EOF > example_config/checkpoints.yaml
---
checkpoints: []
EOF

print_step "Delete the old checkpoint snapshots..."
rm -rf example_config/*.chk
rm -rf example_config/*.json

print_step "Generate the checkpoint snapshots..."
cargo run --bin generate_chk_snapshots
check_error "Failed to generate the checkpoint snapshots"
