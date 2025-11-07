// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# init --protocol-version 13 --simulator --accounts C

//# create-checkpoint

//# advance-epoch

//# programmable --sender C --inputs 10000000000 @C
//> SplitCoins(Gas, [Input(0)]);
//> TransferObjects([Result(0)], Input(1));

//# programmable --sender C --inputs 5000000000 @C
//> SplitCoins(Gas, [Input(0)]);
//> TransferObjects([Result(0)], Input(1));

//# run 0x3::iota_system::request_add_stake --args object(0x5) object(3,0) @validator_0 --sender C

//# create-checkpoint

//# run-graphql
{
  epoch(id: 1) {
    epochId
    referenceGasPrice
    validatorSet {
      totalStake
      activeValidators {
        nodes {
          name
        }
      }
    }
    startTimestamp
    totalCheckpoints
    totalTransactions
    totalGasFees
    totalStakeRewards
    fundSize
    netInflow
    fundInflow
    fundOutflow
    storageFund {
      totalObjectStorageRebates
      nonRefundableBalance
    }
    safeMode {
      enabled
    }
    systemStateVersion
    checkpoints(last: 1) {
      nodes {
        sequenceNumber
      }
    }
    transactionBlocks(last: 1) {
      nodes {
        digest
      }
    }
    endTimestamp
  }
}

//# create-checkpoint

//# advance-epoch

//# create-checkpoint

//# run-graphql
{
  epoch(id: 1) {
    epochId
    referenceGasPrice
    validatorSet {
      totalStake
      activeValidators {
        nodes {
          name
        }
      }
    }
    startTimestamp
    totalCheckpoints
    totalTransactions
    totalGasFees
    totalStakeRewards
    fundSize
    netInflow
    fundInflow
    fundOutflow
    storageFund {
      totalObjectStorageRebates
      nonRefundableBalance
    }
    safeMode {
      enabled
    }
    systemStateVersion
    checkpoints(last: 1) {
      nodes {
        sequenceNumber
      }
    }
    transactionBlocks(last: 1) {
      nodes {
        digest
      }
    }
    endTimestamp
  }
}

//# run-graphql
{
  epoch(id: 2) {
    epochId
    referenceGasPrice
    validatorSet {
      totalStake
      activeValidators {
        nodes {
          name
        }
      }
    }
    startTimestamp
    totalCheckpoints
    totalTransactions
    totalGasFees
    totalStakeRewards
    fundSize
    netInflow
    fundInflow
    fundOutflow
    storageFund {
      totalObjectStorageRebates
      nonRefundableBalance
    }
    safeMode {
      enabled
    }
    systemStateVersion
    checkpoints(last: 1) {
      nodes {
        sequenceNumber
      }
    }
    transactionBlocks(last: 1) {
      nodes {
        digest
      }
    }
    endTimestamp
  }
}
