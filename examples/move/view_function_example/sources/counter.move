// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// A minimal shared counter, exposing a `#[view]` function to read its value
/// without submitting a transaction.
module view_function_example::counter {
    public struct Counter has key {
        id: UID,
        value: u64,
    }

    fun init(ctx: &mut TxContext) {
        transfer::share_object(Counter {
            id: object::new(ctx),
            value: 0,
        });
    }

    public fun increment(counter: &mut Counter) {
        counter.value = counter.value + 1;
    }

    #[view]
    public fun value(counter: &Counter): u64 {
        counter.value
    }
}
