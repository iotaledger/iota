// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// DEPRECATED child count no longer tracked
// tests transferring a wrapped object that has never previously been in storage

//# init --addresses test=0x0 --accounts A B

//# publish

module test::m {
    public struct S has key, store {
        id: iota::object::UID,
    }

    public struct R has key {
        id: iota::object::UID,
        s: S,
    }

    public entry fun create(ctx: &mut TxContext) {
        let parent = iota::object::new(ctx);
        let child = S { id: iota::object::new(ctx) };
        iota::transfer::transfer(R { id: parent, s: child }, tx_context::sender(ctx))
    }

    public entry fun unwrap_and_transfer(r: R, ctx: &mut TxContext) {
        let R { id, s } = r;
        iota::object::delete(id);
        iota::transfer::transfer(s, tx_context::sender(ctx));
    }
}

//
// Test sharing
//

//# run test::m::create --sender A

//# run test::m::unwrap_and_transfer --args object(2,0) --sender A
