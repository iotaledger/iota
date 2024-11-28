Query: `owner`

```graphql
{
  owner(address: "0x1", rootVersion: null) {
    address
    objects {
      edges {
        node {
          address
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
    coins {
      edges {
        node {
          balance {
            coinObjectCount
            totalBalance
          }
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
    asAddress {
      address
    }
    asObject {
      digest
    }
    dynamicField(
      name: {type: "0x0000000000000000000000000000000000000000000000000000000000000001::string::String", bcs: "A2RmNQ=="}
    ) {
      __typename
    }
    dynamicObjectField(
      name: {type: "0x0000000000000000000000000000000000000000000000000000000000000001::string::String", bcs: "A2RmNQ=="}
    ) {
      __typename
    }
    dynamicFields {
      edges {
        node {
          __typename
        }
      }
    }
  }
}
```

tested by [crates/iota-graphql-e2e-tests/tests/call/owned_objects.move](../../../iota-graphql-e2e-tests/tests/call/owned_objects.move):

```graphql
//# run-graphql
{
  owner(address: "0x42") {
    objects {
      edges {
        node {
          owner {
              __typename
              ... on AddressOwner {
              owner {
                  address
              }
            }
          }
        }
      }
    }
  }
}
```

tested by [crates/iota-graphql-e2e-tests/tests/call/dynamic_fields.move](../../../iota-graphql-e2e-tests/tests/call/dynamic_fields.move):

```graphql
//# run-graphql
{
  owner(address: "@{obj_2_0}") {
    dynamicFields {
      nodes {
        name {
          type {
            repr
          }
          data
          bcs
        }
        value {
          ... on MoveObject {
            __typename
          }
          ... on MoveValue {
            bcs
            data
            __typename
          }
        }
      }
    }
  }
}

//# run-graphql
{
  owner(address: "@{obj_2_0}") {
    dynamicField(name: {type: "u64", bcs: "AAAAAAAAAAA="}) {
      name {
        type {
          repr
        }
        data
        bcs
      }
      value {
        ... on MoveValue {
          __typename
          bcs
          data
        }
      }
    }
  }
}

//# run-graphql
{
  owner(address: "@{obj_2_0}") {
    dynamicObjectField(name: {type: "u64", bcs: "AAAAAAAAAAA="}) {
      value {
        ... on MoveObject {
          __typename
        }
      }
    }
  }
}
```
