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
// Or this limit should be set to how many times tx can be deferred.
// NOTE: 1_500 corresponds to around 1 minute of data, considering that there
// are ~25 commit rounds per second. If we want to evict data from the buffer
// based on time, we might want to use `moka::sync::Cache` instead of `VecDeque`
// as a ring buffer.
const MULTI_COMMIT_CONGESTION_INFO_MAX_NUM_COMMITS: u32 = 1_500;

/// Holds shared object congestion data for a single scheduled transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScheduledTransactionCongestionInfo {
    /// Gas price of a scheduled shared-object transaction.
    pub(crate) gas_price: u64,

    /// Estimated execution duration of a scheduled shared-object transaction.
    pub(crate) estimated_execution_duration: ExecutionTime,
}

impl ScheduledTransactionCongestionInfo {
    /// Create a new scheduled transaction data.
    pub fn new(gas_price: u64, estimated_execution_duration: ExecutionTime) -> Self {
        Self {
            gas_price,
            estimated_execution_duration,
        }
    }
}

/// Holds shared object congestion data for a single shared object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PerObjectCongestionInfo {
    /// List of congestion data for scheduled transactions operating on this
    /// shared object.
    pub(crate) scheduled_transactions_data: Vec<ScheduledTransactionCongestionInfo>,
}

impl PerObjectCongestionInfo {
    /// Create/initialize a new `PerObjectCongestionInfo` with empty shared
    /// object congestion data.
    pub fn new() -> Self {
        Self {
            scheduled_transactions_data: Vec::new(),
        }
    }
}

impl Default for PerObjectCongestionInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Holds shared object congestion data for a single consensus commit round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PerCommitCongestionInfo {
    /// Consensus commit round number
    pub(crate) commit_round: CommitRound,

    /// Shared object congestion data for multiple shared objects appearing
    /// in a single consensus commit round.
    pub(crate) objects_data: HashMap<ObjectID, PerObjectCongestionInfo>,
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

    /// Update shared object congestion info for a single scheduled consensus
    /// certificate.
    pub fn update_for_scheduled_consensus_certificate(
        &mut self,
        certificate: &VerifiedExecutableTransaction,
        estimated_execution_duration: ExecutionTime,
    ) {
        // Only process certificates with shared objects
        if certificate.contains_shared_object() {
            let scheduled_transaction_congestion_info = ScheduledTransactionCongestionInfo::new(
                certificate.transaction_data().gas_price(),
                estimated_execution_duration,
            );

            for object in certificate.shared_input_objects() {
                self.objects_data
                    .entry(object.id)
                    .and_modify(|object_data| {
                        object_data
                            .scheduled_transactions_data
                            .push(scheduled_transaction_congestion_info);
                    })
                    .or_insert(PerObjectCongestionInfo {
                        scheduled_transactions_data: vec![scheduled_transaction_congestion_info],
                    });
            }
        }
    }
}

/// Holds shared object congestion data for multiple consensus commit rounds.
/// Under the hood, this utilizes a ring buffer to store the data for multiple
/// consensus commit rounds, so that if the buffer capacity is reached, the
/// data for the oldest commit will be dropped and the data for the new
/// consensus commit round will be added.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct MultiCommitCongestionInfo {
    /// Shared object congestion data from multiple consensus commit rounds.
    pub(crate) commits_data: VecDeque<PerCommitCongestionInfo>,
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
    pub fn update_per_commit_congestion_info_for_scheduled_consensus_certificate(
        &mut self,
        certificate: &VerifiedExecutableTransaction,
        estimated_execution_duration: ExecutionTime,
    ) {
        self.commits_data
            .iter_mut()
            .last()
            .expect(
                "per-commit congestion info for the current consensus commit round must have \
                    been added earlier",
            )
            .update_for_scheduled_consensus_certificate(certificate, estimated_execution_duration);
    }
}

impl Default for MultiCommitCongestionInfo {
    fn default() -> Self {
        Self::new(MULTI_COMMIT_CONGESTION_INFO_MAX_NUM_COMMITS as usize)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, VecDeque};

