// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Test objectKeys lookup for wrapped object tombstone versions.
// cp 1: Foo and Foo2 created (version 2)
// cp 2: Both wrapped (tombstone version 3), Wrapper transferred 3 times to
//       pump lamport version
// cp 3: Both unwrapped, backward_history stores lamport-1 = 6
// cp 4: Foo wrapped again (second tombstone v8), Wrapper transferred 3 times
// cp 5: Foo unwrapped again, backward_history stores lamport-1 = 11
// Tests:
//   - old_active_version (v2): returns INDEXED, not a false tombstone
//   - first_real_tombstone (v3): returns WRAPPED_OR_DELETED via objects_version
//   - first_lamport_minus_one (v6): version corrected from 6 to 3
//   - batch_lamport_correction (v6 for Foo + v6 for Foo2): both corrected to v3
//   - both_real_tombstones (v3 + v8 for same object): returns only the latest
//     version (v8) since DISTINCT ON keeps one result per object
//   - both_lamport_approximations (v6 + v11 for same object): returns only the
//     latest corrected version (v8)

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
//> 1: P0::m::create_foo();
//> TransferObjects([Result(0), Result(1)], Input(0))

//# create-checkpoint

//# programmable --sender A --inputs @A object(2,0) object(2,1)
//> 0: P0::m::wrap_foo(Input(1));
//> 1: P0::m::wrap_foo(Input(2));
//> TransferObjects([Result(0), Result(1)], Input(0))

//# transfer-object 4,0 --sender A --recipient A

//# transfer-object 4,0 --sender A --recipient A

//# transfer-object 4,0 --sender A --recipient A

//# create-checkpoint

//# programmable --sender A --inputs @A object(4,0) object(4,1)
//> 0: P0::m::unwrap_foo(Input(1));
//> 1: P0::m::unwrap_foo(Input(2));
//> TransferObjects([Result(0), Result(1)], Input(0))

//# create-checkpoint

// Re-wrap Foo to create a second tombstone version for the same object.
//# programmable --sender A --inputs @A object(2,0)
//> 0: P0::m::wrap_foo(Input(1));
//> TransferObjects([Result(0)], Input(0))

//# transfer-object 11,0 --sender A --recipient A

//# transfer-object 11,0 --sender A --recipient A

//# transfer-object 11,0 --sender A --recipient A

//# create-checkpoint

//# programmable --sender A --inputs @A object(11,0)
//> 0: P0::m::unwrap_foo(Input(1));
//> TransferObjects([Result(0)], Input(0))

//# create-checkpoint

//# run-graphql
{
  old_active_version: objects(filter: {objectKeys: [{objectId: "@{obj_2_0}", version: 2}]}) {
    nodes {
      address
      status
      version
    }
  }
  first_real_tombstone: objects(filter: {objectKeys: [{objectId: "@{obj_2_0}", version: 3}]}) {
    nodes {
      address
      status
      version
    }
  }
  first_lamport_minus_one: objects(filter: {objectKeys: [{objectId: "@{obj_2_0}", version: 6}]}) {
    nodes {
      address
      status
      version
    }
  }
  batch_lamport_correction: objects(filter: {objectKeys: [
    {objectId: "@{obj_2_0}", version: 6},
    {objectId: "@{obj_2_1}", version: 6}
  ]}) {
    nodes {
      address
      status
      version
    }
  }
  both_real_tombstones: objects(filter: {objectKeys: [
    {objectId: "@{obj_2_0}", version: 3},
    {objectId: "@{obj_2_0}", version: 8}
  ]}) {
    nodes {
      address
      status
      version
    }
  }
  both_lamport_approximations: objects(filter: {objectKeys: [
    {objectId: "@{obj_2_0}", version: 6},
    {objectId: "@{obj_2_0}", version: 11}
  ]}) {
    nodes {
      address
      status
      version
    }
  }
}
