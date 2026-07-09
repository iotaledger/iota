#!/usr/bin/env bash
# Fail if Cargo.lock has more than one fastcrypto: it is shared with the
# iota-rust-sdk (via iota-sdk-crypto) and both must resolve to one version.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
lockfile="$root/Cargo.lock"

count=$(grep -c '^name = "fastcrypto"$' "$lockfile" || true)

if [ "$count" != "1" ]; then
  echo "error: expected exactly one 'fastcrypto' entry in Cargo.lock, found ${count}."
  echo "The iota workspace and iota-rust-sdk (iota-sdk-crypto) must use the same fastcrypto version."
  echo "See the [patch.\"https://github.com/MystenLabs/fastcrypto\"] section in the root Cargo.toml."
  grep -A2 '^name = "fastcrypto"$' "$lockfile" | grep -E '^(name|version|source) ' || true
  exit 1
fi

echo "fastcrypto: single version in Cargo.lock — OK"
