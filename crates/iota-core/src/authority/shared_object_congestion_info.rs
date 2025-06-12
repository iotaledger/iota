use std::collections::HashMap;

use iota_types::{
    base_types::{CommitRound, ObjectID},
    executable_transaction::VerifiedExecutableTransaction,
    transaction::TransactionDataAPI,
};

use super::shared_object_congestion_tracker::ExecutionTime;

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
