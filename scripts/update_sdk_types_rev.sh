#!/bin/bash

# Script to update the rev for iota-sdk-types in the three Cargo.toml files
# and update the corresponding Cargo.lock files by running 'timeout 5s cargo check'
# in each directory to resolve dependencies and update the lock without full compilation.

# Usage: ./scripts/update_sdk_types_rev.sh <new_rev>

NEW_REV=$1

if [ -z "$NEW_REV" ]; then
    echo "Usage: $0 <new_rev>"
    echo "Example: $0 abc123def456"
    exit 1
fi

# Automatically find the current rev from the main Cargo.toml
OLD_REV=$(grep -oP 'iota-sdk-types.*rev = "\K[^"]+' Cargo.toml)

if [ -z "$OLD_REV" ]; then
    echo "Error: Could not find current rev in Cargo.toml"
    exit 1
fi

echo "Current rev: $OLD_REV"
echo "New rev: $NEW_REV"

# Update the three Cargo.toml files
echo "Updating Cargo.toml files..."

sed -i "s/$OLD_REV/$NEW_REV/g" Cargo.toml
sed -i "s/$OLD_REV/$NEW_REV/g" examples/tic-tac-toe/cli/Cargo.toml
sed -i "s/$OLD_REV/$NEW_REV/g" docs/examples/rust/Cargo.toml

echo "Updated rev in Cargo.toml files."

echo "Updating Cargo.lock for workspace..."
timeout 5s cargo check

echo "Updating Cargo.lock for examples/tic-tac-toe/cli..."
cd examples/tic-tac-toe/cli
timeout 5s cargo check
cd ../../..

echo "Updating Cargo.lock for docs/examples/rust..."
cd docs/examples/rust
timeout 5s cargo check
cd ../../..

echo "All updates complete."
