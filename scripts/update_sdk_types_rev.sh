#!/bin/bash

# Script to update the rev for iota-sdk-types in the three Cargo.toml files
# and update the corresponding Cargo.lock files by running 'cargo check'
# in each directory to resolve dependencies and update the lock without full compilation.

# Usage: ./scripts/update_sdk_types_rev.sh <new_rev>

NEW_REV=$1

if [ -z "$NEW_REV" ]; then
    echo "Usage: $0 <new_rev>"
    echo "Example: $0 abc123def456"
    exit 1
fi

# Cross-platform in-place sed using a temp file
sed_inplace() {
    local expr="$1"
    local file="$2"
    sed "$expr" "$file" > "$file.tmp" && mv "$file.tmp" "$file"
}

# Cross-platform timeout: prefer timeout/gtimeout, fall back to background process
run_with_timeout() {
    local secs="$1"
    shift
    if command -v timeout >/dev/null 2>&1; then
        timeout "${secs}s" "$@"
    elif command -v gtimeout >/dev/null 2>&1; then
        gtimeout "${secs}s" "$@"
    else
        "$@" &
        local pid=$!
        ( sleep "$secs"; kill "$pid" 2>/dev/null ) &
        local watcher=$!
        wait "$pid" 2>/dev/null
        kill "$watcher" 2>/dev/null
        wait "$watcher" 2>/dev/null
    fi
}

# Automatically find the current rev from the main Cargo.toml
OLD_REV=$(sed -n 's/.*iota-sdk-types.*rev = "\([^"]*\)".*/\1/p' Cargo.toml | head -1)

if [ -z "$OLD_REV" ]; then
    echo "Error: Could not find current rev in Cargo.toml"
    exit 1
fi

echo "Current rev: $OLD_REV"
echo "New rev: $NEW_REV"

# Update the three Cargo.toml files
echo "Updating Cargo.toml files..."

sed_inplace "s/$OLD_REV/$NEW_REV/g" Cargo.toml
sed_inplace "s/$OLD_REV/$NEW_REV/g" examples/tic-tac-toe/cli/Cargo.toml
sed_inplace "s/$OLD_REV/$NEW_REV/g" docs/examples/rust/Cargo.toml

echo "Updated rev in Cargo.toml files."

echo "Updating Cargo.lock for workspace..."
run_with_timeout 5 cargo check

echo "Updating Cargo.lock for examples/tic-tac-toe/cli..."
cd examples/tic-tac-toe/cli
run_with_timeout 5 cargo check
cd ../../..

echo "Updating Cargo.lock for docs/examples/rust..."
cd docs/examples/rust
run_with_timeout 5 cargo check
cd ../../..

echo "All updates complete."
