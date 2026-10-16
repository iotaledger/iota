#!/usr/bin/env bash
# Tests for rustc_wrapper.sh, findings.sh and compare.sh. The rustc cases compile
# a small `sdk` crate and a `user` crate through the wrapper with the pinned nightly.
set -uo pipefail
export LC_ALL=C

here="$(cd "$(dirname "$0")" && pwd)"
NIGHTLY="${NIGHTLY:-nightly-2026-06-29}"
work="$(mktemp -d "${TMPDIR:-/tmp}/nelint-test.XXXXXX")"
trap 'rm -rf "$work"' EXIT
cd "$work" || exit 1
echo user > pkgs

failed=0
pass() { echo "ok   $1"; }
fail() { echo "FAIL $1"; shift; printf '     %s\n' "$@"; failed=1; }

build_sdk() {
  printf '#[non_exhaustive]\npub enum E { %s }\n#[non_exhaustive]\npub struct S { pub x: u8, pub y: u8 }\n' "$1" > sdk.rs
  rustc +"$NIGHTLY" --edition 2021 --crate-type lib --crate-name sdk -o libsdk.rlib sdk.rs \
    || { echo "FAIL could not compile sdk.rs"; exit 1; }
}

# lint <user source> <package name>: prints the findings of compiling it.
lint() {
  printf '%s\n' "$1" > user.rs
  CARGO_PKG_NAME="$2" NELINT_PKGS_FILE=pkgs "$here/rustc_wrapper.sh" rustc +"$NIGHTLY" \
    --edition 2021 --crate-type lib --crate-name user --emit=metadata -o user.rmeta \
    -L . --extern sdk=libsdk.rlib user.rs > log.txt 2>&1 \
    || { echo "FAIL could not compile user.rs:"; cat log.txt; exit 1; }
  "$here/findings.sh" log.txt
}

# expect_compare <name> <expected exit> <allowlist file> <findings file>
expect_compare() {
  "$here/compare.sh" "$3" "$4" > out.txt 2>&1
  local rc=$?
  if [[ "$rc" -eq "$2" ]]; then pass "$1"; else fail "$1" "compare.sh exited $rc, expected $2:" "$(cat out.txt)"; fi
}

one_match='pub fn f(e: &sdk::E) -> u8 { match e { sdk::E::A => 0, _ => 1 } }'
two_matches="$one_match
pub fn g(e: &sdk::E) -> u8 { match e { sdk::E::B => 0, _ => 1 } }"
struct_rest='pub fn h(s: &sdk::S) -> u8 { let sdk::S { x, .. } = s; *x }'

build_sdk 'A, B, C, D'

got="$(lint "$one_match
$struct_rest" user)"
want='user.rs | &sdk::E | sites=1
user.rs | sdk::S | sites=1'
if [[ "$got" == "$want" ]]; then pass "a wildcard match and a .. struct pattern give one key each"
else fail "a wildcard match and a .. struct pattern give one key each" "got:" "$got"; fi

got="$(lint "$two_matches" user)"
if [[ "$got" == 'user.rs | &sdk::E | sites=2' ]]; then pass "two wildcard matches on one type in one file count as sites=2"
else fail "two wildcard matches on one type in one file count as sites=2" "got:" "$got"; fi

got="$(lint "$one_match" other)"
if [[ -z "$got" ]]; then pass "a crate outside the scope gets no lint"
else fail "a crate outside the scope gets no lint" "got:" "$got"; fi

lint "$one_match" user > allowlist.txt
build_sdk 'A, B, C, D, F'
lint "$one_match" user > findings.txt
expect_compare "an allowlisted match stays green when the enum gains a variant" 0 allowlist.txt findings.txt

lint "$two_matches" user > findings.txt
expect_compare "a new wildcard match on an allowlisted type turns red" 1 allowlist.txt findings.txt

printf '%s\n' 'user.rs | &sdk::E | sites=1' 'gone.rs | &sdk::E | sites=1' > allowlist.txt
lint "$one_match" user > findings.txt
expect_compare "an allowlist entry no longer found turns red" 1 allowlist.txt findings.txt

printf '%s\n' '# header' 'b.rs | T | sites=1   # reason' '' 'a.rs | T + Send | sites=1' 'a.rs | T | sites=1' > allowlist.txt
printf '%s\n' 'a.rs | T | sites=1' 'a.rs | T + Send | sites=1' 'b.rs | T | sites=1' > findings.txt
expect_compare "comments, blank lines, # reasons and line order do not matter" 0 allowlist.txt findings.txt

printf '%s\n' 'warning: some variants are not matched explicitly' ' --> a.rs:1:1' > log.txt
if "$here/findings.sh" log.txt > /dev/null 2>&1; then fail "a finding without its type note fails findings.sh" "findings.sh exited 0"
else pass "a finding without its type note fails findings.sh"; fi

exit "$failed"
