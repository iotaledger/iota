Query: `executeTransactionBlock`

```graphql
mutation {
  executeTransactionBlock(txBytes: $tx, signatures: $sigs) {
    effects {
      transactionBlock {
        digest
      }
      status
      lamportVersion
      errors
      dependencies {
        edges {
          node {
            digest
            bcs
          }
        }
      }
      gasEffects {
        __typename
        gasObject {
          digest
          storageRebate
          bcs
        }
        gasSummary {
          computationCost
          computationCostBurned
          storageCost
          storageRebate
          nonRefundableStorageFee
        }
      }
      unchangedSharedObjects {
        edges {
          node {
            __typename
          }
        }
      }
      objectChanges {
        edges {
          node {
            idCreated
            idDeleted
          }
        }
      }
      balanceChanges {
        edges {
          node {
            amount
          }
        }
      }
      events {
        edges {
          node {
            timestamp
          }
        }
      }
      timestamp
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
      checkpoint {
        previousCheckpointDigest
        networkTotalTransactions
      }
      bcs
    }
    errors
  }
}
```

tested by [crates/iota-graphql-rpc/tests/e2e_tests.rs](../../../iota-graphql-rpc/tests/e2e_tests.rs):

```graphql
{
  executeTransactionBlock(txBytes: $tx, signatures: $sigs) {
    effects {
      transactionBlock {
        digest
      }
    }
    errors
  }
}
```

tested by [crates/iota-graphql-rpc/tests/e2e_tests.rs](../../../iota-graphql-rpc/tests/e2e_tests.rs):

```graphql
mutation {
  executeTransactionBlock(txBytes: "{}", signatures: "{}") {
    effects {
      status
    }
  }
}
```
