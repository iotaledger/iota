Query: `checkpoint`

```
{
  checkpoint {
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
```

tested by [crates/iota-graphql-e2e-tests/tests/call/simple.move](crates/iota-graphql-e2e-tests/tests/call/simple.move):

```
//# run-graphql
{
  checkpoint {
    sequenceNumber
  }
}
```

tested by [crates/iota-graphql-e2e-tests/tests/transactions/at_checkpoint.move](crates/iota-graphql-e2e-tests/tests/transactions/at_checkpoint.move):

```
//# run-graphql
{   # Via a checkpoint query
    c0: checkpoint(id: { sequenceNumber: 0 }) { transactionBlocks { nodes { ...Tx } } }
    c1: checkpoint(id: { sequenceNumber: 1 }) { transactionBlocks { nodes { ...Tx } } }
    c2: checkpoint(id: { sequenceNumber: 2 }) { transactionBlocks { nodes { ...Tx } } }
    c3: checkpoint(id: { sequenceNumber: 3 }) { transactionBlocks { nodes { ...Tx } } }
    c4: checkpoint(id: { sequenceNumber: 4 }) { transactionBlocks { nodes { ...Tx } } }
}
```
