// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// 1. create Parent (version 2)
// 2. create Child (version 3)
// 3. add Child to Parent -> Parent (v4) - Field(v4) - Child (v4)
// 4. mutate Parent -> Parent (v5)
// 5. reclaim Child -> Parent(v6), Child (v6), Field deleted

// We check consistency by querying the dynamic fields rooted on parent
// throughout the parent's evolution.

// The stored values should resolve the owner to the correct version of the Field.

// Parent - dynamic fields resolve the owner correctly to Field1(v4)
// Child1 (v4) should have the Field as an owner
// Child1 (v5) does not exist
// Child1 (v6) does not have an owner

//# init --protocol-version 29 --addresses Test=0x0 --accounts A --simulator

//# publish
module Test::M1 {
    use iota::dynamic_object_field as ofield;

    public struct Parent has key, store {
        id: UID,
        count: u64
    }

    public struct Child has key, store {
        id: UID,
        count: u64,
    }

    public entry fun parent(recipient: address, ctx: &mut TxContext) {
        transfer::public_transfer(
            Parent { id: object::new(ctx), count: 0 },
            recipient
        )
    }

    public entry fun mutate_parent(parent: &mut Parent) {
        parent.count = parent.count + 42;
    }

    public entry fun child(recipient: address, ctx: &mut TxContext) {
        transfer::public_transfer(
            Child { id: object::new(ctx), count: 0 },
            recipient
        )
    }

    public fun add_child(parent: &mut Parent, child: Child, name: u64) {
        ofield::add(&mut parent.id, name, child);
    }

    public fun reclaim_child(parent: &mut Parent, name: u64, recipient: address) {
        let child: Child =  ofield::remove(&mut parent.id, name);
        transfer::public_transfer(child, recipient)
    }
}

//# run Test::M1::parent --sender A --args @A

//# run Test::M1::child --sender A --args @A

//# run Test::M1::add_child --sender A --args object(2,0) object(3,0) 42

//# run Test::M1::mutate_parent --sender A --args object(2,0)

//# run Test::M1::reclaim_child --sender A --args object(2,0) 42 @A

//# create-checkpoint

//# run-graphql
{
  object(address: "@{obj_2_0}", version: 4) {
    dynamicFields {
      nodes {
        value {
            ... on MoveObject {
              address
              version
              contents {
                json
              }
              owner {
                ... on Parent {
                  parent {
                    address
                    version
                    status
                    asMoveObject {
                      contents {
                        type {
                          repr
                        }
                      }
                    }
                  }
                }
              }
            }
        }
      }
    }
  }
}

//# run-graphql
{
  object(address: "@{obj_2_0}", version: 5) {
    dynamicFields {
      nodes {
        value {
            ... on MoveObject {
              address
              version
              contents {
                json
              }
              owner {
                ... on Parent {
                  parent {
                    address
                    version
                    status
                    asMoveObject {
                      contents {
                        type {
                          repr
                        }
                      }
                    }
                  }
                }
              }
            }
        }
      }
    }
  }
}

//# run-graphql
{
  object(address: "@{obj_3_0}", version: 4) {
    asMoveObject {
          address
          version
          contents {
            json
          }
    }
    owner {
        ... on Parent {
          parent {
            address
            version
            status
            asMoveObject {
              contents {
                type {
                  repr
                }
              }
            }
          }
        }
    }
  }
}

//# run-graphql
{
  object(address: "@{obj_3_0}", version: 6) {
    asMoveObject {
          address
          version
          contents {
            json
          }
    }
    owner {
        ... on Parent {
          parent {
            address
            version
            status
            asMoveObject {
              contents {
                type {
                  repr
                }
              }
            }
          }
        }
    }
  }
}
