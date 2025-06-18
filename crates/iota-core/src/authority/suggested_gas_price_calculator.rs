use std::collections::HashMap;

use iota_types::{
    base_types::ObjectID, executable_transaction::VerifiedExecutableTransaction,
    transaction::TransactionDataAPI,
};

use super::shared_object_congestion_tracker::ExecutionTime;

/// Holds shared object congestion info for a single scheduled shared-object
/// transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScheduledTransactionCongestionInfo {
    /// Gas price of a scheduled shared-object transaction.
    gas_price: u64,

    /// Estimated execution duration of a scheduled shared-object transaction.
    estimated_execution_duration: ExecutionTime,
}

impl ScheduledTransactionCongestionInfo {
    /// Create a new congestion info for scheduled shared-object transaction
    /// with gas price `gas_price` and estimated execution duration
    /// `estimated_execution_duration`.
    pub fn new(gas_price: u64, estimated_execution_duration: ExecutionTime) -> Self {
        Self {
            gas_price,
            estimated_execution_duration,
        }
    }
}

/// Holds shared object congestion info for a single shared object.
type PerObjectCongestionInfo = Vec<ScheduledTransactionCongestionInfo>;

/// Holds shared object congestion data for a single consensus commit round.
type PerCommitCongestionInfo = HashMap<ObjectID, PerObjectCongestionInfo>;

/// `SuggestedGasPriceCalculator` calculates suggested gas prices for
/// deferred/cancelled shared-object transactions, using congestion
/// info from a single consensus commit.
pub(crate) struct SuggestedGasPriceCalculator {
    congestion_info: PerCommitCongestionInfo,
    max_execution_duration_per_commit: ExecutionTime,
    max_gas_price: u64,
}

impl SuggestedGasPriceCalculator {
    /// Create a new `SuggestedGasPriceCalculator` with empty shared
    /// object congestion data for a given consensus commit round.
    pub fn new(max_execution_duration_per_commit: ExecutionTime, max_gas_price: u64) -> Self {
        Self {
            congestion_info: PerCommitCongestionInfo::new(),
            max_execution_duration_per_commit,
            max_gas_price,
        }
    }

    /// Update per-commit congestion info for a single certificate. This should
    /// only be called for scheduled certificates that contain shared object(s).
    pub fn update_congestion_info(
        &mut self,
        certificate: &VerifiedExecutableTransaction,
        estimated_execution_duration: ExecutionTime,
    ) {
        let scheduled_transaction_congestion_info = ScheduledTransactionCongestionInfo::new(
            certificate.transaction_data().gas_price(),
            estimated_execution_duration,
        );

        certificate.shared_input_objects().for_each(|object| {
            self.congestion_info
                .entry(object.id)
                .and_modify(|per_object_congestion_info| {
                    per_object_congestion_info.push(scheduled_transaction_congestion_info);
                })
                .or_insert(PerObjectCongestionInfo::from([
                    scheduled_transaction_congestion_info,
                ]));
        });
    }

    /// Calculate a suggested gas price using single-commit congestion data.
    /// This should only be called for certificates deferred/cancelled due
    /// to shared object congestion.
    pub fn calculate_suggested_gas_price(
        &self,
        certificate: &VerifiedExecutableTransaction,
        estimated_execution_duration: ExecutionTime,
    ) -> u64 {
        let passing_gas_price = certificate
            .shared_input_objects()
            .filter_map(|object| {
                self.congestion_info
                    .get(&object.id)
                    // If there is no congestion info for this object, that means none of the
                    // scheduled transactions touched this object so far, so we can ignore it.
                    .map(|per_object_congestion_info| {
                        // Calculate available estimated execution duration for this shared object
                        let available_estimated_execution_duration = self
                            .max_execution_duration_per_commit
                            - per_object_congestion_info
                                .iter()
                                .map(|ci| ci.estimated_execution_duration)
                                .sum::<ExecutionTime>();

                        if estimated_execution_duration > available_estimated_execution_duration {
                            // Certificate's estimated execution duration is larger than the
                            // available execution duration for this shared object. In other words,
                            // this object is congested. There must exist the lowest gas price of
                            // scheduled transaction touching this shared object such that the
                            // deferred/cancelled certificate's would fully fit, i.e., would be
                            // scheduled.

                            let mut passing_gas_price = None;
                            let mut accum_estimated_execution_duration =
                                available_estimated_execution_duration;

                            for tx_congestion_info in per_object_congestion_info.iter().rev() {
                                accum_estimated_execution_duration +=
                                    tx_congestion_info.estimated_execution_duration;
                                if accum_estimated_execution_duration
                                    >= estimated_execution_duration
                                {
                                    // The accumulated estimated execution duration is sufficient
                                    // to fit this certificate, so we take the gas price of the
                                    // corresponding transaction and break the loop.
                                    passing_gas_price = Some(tx_congestion_info.gas_price);
                                    break;
                                }
                            }

                            passing_gas_price.unwrap_or_else(|| {
                                panic!(
                                    "Could not find the passing gas price for transaction with \
                                        estimated execution duration of {}, meaning that this \
                                        transaction alone would not even fit into maximum \
                                        execution duration per commit {}",
                                    estimated_execution_duration,
                                    self.max_execution_duration_per_commit,
                                )
                            })
                        } else {
                            // This `else` branch means the following: the certificate would
                            // be scheduled if it only touched this shared object. In other words,
                            // this shared object alone is not the reason for deferring/cancelling
                            // this certificate, so we can ignore congestion data for this object.

                            0
                        }
                    })
            })
            // Take max, which means the worst case object (most congested)
            .max()
            .expect(
                "At least one of the shared input objects should have appeared in scheduled \
                    transactions earlier.",
            );

        assert_ne!(
            passing_gas_price, 0,
            "At least one of the shared input objects must have been congested."
        );

        // Suggested gas price equals passing_gas_price + 1. We add 1 to make this
        // transaction would be scheduled if the same commit structure was repeated.
        let suggested_gas_price = passing_gas_price + 1;

        // Make sure suggested_gas_price is not larger than the maximum possible gas
        // price.
        self.max_gas_price.min(suggested_gas_price)
    }
}
