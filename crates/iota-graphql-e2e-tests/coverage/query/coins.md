Query: `type`

```graphql
{
  coins {
    edges {
      node {
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
        contents {
          __typename
          data
        }
        display {
          value
          error
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
        coinBalance
      }
    }
  }
}
```

tested by [crates/iota-graphql-e2e-tests/tests/objects/coin.move](../../../iota-graphql-e2e-tests/tests/objects/coin.move):

```graphql
//# run-graphql
fragment C on Coin {
  coinBalance
  contents { type { repr } }
}

{
  iotaCoins: coins {
    edges {
      cursor
      node { ...C }
    }
  }

  fakeCoins: coins(type: "@{P0}::fake::FAKE") {
    edges {
      cursor
      node { ...C }
    }
  }

  address(address: "@{A}") {
    coins {
      edges {
        cursor
        node { ...C }
      }
    }

    allBalances: balances {
      edges {
        cursor
        node {
          coinType { repr }
          coinObjectCount
          totalBalance
        }
      }
    }

    firstBalance: balances(first: 1) {
      edges { cursor }
    }

    lastBalance: balances(last: 1) {
      edges { cursor }
    }
  }
}
```
