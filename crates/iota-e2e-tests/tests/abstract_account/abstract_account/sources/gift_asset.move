// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module abstract_account::gift_asset;

use iota::transfer;

/// Simple child object used in tests.
public struct Gift has key, store { id: UID }

/// Mint a new Gift.
public fun mint(ctx: &mut TxContext): Gift {
    Gift { id: object::new(ctx) }
}

/// Send a Gift to a target IOTA ID (address or object ID are the same 32 bytes).
/// See “Transfer to Object” — public_transfer to an object ID is valid.
public fun send_to(g: Gift, target: address) {
    transfer::public_transfer(g, target)
}

public fun drop(self: Gift) {
    let Gift { id } = self;
    object::delete(id);
}
