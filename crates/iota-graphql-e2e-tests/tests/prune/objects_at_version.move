// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Exercises GraphQL object-at-version queries under pruning, with no
// historical fallback configured. Covers:
// - `Query.object(address, version)`
// - `Owner.dynamicField` at a `rootVersion`

//# init --protocol-version 12 --addresses P0=0x0 --accounts A --simulator --epochs-to-keep 1

//# publish
module P0::m {
    use iota::dynamic_field as field;

    public struct Foo has key, store { id: UID, value: u64 }

    public entry fun create(ctx: &mut TxContext) {
        transfer::public_transfer(Foo { id: object::new(ctx), value: 0 }, ctx.sender())
    }

    public entry fun bump(foo: &mut Foo) {
        foo.value = foo.value + 1;
    }

    public entry fun add_df(foo: &mut Foo, name: u64, value: u64) {
        field::add(&mut foo.id, name, value);
    }

    public entry fun mutate_df(foo: &mut Foo, name: u64, value: u64) {
        *field::borrow_mut<u64, u64>(&mut foo.id, name) = value;
    }
}

//# run P0::m::create --sender A

//# run P0::m::add_df --sender A --args object(2,0) 42 1

//# create-checkpoint

//# run P0::m::mutate_df --sender A --args object(2,0) 42 2

//# run P0::m::bump --sender A --args object(2,0)

//# create-checkpoint

//# advance-epoch

//# create-checkpoint

//# advance-epoch

//# create-checkpoint

//# advance-epoch

//# create-checkpoint

//# create-checkpoint

//# create-checkpoint

//# run-graphql --wait-for-checkpoint-pruned 6
# A: Query.object at a version whose row content has been pruned — no
# fallback. The version resolves as non-existent.
{
  object(address: "@{obj_2_0}", version: 3) {
    version
    status
  }
}

//# run-graphql
# B: Query.object at the current version resolves.
{
  object(address: "@{obj_2_0}", version: 5) {
    version
    status
    asMoveObject {
      contents { json }
    }
  }
}

//# run-graphql
# C: Query.object at a version that never existed resolves as non-existent.
{
  object(address: "@{obj_2_0}", version: 100) {
    version
    status
  }
}

//# run-graphql
# D: a dynamic field looked up at a pruned parent version — the field
# object's history at that version has been pruned, so the field resolves as
# non-existent.
{
  owner(address: "@{obj_2_0}", rootVersion: 3) {
    dynamicField(name: {type: "u64", bcs: "KgAAAAAAAAA="}) {
      value {
        ... on MoveValue { json }
      }
    }
  }
}

//# run-graphql
# E: the same dynamic field at the current parent version resolves.
{
  owner(address: "@{obj_2_0}", rootVersion: 5) {
    dynamicField(name: {type: "u64", bcs: "KgAAAAAAAAA="}) {
      value {
        ... on MoveValue { json }
      }
    }
  }
}
