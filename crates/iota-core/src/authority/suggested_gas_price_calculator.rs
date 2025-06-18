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

        // TODO: should only consider mutable objects like in bump_object_execution_slots
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
            // TODO: should only consider mutable objects like in bump_object_execution_slots
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use iota_protocol_config::PerObjectCongestionControlMode;
    use iota_types::base_types::ObjectID;
    use rstest::rstest;

    use super::SuggestedGasPriceCalculator;
    use crate::authority::{
        shared_object_congestion_tracker::{
            SequencingResult, SharedObjectCongestionTracker,
            shared_object_test_utils::build_transaction,
        },
        suggested_gas_price_calculator::{
            PerCommitCongestionInfo, PerObjectCongestionInfo, ScheduledTransactionCongestionInfo,
        },
    };

    const MAX_GAS_PRICE: u64 = 100_000;

    #[test]
    fn update_congestion_info() {
        let max_execution_duration_per_commit = 10; // not important in this test

        let mut suggested_gas_price_calculator =
            SuggestedGasPriceCalculator::new(max_execution_duration_per_commit, MAX_GAS_PRICE);

        let object_1 = ObjectID::random();
        let object_2 = ObjectID::random();
        let object_3 = ObjectID::random();
        let object_4 = ObjectID::random();
        let object_5 = ObjectID::random();

        // Construct the first certificate with some shared input objects,
        // gas price, estimated execution duration, and update calculator's
        // congestion info for this certificate.
        let objects_1 = vec![(object_1, true), (object_2, false)];
        let gas_budget_1 = 1_003_000; // not important in this test
        let gas_price_1 = 1_003;
        let certificate_1 = build_transaction(&objects_1, gas_budget_1, gas_price_1);
        let estimated_execution_duration_1 = 3;
        suggested_gas_price_calculator
            .update_congestion_info(&certificate_1, estimated_execution_duration_1);
        //
        let object_1_expected_congestion_info =
            PerObjectCongestionInfo::from([ScheduledTransactionCongestionInfo {
                gas_price: gas_price_1,
                estimated_execution_duration: estimated_execution_duration_1,
            }]);
        let object_2_expected_congestion_info =
            PerObjectCongestionInfo::from([ScheduledTransactionCongestionInfo {
                gas_price: gas_price_1,
                estimated_execution_duration: estimated_execution_duration_1,
            }]);
        assert_eq!(
            suggested_gas_price_calculator.congestion_info,
            PerCommitCongestionInfo::from([
                (object_1, object_1_expected_congestion_info),
                (object_2, object_2_expected_congestion_info),
            ]),
        );

        // Construct the second certificate with some shared input objects,
        // gas price, estimated execution duration, and update calculator's
        // congestion info for this certificate.
        let objects_2 = vec![(object_2, true), (object_3, false), (object_4, true)];
        let gas_budget_2 = 1_002_000; // not important in this test
        let gas_price_2 = 1_002;
        let certificate_2 = build_transaction(&objects_2, gas_budget_2, gas_price_2);
        let estimated_execution_duration_2 = 2;
        suggested_gas_price_calculator
            .update_congestion_info(&certificate_2, estimated_execution_duration_2);
        //
        let object_1_expected_congestion_info =
            PerObjectCongestionInfo::from([ScheduledTransactionCongestionInfo {
                gas_price: gas_price_1,
                estimated_execution_duration: estimated_execution_duration_1,
            }]);
        let object_2_expected_congestion_info = PerObjectCongestionInfo::from([
            ScheduledTransactionCongestionInfo {
                gas_price: gas_price_1,
                estimated_execution_duration: estimated_execution_duration_1,
            },
            ScheduledTransactionCongestionInfo {
                gas_price: gas_price_2,
                estimated_execution_duration: estimated_execution_duration_2,
            },
        ]);
        let object_3_expected_congestion_info =
            PerObjectCongestionInfo::from([ScheduledTransactionCongestionInfo {
                gas_price: gas_price_2,
                estimated_execution_duration: estimated_execution_duration_2,
            }]);
        let object_4_expected_congestion_info =
            PerObjectCongestionInfo::from([ScheduledTransactionCongestionInfo {
                gas_price: gas_price_2,
                estimated_execution_duration: estimated_execution_duration_2,
            }]);
        assert_eq!(
            suggested_gas_price_calculator.congestion_info,
            PerCommitCongestionInfo::from([
                (object_1, object_1_expected_congestion_info),
                (object_2, object_2_expected_congestion_info),
                (object_3, object_3_expected_congestion_info),
                (object_4, object_4_expected_congestion_info),
            ]),
        );

        // Construct the third certificate with some shared input objects,
        // gas price, estimated execution duration, and update calculator's
        // congestion info for this certificate.
        let objects_3 = vec![(object_4, false), (object_5, true)];
        let gas_budget_3 = 1_001_000; // not important in this test
        let gas_price_3 = 1_001;
        let certificate_3 = build_transaction(&objects_3, gas_budget_3, gas_price_3);
        let estimated_execution_duration_3 = 1;
        suggested_gas_price_calculator
            .update_congestion_info(&certificate_3, estimated_execution_duration_3);
        //
        let object_1_expected_congestion_info =
            PerObjectCongestionInfo::from([ScheduledTransactionCongestionInfo {
                gas_price: gas_price_1,
                estimated_execution_duration: estimated_execution_duration_1,
            }]);
        let object_2_expected_congestion_info = PerObjectCongestionInfo::from([
            ScheduledTransactionCongestionInfo {
                gas_price: gas_price_1,
                estimated_execution_duration: estimated_execution_duration_1,
            },
            ScheduledTransactionCongestionInfo {
                gas_price: gas_price_2,
                estimated_execution_duration: estimated_execution_duration_2,
            },
        ]);
        let object_3_expected_congestion_info =
            PerObjectCongestionInfo::from([ScheduledTransactionCongestionInfo {
                gas_price: gas_price_2,
                estimated_execution_duration: estimated_execution_duration_2,
            }]);
        let object_4_expected_congestion_info = PerObjectCongestionInfo::from([
            ScheduledTransactionCongestionInfo {
                gas_price: gas_price_2,
                estimated_execution_duration: estimated_execution_duration_2,
            },
            ScheduledTransactionCongestionInfo {
                gas_price: gas_price_3,
                estimated_execution_duration: estimated_execution_duration_3,
            },
        ]);
        let object_5_expected_congestion_info =
            PerObjectCongestionInfo::from([ScheduledTransactionCongestionInfo {
                gas_price: gas_price_3,
                estimated_execution_duration: estimated_execution_duration_3,
            }]);
        assert_eq!(
            suggested_gas_price_calculator.congestion_info,
            PerCommitCongestionInfo::from([
                (object_1, object_1_expected_congestion_info),
                (object_2, object_2_expected_congestion_info),
                (object_3, object_3_expected_congestion_info),
                (object_4, object_4_expected_congestion_info),
                (object_5, object_5_expected_congestion_info),
            ]),
        );
    }

    #[rstest]
    fn calculate_suggested_gas_price(
        #[values(
            PerObjectCongestionControlMode::TotalTxCount,
            PerObjectCongestionControlMode::TotalGasBudget
        )]
        mode: PerObjectCongestionControlMode,
    ) {
        // Commit round and previously deferred transaction digests are not important in
        // this test.
        let commit_round = 0;
        let previously_deferred_tx_digests = HashMap::new();

        // Allow only two transactions per shared object per commit. In the
        // `TotalGasBudget` mode, gas budget of transactions will be set
        // accordingly.
        let max_execution_duration_per_commit = match mode {
            PerObjectCongestionControlMode::None => unreachable!(),
            PerObjectCongestionControlMode::TotalTxCount => 2,
            PerObjectCongestionControlMode::TotalGasBudget => 3_000_000,
        };

        // Suggested gas price calculator will not be used in the old sequencer anyway,
        // so `assign_min_free_execution_slot` is set to `true`.
        let mut shared_object_congestion_tracker = SharedObjectCongestionTracker::new(mode, true);

        let mut suggested_gas_price_calculator =
            SuggestedGasPriceCalculator::new(max_execution_duration_per_commit, MAX_GAS_PRICE);

        let object_1 = ObjectID::random();
        let object_2 = ObjectID::random();
        let object_3 = ObjectID::random();
        let object_4 = ObjectID::random();
        let object_5 = ObjectID::random();

        // Construct the first certificate with some shared input objects,
        // gas price, estimated execution duration, and update calculator's
        // congestion info for this certificate.
        let objects_1 = vec![(object_1, true), (object_2, true)];
        let gas_budget_1 = 1_003_000;
        let gas_price_1 = 1_003;
        let certificate_1 = build_transaction(&objects_1, gas_budget_1, gas_price_1);
        let estimated_execution_duration_1 =
            shared_object_congestion_tracker.get_estimated_execution_duration(&certificate_1);

        // Try sequencing the first certificate
        let shared_input_objects_1: Vec<_> = certificate_1.shared_input_objects().collect();
        shared_object_congestion_tracker.initialize_object_execution_slots(&shared_input_objects_1);
        let sequencing_result = shared_object_congestion_tracker.try_schedule(
            &certificate_1,
            max_execution_duration_per_commit,
            &previously_deferred_tx_digests,
            commit_round,
        );
        // Shared-object transactions allocations at this step should look as follows:
        // |----------------------------------------------------------------|
        // |  object_1  |  object_2  |  object_3  |  object_4  |  object_5  |
        // |----------------------------------------------------------------|
        // |:::::::::::::::MAX_EXECUTION_DURATION_PER_COMMIT::::::::::::::::|
        // |----------------------------------------------------------------|
        // |                                                                |
        // |----------------------------------------------------------------|
        // |   certificate_1 (1003)  |                                      |
        // |----------------------------------------------------------------|
        if let SequencingResult::Schedule(start_time) = sequencing_result {
            shared_object_congestion_tracker
                .bump_object_execution_slots(&certificate_1, start_time);
            suggested_gas_price_calculator
                .update_congestion_info(&certificate_1, estimated_execution_duration_1);
        } else {
            panic!("Certificate 1 must be scheduled");
        }

        // Construct the second certificate with some shared input objects,
        // gas price, estimated execution duration, and update calculator's
        // congestion info for this certificate.
        let objects_2 = vec![(object_2, true), (object_3, true), (object_4, true)];
        let gas_budget_2 = 1_002_000;
        let gas_price_2 = 1_002;
        let certificate_2 = build_transaction(&objects_2, gas_budget_2, gas_price_2);
        let estimated_execution_duration_2 =
            shared_object_congestion_tracker.get_estimated_execution_duration(&certificate_2);

        // Try sequencing the second certificate
        let shared_input_objects_2: Vec<_> = certificate_2.shared_input_objects().collect();
        shared_object_congestion_tracker.initialize_object_execution_slots(&shared_input_objects_2);
        let sequencing_result_2 = shared_object_congestion_tracker.try_schedule(
            &certificate_2,
            max_execution_duration_per_commit,
            &previously_deferred_tx_digests,
            commit_round,
        );
        // Shared-object transactions allocations at this step looks as follows:
        // |----------------------------------------------------------------|
        // |  object_1  |  object_2  |  object_3  |  object_4  |  object_5  |
        // |----------------------------------------------------------------|
        // |:::::::::::::::MAX_EXECUTION_DURATION_PER_COMMIT::::::::::::::::|
        // |----------------------------------------------------------------|
        // |            |         certificate_2 (1002)         |            |
        // |----------------------------------------------------------------|
        // |   certificate_1 (1003)  |                                      |
        // |----------------------------------------------------------------|
        if let SequencingResult::Schedule(start_time) = sequencing_result_2 {
            shared_object_congestion_tracker
                .bump_object_execution_slots(&certificate_2, start_time);
            suggested_gas_price_calculator
                .update_congestion_info(&certificate_2, estimated_execution_duration_2);
        } else {
            panic!("Certificate 2 must be scheduled");
        }

        // Construct the third certificate with some shared input objects,
        // gas price, estimated execution duration, and update calculator's
        // congestion info for this certificate.
        let objects_3 = vec![(object_4, true), (object_5, true)];
        let gas_budget_3 = 1_001_000;
        let gas_price_3 = 1_001;
        let certificate_3 = build_transaction(&objects_3, gas_budget_3, gas_price_3);
        let estimated_execution_duration_3 =
            shared_object_congestion_tracker.get_estimated_execution_duration(&certificate_3);

        // Try sequencing the second certificate
        let shared_input_objects_3: Vec<_> = certificate_3.shared_input_objects().collect();
        shared_object_congestion_tracker.initialize_object_execution_slots(&shared_input_objects_3);
        let sequencing_result_3 = shared_object_congestion_tracker.try_schedule(
            &certificate_3,
            max_execution_duration_per_commit,
            &previously_deferred_tx_digests,
            commit_round,
        );
        // Shared-object transactions allocations at this step looks as follows:
        // |----------------------------------------------------------------|
        // |  object_1  |  object_2  |  object_3  |  object_4  |  object_5  |
        // |----------------------------------------------------------------|
        // |:::::::::::::::MAX_EXECUTION_DURATION_PER_COMMIT::::::::::::::::|
        // |----------------------------------------------------------------|
        // |            |         certificate_2 (1002)         |            |
        // |----------------------------------------------------------------|
        // |   certificate_1 (1003)  |            |   certificate_3 (1001)  |
        // |----------------------------------------------------------------|
        if let SequencingResult::Schedule(start_time) = sequencing_result_3 {
            shared_object_congestion_tracker
                .bump_object_execution_slots(&certificate_3, start_time);
            suggested_gas_price_calculator
                .update_congestion_info(&certificate_3, estimated_execution_duration_3);
        } else {
            panic!("Certificate 3 must be scheduled");
        }

        // Construct the fourth certificate with some shared input objects,
        // gas price, estimated execution duration, and update calculator's
        // congestion info for this certificate.
        let objects_4 = vec![
            (object_1, true),
            (object_2, true),
            (object_3, true),
            (object_4, true),
            (object_5, true),
        ];
        let gas_budget_4 = 1_000_000;
        let gas_price_4 = 1_000;
        let certificate_4 = build_transaction(&objects_4, gas_budget_4, gas_price_4);
        let estimated_execution_duration_4 =
            shared_object_congestion_tracker.get_estimated_execution_duration(&certificate_4);

        // Try sequencing the fourth certificate
        let shared_input_objects_4: Vec<_> = certificate_4.shared_input_objects().collect();
        shared_object_congestion_tracker.initialize_object_execution_slots(&shared_input_objects_4);
        let sequencing_result_4 = shared_object_congestion_tracker.try_schedule(
            &certificate_4,
            max_execution_duration_per_commit,
            &previously_deferred_tx_digests,
            commit_round,
        );
        // Shared-object transactions allocations at this step looks as follows:
        // |----------------------------------------------------------------|
        // |  object_1  |  object_2  |  object_3  |  object_4  |  object_5  |
        // |----------------------------------------------------------------|
        // |:::::::::::::::MAX_EXECUTION_DURATION_PER_COMMIT::::::::::::::::|
        // |----------------------------------------------------------------|
        // |            |         certificate_2 (1002)         |            |
        // |----------------------------------------------------------------|
        // |   certificate_1 (1003)  |            |   certificate_3 (1001)  |
        // |----------------------------------------------------------------|
        if let SequencingResult::Defer(_key, congested_objects) = sequencing_result_4 {
            assert_eq!(
                congested_objects,
                objects_4.into_iter().map(|(id, _)| id).collect::<Vec<_>>(),
            );
            let suggested_gas_price = suggested_gas_price_calculator
                .calculate_suggested_gas_price(&certificate_4, estimated_execution_duration_4);

            // To be scheduled, certificate_4 should have payed at least the gas
            // price of certificate_2 plus 1.
            assert_eq!(suggested_gas_price, gas_price_2 + 1);
        } else {
            panic!("Certificate 4 must be deferred");
        }

        // Construct the fifth certificate with some shared input objects,
        // gas price, estimated execution duration, and update calculator's
        // congestion info for this certificate.
        let objects_5 = vec![(object_1, true), (object_2, true)];
        let gas_budget_5 = 2_000_000;
        let gas_price_5 = 1_000;
        let certificate_5 = build_transaction(&objects_5, gas_budget_5, gas_price_5);
        let estimated_execution_duration_5 =
            shared_object_congestion_tracker.get_estimated_execution_duration(&certificate_5);

        // Try sequencing the fourth certificate
        let shared_input_objects_5: Vec<_> = certificate_5.shared_input_objects().collect();
        shared_object_congestion_tracker.initialize_object_execution_slots(&shared_input_objects_5);
        let sequencing_result_5 = shared_object_congestion_tracker.try_schedule(
            &certificate_5,
            max_execution_duration_per_commit,
            &previously_deferred_tx_digests,
            commit_round,
        );
        // Shared-object transactions allocations at this step looks as follows:
        // |----------------------------------------------------------------|
        // |  object_1  |  object_2  |  object_3  |  object_4  |  object_5  |
        // |----------------------------------------------------------------|
        // |:::::::::::::::MAX_EXECUTION_DURATION_PER_COMMIT::::::::::::::::|
        // |----------------------------------------------------------------|
        // |            |         certificate_2 (1002)         |            |
        // |----------------------------------------------------------------|
        // |   certificate_1 (1003)  |            |   certificate_3 (1001)  |
        // |----------------------------------------------------------------|
        if let SequencingResult::Defer(_key, congested_objects) = sequencing_result_5 {
            assert_eq!(
                congested_objects,
                objects_5.into_iter().map(|(id, _)| id).collect::<Vec<_>>(),
            );
            let suggested_gas_price = suggested_gas_price_calculator
                .calculate_suggested_gas_price(&certificate_5, estimated_execution_duration_5);

            match mode {
                PerObjectCongestionControlMode::None => unreachable!(),
                PerObjectCongestionControlMode::TotalTxCount => {
                    // To be scheduled, certificate_4 should have payed at least the gas
                    // price of certificate_2 plus 1. `TotalTxCount` mode is blind to
                    // gas budget.
                    assert_eq!(suggested_gas_price, gas_price_2 + 1);
                }
                PerObjectCongestionControlMode::TotalGasBudget => {
                    // To be scheduled, certificate_4 should have payed at least the gas
                    // price of certificate_1 plus 1.
                    assert_eq!(suggested_gas_price, gas_price_1 + 1);
                }
            }
        } else {
            panic!("Certificate 5 must be deferred");
        }
    }
}