    use iota_types::{
        base_types::{CommitRound, ObjectID},
        executable_transaction::VerifiedExecutableTransaction,
    };
    use parking_lot::Mutex;

    use super::{MultiCommitCongestionInfo, ScheduledTransactionCongestionInfo};
    use crate::authority::{
        shared_object_congestion_info::{PerCommitCongestionInfo, PerObjectCongestionInfo},
        shared_object_congestion_tracker::{
            ExecutionTime, shared_object_test_utils::build_transaction,
        },
    };

    const MAX_NUM_COMMITS_TO_STORE_DATA: u32 = 2;

    // Dummy `AuthorityPerEpochStore` used to test `MultiCommitCongestionInfo`
    pub struct DummyAuthorityPerEpochStore {
        congestion_info: Mutex<MultiCommitCongestionInfo>,
    }

    impl DummyAuthorityPerEpochStore {
        fn new() -> Self {
            Self {
                congestion_info: Mutex::new(MultiCommitCongestionInfo::new(
                    MAX_NUM_COMMITS_TO_STORE_DATA as usize,
                )),
            }
        }

        fn add_new_per_commit_congestion_info(&self, commit_round: CommitRound) {
            let mut congestion_info = self.congestion_info.lock();
            (*congestion_info).add_new_per_commit_congestion_info(commit_round);
        }

        /// Update per-commit congestion info for a single consensus certificate
        /// in the current consensus commit round.
        fn update_per_commit_congestion_info_for_scheduled_consensus_certificate(
            &self,
            certificate: &VerifiedExecutableTransaction,
            estimated_execution_duration: ExecutionTime,
        ) {
            let mut congestion_info = self.congestion_info.lock();
            (*congestion_info)
                .update_per_commit_congestion_info_for_scheduled_consensus_certificate(
                    certificate,
                    estimated_execution_duration,
                );
        }
    }

