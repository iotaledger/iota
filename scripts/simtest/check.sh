#!/bin/bash -e
# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

root_dir=$(git rev-parse --show-toplevel)
pushd "$root_dir"

# verify that git repo is clean
if [[ -n $(git status -s) ]]; then
  echo "Working directory is not clean. Please commit all changes before running this script."
  git status -s
  exit 1
fi

# apply git patch
git apply ./scripts/simtest/config-patch

cleanup() {
  git checkout .
  popd
}
trap cleanup EXIT

export SIMTEST_STATIC_INIT_MOVE=$root_dir"/examples/move/basics"

RUST_FLAGS=('"--cfg"' '"msim"')

if [ -n "$LOCAL_MSIM_PATH" ]; then
  cargo_patch_args=(
    --config "patch.crates-io.tokio.path = \"$LOCAL_MSIM_PATH/msim-tokio\""
    --config "patch.'https://github.com/iotaledger/iota-sim'.msim.path = \"$LOCAL_MSIM_PATH/msim\""
    --config "patch.crates-io.futures-timer.path = \"$LOCAL_MSIM_PATH/mocked-crates/futures-timer\""
  )
else
  cargo_patch_args=(
    --config 'patch.crates-io.tokio.git = "https://github.com/iotaledger/iota-sim.git"'
    --config 'patch.crates-io.tokio.branch = "tokio-1.49.0"'
    --config 'patch.crates-io.futures-timer.git = "https://github.com/iotaledger/iota-sim.git"'
    --config 'patch.crates-io.futures-timer.branch = "tokio-1.49.0"'
  )
fi

rust_flags_str=$(IFS=, ; echo "${RUST_FLAGS[*]}")

cargo check --profile simulator \
  --config "target.'cfg(all())'.rustflags = [$rust_flags_str]" \
  "${cargo_patch_args[@]}" \
  "$@"
