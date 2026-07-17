#!/usr/bin/env bash
# Bump the workspace version and propagate it to dependent files.
#
# Usage: scripts/bump_version.sh <minor|patch|none> <alpha|beta|rc|release>
#
# The first argument controls which segment of MAJOR.MINOR.PATCH gets
# incremented; "none" keeps the base version unchanged (useful for promoting
# a pre-release in place, e.g. alpha -> beta).
#
# The second argument sets the pre-release suffix; "release" means no suffix.
#
# Examples:
#   scripts/bump_version.sh minor alpha    # 1.24.0       -> 1.25.0-alpha
#   scripts/bump_version.sh none  beta     # 1.24.0-alpha -> 1.24.0-beta
#   scripts/bump_version.sh none  release  # 1.24.0-rc    -> 1.24.0
#   scripts/bump_version.sh patch release  # 1.24.0       -> 1.24.1

set -euo pipefail

if [ $# -ne 2 ]; then
    echo "Usage: $0 <minor|patch|none> <alpha|beta|rc|release>" >&2
    exit 1
fi

bump="$1"
prerelease="$2"

case "$bump" in
    minor|patch|none) ;;
    *)
        echo "Error: first argument must be one of: minor, patch, none (got '$bump')" >&2
        exit 1
        ;;
esac

case "$prerelease" in
    alpha|beta|rc|release) ;;
    *)
        echo "Error: second argument must be one of: alpha, beta, rc, release (got '$prerelease')" >&2
        exit 1
        ;;
esac

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

if [[ ! "$old_version" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)(-[A-Za-z0-9.]+)?$ ]]; then
    echo "Error: current version '$old_version' is not a valid MAJOR.MINOR.PATCH[-prerelease]" >&2
    exit 1
fi

major="${BASH_REMATCH[1]}"
minor="${BASH_REMATCH[2]}"
patch="${BASH_REMATCH[3]}"

case "$bump" in
    minor)
        minor=$((minor + 1))
        patch=0
        ;;
    patch)
        patch=$((patch + 1))
        ;;
    none) ;;
esac

# Forms used across the touched files:
#   new_base  = MAJOR.MINOR.PATCH (no prerelease) e.g. 1.24.0
#   new_short = MAJOR.MINOR                       e.g. 1.24
new_base="$major.$minor.$patch"
new_short="$major.$minor"

if [ "$prerelease" = "release" ]; then
    new_version="$new_base"
else
    new_version="$new_base-$prerelease"
fi

# Expose the resolved version to GitHub Actions if running there.
if [ -n "${GITHUB_ENV:-}" ]; then
    echo "IOTA_VERSION=$new_version" >> "$GITHUB_ENV"
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
