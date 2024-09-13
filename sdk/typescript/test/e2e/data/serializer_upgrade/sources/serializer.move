// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module serializer::serializer_tests {
    use iota::tx_context::{Self, TxContext};
    use iota::transfer;
    use iota::object::{Self, UID};
    use iota::clock::Clock;

    struct MutableShared has key {
        id: UID,
        value: u64,
    }

    fun init(ctx: &mut TxContext) {
        transfer::share_object(MutableShared {
            id: object::new(ctx),
            value: 1,
        })
    }

    public entry fun use_clock(_clock: &Clock) {}

    public entry fun list<T: key + store, C>(
        item: T,
        ctx: &mut TxContext
    ) {
        transfer::public_transfer(item, tx_context::sender(ctx))
    }

    public fun return_struct<T: key + store>(
        item: T,
    ): T {
        item
    }

    public entry fun value(clock: &MutableShared) {
        assert!(clock.value > 10, 2);
    }

    public entry fun set_value(clock: &mut MutableShared) {
        clock.value = 20;
    }

    public entry fun delete_value(clock: MutableShared) {
        let MutableShared { id, value: _ } = clock;
        object::delete(id);
    }

    public fun test_abort() {
        abort 1
    }
}
