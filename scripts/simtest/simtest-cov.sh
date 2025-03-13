#!/bin/bash -e
# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# INPUTS
# If tests timeout on your machine, override the per test watchdog timeout:
MSIM_WATCHDOG_TIMEOUT_MS=${MSIM_WATCHDOG_TIMEOUT_MS:-180000}

# verify that git repo is clean
if [[ -n $(git status -s) ]]; then
  echo "Working directory is not clean. Please commit all changes before running this script."
  git status -s
  exit 1
fi

# apply git patch
git apply ./scripts/simtest/config-patch

root_dir=$(git rev-parse --show-toplevel)
export SIMTEST_STATIC_INIT_MOVE=$root_dir"/examples/move/basics"

MSIM_WATCHDOG_TIMEOUT_MS=${MSIM_WATCHDOG_TIMEOUT_MS} MSIM_TEST_SEED=1 cargo llvm-cov --ignore-run-fail --no-report nextest

# remove the patch
git checkout .cargo/config.toml Cargo.toml Cargo.lock
