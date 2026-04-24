// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Test objectKeys lookup for wrapped object tombstone versions.
// cp 1: Foo created (version 2)
// cp 2: Foo wrapped (tombstone version 3), Wrapper transferred 3 times to
//       pump lamport version
// cp 3: Foo unwrapped (version 7), backward_history stores lamport-1 = 6
// Query via objectKeys with version 3 (real tombstone): empty
// Query via objectKeys with version 6 (lamport-1): returns WRAPPED_OR_DELETED

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

//# transfer-object 4,0 --sender A --recipient A

//# transfer-object 4,0 --sender A --recipient A

//# transfer-object 4,0 --sender A --recipient A

//# create-checkpoint

//# programmable --sender A --inputs @A object(4,0)
//> 0: P0::m::unwrap_foo(Input(1));
//> TransferObjects([Result(0)], Input(0))

//# create-checkpoint

//# run-graphql
{
  real_tombstone: objects(filter: {objectKeys: [{objectId: "@{obj_2_0}", version: 3}]}) {
    nodes {
      address
      status
      version
    }
  }
  lamport_minus_one: objects(filter: {objectKeys: [{objectId: "@{obj_2_0}", version: 6}]}) {
    nodes {
      address
      status
      version
    }
  }
}
