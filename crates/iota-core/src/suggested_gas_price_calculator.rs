// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::VecDeque;

use im::HashMap;
use iota_types::{
    base_types::{CommitRound, ObjectID},
    messages_checkpoint::CheckpointSequenceNumber,
};

/// Number of consensus commits for which the data for suggested gas
/// price calculations should be taken into account. The suggested
/// gas price calculator utilizes a ring buffer under the hood, so
/// if this capacity is reached, data for the oldest commit will
/// be dropped.
// NOTE:
// - maybe make this a protocol config parameter?
// - 1_500 corresponds to around 1 minute of data, considering that there are
//   ~25 commit rounds per second. If we want to evict data from the buffer
//   based on time, we might want to use moka::sync::Cache instead of VecDeque
//   as a ring buffer.
const SUGGESTED_GAS_PRICE_CALCULATOR_MAX_NUM_COMMITS: u32 = 1_500;

/// Holds data used used for the suggested gas price calculations for
/// a single consensus commit round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerCommitSuggestedGasPriceData {
    /// Sequence number of the checkpoint that includes the commit.
    /// This is needed to uniquely distinguish commit rounds as
    /// consensus commit round number is reset on epoch boundary.
    checkpoint: CheckpointSequenceNumber,

    /// Consensus commit round number
    commit_round: CommitRound,

    /// For each shared object that appears in a scheduled (i.e., non-deferred)
    /// transaction, this stores the gas price of that transaction.
    shared_object_data: HashMap<
        ObjectID,
        // gas prices of scheduled transactions operating on a shared object
        Vec<u64>,
    >,
}

impl PerCommitSuggestedGasPriceData {
    /// Create/initialize a new `PerCommitSuggestedGasPriceData` for given
    /// checkpoint sequence number `checkpoint` and consensus commit round
    /// number `commit_round`.
    pub fn new(checkpoint: CheckpointSequenceNumber, commit_round: CommitRound) -> Self {
        Self {
            checkpoint,
            commit_round,
            shared_object_data: HashMap::new(),
        }
    }
}

/// Suggested gas price calculator is a component that gathers shared-object
/// congestion data to calculate suggested gas prices for shared-object
/// transactions.
pub struct SuggestedGasPriceCalculator {
    /// Ring buffer holding the data from multiple consensus commit rounds.
    /// The data is used to calculate suggested gas prices for shared-object
    /// transactions.
    multi_commit_data: VecDeque<PerCommitSuggestedGasPriceData>,
}

impl SuggestedGasPriceCalculator {
    /// Create a new `SuggestedGasPriceCalculator` with the default pre-defined
    /// capacity of its inner ring buffer for storing the data from multiple
    /// commits.
    pub fn new() -> Self {
        Self::new_with_capacity(SUGGESTED_GAS_PRICE_CALCULATOR_MAX_NUM_COMMITS as usize)
    }

    /// Create a new `SuggestedGasPriceCalculator` with a given `capacity` for
    /// its inner ring buffer for storing the data from multiple commits.
    pub fn new_with_capacity(capacity: usize) -> Self {
        Self {
            multi_commit_data: VecDeque::with_capacity(capacity),
        }
    }

    /// Add the data from a single commit to this suggested gas price
    /// calculator. If the calculator's ring buffer capacity has been
    /// reached, the data for the oldest commit will be dropped, and
    /// the data for this commit round will be added to the front of
    /// the ring buffer.
    pub fn add_commit_data(&mut self, commit_data: PerCommitSuggestedGasPriceData) {
        if self.multi_commit_data.len() == self.multi_commit_data.capacity() {
            self.multi_commit_data.pop_front();
        }

        self.multi_commit_data.push_back(commit_data);
    }
}

impl Default for SuggestedGasPriceCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod suggested_gas_price_calculator_tests {
    use super::{PerCommitSuggestedGasPriceData, SuggestedGasPriceCalculator};

    #[test]
    fn test_suggested_gas_price_calculator_add_commit_data() {
        let capacity = 2;
        let mut sgp_calc = SuggestedGasPriceCalculator::new_with_capacity(capacity);

        let commit_0_data = PerCommitSuggestedGasPriceData::new(0, 0);

        sgp_calc.add_commit_data(commit_0_data.clone());
        assert_eq!(sgp_calc.multi_commit_data, [commit_0_data.clone()]);

        let commit_1_data = PerCommitSuggestedGasPriceData::new(0, 1);
        sgp_calc.add_commit_data(commit_1_data.clone());
        assert_eq!(
            sgp_calc.multi_commit_data,
            [commit_0_data, commit_1_data.clone()]
        );

        let commit_2_data = PerCommitSuggestedGasPriceData::new(0, 2);
        sgp_calc.add_commit_data(commit_2_data.clone());
        assert_eq!(sgp_calc.multi_commit_data, [commit_1_data, commit_2_data]);
    }
}
