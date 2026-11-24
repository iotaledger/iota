// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Immutable objects of a chosen size, for tests that pass large objects as
/// transaction inputs.
module large_object::large_object;

/// An immutable object that holds `chunks`.
public struct LargeObject has key {
    id: UID,
    chunks: vector<vector<u8>>,
}

/// Freezes a new `LargeObject` that holds `copies` copies of `chunk`.
public fun freeze_new(chunk: vector<u8>, copies: u64, ctx: &mut TxContext) {
    let mut chunks = vector[];
    let mut i = 0;
    while (i < copies) {
        chunks.push_back(copy chunk);
        i = i + 1;
    };
    transfer::freeze_object(LargeObject { id: object::new(ctx), chunks });
}
