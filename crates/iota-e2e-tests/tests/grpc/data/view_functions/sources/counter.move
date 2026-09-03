// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module view_functions::counter {
    public struct Counter has key {
        id: UID,
        value: u64,
    }

    fun init(ctx: &mut TxContext) {
        transfer::share_object(Counter {
            id: object::new(ctx),
            value: 42,
        });
    }

    #[view]
    public fun value(counter: &Counter): u64 {
        counter.value
    }

    /// A public function without the `#[view]` attribute, used to check that
    /// view calls to non-view functions are rejected.
    public fun value_not_view(counter: &Counter): u64 {
        counter.value
    }

    /// Mutates on-chain state, used to check that a view call reflects a
    /// state change made by a normal transaction.
    public fun bump(counter: &mut Counter) {
        counter.value = counter.value + 1;
    }
}
