// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module reviews_rating::moderator {
    use iota::tx_context::{sender};

    /// Represents a moderator that can be used to delete reviews
    public struct Moderator has key {
        id: UID,
    }

    /// A capability that can be used to setup moderators
    public struct ModCap has key, store {
        id: UID,
    }

    /// Initialize ModCap and transfer it to the sender
    fun init(ctx: &mut TxContext) {
        let mod_cap = ModCap {
            id: iota::object::new(ctx),  // IOTA object creation
        };
        iota::transfer::transfer(mod_cap, sender(ctx));  // Transfer using IOTA method
    }

    /// Adds a moderator
    public fun add_moderator(
        _: &ModCap,
        recipient: address,
        ctx: &mut TxContext,
    ) {
        // Generate an NFT (moderator role) and transfer it to the recipient
        let mod = Moderator {
            id: iota::object::new(ctx),  // IOTA object creation
        };
        iota::transfer::transfer(mod, recipient);  // Transfer using IOTA method
    }

    /// Deletes a moderator
    public fun delete_moderator(
        mod: Moderator
    ) {
        let Moderator { id } = mod;
        iota::object::delete(id);  // IOTA object deletion
    }
}
