// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module move_test_code::tto_coin {
    use iota::coin::Coin;
    use iota::iota::IOTA;
    use iota::transfer::Receiving;

    public struct A has key, store {
        id: UID,
    }

    public fun start(coin: Coin<IOTA>, ctx: &mut TxContext) {
        let a = A { id: object::new(ctx) };
        let a_address = object::id_address(&a);

        transfer::public_transfer(a, tx_context::sender(ctx));
        transfer::public_transfer(coin, a_address);
    }

    public entry fun receive(parent: &mut A, x: Receiving<Coin<IOTA>>) {
        let coin = transfer::public_receive(&mut parent.id, x);
        transfer::public_transfer(coin, @0x0);
    }
}
