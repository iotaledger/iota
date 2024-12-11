Query: `dryRunTransactionBlock`

```graphql
{
  dryRunTransactionBlock(
    txBytes: "AAIAIC3Pg7fIBNN95RZluYu1Ll8icKxoO/oGYoEAfLP0szGAAAjoAwAAAAAAAAICAAEBAQABAQIAAAEAAA=="
    txMeta: {}
  ) {
    transaction {
      digest
      sender {
        address
      }
      gasInput {
        gasSponsor {
          address
        }
        gasPayment {
          edges {
            node {
              digest
              storageRebate
              bcs
            }
          }
        }
        gasPrice
        gasBudget
      }
    }
    error
    results {
      mutatedReferences {
        input {
          __typename
          ... on Input {
            ix
          }
          ... on Result {
            cmd
            ix
          }
        }
        type {
          repr
        }
        bcs
      }
      returnValues {
        type {
          repr
          signature
          layout
          abilities
        }
        bcs
      }
    }
  }
}
```

tested by [crates/iota-graphql-rpc/tests/e2e_tests.rs](../../../iota-graphql-rpc/tests/e2e_tests.rs):

```graphql
{
    dryRunTransactionBlock(txBytes: $tx, txMeta: {}) {
        results {
            mutatedReferences {
                input {
                    __typename
                }
            }
        }
        transaction {
            digest
            sender {
                address
            }
            gasInput {
                gasSponsor {
                    address
                }
                gasPrice
            }
        }
        error
    }
}
```
