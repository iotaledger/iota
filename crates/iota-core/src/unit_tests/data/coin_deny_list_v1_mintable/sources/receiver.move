// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// An object coins can be sent to and received from, or stored in as dynamic
/// object fields and taken out of.
module coin_deny_list_v1_mintable::receiver {
    use iota::coin::Coin;
    use iota::dynamic_object_field as dof;
    use iota::transfer::Receiving;

    /// The name of the dynamic object field a coin is stored under.
    const COIN: vector<u8> = b"coin";

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

    /// Stores `coin` as a dynamic object field of `parent`.
    public fun add_coin<T>(parent: &mut Parent, coin: Coin<T>) {
        dof::add(&mut parent.id, COIN, coin);
    }

    /// Takes the coin stored as a dynamic object field of `parent` and sends
    /// it on to `recipient`.
    public fun take_and_send<T>(parent: &mut Parent, recipient: address) {
        let coin: Coin<T> = dof::remove(&mut parent.id, COIN);
        transfer::public_transfer(coin, recipient);
    }
}
