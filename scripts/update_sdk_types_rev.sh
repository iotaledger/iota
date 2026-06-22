#!/bin/bash

# Script to update the rev for iota-sdk-types and iota-sdk-crypto in the
# Cargo.toml files and update the corresponding Cargo.lock files by running
# 'cargo check' in each directory to resolve dependencies and update the lock
# without full compilation.

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

# Replace the rev for a given crate in a single Cargo.toml. Only lines that
# mention the crate name are touched, so crates with different revs in the
# same file are updated independently.
update_rev() {
    local crate="$1"
    local file="$2"
    local old_rev
    old_rev=$(sed -n "s/.*${crate}.*rev = \"\([^\"]*\)\".*/\1/p" "$file" | head -1)
    if [ -z "$old_rev" ]; then
        return
    fi
    echo "  $file [$crate]: $old_rev -> $NEW_REV"
    sed_inplace "/${crate}/s/rev = \"[^\"]*\"/rev = \"$NEW_REV\"/" "$file"
}

# Update every pinned iota-rust-sdk crate in the given Cargo.toml.
update_file() {
    local file="$1"
    update_rev iota-sdk-types "$file"
    update_rev iota-sdk-crypto "$file"
    update_rev iota-sdk-grpc-types "$file"
    update_rev iota-sdk-grpc-client "$file"
    update_rev iota-sdk-graphql-client "$file"
    update_rev iota-sdk-transaction-builder "$file"
}

echo "New rev: $NEW_REV"
echo "Updating Cargo.toml files..."

update_file Cargo.toml
update_file crates/iota-genesis-builder/Cargo.toml
update_file crates/iota-rust-sdk/Cargo.toml
update_file examples/tic-tac-toe/cli/Cargo.toml
update_file docs/examples/rust/Cargo.toml

echo "Updated rev in Cargo.toml files."

echo "Updating Cargo.lock for workspace..."
run_with_timeout 5 cargo check

echo "Updating Cargo.lock for docs/examples/rust..."
cd docs/examples/rust
run_with_timeout 5 cargo check
cd ../../..

echo "All updates complete."
