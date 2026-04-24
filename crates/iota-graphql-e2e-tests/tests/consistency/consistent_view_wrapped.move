// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Test consistent view returns WRAPPED_OR_DELETED for objects that were wrapped
// at the viewed checkpoint but later unwrapped.
// cp 1: Foo created
// cp 2: Foo wrapped into Wrapper
// cp 3: Wrapper destroyed (Foo unwrapped back to active)
// Query Foo at cp 2 via objectIds + cursor: should be WRAPPED_OR_DELETED

//# init --protocol-version 4 --addresses P0=0x0 --accounts A --simulator --objects-snapshot-min-checkpoint-lag 1

//# publish
module P0::m {
    public struct Foo has key, store {
        id: UID,
    }

    public struct Wrapper has key, store {
        id: UID,
        foo: Foo,
    }

    public fun create_foo(ctx: &mut TxContext): Foo {
        Foo { id: object::new(ctx) }
    }

    public fun wrap_foo(foo: Foo, ctx: &mut TxContext): Wrapper {
        Wrapper { id: object::new(ctx), foo }
    }

    public fun unwrap_foo(w: Wrapper): Foo {
        let Wrapper { id, foo } = w;
        object::delete(id);
        foo
    }
}

//# programmable --sender A --inputs @A
//> 0: P0::m::create_foo();
//> TransferObjects([Result(0)], Input(0))

//# create-checkpoint

//# programmable --sender A --inputs @A object(2,0)
//> 0: P0::m::wrap_foo(Input(1));
//> TransferObjects([Result(0)], Input(0))

//# create-checkpoint

//# transfer-object 4,0 --sender A --recipient A

//# transfer-object 4,0 --sender A --recipient A

//# transfer-object 4,0 --sender A --recipient A

//# programmable --sender A --inputs @A object(4,0)
//> 0: P0::m::unwrap_foo(Input(1));
//> TransferObjects([Result(0)], Input(0))

//# create-checkpoint

//# run-graphql --cursors bcs(@{obj_0_0},2)
{
  objects(filter: {objectIds: ["@{obj_2_0}"]}, before: "@{cursor_0}") {
    nodes {
      address
      status
      version
    }
  }
}
