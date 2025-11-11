// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module wat_counter::wat_counter {
    use iota::object;
    use iota::tx_context::TxContext;
    use iota::transfer;

    /// Count [wat](https://www.destroyallsoftware.com/talks/wat) reactions.
    struct Wat has key {
        id: object::UID,
        counter: u64,
    }

    fun init(ctx: &mut TxContext) {
        let review = Wat {
            id: object::new(ctx),
            counter: 10,
        };
        transfer::share_object(review);
    }

    public fun get_counter(wat_obj: &Wat): u64 {
        wat_obj.counter
    }

    public fun get_wat_object(wat_obj: &Wat): &Wat{
        wat_obj
    }
}
