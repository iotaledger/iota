// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# init --addresses tto=0x0

//# publish
module tto::M1 {
    use iota::transfer::Receiving;

    public struct A has key, store {
        id: UID,
    }

    public struct B has key, store {
        id: UID,
    }

    public fun start(ctx: &mut TxContext) {
        let a = A { id: object::new(ctx) };
        let a_address = object::id_address(&a);
        let b = B { id: object::new(ctx) };
        transfer::public_share_object(a);
        transfer::public_transfer(b, a_address);
    }

    public entry fun receiver(parent: &mut A, x: Receiving<B>) {
        let b = transfer::receive(&mut parent.id, x);
        transfer::public_transfer(b, @tto);
    }
}

//# run tto::M1::start

//# view-object 2,0

//# view-object 2,1

//# run tto::M1::receiver --args object(2,0) receiving(2,1)

//# view-object 2,0

//# view-object 2,1

//# run tto::M1::receiver --args object(2,0) receiving(2,1)@3
