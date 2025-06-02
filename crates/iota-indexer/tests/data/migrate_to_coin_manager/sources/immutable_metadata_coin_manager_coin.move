// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module migrate_coin::immutable_metadata_coin_manager_coin {
    use iota::coin_manager;
    use iota::coin::{CoinMetadata, TreasuryCap};
    use iota::transfer;
    use iota::tx_context::{Self, TxContext};

    /// Phantom parameter T can only be initialized in the `create_guardian`
    /// function. But the types passed here must have `drop`.
    struct Guardian<phantom T: drop> has key, store {
        id: iota::object::UID
    }

    /// This type is the witness resource and is intended to be used only once.
    struct IMMUTABLE_METADATA_COIN_MANAGER_COIN has drop {}

    /// The first argument of this function is an actual instance of the
    /// type T with `drop` ability. It is dropped as soon as received.
    public fun create_guardian<T: drop>(
        _witness: T, ctx: &mut TxContext
     ): Guardian<T> {
            Guardian { id: iota::object::new(ctx) }
     }

    /// Module initializer is the best way to ensure that the
    /// code is called only once. With `Witness` pattern it is
    /// often the best practice.
    fun init(witness: IMMUTABLE_METADATA_COIN_MANAGER_COIN, ctx: &mut tx_context::TxContext) {
        transfer::transfer(create_guardian(witness, ctx), tx_context::sender(ctx))
     }

    #[allow(lint(self_transfer, share_owned))]
    public fun migrate_to_manager<T, S: drop> (otw:Guardian<S>, cap: TreasuryCap<T>, meta: &CoinMetadata<T>, ctx: &mut tx_context::TxContext) {
        transfer::public_freeze_object(otw);

        let (cm_treasury_cap, manager) = coin_manager::new_with_immutable_metadata(cap, meta, ctx);

        // Transfer the `CoinManagerTreasuryCap` to the creator of the `Coin`.
        transfer::public_transfer(cm_treasury_cap, tx_context::sender(ctx));

        // Publicly share the `CoinManager` object for convenient usage by anyone interested.
        transfer::public_share_object(manager);
    }
}
