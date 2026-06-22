// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Test that the unfiltered top-level objects() query returns consistent
// results at a past checkpoint, not the current state.
// cp 1: Object created (version 2)
// cp 2: Object mutated (version 3)
// Query objects() without filter at cp 1: should show version 2, not 3.

//# init --protocol-version 4 --addresses P0=0x0 --accounts A --simulator

//# publish
module P0::m {
    public struct Obj has key, store {
        id: UID,
        value: u64,
    }

    public entry fun create(value: u64, recipient: address, ctx: &mut TxContext) {
        transfer::public_transfer(
            Obj { id: object::new(ctx), value },
            recipient
        )
    }

    public entry fun update(o: &mut Obj, value: u64) {
        o.value = value;
    }
}

//# run P0::m::create --sender A --args 1 @A

//# create-checkpoint

//# run P0::m::update --sender A --args object(2,0) 2

//# create-checkpoint

//# run-graphql --cursors bcs(@{obj_0_0},1)
{
  objects(before: "@{cursor_0}", last: 50) {
    nodes {
      address
      version
    }
  }
}

//# run-graphql --cursors bcs(@{obj_0_0},2)
{
  objects(before: "@{cursor_0}", last: 50) {
    nodes {
      address
      version
    }
  }
}
