Query: `transactionBlocks`

```
{
  transactionBlocks {
    edges {
      node {
        digest
        sender {
          address
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
        gasInput {
          gasPrice
          gasBudget
        }
        kind {
          __typename
        }
        signatures
        effects {
          status
          errors
          timestamp
        }
        expiration {
          referenceGasPrice
          endTimestamp
          totalCheckpoints
          totalTransactions
          totalGasFees
          totalStakeRewards
          fundSize
          netInflow
          fundInflow
          fundOutflow
          systemStateVersion
          iotaTotalSupply
          iotaTreasuryCapId
          liveObjectSetDigest
        }
        bcs
      }
    }
  }
}
```

tested by [crates/iota-graphql-e2e-tests/tests/consistency/balances.move](../../../iota-graphql-e2e-tests/tests/consistency/balances.move):

```
//# run-graphql --cursors {"c":2,"t":1,"i":false}
# Emulating viewing transaction blocks at checkpoint 2. Fake coin balance should be 700.
{
  transactionBlocks(first: 1, after: "@{cursor_0}", filter: {signAddress: "@{A}"}) {
    nodes {
      sender {
        fakeCoinBalance: balance(type: "@{P0}::fake::FAKE") {
          totalBalance
        }
        allBalances: balances {
          nodes {
            coinType {
              repr
            }
            coinObjectCount
            totalBalance
          }
        }
      }
    }
  }
}
```

tested by [crates/iota-graphql-e2e-tests/tests/transaction_block_effects/object_changes.move](../../../iota-graphql-e2e-tests/tests/transaction_block_effects/object_changes.move):

```
//# run-graphql
{
  transactionBlocks(first: 1) {
    nodes {
      effects {
        objectChanges {
          pageInfo {
            hasPreviousPage
            hasNextPage
            startCursor
            endCursor
          }
          edges {
            cursor
          }
        }
      }
    }
  }
}
```

tested by [crates/iota-graphql-e2e-tests/tests/consistency/epochs/transaction_blocks.move](../../../iota-graphql-e2e-tests/tests/consistency/epochs/transaction_blocks.move):

```
//# run-graphql --cursors {"t":5,"i":false,"c":6}
# Verify that with a cursor, we are locked into a view as if we were at the checkpoint stored in
# the cursor. Compare against `without_cursor`, which should show the latest state at the actual
# latest checkpoint. There should only be 1 transaction block in the `with_cursor` query, but
# multiple in the second
{
  checkpoint {
    sequenceNumber
  }
  with_cursor: transactionBlocks(after: "@{cursor_0}", filter: {signAddress: "@{A}"}) {
    edges {
      cursor
      node {
        digest
        sender {
          objects {
            edges {
              cursor
            }
          }
        }
      }
    }
  }
  without_cursor: transactionBlocks(filter: {signAddress: "@{A}"}) {
    edges {
      cursor
      node {
        digest
        sender {
          objects {
            edges {
              cursor
            }
          }
        }
      }
    }
  }
}
```
