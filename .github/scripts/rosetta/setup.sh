#!/bin/bash
# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

echo "Install binaries"
cargo install --locked --bin iota --path crates/iota
cargo install --locked --bin iota-rosetta --path crates/iota-rosetta

echo "run Iota genesis"
iota genesis

echo "generate rosetta configuration"
iota-rosetta generate-rosetta-cli-config --online-url http://127.0.0.1:9002 --offline-url http://127.0.0.1:9003

echo "install rosetta-cli"
curl -sSfL https://raw.githubusercontent.com/coinbase/rosetta-cli/master/scripts/install.sh | sh -s