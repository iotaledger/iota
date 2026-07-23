#!/usr/bin/env bash
#
# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
#
# Publishes the `view_functions` example package to a local network and calls
# its `#[view]` functions over JSON-RPC with `curl`, including a generic view
# invoked with a type argument.
#
# Transactions (publish, object creation) go through the `iota` CLI because they
# must be signed; the read-only view calls are plain `iota_view` JSON-RPC
# requests made with `curl`.
#
# Prerequisites:
#   - A running local network with a faucet, e.g.:
#       cargo run --release --bin iota-localnet -- start --force-regenesis --with-faucet
#   - A current `iota` CLI that understands `#[view]`. By default the script uses
#     the binary built from this repo (target/release/iota, else target/debug);
#     override with IOTA=/path/to/iota. An older CLI silently drops `#[view]`.
#   - The CLI's active environment must point at the local network (JSON-RPC on
#     127.0.0.1:9000); view metadata is not recorded on testnet or mainnet.
#   - `jq` and `curl` on PATH.
#
# Overrides via environment variables: IOTA, RPC_URL, FAUCET_URL, GAS_BUDGET.
#
# Usage: docs/examples/shell/call-view-function.sh

set -euo pipefail

RPC_URL="${RPC_URL:-http://127.0.0.1:9000}"
FAUCET_URL="${FAUCET_URL:-http://127.0.0.1:9123/v1/gas}"
GAS_BUDGET="${GAS_BUDGET:-500000000}"
IOTA_COIN_TYPE="0x2::coin::Coin<0x2::iota::IOTA>"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
PACKAGE_PATH="$REPO_ROOT/examples/move/view_functions"

# The CLI must understand the `#[view]` attribute; an older `iota` on PATH would
# silently drop it and publish a package with no view metadata. Prefer the CLI
# built from this repo unless IOTA is set explicitly.
if [ -z "${IOTA:-}" ]; then
    if [ -x "$REPO_ROOT/target/release/iota" ]; then
        IOTA="$REPO_ROOT/target/release/iota"
    elif [ -x "$REPO_ROOT/target/debug/iota" ]; then
        IOTA="$REPO_ROOT/target/debug/iota"
    else
        IOTA="iota"
    fi
fi

for tool in "$IOTA" jq curl; do
    command -v "$tool" >/dev/null 2>&1 || {
        echo "error: '$tool' not found on PATH" >&2
        exit 1
    }
done

# `--allow-view-function` only exists on a view-aware CLI, so its presence tells
# us the binary recognizes `#[view]`.
if ! "$IOTA" move build --help 2>&1 | grep -q "allow-view-function"; then
    echo "error: the '$IOTA' CLI is too old to understand #[view] functions." >&2
    echo "       Build the current CLI and point IOTA at it:" >&2
    echo "         cargo build --release -p iota" >&2
    echo "         IOTA=$REPO_ROOT/target/release/iota $0" >&2
    exit 1
fi

# Make a JSON-RPC request against $RPC_URL and return the raw response body.
rpc() {
    local method=$1 params=$2
    curl -sS -X POST "$RPC_URL" \
        -H 'Content-Type: application/json' \
        -d "$(jq -cn --arg m "$method" --argjson p "$params" \
            '{jsonrpc:"2.0", id:1, method:$m, params:$p}')"
}

# Call the `iota_view` method and pretty-print the result.
# Arguments: <function-name> <type-args-json-array> <args-json-array>
view() {
    local function_name=$1 type_args=$2 args=$3
    rpc iota_view "$(jq -cn --arg fn "$function_name" --argjson ta "$type_args" --argjson a "$args" \
        '[$fn, $ta, $a]')" | jq .
}

# Return the object ID of the single `created` object whose type contains $1.
created_object_id() {
    jq -r --arg t "$1" \
        '.objectChanges[] | select(.type == "created" and (.objectType | contains($t))) | .objectId'
}

echo "==> JSON-RPC endpoint: $RPC_URL"
if ! rpc iota_getChainIdentifier '[]' | jq -e '.result' >/dev/null 2>&1; then
    echo "error: no node reachable at $RPC_URL (is the local network running?)" >&2
    exit 1
fi

SENDER="$($IOTA client active-address)"
echo "==> Active address: $SENDER"

echo "==> Requesting gas from the faucet"
curl -sS -X POST "$FAUCET_URL" \
    -H 'Content-Type: application/json' \
    -d "$(jq -cn --arg r "$SENDER" '{FixedAmountRequest: {recipient: $r}}')" >/dev/null
# The faucet funds asynchronously; wait until at least one coin shows up.
for _ in $(seq 1 30); do
    coin_count="$(rpc iotax_getCoins "[\"$SENDER\"]" | jq '.result.data | length')"
    [ "$coin_count" -gt 0 ] && break
    sleep 1
done
[ "${coin_count:-0}" -gt 0 ] || {
    echo "error: address was not funded by the faucet" >&2
    exit 1
}

echo "==> Publishing the view_functions package"
publish="$($IOTA client publish "$PACKAGE_PATH" --gas-budget "$GAS_BUDGET" --json)"
package_id="$(echo "$publish" | jq -r '.objectChanges[] | select(.type == "published") | .packageId')"
counter_id="$(echo "$publish" | created_object_id "::counter::Counter")"
echo "    package: $package_id"
echo "    counter: $counter_id"

echo "==> Creating a leaderboard and recording a score"
create="$($IOTA client call --package "$package_id" --module leaderboard --function create \
    --gas-budget "$GAS_BUDGET" --json)"
leaderboard_id="$(echo "$create" | created_object_id "::leaderboard::Leaderboard")"
$IOTA client call --package "$package_id" --module leaderboard --function submit_score \
    --args "$leaderboard_id" "$SENDER" 2500 --gas-budget "$GAS_BUDGET" --json >/dev/null
echo "    leaderboard: $leaderboard_id"

echo "==> Creating a Vault<Coin<IOTA>>"
# Split a small, dedicated coin off an existing gas coin. Because it holds only
# 1000 NANOS it is too small to be auto-selected as the gas payment, so it can
# be handed to `vault::create` by value without conflicting with gas selection.
gas_coin="$(rpc iotax_getCoins "[\"$SENDER\"]" | jq -r '.result.data[0].coinObjectId')"
split="$($IOTA client split-coin --coin-id "$gas_coin" --amounts 1000 \
    --gas-budget "$GAS_BUDGET" --json)"
vault_coin="$(echo "$split" | created_object_id "::coin::Coin<")"
vault="$($IOTA client call --package "$package_id" --module vault --function create \
    --type-args "$IOTA_COIN_TYPE" --args "$vault_coin" 0 "$SENDER" \
    --gas-budget "$GAS_BUDGET" --json)"
vault_id="$(echo "$vault" | created_object_id "::vault::Vault<")"
echo "    vault: $vault_id"

echo
echo "==> iota_view: counter::value (no type arguments)"
view "$package_id::counter::value" '[]' "[\"$counter_id\"]"

echo "==> iota_view: leaderboard::total_entries"
view "$package_id::leaderboard::total_entries" '[]' "[\"$leaderboard_id\"]"

echo "==> iota_view: leaderboard::highest_score (struct return value)"
view "$package_id::leaderboard::highest_score" '[]' "[\"$leaderboard_id\"]"

echo "==> iota_view: vault::item (generic; type argument filled in)"
view "$package_id::vault::item" "[\"$IOTA_COIN_TYPE\"]" "[\"$vault_id\"]"
