// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Tests that an objectKeys filter can pin multiple versions of the same
// object at once, and that cursor pagination works across rows that share an
// object_id and differ only in version.

//# init --protocol-version 4 --addresses Test=0x0 --accounts A --simulator

//# publish
module Test::M1 {
    public struct Object has key, store {
        id: UID,
        value: u64,
    }

    public entry fun create(value: u64, recipient: address, ctx: &mut TxContext) {
        transfer::public_transfer(
            Object { id: object::new(ctx), value },
            recipient
        )
    }

    public entry fun update(value: u64, o: &mut Object) {
        o.value = value;
    }
}

//# run Test::M1::create --args 0 @A

//# run Test::M1::update --sender A --args 100 object(2,0)

//# run Test::M1::update --sender A --args 200 object(2,0)

//# create-checkpoint

//# run Test::M1::update --sender A --args 300 object(2,0)

//# create-checkpoint

//# run-graphql
# All four versions of the same object are returned: three from
# objects_backward_history and the current one from checkpointed_objects.
{
  objects(
    filter: {
      objectKeys: [
        {objectId: "@{obj_2_0}", version: 3},
        {objectId: "@{obj_2_0}", version: 4},
        {objectId: "@{obj_2_0}", version: 5},
        {objectId: "@{obj_2_0}", version: 6}
      ]
    }
  ) {
    pageInfo {
      hasPreviousPage
      hasNextPage
    }
    nodes {
      version
      asMoveObject {
        contents {
          json
        }
      }
    }
  }
}

//# run-graphql
# Page boundary falls inside the same-id cluster: the first page ends between
# version 4 and version 5.
{
  objects(
    first: 2
    filter: {
      objectKeys: [
        {objectId: "@{obj_2_0}", version: 3},
        {objectId: "@{obj_2_0}", version: 4},
        {objectId: "@{obj_2_0}", version: 5},
        {objectId: "@{obj_2_0}", version: 6}
      ]
    }
  ) {
    pageInfo {
      hasPreviousPage
      hasNextPage
    }
    nodes {
      version
    }
  }
}

//# run-graphql --cursors bcs(@{obj_2_0},4,2)
# Resuming from a cursor pointing at version 4 returns the remaining
# versions 5 and 6.
{
  objects(
    after: "@{cursor_0}"
    filter: {
      objectKeys: [
        {objectId: "@{obj_2_0}", version: 3},
        {objectId: "@{obj_2_0}", version: 4},
        {objectId: "@{obj_2_0}", version: 5},
        {objectId: "@{obj_2_0}", version: 6}
      ]
    }
  ) {
    pageInfo {
      hasPreviousPage
      hasNextPage
    }
    nodes {
      version
    }
  }
}

//# run-graphql --cursors bcs(@{obj_2_0},5,2)
# Paginating backwards from a cursor pointing at version 5 returns
# versions 3 and 4.
{
  objects(
    last: 2
    before: "@{cursor_0}"
    filter: {
      objectKeys: [
        {objectId: "@{obj_2_0}", version: 3},
        {objectId: "@{obj_2_0}", version: 4},
        {objectId: "@{obj_2_0}", version: 5},
        {objectId: "@{obj_2_0}", version: 6}
      ]
    }
  ) {
    pageInfo {
      hasPreviousPage
      hasNextPage
    }
    nodes {
      version
    }
  }
}
