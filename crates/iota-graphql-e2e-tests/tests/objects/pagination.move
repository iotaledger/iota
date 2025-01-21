// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# init --protocol-version 3 --addresses Test=0x0 A=0x42 --simulator

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
}

//# run Test::M1::create --args 0 @A
# create obj_2_0 (1st object) for A

//# run Test::M1::create --args 1 @A
# create obj_3_0 (2nd object) for A

//# run Test::M1::create --args 2 @A
# create obj_4_0 (3rd object) for A

//# run Test::M1::create --args 3 @A
# create obj_5_0 (4th object) for A

//# run Test::M1::create --args 4 @A
# create obj_6_0 (5th object) for A

//# create-checkpoint

//# run-graphql
{
  # select all objects owned by A
  address(address: "@{A}") {
    objects {
      edges {
        cursor
      }
    }
  }
}

//# run-graphql
{
  # select the first 2 objects owned by A
  address(address: "@{A}") {
    objects(first: 2) {
      edges {
        cursor
      }
    }
  }
}

//# run-graphql
{
  # show the order of all object owned by A
  # order is defined by the bytes of their address
  address(address: "@{A}") {
    objects {
      edges {
        cursor
        node {
            address
        }
      }
    }
  }
  obj_3_0: object(address: "@{obj_3_0}") {
    address
  }
  obj_5_0: object(address: "@{obj_5_0}") {
    address
  }
  obj_6_0: object(address: "@{obj_6_0}") {
    address
  }
  obj_4_0: object(address: "@{obj_4_0}") {
    address
  }
  obj_2_0: object(address: "@{obj_2_0}") {
    address
  }
}

//# run-graphql --cursors @{obj_5_0}
{
  address(address: "@{A}") {
    # select the 5th and 3rd objects
    # note that order does not correspond
    # to order in which objects were created
    objects(first: 2 after: "@{cursor_0}") {
      edges {
        cursor
      }
    }
  }
}

//# run-graphql --cursors @{obj_4_0}
{
  address(address: "@{A}") {
    # select 1st object
    objects(first: 1 after: "@{cursor_0}") {
      edges {
        cursor
      }
    }
  }
}

//# run-graphql --cursors @{obj_3_0}
{
  address(address: "@{A}") {
    # select no object
    objects(last: 2 before: "@{cursor_0}") {
      edges {
        cursor
      }
    }
  }
}

//# run-graphql
{
  address(address: "@{A}") {
    objects(last: 2) {
      edges {
        cursor
        node {
            address
        }
      }
    }
  }
}
