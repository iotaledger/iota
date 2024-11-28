Query: `checkpoints`

```
{
  checkpoints {
    edges {
      node {
        digest
        sequenceNumber
        timestamp
        validatorSignatures
        previousCheckpointDigest
        networkTotalTransactions
        rollingGasSummary {
          computationCost
          computationCostBurned
          storageCost
          storageRebate
          nonRefundableStorageFee
        }
        epoch {
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
  }
}
```

tested by [crates/iota-graphql-e2e-tests/tests/limits/output_node_estimation.move](../../../iota-graphql-e2e-tests/tests/limits/output_node_estimation.move):

```
//# run-graphql --show-usage
# build on previous example with nested connection
{
  checkpoints {                                             # 1
    nodes {                                                 # 1
      transactionBlocks {                                   # 20
        edges {                                             # 20
          txns: node {                                      # 400
            digest                                          # 400
          }
        }
      }
    }
  }
}
```

tested by [crates/iota-graphql-e2e-tests/tests/consistency/checkpoints/transaction_blocks.move](../../../iota-graphql-e2e-tests/tests/consistency/checkpoints/transaction_blocks.move):

```
{
  checkpoints {
    nodes {
      sequenceNumber
      transactionBlocks(filter: { signAddress: "@{A}"}) {
        edges {
          cursor
          node {
            digest
            sender {
                objects(last: 1) {
                    edges {
                        cursor
                    }
                }
            }
          }
        }
      }
    }
  }
}
```
