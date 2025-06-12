use iota_types::{
    base_types::CommitRound, executable_transaction::VerifiedExecutableTransaction,
    transaction::TransactionDataAPI,
};

use super::{
    shared_object_congestion_info::PerCommitCongestionInfo,
    shared_object_congestion_tracker::ExecutionTime,
};

/// `PerCommitSuggestedGasPriceCalculator` calculates suggested gas prices
/// for deferred/cancelled shared-object transactions, using congestion
/// info from a single consensus commit.
pub(crate) struct PerCommitSuggestedGasPriceCalculator {
    congestion_info: PerCommitCongestionInfo,
}

impl PerCommitSuggestedGasPriceCalculator {
    /// Create/initialize a new `PerCommitSuggestedGasPriceCalculator` with
    /// empty shared object congestion data for a given consensus commit
    /// round.
    pub fn new(commit_round: CommitRound) -> Self {
        Self {
            congestion_info: PerCommitCongestionInfo::new(commit_round),
        }
    }

    /// Update per-commit congestion info for a single certificate. This should
    /// only be called for scheduled certificates that contain shared object(s).
    pub fn update_congestion_info(
        &mut self,
        certificate: &VerifiedExecutableTransaction,
        estimated_execution_duration: ExecutionTime,
    ) {
        self.congestion_info
            .update_for_scheduled_consensus_certificate(certificate, estimated_execution_duration);
    }

    /// Calculate a suggested gas price using single-commit congestion data.
    /// This should only be called for certificates deferred/cancelled due
    /// to shared object congestion.
    pub fn calculate_suggested_gas_price(
        &self,
        certificate: &VerifiedExecutableTransaction,
    ) -> u64 {
        certificate.transaction_data().gas_price()
    }
}
