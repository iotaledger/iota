// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module coin_manager_coin::immutable_metadata_coin {
    use iota::coin_manager;
    use std::option;
    use iota::coin::{Self, CoinMetadata};
    use iota::transfer;
    use iota::tx_context::{Self, TxContext};

    struct IMMUTABLE_METADATA_COIN has drop {}

    struct HiddenCoinMetadata<phantom T> has key, store {
        id: iota::object::UID,
        metadata: CoinMetadata<T>,
    }

    #[allow(lint(self_transfer, share_owned))]
    fun init(witness: IMMUTABLE_METADATA_COIN, ctx: &mut TxContext) {
        let (cap, meta) = coin::create_currency(
            witness,
            0,
            b"IMMMETA",
            b"Immutable Meta Coin",
            b"Immutable Meta  description.",
            option::none(),
            ctx
        );

        let (cm_treasury_cap, manager) = coin_manager::new_with_immutable_metadata(cap, &meta, ctx);

        // Hide metadata inside of some dummy object, so it's not accessible directly via ID.
        // So that Node and Indexer will not be able to use it and will have to rely on CoinManager only.
        let hidden = HiddenCoinMetadata {
            id: iota::object::new(ctx),
            metadata: meta,
        };
        transfer::public_transfer(hidden, tx_context::sender(ctx));

        // Transfer the `CoinManagerTreasuryCap` to the creator of the `Coin`.
        transfer::public_transfer(cm_treasury_cap, tx_context::sender(ctx));

        // Publicly share the `CoinManager` object for convenient usage by anyone interested.
        transfer::public_share_object(manager);
    }
}
