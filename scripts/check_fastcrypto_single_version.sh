#!/usr/bin/env bash
# fastcrypto is shared with the iota-rust-sdk (via iota-sdk-crypto), so the whole
# tree must resolve to one fastcrypto version in Cargo.lock.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
lockfile="$root/Cargo.lock"

count=$(grep -c '^name = "fastcrypto"$' "$lockfile" || true)
if [ "$count" != "1" ]; then
  echo "error: expected exactly one 'fastcrypto' entry in Cargo.lock, found ${count}."
  echo "The iota workspace and iota-rust-sdk (iota-sdk-crypto) must use the same fastcrypto version."
  grep -A2 '^name = "fastcrypto"$' "$lockfile" | grep -E '^(name|version|source) ' || true
  exit 1
fi

version=$(grep -A1 '^name = "fastcrypto"$' "$lockfile" | sed -n 's/^version = "\(.*\)"$/\1/p')
echo "fastcrypto: single version ${version} — OK"
