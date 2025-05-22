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
// TODO:
// - maybe make this a protocol config parameter?
// - 1_500 corresponds to around 1 minute of data, considering that there are
//   ~25 commit rounds per second. If we want to evict data from the buffer
//   based on time, we might want to use moka::sync::Cache instead of VecDeque
//   as a ring buffer.
const SUGGESTED_GAS_PRICE_CALCULATOR_MAX_NUM_COMMITS: u32 = 1_500;

/// Holds data used used for the suggested gas price calculations for
/// a single consensus commit round.
pub struct PerCommitDataForSuggestedGasPriceCalc {
    // NOTE: Since consensus commit round is reset on epoch boundary,
    // we also collect checkpoint sequence number to uniquely
    // distinguish commit rounds.
    checkpoint: CheckpointSequenceNumber,

    commit_round: CommitRound,

    // For each shared object that appears in a scheduled (i.e., non-cancelled)
    // transaction, store gas price of that transaction.
    shared_object_data: HashMap<
        ObjectID,
        // gas prices of scheduled transactions operating on a shared object
        Vec<u64>,
    >,
}

/// Suggested gas price calculator is a component that gathers shared-object
/// congestion data to calculate suggested gas prices for shared-object
/// transactions.
pub struct SuggestedGasPriceCalculator {
    /// Ring buffer holding data used for the calculations for multiple
    /// consensus commit rounds.
    multi_commit_data: VecDeque<PerCommitDataForSuggestedGasPriceCalc>,
}

impl SuggestedGasPriceCalculator {
    pub fn new() -> Self {
        Self {
            multi_commit_data: VecDeque::with_capacity(
                SUGGESTED_GAS_PRICE_CALCULATOR_MAX_NUM_COMMITS as usize,
            ),
        }
    }

    pub fn add_commit_data(&mut self, commit_data: PerCommitDataForSuggestedGasPriceCalc) {
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
