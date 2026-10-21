#!/bin/bash -x
# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# This script runs as part of the rust.yml CI workflow.
# It will detect new tests added to the repository since the last commit
# to the develop branch, and run them 20 times each with a different seed.

MERGE_BASE=$(git merge-base HEAD origin/develop)

# find the tests whose `#[sim_test]` attribute was added since the merge base.
# The attribute may be indented (a test inside a module), take arguments, and be
# followed by further attributes or comments before the `fn` line. Ignored tests
# are left out: nextest does not run them, and a filter selecting no test fails.
NEW_TESTS=( $(git diff "$MERGE_BASE" | awk '
  /^\+[[:space:]]*#\[sim_test(\]|\()/ { pending = 1; ignored = 0; next }
  !pending { next }
  /^-/ { next }
  /^[+ ][[:space:]]*#\[ignore(\]|[[:space:]]|=)/ { ignored = 1; next }
  /^[+ ][[:space:]]*(#\[|\/\/)/ { next }
  {
    if (!ignored && match($0, /fn +[a-z_0-9]+/)) {
      name = substr($0, RSTART, RLENGTH)
      sub(/^fn +/, "", name)
      print name
    }
    pending = 0
  }') )

echo "Detected new tests: ${NEW_TESTS[@]}"

# exit early if NEW_TESTS is empty
if [ -z "${NEW_TESTS}" ]; then
  echo "No new tests detected, exiting"
  exit 0
fi

# iterate over NEW_TESTS and wrap each element in a filter matching it as the
# whole name or the last segment of a module path: a test binary made of
# modules names its tests `module::path::name`, which an exact match misses
for i in "${!NEW_TESTS[@]}"; do
  NEW_TESTS[$i]="test(/(^|::)${NEW_TESTS[$i]}\$/)"
done

# join NEW_TESTS with "or"
TEST_FILTER=$(printf %s' or ' "${NEW_TESTS[@]}" | sed 's/ or *$//')

# use seed of 2, since 1 was already used by the main job
MSIM_TEST_NUM=20 MSIM_TEST_SEED=2 cargo simtest -E "$TEST_FILTER" "$@"
