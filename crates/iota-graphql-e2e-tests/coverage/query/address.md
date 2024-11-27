Query: `address`

```
{
  address(address: "0x1") {
    address
		objects {
		  edges {
		    node {
		      digest
		      storageRebate
		      bcs
		    }
		  }
		}
    balance {
      coinObjectCount
      totalBalance
    }
    balances {
      edges {
        node {
          coinObjectCount
          totalBalance
        }
      }
    }
    coins {
      edges {
        node {
          digest
          storageRebate
          bcs
          coinBalance
        }
      }
    }
    stakedIotas {
      edges {
        node {
          digest
          storageRebate
          bcs
          poolId
          principal
          estimatedReward
        }
      }
    }
    transactionBlocks {
      edges {
        node {
          digest
          bcs
        }
      }
    }
  }
}
```

tested by [crates/iota-graphql-e2e-tests/tests/call/owned_objects.move](crates/iota-graphql-e2e-tests/tests/call/owned_objects.move):

```
//# run-graphql
{
  address(address: "0x42") {
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

tested by [crates/iota-graphql-e2e-tests/tests/transaction_block_effects/balance_changes.move](crates/iota-graphql-e2e-tests/tests/transaction_block_effects/balance_changes.move):

```
//# run-graphql
{
  address(address: "@{C}") {
    transactionBlocks(last: 1) {
      nodes {
        effects {
          balanceChanges {
            pageInfo {
              hasPreviousPage
              hasNextPage
              startCursor
              endCursor
            }
            edges {
              node {
                amount
              }
              cursor
            }
          }
        }
      }
    }
  }
}
```

tested by [crates/iota-graphql-e2e-tests/tests/consistency/coins.move](crates/iota-graphql-e2e-tests/tests/consistency/coins.move):

```
//# run-graphql
{
  queryCoins: coins(type: "@{P0}::fake::FAKE") {
    edges {
      cursor
      node {
        owner {
          ... on AddressOwner {
            owner {
              address
              coins(type: "@{P0}::fake::FAKE") {
                edges {
                  cursor
                  node {
                    contents {
                      json
                    }
                  }
                }
              }
            }
          }
        }
        contents {
          json
        }
      }
    }
  }
  addressCoinsA: address(address: "@{A}") {
    coins(type: "@{P0}::fake::FAKE") {
      edges {
        cursor
        node {
          contents {
            json
          }
        }
      }
    }
  }
  addressCoinsB: address(address: "@{B}") {
    coins(type: "@{P0}::fake::FAKE") {
      edges {
        cursor
        node {
          contents {
            json
          }
        }
      }
    }
  }
}
```

tested by [crates/iota-graphql-e2e-tests/tests/consistency/staked_iota.move](crates/iota-graphql-e2e-tests/tests/consistency/staked_iota.move):

```
//# run-graphql
{
  address(address: "@{C}") {
    stakedIotas {
      edges {
        cursor
        node {
          principal
        }
      }
    }
  }
}
```