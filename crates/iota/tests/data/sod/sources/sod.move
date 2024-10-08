// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module sod::sod {
    use iota::object::{Self, UID};
    use iota::tx_context::TxContext;
    use iota::transfer;

    struct A has key, store {
        id: UID,
    }

    public fun start(ctx: &mut TxContext) {
        let a = A { id: object::new(ctx) };
        transfer::public_share_object(a);
    }

    public entry fun delete(a: A) {
        let A { id } = a;
        object::delete(id);
    }
}
