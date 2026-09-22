#!/usr/bin/env bash
#
# Report iota-sdk-types #[non_exhaustive] enum variants that a `match` leaves to
# a wildcard arm. A new SDK variant would otherwise land silently in that arm.
#
# This is a REPORT, not a gate: it runs the lint at warn level and never fails on
# a finding. It fails only its own self-test (see bottom), i.e. when the lint is
# no longer being applied at all. Do not run this under `-D warnings`; findings
# are warnings by contract.
#
# Coverage is derived, not curated: every workspace crate that can reach
# iota-sdk-types in its dependency tree is in scope, i.e. every crate that could
# name an SDK type (directly, or through a re-export from another crate). Crates
# with no path to iota-sdk-types are excluded, so their unrelated non_exhaustive
# enums stay out. See README.md for the known coverage gaps.
#
# No crate source is edited. The unstable, nightly-only lint
# (non_exhaustive_omitted_patterns) is injected through a RUSTC_WORKSPACE_WRAPPER
# that appends the crate attributes only when the crate being compiled is in
# scope (keyed on CARGO_PKG_NAME). The wrapper runs for workspace members only,
# and keying on the crate's own identity means a dependency of an in-scope crate
# is never injected, so third-party enums in the dependency graph do not trip it.
set -uo pipefail

# Resolve the wrapper path from $0 before changing directory: $0 may be relative
# to the caller's cwd, which the `cd` below would otherwise invalidate.
here="$(cd "$(dirname "$0")" && pwd)"
WRAPPER="${WRAPPER:-$here/rustc_wrapper.sh}"

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

NIGHTLY="${NIGHTLY:-nightly-2026-06-29}"

pkgs_file="$(mktemp "${TMPDIR:-/tmp}/nelint-pkgs.XXXXXX")"
members_file="$(mktemp "${TMPDIR:-/tmp}/nelint-members.XXXXXX")"
log_file="$(mktemp "${TMPDIR:-/tmp}/nelint-out.XXXXXX")"
trap 'rm -f "$pkgs_file" "$members_file" "$log_file"' EXIT

# In scope: workspace members that reach iota-sdk-types in their dependency tree
# (any edge kind, so lib/bin/test/build code is all covered). `cargo tree --invert`
# lists everything that depends on iota-sdk-types, transitively; intersecting with
# the workspace member names drops out-of-workspace crates.
cargo metadata --no-deps --format-version 1 \
  | jq -r '.packages[].name' | sort -u > "$members_file"
cargo tree --invert iota-sdk-types --edges all --prefix none \
  | sed -E 's/ v[0-9].*//' | sort -u \
  | comm -12 "$members_file" - > "$pkgs_file"

# An empty scope means `cargo metadata`/`cargo tree`/jq failed or the dependency
# was renamed away, not that there is nothing to lint. Fail with the real cause
# rather than letting the self-test below misreport it as "lint not applied".
if [[ ! -s "$pkgs_file" ]]; then
  echo "ERROR: found no workspace crates depending on iota-sdk-types (cargo metadata/tree or jq failed?)." >&2
  exit 1
fi
echo "non_exhaustive_omitted_patterns: $(wc -l < "$pkgs_file") crates in scope (report only)"

# --all-features so a match placed behind a `#[cfg(feature = "...")]` that is off
# in the default build is still compiled and linted; otherwise such a match would
# be silently invisible forever. CARGO_TERM_COLOR=never keeps ANSI escapes out of
# the log the self-test greps.
RUSTC_WORKSPACE_WRAPPER="$WRAPPER" \
NELINT_PKGS_FILE="$pkgs_file" \
NELINT_LEVEL=warn \
CARGO_TERM_COLOR=never \
  cargo +"$NIGHTLY" check --workspace --all-targets --all-features 2>&1 | tee "$log_file"
cargo_rc=${PIPESTATUS[0]}

# The lint runs at warn and never fails cargo, so a non-zero exit is a real build
# error (broken code, missing toolchain, ...), not a finding. Surface it as such.
if [[ "$cargo_rc" -ne 0 ]]; then
  echo "ERROR: cargo check failed (exit $cargo_rc); this is a build error, not a lint finding." >&2
  exit "$cargo_rc"
fi

# Self-test: prove the lint was actually injected, independent of how clean the
# codebase happens to be. We assert the run produced at least one
# `non_exhaustive_omitted_patterns` finding: its notes name the lint (a stable
# identifier, unlike the scrutinee-type path, which rustc may print with a trimmed
# or aliased path). In-scope crates match third-party non_exhaustive enums
# (object_store, ...) as well as SDK ones, so a working injection essentially
# always yields at least one finding; zero means the wrapper is not being applied.
# (If every in-scope non_exhaustive match were ever made exhaustive, this would
# need a dedicated canary instead.)
if ! grep -q 'non_exhaustive_omitted_patterns' "$log_file"; then
  echo "ERROR: self-test failed: no non_exhaustive_omitted_patterns findings; the lint is not being applied." >&2
  exit 1
fi
