// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# init --addresses Test=0x0 --accounts A

//# publish
module Test::M {
    public struct Obj has key {
        id: iota::object::UID,
        value: u64
    }

    public entry fun mint(ctx: &mut TxContext) {
        iota::transfer::transfer(
            Obj { id: iota::object::new(ctx), value: 0 },
            tx_context::sender(ctx),
        )
    }

    public entry fun incr(obj: &mut Obj) {
        obj.value = obj.value + 1
    }
}

//# run Test::M::mint --sender A

//# run Test::M::incr --sender A --args object(2,0)
