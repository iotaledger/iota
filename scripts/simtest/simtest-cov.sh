#!/bin/bash -e
# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# verify that git repo is clean
if [[ -n $(git status -s) ]]; then
  echo "Working directory is not clean. Please commit all changes before running this script."
  echo $(git status -s)
  exit 1
fi

# apply git patch
git apply ./scripts/simtest/config-patch

root_dir=$(git rev-parse --show-toplevel)
export SIMTEST_STATIC_INIT_MOVE=$root_dir"/examples/move/basics"

TOOLCHAIN=$(rustup show active-toolchain | cut -d ' ' -f 1)
LLVM_PROFDATA="$HOME/.rustup/toolchains/$TOOLCHAIN/lib/rustlib/x86_64-unknown-linux-gnu/bin/llvm-profdata"

MSIM_WATCHDOG_TIMEOUT_MS=60000 MSIM_TEST_SEED=1 cargo llvm-cov --ignore-run-fail --no-report \
  nextest -vv

# remove the patch
git checkout .cargo/config Cargo.toml Cargo.lock