    #[test]
    fn test_multi_commit_congestion_info_updates() {
        let dummy_epoch_store = DummyAuthorityPerEpochStore::new();

        // Add the first consensus commit round
        let commit_round_1 = 1;
        dummy_epoch_store.add_new_per_commit_congestion_info(commit_round_1);

        // Now there should be empty data for a single commit round, i.e., round 1
        {
            let multi_commit_congestion_info = dummy_epoch_store.congestion_info.lock();

            assert_eq!(
                *multi_commit_congestion_info,
                MultiCommitCongestionInfo {
                    commits_data: VecDeque::from([PerCommitCongestionInfo::new(commit_round_1)]),
                }
            );
        }

        let object_1 = ObjectID::random();
        let object_2 = ObjectID::random();

        // Create a certificate
        let objects_1 = vec![(object_1, true), (object_2, false)];
        let gas_price_1 = 10;
        let gas_budget_1 = 100;
        let certificate_1 = build_transaction(&objects_1, gas_budget_1, gas_price_1);
        // and process this certificate to update the multi-commit congestion info
        dummy_epoch_store.update_per_commit_congestion_info_for_scheduled_consensus_certificate(
            &certificate_1,
            gas_budget_1,
        );

        let scheduled_transaction_congestion_info_1 =
            ScheduledTransactionCongestionInfo::new(gas_price_1, gas_budget_1);

        // Expected congestion info for commit 1 looks as follows:
        let per_commit_congestion_info_1 = PerCommitCongestionInfo {
            commit_round: commit_round_1,
            objects_data: HashMap::from([
                (
                    object_1,
                    PerObjectCongestionInfo {
                        scheduled_transactions_data: vec![scheduled_transaction_congestion_info_1],
                    },
                ),
                (
                    object_2,
                    PerObjectCongestionInfo {
                        scheduled_transactions_data: vec![scheduled_transaction_congestion_info_1],
                    },
                ),
            ]),
        };
        // Verify that multi-commit congestion info contains only
        // `per_commit_congestion_info_1`
        {
            let multi_commit_congestion_info = dummy_epoch_store.congestion_info.lock();

            assert_eq!(
                *multi_commit_congestion_info,
                MultiCommitCongestionInfo {
                    commits_data: VecDeque::from([per_commit_congestion_info_1.clone()]),
                }
            );
        }

        // Add the second consensus commit round
        let commit_round_2 = 2;
        dummy_epoch_store.add_new_per_commit_congestion_info(commit_round_2);

        // Now there should be non-empty data for the first commit round,
        // and empty data for the second one
        {
            let multi_commit_congestion_info = dummy_epoch_store.congestion_info.lock();

            assert_eq!(
                *multi_commit_congestion_info,
                MultiCommitCongestionInfo {
                    commits_data: VecDeque::from([
                        per_commit_congestion_info_1.clone(),
                        PerCommitCongestionInfo::new(commit_round_2)
                    ]),
                }
            );
        }

        // Create a certificate
        let objects_1 = vec![(object_1, true), (object_2, false)];
        let gas_price_1 = 10;
        let gas_budget_1 = 100;
        let certificate_1 = build_transaction(&objects_1, gas_budget_1, gas_price_1);
        // and process this certificate to update the multi-commit congestion info
        dummy_epoch_store.update_per_commit_congestion_info_for_scheduled_consensus_certificate(
            &certificate_1,
            gas_budget_1,
        );

        // Create another certificate
        let objects_2 = vec![(object_2, false)];
        let gas_price_2 = 1;
        let gas_budget_2 = 10;
        let certificate_2 = build_transaction(&objects_2, gas_budget_2, gas_price_2);
        // and process this certificate to update the multi-commit congestion info
        dummy_epoch_store.update_per_commit_congestion_info_for_scheduled_consensus_certificate(
            &certificate_2,
            gas_budget_2,
        );

        let scheduled_transaction_congestion_info_1 =
            ScheduledTransactionCongestionInfo::new(gas_price_1, gas_budget_1);
        let scheduled_transaction_congestion_info_2 =
            ScheduledTransactionCongestionInfo::new(gas_price_2, gas_budget_2);

        // Expected congestion info for commit 1 looks as follows:
        let per_commit_congestion_info_2 = PerCommitCongestionInfo {
            commit_round: commit_round_2,
            objects_data: HashMap::from([
                (
                    object_1,
                    PerObjectCongestionInfo {
                        scheduled_transactions_data: vec![scheduled_transaction_congestion_info_1],
                    },
                ),
                (
                    object_2,
                    PerObjectCongestionInfo {
                        scheduled_transactions_data: vec![
                            scheduled_transaction_congestion_info_1,
                            scheduled_transaction_congestion_info_2,
                        ],
                    },
                ),
            ]),
        };
        // Verify that multi-commit congestion info contains both
        // `per_commit_congestion_info_1` and `per_commit_congestion_info_2`
        {
            let multi_commit_congestion_info = dummy_epoch_store.congestion_info.lock();

            assert_eq!(
                *multi_commit_congestion_info,
                MultiCommitCongestionInfo {
                    commits_data: VecDeque::from([
                        per_commit_congestion_info_1,
                        per_commit_congestion_info_2.clone()
                    ]),
                }
            );
        }

        // Add the third consensus commit round
        let commit_round_3 = 3;
        dummy_epoch_store.add_new_per_commit_congestion_info(commit_round_3);

        // Now there should be non-empty data for the second commit round,
        // and empty data for the third one. Data for the first commit round
        // should be dropped because `MAX_NUM_COMMITS_TO_STORE_DATA` is set to 2.
        {
            let multi_commit_congestion_info = dummy_epoch_store.congestion_info.lock();

            assert_eq!(
                *multi_commit_congestion_info,
                MultiCommitCongestionInfo {
                    commits_data: VecDeque::from([
                        per_commit_congestion_info_2,
                        PerCommitCongestionInfo::new(commit_round_3)
                    ]),
                }
            );
        }
    }
}
