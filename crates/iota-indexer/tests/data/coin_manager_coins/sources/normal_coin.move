// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module coin_manager_coin::normal_coin {
    use iota::coin_manager;
    use std::option;
    use iota::transfer;
    use iota::tx_context::{Self, TxContext};

    struct NORMAL_COIN has drop {}

    fun init(witness: NORMAL_COIN, ctx: &mut TxContext) {
        // Create a `Coin` type and have it managed.
        let (cm_treasury_cap, cm_meta_cap, manager) = coin_manager::create(
            witness,
            0,
            b"NRML",
            b"Normal Coin",
            b"Normal description.",
            option::none(),
            ctx
        );

        // Transfer the `CoinManagerTreasuryCap` to the creator of the `Coin`.
        transfer::public_transfer(cm_treasury_cap, tx_context::sender(ctx));

        // Transfer the `CoinManagerMetadataCap` to the creator of the `Coin`.
        transfer::public_transfer(cm_meta_cap, tx_context::sender(ctx));

        // Publicly share the `CoinManager` object for convenient usage by anyone interested.
        transfer::public_share_object(manager);
    }
}
