Query: `epoch`

```graphql
{
  epoch(id: 1) {
    epochId
    referenceGasPrice
    validatorSet {
      totalStake
      pendingActiveValidatorsId
      pendingActiveValidatorsSize
      stakingPoolMappingsId
      stakingPoolMappingsSize
      inactivePoolsId
      inactivePoolsSize
      validatorCandidatesId
      validatorCandidatesSize
    }
    startTimestamp
    endTimestamp
    totalCheckpoints
    totalTransactions
    totalGasFees
    totalStakeRewards
    fundSize
    netInflow
    fundInflow
    fundOutflow
    protocolConfigs {
      __typename
    }
    systemStateVersion
    iotaTotalSupply
    iotaTreasuryCapId
    liveObjectSetDigest
    checkpoints {
      edges {
        node {
          previousCheckpointDigest
          networkTotalTransactions
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

tested by [crates/iota-graphql-e2e-tests/tests/validator/validator.move](../../../iota-graphql-e2e-tests/tests/validator/validator.move):

```graphql
{
  epoch(id: 1) {
    validatorSet {
      activeValidators {
        nodes {
          name
        }
      }
    }
  }
}
```

tested by [crates/iota-graphql-e2e-tests/tests/consistency/epochs/checkpoints.move](../../../iota-graphql-e2e-tests/tests/consistency/epochs/checkpoints.move):

```graphql
//# run-graphql --cursors {"s":3,"c":4} {"s":7,"c":8} {"s":9,"c":10}
# View checkpoints before the last checkpoint in each epoch, from the perspective of the first
# checkpoint in the next epoch.
{
  checkpoint {
    sequenceNumber
  }
  epoch_0: epoch(id: 0) {
    epochId
    checkpoints(before: "@{cursor_0}") {
      nodes {
        sequenceNumber
      }
    }
  }
  epoch_1: epoch(id: 1) {
    epochId
    checkpoints(before: "@{cursor_1}") {
      nodes {
        sequenceNumber
      }
    }
  }
  epoch_2: epoch(id: 2) {
    epochId
    checkpoints(before: "@{cursor_2}") {
      nodes {
        sequenceNumber
      }
    }
  }
}
```


