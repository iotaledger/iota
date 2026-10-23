// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module tto_shared::M {
    use iota::transfer::Receiving;

    public struct A has key {
        id: UID,
    }

    public struct B has key, store {
        id: UID,
    }

    public struct C has key {
        id: UID,
        wrapped: B,
    }

    /// Shares a parent and sends it a child to receive.
    public fun start(ctx: &mut TxContext) {
        let a = A { id: object::new(ctx) };
        let b = B { id: object::new(ctx) };
        transfer::public_transfer(b, object::id_address(&a));
        transfer::share_object(a);
    }

    public fun receiver(parent: &mut A, x: Receiving<B>) {
        let b = transfer::receive(&mut parent.id, x);
        transfer::public_transfer(b, @tto_shared);
    }

    public fun deleter(parent: &mut A, x: Receiving<B>) {
        let B { id } = transfer::receive(&mut parent.id, x);
        object::delete(id);
    }

    public fun wrapper(parent: &mut A, x: Receiving<B>, ctx: &mut TxContext) {
        let b = transfer::receive(&mut parent.id, x);
        let c = C { id: object::new(ctx), wrapped: b };
        transfer::transfer(c, @tto_shared);
    }
}
