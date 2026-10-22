#!/bin/bash -x
# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# This script runs as part of the nightly CI workflow.
# It detects the `#[sim_test]`s added since a base commit and runs them 20 times
# each with a different seed.
#
# The base commit is taken from BASE_COMMIT; the nightly workflow sets it to the
# commit its previous scheduled run tested. Without it, the base is the merge
# base of HEAD and origin/develop.

if [ -n "$BASE_COMMIT" ]; then
  BASE="$BASE_COMMIT"
else
  BASE=$(git merge-base HEAD origin/develop)
fi
if [ -z "$BASE" ] || ! git cat-file -e "$BASE^{commit}"; then
  echo "Cannot find the base commit '$BASE' to look for new tests from" >&2
  exit 1
fi

# Print "<file> <test name>" for every `#[sim_test]` attribute added in the diff.
# The attribute may be indented (a test inside a module), take arguments, and be
# followed by further attributes, possibly over several lines, or comments
# before the `fn` line.
if ! NEW_TESTS=$(set -o pipefail; git diff "$BASE" -- '*.rs' | awk '
  /^\+\+\+ / { file = substr($0, 7); pending = 0; next }
  /^(diff |@@)/ { pending = 0; next }
  /^\+[ \t]*#\[(iota_macros::)?sim_test(\]|\()/ { pending = 1; next }
  !pending || /^-/ || /^[+ ][ \t]*\/\// { next }
  match($0, /(^|[^A-Za-z0-9_])fn +[A-Za-z_][A-Za-z0-9_]*/) {
    name = substr($0, RSTART, RLENGTH)
    sub(/^.*fn +/, "", name)
    print file, name
    pending = 0
  }'); then
  echo "Cannot diff against the base commit '$BASE'" >&2
  exit 1
fi

echo "Detected new tests: ${NEW_TESTS}"

if [ -z "$NEW_TESTS" ]; then
  echo "No new tests detected, exiting"
  exit 0
fi

# Prints the name of the package the given file belongs to.
package_of() {
  local dir
  dir=$(dirname "$1")
  while [ "$dir" != "." ] && [ ! -f "$dir/Cargo.toml" ]; do
    dir=$(dirname "$dir")
  done
  sed -n 's/^name *= *"\(.*\)"/\1/p' "$dir/Cargo.toml" | head -n 1
}

# Match each test as the whole name or the last segment of a module path: a test
# binary made of modules names its tests `module::path::name`.
FILTERS=()
PACKAGES=()
while read -r FILE NAME; do
  PACKAGE=$(package_of "$FILE")
  if [ -z "$PACKAGE" ]; then
    echo "Cannot find the package of $FILE" >&2
    exit 1
  fi
  FILTERS+=("(package($PACKAGE) and test(/(^|::)${NAME}\$/))")
  PACKAGES+=("$PACKAGE")
done <<< "$NEW_TESTS"

TEST_FILTER=$(printf '%s or ' "${FILTERS[@]}" | sed 's/ or $//')
PACKAGE_ARGS=()
for PACKAGE in $(printf '%s\n' "${PACKAGES[@]}" | sort -u); do
  PACKAGE_ARGS+=(--package "$PACKAGE")
done

# Ignored tests are detected too, but nextest does not run them, so a filter that
# selects only those must not fail the run. Retries would hide a flaky test.
MSIM_TEST_NUM=20 MSIM_TEST_SEED=2 scripts/simtest/cargo-simtest simtest \
  --profile simtestnightly \
  --no-tests=warn \
  --retries 0 \
  "${PACKAGE_ARGS[@]}" \
  -E "$TEST_FILTER" \
  "$@"
