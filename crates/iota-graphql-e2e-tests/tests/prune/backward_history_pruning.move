// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# init --protocol-version 12 --addresses P0=0x0 --accounts A --simulator --epochs-to-keep 1

//# publish
module P0::m {
    public struct Foo has key, store {
        id: UID,
        value: u64,
    }

    public struct Wrapper has key, store {
        id: UID,
        foo: Foo,
    }

    public fun create_foo(ctx: &mut TxContext): Foo {
        Foo { id: object::new(ctx), value: 0 }
    }

    public fun mutate_foo(foo: &mut Foo) {
        foo.value = foo.value + 1;
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

//# advance-epoch

//# programmable --sender A --inputs @A object(4,0)
//> 0: P0::m::unwrap_foo(Input(1));
//> TransferObjects([Result(0)], Input(0))

//# create-checkpoint

//# programmable --sender A --inputs object(2,0)
//> 0: P0::m::mutate_foo(Input(0));

//# create-checkpoint

//# advance-epoch

//# run-graphql
{
  real_tombstone_v3: objects(filter: {objectKeys: [{objectId: "@{obj_2_0}", version: 3}]}) {
    nodes { status version }
  }
  lamport_v6: objects(filter: {objectKeys: [{objectId: "@{obj_2_0}", version: 6}]}) {
    nodes { status version }
  }
  previous_active_v7: objects(filter: {objectKeys: [{objectId: "@{obj_2_0}", version: 7}]}) {
    nodes { status version }
  }
  current_active_v8: objects(filter: {objectKeys: [{objectId: "@{obj_2_0}", version: 8}]}) {
    nodes { status version }
  }
}

//# programmable --sender A --inputs @A
//> 0: P0::m::create_foo();
//> TransferObjects([Result(0)], Input(0))

//# create-checkpoint

//# advance-epoch

//# programmable --sender A --inputs @A
//> 0: P0::m::create_foo();
//> TransferObjects([Result(0)], Input(0))

//# create-checkpoint

//# advance-epoch

//# run-graphql --wait-for-checkpoint-pruned 4
{
  real_tombstone_v3: objects(filter: {objectKeys: [{objectId: "@{obj_2_0}", version: 3}]}) {
    nodes { status version }
  }
  lamport_v6: objects(filter: {objectKeys: [{objectId: "@{obj_2_0}", version: 6}]}) {
    nodes { status version }
  }
  previous_active_v7: objects(filter: {objectKeys: [{objectId: "@{obj_2_0}", version: 7}]}) {
    nodes { status version }
  }
  current_active_v8: objects(filter: {objectKeys: [{objectId: "@{obj_2_0}", version: 8}]}) {
    nodes { status version }
  }
}
