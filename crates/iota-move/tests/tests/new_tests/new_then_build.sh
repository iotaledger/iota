# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# tests that iota-move new followed by iota-move build succeeds

iota-move new example

# we mangle the generated toml file to replace the framework dependency with a local dependency
FRAMEWORK_DIR=$(echo $CARGO_MANIFEST_DIR | sed 's#/crates/iota-move##g')
cat example/Move.toml \
  | sed 's#\(Iota = .*\)git = "[^"]*", \(.*\)#\1\2#' \
  | sed 's#\(Iota = .*\), rev = "[^"]*"\(.*\)#\1\2#' \
  | sed 's#\(Iota = .*\)subdir = "\([^"]*\)"\(.*\)#\1local = "FRAMEWORK/\2"\3#' \
  | sed "s#\(Iota = .*\)FRAMEWORK\(.*\)#\1$FRAMEWORK_DIR\2#" \
  > Move.toml
mv Move.toml example/Move.toml

cd example && iota-move build