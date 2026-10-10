// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// An object coins can be sent to and received from.
module coin_deny_list_v1_mintable::receiver {
    use iota::coin::Coin;
    use iota::transfer::Receiving;

    public struct Parent has key {
        id: UID,
    }

    public fun create(ctx: &mut TxContext) {
        transfer::transfer(Parent { id: object::new(ctx) }, ctx.sender());
    }

    /// Receives the coin sent to `parent` and sends it on to `to`.
    public fun receive_and_send<T>(
        parent: &mut Parent,
        coin: Receiving<Coin<T>>,
        to: address,
    ) {
        let coin = transfer::public_receive(&mut parent.id, coin);
        transfer::public_transfer(coin, to);
    }
}
