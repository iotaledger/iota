// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// DEPRECATED child count no longer tracked
// tests invalid wrapping of a parent object with children, in a single transaction

//# init --addresses test=0x0 --accounts A B

//# publish

module test::m {
    use iota::dynamic_object_field as ofield;

    public struct S has key, store {
        id: iota::object::UID,
    }

    public struct R has key {
        id: iota::object::UID,
        s: S,
    }

    public entry fun test_wrap(ctx: &mut TxContext) {
        let mut id = iota::object::new(ctx);
        let child = S { id: iota::object::new(ctx) };
        ofield::add(&mut id, 0, child);
        let parent = S { id };
        let r = R { id: iota::object::new(ctx), s: parent };
        iota::transfer::transfer(r, tx_context::sender(ctx))
    }
}

//# run test::m::test_wrap --sender A
