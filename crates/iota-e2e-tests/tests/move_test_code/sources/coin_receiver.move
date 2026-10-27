// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// An object coins can be sent to and received from.
module move_test_code::coin_receiver {
    use iota::coin::Coin;
    use iota::transfer::Receiving;

    public struct Parent has key {
        id: UID,
    }

    public fun create(ctx: &mut TxContext) {
        transfer::transfer(Parent { id: object::new(ctx) }, ctx.sender());
    }

    /// Receives the coin sent to `parent` and sends it on to `recipient`.
    public fun receive_and_send<T>(
        parent: &mut Parent,
        coin: Receiving<Coin<T>>,
        recipient: address,
    ) {
        let coin = transfer::public_receive(&mut parent.id, coin);
        transfer::public_transfer(coin, recipient);
    }
}
