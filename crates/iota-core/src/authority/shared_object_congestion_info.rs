use std::collections::{HashMap, VecDeque};

use iota_types::{
    base_types::{CommitRound, ObjectID},
    executable_transaction::VerifiedExecutableTransaction,
    transaction::TransactionDataAPI,
};

use super::shared_object_congestion_tracker::ExecutionTime;

/// The maximum number of consensus commit rounds for which shared
/// object congestion data should be stored. If this limit is reached,
/// the data for the oldest stored commit will be dropped and the data
/// for the new consensus commit round will be added.
// TODO: This should be a protocol config parameter, as the calculations of
// suggested gas price will be different if this limit is changed.
// NOTE: 1_500 corresponds to around 1 minute of data, considering that there
// are ~25 commit rounds per second. If we want to evict data from the buffer
// based on time, we might want to use `moka::sync::Cache` instead of `VecDeque`
// as a ring buffer.
const MULTI_COMMIT_CONGESTION_INFO_MAX_NUM_COMMITS: u32 = 1_500;

/// Scheduling result of a shared-object transaction.
pub(crate) enum SharedObjectTransactionResult {
    Schedule,
    Defer,
}

/// Holds shared object congestion data for a single shared object.
#[derive(Debug, Clone)]
pub(crate) struct PerObjectCongestionInfo {
    /// List of gas prices of SCHEDULED transactions operating on a shared
    /// object.
    scheduled_txs_gas_prices: Vec<u64>,

    /// List of estimated execution durations of transactions operating on a
    /// shared object.
    txs_estimated_exec_durations: Vec<ExecutionTime>,
}

impl PerObjectCongestionInfo {
    /// Create/initialize a new `PerObjectCongestionInfo` with empty shared
    /// object congestion data.
    pub fn new() -> Self {
        Self {
            scheduled_txs_gas_prices: Vec::new(),
            txs_estimated_exec_durations: Vec::new(),
        }
    }
}

impl Default for PerObjectCongestionInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Holds shared object congestion data for a single consensus commit round.
pub(crate) struct PerCommitCongestionInfo {
    /// Consensus commit round number
    commit_round: CommitRound,

    /// Shared object congestion data for multiple shared objects appearing
    /// in a single consensus commit round.
    objects_data: HashMap<ObjectID, PerObjectCongestionInfo>,
}

impl PerCommitCongestionInfo {
    /// Create/initialize a new `PerCommitCongestionInfo` with empty shared
    /// object congestion data for a given commit round.
    pub fn new(commit_round: CommitRound) -> Self {
        Self {
            commit_round,
            objects_data: HashMap::new(),
        }
    }

    /// Update shared object congestion info for a single consensus certificate.
    pub fn update_for_consensus_certificate(
        &mut self,
        certificate: &VerifiedExecutableTransaction,
        estimated_execution_duration: ExecutionTime,
        scheduling_result: SharedObjectTransactionResult,
    ) {
        // Only process certificates with shared objects
        if certificate.contains_shared_object() {
            match scheduling_result {
                SharedObjectTransactionResult::Schedule => {
                    let gas_price = certificate.transaction_data().gas_price();

                    for object in certificate.shared_input_objects() {
                        self.objects_data
                            .entry(object.id)
                            .and_modify(|object_data| {
                                object_data.scheduled_txs_gas_prices.push(gas_price);
                                object_data
                                    .txs_estimated_exec_durations
                                    .push(estimated_execution_duration);
                            })
                            .or_insert(PerObjectCongestionInfo {
                                scheduled_txs_gas_prices: vec![gas_price],
                                txs_estimated_exec_durations: vec![estimated_execution_duration],
                            });
                    }
                }
                SharedObjectTransactionResult::Defer => {
                    for object in certificate.shared_input_objects() {
                        self.objects_data
                            .entry(object.id)
                            .and_modify(|object_data| {
                                object_data
                                    .txs_estimated_exec_durations
                                    .push(estimated_execution_duration)
                            })
                            .or_insert(PerObjectCongestionInfo {
                                scheduled_txs_gas_prices: vec![],
                                txs_estimated_exec_durations: vec![estimated_execution_duration],
                            });
                    }
                }
            }
        }
    }
}

/// Holds shared object congestion data for multiple consensus commit rounds.
/// Under the hood, this utilizes a ring buffer to store the data for multiple
/// consensus commit rounds, so that if the buffer capacity is reached, the
/// data for the oldest commit will be dropped and the data for the new
/// consensus commit round will be added.
pub(crate) struct MultiCommitCongestionInfo {
    /// Shared object congestion data from multiple consensus commit rounds.
    commits_data: VecDeque<PerCommitCongestionInfo>,
}

impl MultiCommitCongestionInfo {
    /// Create/initialize a new multi-commit congestion info with empty data.
    /// `max_num_commits_to_store_data` represents the maximum number of
    /// consensus commit rounds for which the data should be stored. If
    /// this limit is reached, the data for the oldest consensus commit
    /// round will be dropped, and the data for the new consensus commit
    /// round will be added.
    pub fn new(max_num_commits_to_store_data: usize) -> Self {
        Self {
            commits_data: VecDeque::with_capacity(max_num_commits_to_store_data),
        }
    }

    /// Add new empty per-commit congestion data for `commit_round` to this
    /// multi-commit congestion info. If the underlying buffer capacity has
    /// already been reached, the data for the oldest commit will be dropped,
    /// and the data for this commit round will be added to the front of
    /// the ring buffer.
    pub fn add_new_per_commit_congestion_info(&mut self, commit_round: CommitRound) {
        if self.commits_data.len() == self.commits_data.capacity() {
            self.commits_data.pop_front();
        }

        self.commits_data
            .push_back(PerCommitCongestionInfo::new(commit_round));
    }

    /// Update per-commit congestion info for a single consensus certificate
    /// in the current consensus commit round.
    pub fn update_per_commit_congestion_info_for_consensus_certificate(
        &mut self,
        certificate: &VerifiedExecutableTransaction,
        estimated_execution_duration: ExecutionTime,
        scheduling_result: SharedObjectTransactionResult,
    ) {
        self.commits_data
            .iter_mut()
            .last()
            .expect(
                "per-commit congestion info for the current consensus commit round must have \
                    been added earlier",
            )
            .update_for_consensus_certificate(
                certificate,
                estimated_execution_duration,
                scheduling_result,
            );
    }
}

impl Default for MultiCommitCongestionInfo {
    fn default() -> Self {
        Self::new(MULTI_COMMIT_CONGESTION_INFO_MAX_NUM_COMMITS as usize)
    }
}

#[cfg(test)]
mod tests {
    use parking_lot::Mutex;

    struct Data {
        x: Mutex<u32>,
    }

    impl Data {
        fn write(&self) {
            let mut x = self.x.lock();
            *x = 5;
        }
    }

    #[test]
    fn test() {
        let data = Data { x: Mutex::new(0) };

        data.write();

        let x = data.x.lock();
        assert_eq!(*x, 1);
    }
}
