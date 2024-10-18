// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Example coin with a trusted owner responsible for minting/burning (e.g., a stablecoin)
module examples::trusted_coin {
    use std::option;
    use iota::coin::{Self, TreasuryCap};
    use iota::transfer;
    use iota::tx_context::{Self, TxContext};

    /// Name of the coin
    public struct TRUSTED_COIN has drop {}

    /// Register the trusted currency to acquire its `TreasuryCap`. Because
    /// this is a module initializer, it ensures the currency only gets
    /// registered once.
    fun init(witness: TRUSTED_COIN, ctx: &mut TxContext) {
        // Get a treasury cap for the coin and give it to the transaction
        // sender
        let (treasury_cap, metadata) = coin::create_currency<TRUSTED_COIN>(witness, 2, b"TRUSTED", b"", b"", option::none(), ctx);
        transfer::public_freeze_object(metadata);
        transfer::public_transfer(treasury_cap, tx_context::sender(ctx))
    }

    public entry fun mint(treasury_cap: &mut TreasuryCap<TRUSTED_COIN>, amount: u64, ctx: &mut TxContext) {
        let coin = coin::mint<TRUSTED_COIN>(treasury_cap, amount, ctx);
        transfer::public_transfer(coin, tx_context::sender(ctx));
    }
}