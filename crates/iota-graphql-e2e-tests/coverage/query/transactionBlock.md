Query: `transactionBlock`

```
{
  transactionBlock(digest: "63X49x2QuuYNduExZWoJjfXut3s3WDWZ7Tr7nXJu32ZT") {
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
```

