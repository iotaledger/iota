Query: `objects`

```
{
  objects(first: null, last: null, after: null, filter: null) {
    edges {
      node {
        address
        objects {
          edges {
            node {
              digest
            }
          }
        }
        balance {
          coinType {
            repr
            signature
            layout
            abilities
          }
          coinObjectCount
          totalBalance
        }
        balances {
          edges {
            node {
              coinObjectCount
            }
          }
        }
        coins {
          edges {
            node {
              digest
            }
          }
        }
        stakedIotas {
          edges {
            node {
              digest
            }
          }
        }
        version
        status
        digest
        owner {
          __typename
        }
        previousTransactionBlock {
          digest
          bcs
        }
        storageRebate
        receivedTransactionBlocks {
          edges {
            node {
              digest
              bcs
            }
          }
        }
        bcs
        dynamicField(
          name: {type: "0x0000000000000000000000000000000000000000000000000000000000000001::string::String", bcs: "A2RmMQ=="}
        ) {
          __typename
        }
        dynamicObjectField(
          name: {type: "0x0000000000000000000000000000000000000000000000000000000000000001::string::String", bcs: "A2RmNQ=="}
        ) {
          name {
            bcs
          }
        }
        dynamicFields {
          edges {
            node {
              name {
                __typename
              }
            }
          }
        }
        asMoveObject {
          digest
        }
        asMovePackage {
          digest
        }
      }
    }
  }
}
```

tested by [crates/iota-graphql-e2e-tests/tests/consistency/performance/many_objects.move](crates/iota-graphql-e2e-tests/tests/consistency/performance/many_objects.move):

```
//# run-graphql
{
  last_2: objects(last: 2, filter: {type: "@{Test}"}) {
    nodes {
      version
      asMoveObject {
        owner {
          ... on AddressOwner {
            owner {
              address
            }
          }
        }
        contents {
          json
          type {
            repr
          }
        }
      }
    }
  }
  last_4_objs_owned_by_A: address(address: "@{A}") {
    objects(last: 4) {
      nodes {
        owner {
          ... on AddressOwner {
            owner {
              address
            }
          }
        }
        contents {
          json
          type {
            repr
          }
        }
      }
    }
  }
}
```

tested by [crates/iota-graphql-e2e-tests/tests/consistency/objects_pagination.move](crates/iota-graphql-e2e-tests/tests/consistency/objects_pagination.move):

```
//# run-graphql --cursors @{obj_6_0,2}
{
  before_obj_6_0_at_checkpoint_2: objects(filter: {type: "@{Test}"}, before: "@{cursor_0}") {
    nodes {
      version
      asMoveObject {
        contents {
          type {
            repr
          }
          json
        }
      }
    }
  }
}
```

tested by [crates/iota-graphql-e2e-tests/coverage/query/objects.md](crates/iota-graphql-e2e-tests/coverage/query/objects.md):

```
//# run-graphql
{
  objects(filter: {type: "0x2::coin"}) {
    edges {
      node {
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
```