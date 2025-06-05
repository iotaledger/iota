// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Example coin with a trusted owner responsible for minting/burning (e.g., a stablecoin)
module examples::immutable_metadata_trusted_coin {
    use std::option;
    use iota::coin::{Self, TreasuryCap};
    use iota::transfer;
    use iota::tx_context::{Self, TxContext};
    use iota::coin::CoinMetadata;

    /// Name of the coin
    struct IMMUTABLE_METADATA_TRUSTED_COIN has drop {}

    struct HiddenCoinMetadata<phantom T> has key, store {
        id: iota::object::UID,
        metadata: CoinMetadata<T>,
    }

    /// Register the trusted currency to acquire its `TreasuryCap`. Because
    /// this is a module initializer, it ensures the currency only gets
    /// registered once.
    fun init(witness: IMMUTABLE_METADATA_TRUSTED_COIN, ctx: &mut tx_context::TxContext) {
        // Get a treasury cap for the coin and give it to the transaction
        // sender
        let (treasury_cap, metadata) = coin::create_currency<IMMUTABLE_METADATA_TRUSTED_COIN>(
            witness,
            2,
            b"IMM_META_TRUSTED",
            b"Immutable Metadata Trusted Coin",
            b"Immutable Metadata Trusted Coin for test",
            option::none(),
            ctx
        );
        transfer::public_transfer(metadata, tx_context::sender(ctx));
        transfer::public_transfer(treasury_cap, tx_context::sender(ctx))
    }

    public entry fun mint(treasury_cap: &mut TreasuryCap<IMMUTABLE_METADATA_TRUSTED_COIN>, amount: u64, ctx: &mut tx_context::TxContext) {
        let coin = coin::mint<IMMUTABLE_METADATA_TRUSTED_COIN>(treasury_cap, amount, ctx);
        transfer::public_transfer(coin, tx_context::sender(ctx));
    }

    public entry fun transfer(treasury_cap: TreasuryCap<IMMUTABLE_METADATA_TRUSTED_COIN>, recipient: address) {
        transfer::public_transfer(treasury_cap, recipient);
    }

    #[allow(lint(self_transfer))]
    public fun hide_metadata<T> (
        metadata: CoinMetadata<T>,
        ctx: &mut TxContext,
    ) {

        let hidden = HiddenCoinMetadata {
            id: iota::object::new(ctx),
            metadata: metadata,
        };

        transfer::public_transfer(hidden, tx_context::sender(ctx))
    }

    #[test_only]
    /// Wrapper of module initializer for testing
    public fun test_init(ctx: &mut TxContext) {
        init(IMMUTABLE_METADATA_TRUSTED_COIN {}, ctx)
    }
}
