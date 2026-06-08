// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// This test checks if object versions are assigned correctly for an
// object create -> delete -> recreate lifecycle. Assigning the version
// incorrectly (e.g. constant -1 for both NotYetCreated rows) may result
// in the wrong object version being returned by consistent views:
// consistent-view logic selects MIN(version) and so assumes object
// versions are monotonic throughout the lifetime of the object.


//# init --protocol-version 4 --addresses Test=0x0 --accounts A --simulator

//# publish
module Test::M1 {
    use iota::dynamic_object_field as ofield;

    public struct Parent has key, store {
        id: UID,
    }

    public struct Child has key, store {
        id: UID,
        count: u64,
    }

    public entry fun parent(recipient: address, ctx: &mut TxContext) {
        transfer::public_transfer(Parent { id: object::new(ctx) }, recipient)
    }

    public entry fun child(recipient: address, ctx: &mut TxContext) {
        transfer::public_transfer(Child { id: object::new(ctx), count: 0 }, recipient)
    }

    public fun add_child(parent: &mut Parent, child: Child, name: u64) {
        ofield::add(&mut parent.id, name, child);
    }

    public fun reclaim_and_transfer_child(parent: &mut Parent, name: u64, recipient: address) {
        let c: Child = ofield::remove(&mut parent.id, name);
        transfer::public_transfer(c, recipient)
    }
}

//# programmable --sender A --inputs @A
//> 0: Test::M1::child(Input(0));
//> 1: Test::M1::child(Input(0));
//> 2: Test::M1::parent(Input(0));

//# run Test::M1::add_child --sender A --args object(2,2) object(2,1) 41

//# create-checkpoint

//# run Test::M1::add_child --sender A --args object(2,2) object(2,0) 42

//# create-checkpoint

//# run Test::M1::reclaim_and_transfer_child --sender A --args object(2,2) 42 @A

//# create-checkpoint

//# run Test::M1::add_child --sender A --args object(2,2) object(2,0) 42

//# create-checkpoint

//# run-graphql --cursors bcs(@{obj_3_0},1)
{
  cv1_after: owner(address: "@{obj_2_2}") {
    dynamicFields(after: "@{cursor_0}") {
      edges {
        node {
          name { bcs }
          value { ... on MoveObject { contents { json } } }
        }
      }
    }
  }
  cv1_before: owner(address: "@{obj_2_2}") {
    dynamicFields(before: "@{cursor_0}") {
      edges {
        node {
          name { bcs }
          value { ... on MoveObject { contents { json } } }
        }
      }
    }
  }
}

//# run-graphql --cursors bcs(@{obj_3_0},2)
{
  cv2_after: owner(address: "@{obj_2_2}") {
    dynamicFields(after: "@{cursor_0}") {
      edges {
        node {
          name { bcs }
          value { ... on MoveObject { contents { json } } }
        }
      }
    }
  }
  cv2_before: owner(address: "@{obj_2_2}") {
    dynamicFields(before: "@{cursor_0}") {
      edges {
        node {
          name { bcs }
          value { ... on MoveObject { contents { json } } }
        }
      }
    }
  }
}

//# run-graphql --cursors bcs(@{obj_3_0},3)
{
  cv3_after: owner(address: "@{obj_2_2}") {
    dynamicFields(after: "@{cursor_0}") {
      edges {
        node {
          name { bcs }
          value { ... on MoveObject { contents { json } } }
        }
      }
    }
  }
  cv3_before: owner(address: "@{obj_2_2}") {
    dynamicFields(before: "@{cursor_0}") {
      edges {
        node {
          name { bcs }
          value { ... on MoveObject { contents { json } } }
        }
      }
    }
  }
}

//# run-graphql --cursors bcs(@{obj_3_0},4)
{
  cv4_after: owner(address: "@{obj_2_2}") {
    dynamicFields(after: "@{cursor_0}") {
      edges {
        node {
          name { bcs }
          value { ... on MoveObject { contents { json } } }
        }
      }
    }
  }
  cv4_before: owner(address: "@{obj_2_2}") {
    dynamicFields(before: "@{cursor_0}") {
      edges {
        node {
          name { bcs }
          value { ... on MoveObject { contents { json } } }
        }
      }
    }
  }
}
