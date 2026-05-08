#!/usr/bin/env bash
# Bump the workspace version and propagate it to dependent files.
#
# Usage: scripts/bump_version.sh <new-version>
# Example: scripts/bump_version.sh 1.24.0-alpha

set -euo pipefail

if [ $# -ne 1 ]; then
    echo "Usage: $0 <new-version>" >&2
    echo "Example: $0 1.24.0-alpha" >&2
    exit 1
fi

new_version="$1"

if [[ ! "$new_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9.]+)?$ ]]; then
    echo "Error: '$new_version' is not a valid version (expected MAJOR.MINOR.PATCH[-prerelease])" >&2
    exit 1
fi

# Forms used across the touched files:
#   new_base  = MAJOR.MINOR.PATCH (no prerelease) e.g. 1.24.0
#   new_short = MAJOR.MINOR                       e.g. 1.24
new_base="${new_version%%-*}"
new_short="${new_base%.*}"

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

# Read current workspace.package version from Cargo.toml.
old_version="$(awk '
    /^\[workspace\.package\]/ { in_section = 1; next }
    /^\[/                     { in_section = 0 }
    in_section && /^version[[:space:]]*=/ {
        match($0, /"[^"]+"/)
        print substr($0, RSTART + 1, RLENGTH - 2)
        exit
    }
' Cargo.toml)"

if [ -z "$old_version" ]; then
    echo "Error: failed to read current version from Cargo.toml" >&2
    exit 1
fi

if [ "$old_version" = "$new_version" ]; then
    echo "Already at $new_version, nothing to do."
    exit 0
fi

old_base="${old_version%%-*}"
old_short="${old_base%.*}"

echo "Bumping $old_version -> $new_version"

# 1. Cargo.toml — only the version inside [workspace.package]. The sed range
#    address keeps it from touching version keys in other tables.
sed -i.bak -E \
    "/^\[workspace\.package\]/,/^\[/ s|^version = \"$old_version\"|version = \"$new_version\"|" \
    Cargo.toml
rm Cargo.toml.bak

# 2. openrpc.json — top-level "version" field.
sed -i.bak -E \
    "s|(\"version\":[[:space:]]*\")$old_version(\")|\1$new_version\2|" \
    crates/iota-open-rpc/spec/openrpc.json
rm crates/iota-open-rpc/spec/openrpc.json.bak

# 3. GraphQL e2e snapshot — has both the base form (in -testing-no-sha) and the
#    short form (in availableVersions).
sed -i.bak \
    -e "s|$old_base-testing-no-sha|$new_base-testing-no-sha|g" \
    -e "s|\"$old_short\"|\"$new_short\"|g" \
    crates/iota-graphql-e2e-tests/tests/call/simple.snap
rm crates/iota-graphql-e2e-tests/tests/call/simple.snap.bak

# 4. Cargo.lock — refresh workspace member versions only.
cargo update --workspace

echo "Done. Review with: git diff"
