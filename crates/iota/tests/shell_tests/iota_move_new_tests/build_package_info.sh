# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# tests that `iota move build --package-info` reports the package name, its
# direct dependencies, and the on-chain size against the protocol maximum
iota move new example
cat > example/sources/example.move <<EOF
module example::example;

public struct Thing has key, store { id: UID }

public fun new(ctx: &mut TxContext): Thing { Thing { id: object::new(ctx) } }
EOF
cd example
iota move build --package-info
