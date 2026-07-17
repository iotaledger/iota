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

    #[view]
    public fun get_counter(wat_obj: &Wat): u64 {
        wat_obj.counter
    }

    #[view]
    public fun get_wat_object(wat_obj: &Wat): &Wat{
        wat_obj
    }

    #[view]
    public fun has_address_arg(wat_obj: &Wat, flag: bool, addr: address): bool {
        wat_obj.counter == 10 && flag && addr == @0x1
    }

    /// A public function without the `#[view]` attribute, used to check that
    /// view calls to non-view functions are rejected.
    public fun get_counter_not_view(wat_obj: &Wat): u64 {
        wat_obj.counter
    }
}
