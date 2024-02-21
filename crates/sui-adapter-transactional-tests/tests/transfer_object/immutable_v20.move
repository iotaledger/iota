// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// tests TransferObject should fail for an immutable object

//# init --accounts A B --addresses test=0x0 --shared-object-deletion false

//# publish

module test::m {
    use sui::transfer;
    use sui::tx_context::TxContext;
    use sui::object::{Self, UID};

    struct S has store, key { id: UID }
    struct Cup<phantom T: store> has store, key { id: UID }

    public entry fun mint_s(ctx: &mut TxContext) {
        let id = object::new(ctx);
        transfer::public_freeze_object(S { id })
    }
}

//# run test::m::mint_s --sender A

//# view-object 2,0

//# transfer-object 2,0 --sender A --recipient B

//# view-object 2,0
