use std::collections::HashMap;

use iota_types::{
    base_types::{CommitRound, ObjectID},
    executable_transaction::VerifiedExecutableTransaction,
    transaction::TransactionDataAPI,
};

use super::shared_object_congestion_tracker::ExecutionTime;

/// Scheduling result of a shared-object transaction.
pub(crate) enum SharedObjectTransactionResult {
    Schedule,
    Defer,
}

/// Holds shared object congestion data for a single shared object.
#[derive(Debug, Clone)]
pub(crate) struct PerObjectCongestionInfo {
    /// List of gas prices of scheduled transactions operating on a shared
    /// object.
    scheduled_txs_gas_prices: Vec<u64>,

    /// List of estimated execution durations of transactions operating on a
    /// shared object.
    txs_estimated_exec_durations: Vec<ExecutionTime>,
}

impl PerObjectCongestionInfo {
    /// Create/initialize a new `PerObjectCongestionInfo` with empty shared
    /// object congestion info.
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
    /// Shared object congestion data for multiple shared objects appearing
    /// in a single consensus commit round.
    objects_data: HashMap<ObjectID, PerObjectCongestionInfo>,
}

impl PerCommitCongestionInfo {
    /// Create/initialize a new `PerCommitCongestionInfo` with empty shared
    /// object congestion info.
    pub fn new() -> Self {
        Self {
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

impl Default for PerCommitCongestionInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Holds shared object congestion data for multiple consensus commit rounds.
pub(crate) struct MultiCommitCongestionInfo {
    /// Shared object congestion data for consensus commit rounds.
    commits_data: HashMap<CommitRound, PerCommitCongestionInfo>,
}

impl MultiCommitCongestionInfo {
    pub fn new() -> Self {
        Self {
            commits_data: HashMap::new(),
        }
    }

    pub fn insert_new_per_commit_congestion_info(&mut self, commit_round: CommitRound) {
        self.commits_data
            .insert(commit_round, PerCommitCongestionInfo::new());
    }

    pub fn update_per_commit_congestion_info_for_consensus_certificate(
        &mut self,
        commit_round: CommitRound,
        certificate: &VerifiedExecutableTransaction,
        estimated_execution_duration: ExecutionTime,
        scheduling_result: SharedObjectTransactionResult,
    ) {
        let per_commit_congestion_info =
            self.commits_data.get_mut(&commit_round).unwrap_or_else(|| {
                panic!(
                    "per-commit congestion info for round {} must have been inserted earlier",
                    commit_round,
                )
            });
        per_commit_congestion_info.update_for_consensus_certificate(
            certificate,
            estimated_execution_duration,
            scheduling_result,
        );
    }
}

impl Default for MultiCommitCongestionInfo {
    fn default() -> Self {
        Self::new()
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
