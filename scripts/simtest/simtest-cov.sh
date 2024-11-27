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

TOOLCHAIN=nightly-x86_64-unknown-linux-gnu
LLVM_PROFDATA="$HOME/.rustup/toolchains/$TOOLCHAIN/lib/rustlib/x86_64-unknown-linux-gnu/bin/llvm-profdata"

MSIM_WATCHDOG_TIMEOUT_MS=60000 MSIM_TEST_SEED=1 cargo llvm-cov \
  --ignore-run-fail \
  --no-report \
  nextest -vv --cargo-profile simulator

echo "Scanning for corrupted .profraw files. This might take a while."
find target/llvm-cov-target -name '*.profraw' | while read file; do
  if ! "$LLVM_PROFDATA" show "$file" > /dev/null 2>&1; then
      echo "Removing corrupted file: $file"
      rm "$file"
  fi
done 

find target/llvm-cov-target -name '*.profraw' -print0 | xargs -0 $LLVM_PROFDATA merge \
  --failure-mode=warn \
  --sparse \
  --output target/simtest.profdata

# remove the patch
git checkout .cargo/config Cargo.toml Cargo.lock
