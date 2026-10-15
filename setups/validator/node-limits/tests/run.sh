#!/bin/sh
# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# Tests of "node-limits.sh generate".
#
# - tested/*.nft: rules that were tested by hand before the generator was
#   written. The rules for the same values must be equal to them after the
#   renames below.
# - cases/NAME.conf: the rules must be equal to cases/NAME.nft. The case
#   "shipped" uses ../node-limits.conf.
# - Each expected rule file is loaded with nft in a new network namespace. Each
#   network in it must come back from nft as it is written.
# - cases/bad-NAME.conf: generate must fail with the message in the first line
#   ("# error: [<line>: ]<message>").
# - Control: a changed copy of an expected file must make the comparison fail.
#
# Loading needs nft and root, sudo or "unshare -rn". NODE_LIMITS_SKIP_NFT=1
# skips it.
#
# Usage: run.sh [--update]   --update writes the cases/*.nft files again.

set -eu

here=$(cd "$(dirname "$0")" && pwd)
script="$here/../node-limits.sh"
cases="$here/cases"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
failures=0

fail() {
    echo "FAIL $*"
    failures=$((failures + 1))
}

conf_of() {
    if [ "$1" = shipped ]; then echo "$here/../node-limits.conf"; else echo "$cases/$1.conf"; fi
}

# Exit status 0 if the rules for case $1 are equal to the file $2.
same_rules() {
    sh "$script" generate "$(conf_of "$1")" >"$tmp/out.nft" 2>"$tmp/err" && cmp -s "$tmp/out.nft" "$2"
}

# Runs a command in a new, empty network namespace.
in_netns() {
    if [ "$(id -u)" = 0 ]; then
        unshare -n "$@"
    elif unshare -rn true 2>/dev/null; then
        unshare -rn "$@"
    else
        sudo -n unshare -n "$@"
    fi
}

# Loads the rule file $1 and checks that nft lists each network of it as it
# is written (an IPv4 /32 and an IPv6 /128 without the prefix length).
read_back() {
    in_netns sh -c 'nft -f "$1" && nft list table inet iota_node_limits' sh "$1" >"$tmp/listed" 2>&1 || return 1
    for net in $(grep -o 'saddr [0-9a-f.:]*/[0-9]*' "$1" | sed 's/^saddr //; s|^\([0-9.]*\)/32$|\1|; s|/128$||'); do
        grep -qF "saddr $net " "$tmp/listed" || {
            echo "nft lists no network $net"
            return 1
        }
    done
}

# The renames from the hand-tested rules: table and object names, log prefix.
# Comments and the table comment of the generator are not compared.
normalize() {
    sed -e 's/iota_node_limits/node_limits/g; s/handshake_v/half_open_v/g; s/iota-node-limits refused/node_limits refused/' \
        -e 's/[[:space:]]*#.*$//' -e '/^[[:space:]]*comment "node-limits /d' -e '/^$/d' "$1"
}

names="shipped"
for conf in "$cases"/*.conf; do
    name=$(basename "$conf" .conf)
    case "$name" in bad-*) ;; *) names="$names $name" ;; esac
done

if [ "${1:-}" = --update ]; then
    for name in $names; do sh "$script" generate "$(conf_of "$name")" >"$cases/$name.nft"; done
    exit 0
fi

for pair in "shipped:$here/../node-limits.conf" "four-ports:$here/tested/four-ports.conf"; do
    name=${pair%%:*}
    sh "$script" generate "${pair#*:}" >"$tmp/generated.nft"
    normalize "$tmp/generated.nft" >"$tmp/generated.norm"
    normalize "$here/tested/$name.nft" >"$tmp/tested.norm"
    if cmp -s "$tmp/generated.norm" "$tmp/tested.norm"; then
        echo "ok   tested/$name"
    else
        fail "tested/$name: the rules differ from the hand-tested rules"
        diff "$tmp/tested.norm" "$tmp/generated.norm" || true
    fi
done

for name in $names; do
    expected="$cases/$name.nft"
    if same_rules "$name" "$expected"; then
        echo "ok   $name"
    else
        fail "$name: the rules differ from $expected"
        diff "$expected" "$tmp/out.nft" || true
        cat "$tmp/err"
    fi
    if [ "${NODE_LIMITS_SKIP_NFT:-0}" != 1 ]; then
        if read_back "$expected" >"$tmp/nft.log" 2>&1; then
            echo "ok   $name: loaded with nft, networks read back"
        else
            fail "$name: nft"
            cat "$tmp/nft.log"
        fi
    fi
done

for conf in "$cases"/bad-*.conf; do
    name=$(basename "$conf" .conf)
    want=$(sed -n '1s/^# error: //p' "$conf")
    case "$want" in
    [0-9]*:*) needle="$name.conf:$want" ;;
    *) needle="$want" ;;
    esac
    if [ -z "$want" ]; then
        fail "$name: the first line has no \"# error: \" message"
    elif sh "$script" generate "$conf" >/dev/null 2>"$tmp/err"; then
        fail "$name: generate succeeded"
    elif grep -qF -- "$needle" "$tmp/err"; then
        echo "ok   $name"
    else
        fail "$name: expected \"$needle\", got: $(cat "$tmp/err")"
    fi
done

# A values file with CRLF line ends gets its own message.
sed 's/$/\r/' "$cases/loopback-off.conf" >"$tmp/crlf.conf"
if sh "$script" generate "$tmp/crlf.conf" >/dev/null 2>"$tmp/err"; then
    fail "crlf: generate succeeded"
elif grep -qF "line ends" "$tmp/err"; then
    echo "ok   crlf"
else
    fail "crlf: expected a message about line ends, got: $(cat "$tmp/err")"
fi

# Control: the comparison must notice a changed expected file.
sed 's/ct count over 16/ct count over 17/' "$cases/shipped.nft" >"$tmp/changed.nft"
if cmp -s "$tmp/changed.nft" "$cases/shipped.nft" || same_rules shipped "$tmp/changed.nft"; then
    fail "control: a changed expected file was not noticed"
else
    echo "ok   control: a changed expected file is noticed"
fi

if [ "$failures" -ne 0 ]; then
    echo "$failures failure(s)"
    exit 1
fi
echo "all passed"
