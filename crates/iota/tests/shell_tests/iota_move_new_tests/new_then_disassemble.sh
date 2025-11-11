# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# tests that iota move new followed by iota move disassemble succeeds


iota move new example
cat > example/sources/example.move <<EOF
module example::example;

public fun foo(_ctx: &mut TxContext) {}
EOF
cd example

echo "=== Build ===" | tee /dev/stderr
iota move build

echo "=== Disassemble ===" | tee /dev/stderr
iota move disassemble build/example/bytecode_modules/example.mv
