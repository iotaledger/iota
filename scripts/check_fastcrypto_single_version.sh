#!/usr/bin/env bash
# fastcrypto is shared with the iota-rust-sdk (via iota-sdk-crypto), so the whole
# tree must resolve to one fastcrypto. This checks two things in Cargo.lock:
#   1. there is exactly one fastcrypto version; and
#   2. the crates.io fastcrypto we patch to was published from the same commit the
#      git-pinned fastcrypto-tbls/-vdf/-zkp reference (compared via the crate's
#      .cargo_vcs_info.json), so a matching version number can't hide a different
#      source.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
lockfile="$root/Cargo.lock"

# 1) exactly one fastcrypto version
count=$(grep -c '^name = "fastcrypto"$' "$lockfile" || true)
if [ "$count" != "1" ]; then
  echo "error: expected exactly one 'fastcrypto' entry in Cargo.lock, found ${count}."
  echo "The iota workspace and iota-rust-sdk (iota-sdk-crypto) must use the same fastcrypto version."
  grep -A2 '^name = "fastcrypto"$' "$lockfile" | grep -E '^(name|version|source) ' || true
  exit 1
fi

version=$(grep -A1 '^name = "fastcrypto"$' "$lockfile" | sed -n 's/^version = "\(.*\)"$/\1/p')

# 2) the git revision the fastcrypto-tbls/-vdf/-zkp sub-crates are pinned to (if any)
rev=$(grep -oE 'git\+https://github.com/[^"#]*fastcrypto[^"#]*#[0-9a-f]{40}' "$lockfile" \
  | grep -oE '[0-9a-f]{40}$' | sort -u)
if [ -z "$rev" ]; then
  echo "fastcrypto: single version ${version}, no git-pinned sub-crates — OK"
  exit 0
fi
if [ "$(printf '%s\n' "$rev" | wc -l | tr -d ' ')" != "1" ]; then
  echo "error: fastcrypto sub-crates are pinned to multiple git revisions:"
  printf '%s\n' "$rev"
  exit 1
fi

# 3) the commit crates.io published this fastcrypto version from
crate_url="https://static.crates.io/crates/fastcrypto/fastcrypto-${version}.crate"
tmp=$(mktemp)
if ! curl -sSL --retry 3 "$crate_url" -o "$tmp"; then
  echo "warning: could not download ${crate_url}; skipped git-revision cross-check (single-version check passed)."
  rm -f "$tmp"
  exit 0
fi
vcs_sha=$(tar -xzOf "$tmp" "fastcrypto-${version}/.cargo_vcs_info.json" 2>/dev/null | grep -oE '[0-9a-f]{40}' | head -1 || true)
rm -f "$tmp"

if [ -z "$vcs_sha" ]; then
  echo "warning: fastcrypto-${version} has no .cargo_vcs_info.json; skipped git-revision cross-check (single-version check passed)."
  exit 0
fi

if [ "$vcs_sha" != "$rev" ]; then
  echo "error: crates.io fastcrypto ${version} was published from ${vcs_sha},"
  echo "but fastcrypto-tbls/-vdf/-zkp are pinned to ${rev}."
  echo "Point the sub-crate 'rev' at the commit crates.io published fastcrypto ${version} from."
  exit 1
fi

echo "fastcrypto: single version ${version}; git rev ${rev} matches the crates.io publish — OK"
