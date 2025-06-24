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
    max_execution_duration_per_commit: Option<ExecutionTime>,
    min_free_execution_slot_assigned: bool,
    reference_gas_price: u64,
    max_gas_price: u64,
}

impl SuggestedGasPriceCalculator {
    /// Create a new `SuggestedGasPriceCalculator` with empty shared
    /// object congestion data for a given consensus commit round.
    pub fn new(
        max_execution_duration_per_commit: Option<ExecutionTime>,
        min_free_execution_slot_assigned: bool,
        reference_gas_price: u64,
        max_gas_price: u64,
    ) -> Self {
        Self {
            congestion_info: PerCommitCongestionInfo::new(),
            max_execution_duration_per_commit,
            min_free_execution_slot_assigned,
            reference_gas_price,
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
        // If we don't have a max execution duration or
        // `min_free_execution_slot_assigned` is false (which realizes
        // the old Sui's canonical sequencer logic), we don't need to update
        // the congestion info since the reference gas price will be suggested.
        if self.max_execution_duration_per_commit.is_none()
            || !self.min_free_execution_slot_assigned
        {
            return;
        }

        let scheduled_transaction_congestion_info = ScheduledTransactionCongestionInfo::new(
            certificate.transaction_data().gas_price(),
            estimated_execution_duration,
        );

        certificate
            .shared_input_objects()
            // Only consider shared objects accessed mutably as objects accessed immutably
            // do not change object's execution slots in the sequencer.
            .filter(|object| object.mutable)
            .for_each(|object| {
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
        // If we don't have a max execution duration, suggest the reference gas price.
        if self.max_execution_duration_per_commit.is_none() {
            return self.reference_gas_price;
        }

        // If `min_free_execution_slot_assigned` is false (which realizes
        // the old Sui's canonical sequencer logic), suggest the reference gas price.
        if !self.min_free_execution_slot_assigned {
            return self.reference_gas_price;
        }

        let max_execution_duration_per_commit = self
            .max_execution_duration_per_commit
            .expect("max_execution_duration_per_commit must not be None at this step");
        let passing_gas_price = certificate
            .shared_input_objects()
            .filter_map(|object| {
                self.congestion_info
                    .get(&object.id)
                    // If there is no congestion info for this object, that means none of the
                    // scheduled transactions touched this object so far, so we can ignore it.
                    .map(|per_object_congestion_info| {
                        // Calculate available estimated execution duration for this shared object
                        let available_estimated_execution_duration =
                            max_execution_duration_per_commit
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
                                    estimated_execution_duration, max_execution_duration_per_commit,
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
        suggested_gas_price.min(self.max_gas_price)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use iota_protocol_config::{PerObjectCongestionControlMode, ProtocolConfig};
    use iota_types::base_types::ObjectID;
    use rstest::rstest;

    use super::SuggestedGasPriceCalculator;
    use crate::authority::{
        shared_object_congestion_tracker::{
            ExecutionTime, SequencingResult, SharedObjectCongestionTracker,
            shared_object_test_utils::build_transaction,
        },
        suggested_gas_price_calculator::{
            PerCommitCongestionInfo, PerObjectCongestionInfo, ScheduledTransactionCongestionInfo,
        },
    };

    const REFERENCE_GAS_PRICE: u64 = 1_000;

    #[rstest]
    fn update_congestion_info_test(
        #[values(
            None,
            Some(10), // the value is not important in this test
        )]
        max_execution_duration_per_commit: Option<ExecutionTime>,
        #[values(false, true)] min_free_execution_slot_assigned: bool,
    ) {
        let max_gas_price = ProtocolConfig::get_for_max_version_UNSAFE().max_gas_price();
        let mut suggested_gas_price_calculator = SuggestedGasPriceCalculator::new(
            max_execution_duration_per_commit,
            min_free_execution_slot_assigned,
            REFERENCE_GAS_PRICE,
            max_gas_price,
        );

        let object_1 = ObjectID::random();
        let object_2 = ObjectID::random();
        let object_3 = ObjectID::random();
        let object_4 = ObjectID::random();
        let object_5 = ObjectID::random();

        // Construct the first certificate that touches shared objects:
        // - `object_1` by mutable reference,
        // - `object_2` by immutable reference.
        let objects_1 = vec![(object_1, true), (object_2, false)];
        let gas_budget_1 = 1_003_000; // not important in this test
        let gas_price_1 = 1_003;
        let certificate_1 = build_transaction(&objects_1, gas_budget_1, gas_price_1);
        let estimated_execution_duration_1 = 3;
        // Update the calculator's congestion info for this certificate.
        suggested_gas_price_calculator
            .update_congestion_info(&certificate_1, estimated_execution_duration_1);
        //
        if let Some(_max_execution_duration_per_commit) = max_execution_duration_per_commit {
            if min_free_execution_slot_assigned {
                // Note that `object_2` should not appear because it is accessed immutably.
                let object_1_expected_congestion_info =
                    PerObjectCongestionInfo::from([ScheduledTransactionCongestionInfo {
                        gas_price: gas_price_1,
                        estimated_execution_duration: estimated_execution_duration_1,
                    }]);
                assert_eq!(
                    suggested_gas_price_calculator.congestion_info,
                    PerCommitCongestionInfo::from([(object_1, object_1_expected_congestion_info)]),
                );
            } else {
                // We don't have `min_free_execution_slot_assigned` set to `true`,
                // so there is no need in updating the calculator's congestion info.
                assert_eq!(
                    suggested_gas_price_calculator.congestion_info,
                    PerCommitCongestionInfo::new()
                );
            }
        } else {
            // We don't have max execution duration per commit, so there is no need
            // in updating the calculator's congestion info.
            assert_eq!(
                suggested_gas_price_calculator.congestion_info,
                PerCommitCongestionInfo::new()
            );
        }

        // Construct the second certificate that touches shared objects:
        // - `object_2` by mutable reference,
        // - `object_3` by immutable reference,
        // - `object_4` by mutable reference.
        let objects_2 = vec![(object_2, true), (object_3, false), (object_4, true)];
        let gas_budget_2 = 1_002_000; // not important in this test
        let gas_price_2 = 1_002;
        let certificate_2 = build_transaction(&objects_2, gas_budget_2, gas_price_2);
        let estimated_execution_duration_2 = 2;
        // Update the calculator's congestion info for this certificate.
        suggested_gas_price_calculator
            .update_congestion_info(&certificate_2, estimated_execution_duration_2);
        //
        if let Some(_max_execution_duration_per_commit) = max_execution_duration_per_commit {
            if min_free_execution_slot_assigned {
                // Note that `object_3` should not appear because it is accessed immutably.
                let object_1_expected_congestion_info =
                    PerObjectCongestionInfo::from([ScheduledTransactionCongestionInfo {
                        gas_price: gas_price_1,
                        estimated_execution_duration: estimated_execution_duration_1,
                    }]);
                let object_2_expected_congestion_info =
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
                        (object_4, object_4_expected_congestion_info),
                    ]),
                );
            } else {
                // We don't have `min_free_execution_slot_assigned` set to `true`,
                // so there is no need in updating the calculator's congestion info.
                assert_eq!(
                    suggested_gas_price_calculator.congestion_info,
                    PerCommitCongestionInfo::new()
                );
            }
        } else {
            // We don't have max execution duration per commit, so there is no need
            // in updating the calculator's congestion info.
            assert_eq!(
                suggested_gas_price_calculator.congestion_info,
                PerCommitCongestionInfo::new()
            );
        }

        // Construct the third certificate that touches shared objects:
        // - `object_4` by immutable reference,
        // - `object_5` by mutable reference.
        let objects_3 = vec![(object_4, false), (object_5, true)];
        let gas_budget_3 = 1_001_000; // not important in this test
        let gas_price_3 = 1_001;
        let certificate_3 = build_transaction(&objects_3, gas_budget_3, gas_price_3);
        let estimated_execution_duration_3 = 1;
        // Update the calculator's congestion info for this certificate.
        suggested_gas_price_calculator
            .update_congestion_info(&certificate_3, estimated_execution_duration_3);
        //
        if let Some(_max_execution_duration_per_commit) = max_execution_duration_per_commit {
            if min_free_execution_slot_assigned {
                // Note that `object_3` should not appear because it is accessed immutably.
                let object_1_expected_congestion_info =
                    PerObjectCongestionInfo::from([ScheduledTransactionCongestionInfo {
                        gas_price: gas_price_1,
                        estimated_execution_duration: estimated_execution_duration_1,
                    }]);
                let object_2_expected_congestion_info =
                    PerObjectCongestionInfo::from([ScheduledTransactionCongestionInfo {
                        gas_price: gas_price_2,
                        estimated_execution_duration: estimated_execution_duration_2,
                    }]);
                let object_4_expected_congestion_info =
                    PerObjectCongestionInfo::from([ScheduledTransactionCongestionInfo {
                        gas_price: gas_price_2,
                        estimated_execution_duration: estimated_execution_duration_2,
                    }]);
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
                        (object_4, object_4_expected_congestion_info),
                        (object_5, object_5_expected_congestion_info),
                    ]),
                );
            } else {
                // We don't have `min_free_execution_slot_assigned` set to `true`,
                // so there is no need in updating the calculator's congestion info.
                assert_eq!(
                    suggested_gas_price_calculator.congestion_info,
                    PerCommitCongestionInfo::new()
                );
            }
        } else {
            // We don't have max execution duration per commit, so there is no need
            // in updating the calculator's congestion info.
            assert_eq!(
                suggested_gas_price_calculator.congestion_info,
                PerCommitCongestionInfo::new()
            );
        }
    }

    #[rstest]
    fn calculate_suggested_gas_price_test(
        #[values(
            PerObjectCongestionControlMode::TotalTxCount,
            PerObjectCongestionControlMode::TotalGasBudget
        )]
        mode: PerObjectCongestionControlMode,
        #[values(false, true)] min_free_execution_slot_assigned: bool,
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

        let mut shared_object_congestion_tracker =
            SharedObjectCongestionTracker::new(mode, min_free_execution_slot_assigned);

        let max_gas_price = ProtocolConfig::get_for_max_version_UNSAFE().max_gas_price();
        let mut suggested_gas_price_calculator = SuggestedGasPriceCalculator::new(
            Some(max_execution_duration_per_commit),
            min_free_execution_slot_assigned,
            REFERENCE_GAS_PRICE,
            max_gas_price,
        );

        let object_1 = ObjectID::random();
        let object_2 = ObjectID::random();
        let object_3 = ObjectID::random();
        let object_4 = ObjectID::random();
        let object_5 = ObjectID::random();

        // Construct a certificate that touches the following shared objects:
        // - `object_1` by mutable reference,
        // - `object_2` by immutable reference.
        let objects_1 = vec![(object_1, true), (object_2, false)];
        let gas_budget_1 = 1_010_000;
        // Set this certificate's gas price to the max gas price to test whether the
        // calculator does not suggest anything larger than the max gas price.
        let gas_price_1 = max_gas_price;
        let certificate_1 = build_transaction(&objects_1, gas_budget_1, gas_price_1);
        let estimated_execution_duration_1 =
            shared_object_congestion_tracker.get_estimated_execution_duration(&certificate_1);
        // Try sequencing this certificate
        let shared_input_objects_1: Vec<_> = certificate_1.shared_input_objects().collect();
        shared_object_congestion_tracker.initialize_object_execution_slots(&shared_input_objects_1);
        let sequencing_result_1 = shared_object_congestion_tracker.try_schedule(
            &certificate_1,
            max_execution_duration_per_commit,
            &previously_deferred_tx_digests,
            commit_round,
        );
        // Shared object transactions allocations should look as follows:
        // |------------------------------------------------------|
        // | object 1 | object 2 | object 3 | object 4 | object 5 |
        // |----------|----------|----------|----------|----------|
        // |          |          |          |          |          |
        // |----------|----------|----------|----------|----------|
        // | cert. 1  |          |          |          |          |
        // |------------------------------------------------------|
        if let SequencingResult::Schedule(start_time) = sequencing_result_1 {
            shared_object_congestion_tracker
                .bump_object_execution_slots(&certificate_1, start_time);
            suggested_gas_price_calculator
                .update_congestion_info(&certificate_1, estimated_execution_duration_1);
        } else {
            panic!("Certificate 1 must be scheduled");
        }

        // Construct a certificate that touches the following shared objects:
        // - `object_2` by mutable reference,
        // - `object_3` by immutable reference,
        // - `object_4` by mutable reference.
        let objects_2 = vec![(object_2, true), (object_3, false), (object_4, true)];
        let gas_budget_2 = 1_009_000;
        let gas_price_2 = 1_009;
        let certificate_2 = build_transaction(&objects_2, gas_budget_2, gas_price_2);
        let estimated_execution_duration_2 =
            shared_object_congestion_tracker.get_estimated_execution_duration(&certificate_2);
        // Try sequencing this certificate
        let shared_input_objects_2: Vec<_> = certificate_2.shared_input_objects().collect();
        shared_object_congestion_tracker.initialize_object_execution_slots(&shared_input_objects_2);
        let sequencing_result_2 = shared_object_congestion_tracker.try_schedule(
            &certificate_2,
            max_execution_duration_per_commit,
            &previously_deferred_tx_digests,
            commit_round,
        );
        // Shared object transactions allocations should look as follows:
        // |------------------------------------------------------|
        // | object 1 | object 2 | object 3 | object 4 | object 5 |
        // |----------|----------|----------|----------|----------|
        // |          |          |          |          |          |
        // |----------|----------|----------|----------|----------|
        // | cert. 1  | cert. 2  |          | cert. 2  |          |
        // |------------------------------------------------------|
        if let SequencingResult::Schedule(start_time) = sequencing_result_2 {
            shared_object_congestion_tracker
                .bump_object_execution_slots(&certificate_2, start_time);
            suggested_gas_price_calculator
                .update_congestion_info(&certificate_2, estimated_execution_duration_2);
        } else {
            panic!("Certificate 2 must be scheduled");
        }

        // Construct a certificate that touches the following shared objects:
        // - `object_4` by immutable reference,
        // - `object_5` by mutable reference.
        let objects_3 = vec![(object_4, false), (object_5, true)];
        let gas_budget_3 = 1_008_000;
        let gas_price_3 = 1_008;
        let certificate_3 = build_transaction(&objects_3, gas_budget_3, gas_price_3);
        let estimated_execution_duration_3 =
            shared_object_congestion_tracker.get_estimated_execution_duration(&certificate_3);
        // Try sequencing this certificate
        let shared_input_objects_3: Vec<_> = certificate_3.shared_input_objects().collect();
        shared_object_congestion_tracker.initialize_object_execution_slots(&shared_input_objects_3);
        let sequencing_result_3 = shared_object_congestion_tracker.try_schedule(
            &certificate_3,
            max_execution_duration_per_commit,
            &previously_deferred_tx_digests,
            commit_round,
        );
        // Shared object transactions allocations should look as follows:
        // |------------------------------------------------------|
        // | object 1 | object 2 | object 3 | object 4 | object 5 |
        // |----------|----------|----------|----------|----------|
        // |          |          |          |          | cert. 3  |
        // |----------|----------|----------|----------|----------|
        // | cert. 1  | cert. 2  |          | cert. 2  |          |
        // |------------------------------------------------------|
        if let SequencingResult::Schedule(start_time) = sequencing_result_3 {
            shared_object_congestion_tracker
                .bump_object_execution_slots(&certificate_3, start_time);
            suggested_gas_price_calculator
                .update_congestion_info(&certificate_3, estimated_execution_duration_3);
        } else {
            panic!("Certificate 3 must be scheduled");
        }

        // Construct a certificate that touches the following shared objects:
        // - `object_3` by mutable reference.
        let objects_4 = vec![(object_3, true)];
        let gas_budget_4 = 1_007_000;
        let gas_price_4 = 1_007;
        let certificate_4 = build_transaction(&objects_4, gas_budget_4, gas_price_4);
        let estimated_execution_duration_4 =
            shared_object_congestion_tracker.get_estimated_execution_duration(&certificate_4);
        // Try sequencing this certificate
        let shared_input_objects_4: Vec<_> = certificate_4.shared_input_objects().collect();
        shared_object_congestion_tracker.initialize_object_execution_slots(&shared_input_objects_4);
        let sequencing_result_4 = shared_object_congestion_tracker.try_schedule(
            &certificate_4,
            max_execution_duration_per_commit,
            &previously_deferred_tx_digests,
            commit_round,
        );
        // Shared object transactions allocations should look as follows:
        // |------------------------------------------------------|
        // | object 1 | object 2 | object 3 | object 4 | object 5 |
        // |----------|----------|----------|----------|----------|
        // |          |          |          |          | cert. 3  |
        // |----------|----------|----------|----------|----------|
        // | cert. 1  | cert. 2  | cert. 4  | cert. 2  |          |
        // |------------------------------------------------------|
        if let SequencingResult::Schedule(start_time) = sequencing_result_4 {
            shared_object_congestion_tracker
                .bump_object_execution_slots(&certificate_4, start_time);
            suggested_gas_price_calculator
                .update_congestion_info(&certificate_4, estimated_execution_duration_4);
        } else {
            panic!("Certificate 4 must be scheduled");
        }

        // Construct a certificate that touches the following shared objects:
        // - `object_5` by mutable reference.
        let objects_5 = vec![(object_5, true)];
        let gas_budget_5 = 1_006_000;
        let gas_price_5 = 1_006;
        let certificate_5 = build_transaction(&objects_5, gas_budget_5, gas_price_5);
        let estimated_execution_duration_5 =
            shared_object_congestion_tracker.get_estimated_execution_duration(&certificate_5);
        // Try sequencing this certificate
        let shared_input_objects_5: Vec<_> = certificate_5.shared_input_objects().collect();
        shared_object_congestion_tracker.initialize_object_execution_slots(&shared_input_objects_5);
        let sequencing_result_5 = shared_object_congestion_tracker.try_schedule(
            &certificate_5,
            max_execution_duration_per_commit,
            &previously_deferred_tx_digests,
            commit_round,
        );
        // If `min_free_execution_slot_assigned = true`, shared object transactions
        // allocations should look as follows:
        // |------------------------------------------------------|
        // | object 1 | object 2 | object 3 | object 4 | object 5 |
        // |----------|----------|----------|----------|----------|
        // |          |          |          |          | cert. 3  |
        // |----------|----------|----------|----------|----------|
        // | cert. 1  | cert. 2  | cert. 4  | cert. 2  | cert. 5  |
        // |------------------------------------------------------|
        // If `min_free_execution_slot_assigned = false`, this certificate must be
        // deferred.
        if min_free_execution_slot_assigned {
            // ^ this corresponds the new sequencer's logic
            if let SequencingResult::Schedule(start_time) = sequencing_result_5 {
                shared_object_congestion_tracker
                    .bump_object_execution_slots(&certificate_5, start_time);
                suggested_gas_price_calculator
                    .update_congestion_info(&certificate_5, estimated_execution_duration_5);
            } else {
                panic!("Certificate 5 must be scheduled in the new sequencer");
            }
        } else {
            // ^ this corresponds the old sequencer's logic
            if let SequencingResult::Defer(_key, congested_objects) = sequencing_result_5 {
                assert_eq!(congested_objects, vec![object_5]);
                let suggested_gas_price = suggested_gas_price_calculator
                    .calculate_suggested_gas_price(&certificate_5, estimated_execution_duration_5);
                assert_eq!(suggested_gas_price, REFERENCE_GAS_PRICE);
            } else {
                panic!("Certificate 5 must be deferred in the old sequencer");
            }
        }

        // Construct a certificate that touches the following shared objects:
        // - `object_5` by mutable reference.
        let objects_6 = vec![(object_5, true)];
        let gas_budget_6 = 1_005_000;
        let gas_price_6 = 1_005;
        let certificate_6 = build_transaction(&objects_6, gas_budget_6, gas_price_6);
        let estimated_execution_duration_6 =
            shared_object_congestion_tracker.get_estimated_execution_duration(&certificate_6);
        // Try sequencing this certificate
        let shared_input_objects_6: Vec<_> = certificate_6.shared_input_objects().collect();
        shared_object_congestion_tracker.initialize_object_execution_slots(&shared_input_objects_6);
        let sequencing_result_6 = shared_object_congestion_tracker.try_schedule(
            &certificate_6,
            max_execution_duration_per_commit,
            &previously_deferred_tx_digests,
            commit_round,
        );
        // Shared object transactions allocations should look as follows:
        // |------------------------------------------------------|
        // | object 1 | object 2 | object 3 | object 4 | object 5 |
        // |----------|----------|----------|----------|----------|
        // |          |          |          |          | cert. 3  |
        // |----------|----------|----------|----------|----------|
        // | cert. 1  | cert. 2  | cert. 4  | cert. 2  | cert. 5  |
        // |------------------------------------------------------|
        // That is, this certificate must be deferred for both
        // `min_free_execution_slot_assigned` being `true` and `false`.
        if let SequencingResult::Defer(_key, congested_objects) = sequencing_result_6 {
            let suggested_gas_price = suggested_gas_price_calculator
                .calculate_suggested_gas_price(&certificate_6, estimated_execution_duration_6);
            if min_free_execution_slot_assigned {
                // ^ this corresponds the new sequencer's logic
                assert_eq!(
                    congested_objects,
                    objects_6.into_iter().map(|(id, _)| id).collect::<Vec<_>>(),
                );
                assert_eq!(suggested_gas_price, gas_price_5 + 1);
            } else {
                // ^ this corresponds the old sequencer's logic
                assert_eq!(congested_objects, vec![object_5]);
                assert_eq!(suggested_gas_price, REFERENCE_GAS_PRICE);
            }
        } else {
            panic!("Certificate 6 must be deferred");
        }

        // FIX: this example needs a fix in the calculator
        // Construct a certificate that touches the following shared objects:
        // - `object_1` by mutable reference,
        // - `object_2` by mutable reference,
        // - `object_3` by mutable reference,
        // - `object_4` by mutable reference,
        // - `object_5` by mutable reference.
        let objects_7 = vec![
            (object_1, true),
            (object_2, true),
            (object_3, true),
            (object_4, true),
            (object_5, true),
        ];
        let gas_budget_7 = 1_005_000;
        let gas_price_7 = 1_005;
        let certificate_7 = build_transaction(&objects_7, gas_budget_7, gas_price_7);
        let estimated_execution_duration_7 =
            shared_object_congestion_tracker.get_estimated_execution_duration(&certificate_7);
        // Try sequencing this certificate
        let shared_input_objects_7: Vec<_> = certificate_7.shared_input_objects().collect();
        shared_object_congestion_tracker.initialize_object_execution_slots(&shared_input_objects_7);
        let sequencing_result_7 = shared_object_congestion_tracker.try_schedule(
            &certificate_7,
            max_execution_duration_per_commit,
            &previously_deferred_tx_digests,
            commit_round,
        );
        // Shared object transactions allocations should look as follows:
        // |------------------------------------------------------|
        // | object 1 | object 2 | object 3 | object 4 | object 5 |
        // |----------|----------|----------|----------|----------|
        // |          |          |          |          | cert. 3  |
        // |----------|----------|----------|----------|----------|
        // | cert. 1  | cert. 2  | cert. 4  | cert. 2  | cert. 5  |
        // |------------------------------------------------------|
        // That is, this certificate must be deferred for both
        // `min_free_execution_slot_assigned` being `true` and `false`.
        if let SequencingResult::Defer(_key, congested_objects) = sequencing_result_7 {
            let suggested_gas_price = suggested_gas_price_calculator
                .calculate_suggested_gas_price(&certificate_7, estimated_execution_duration_7);
            if min_free_execution_slot_assigned {
                // ^ this corresponds the new sequencer's logic
                assert_eq!(
                    congested_objects,
                    objects_7.into_iter().map(|(id, _)| id).collect::<Vec<_>>(),
                );
                assert_eq!(suggested_gas_price, /* TODO */ gas_price_5 + 1);
            } else {
                // ^ this corresponds the old sequencer's logic
                assert_eq!(congested_objects, vec![object_5]);
                assert_eq!(suggested_gas_price, REFERENCE_GAS_PRICE);
            }
        } else {
            panic!("Certificate 7 must be deferred");
        }
    }

    #[rstest]
    fn temp_test(
        #[values(
            PerObjectCongestionControlMode::TotalTxCount,
            PerObjectCongestionControlMode::TotalGasBudget
        )]
        mode: PerObjectCongestionControlMode,
        #[values(false, true)] min_free_execution_slot_assigned: bool,
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
            PerObjectCongestionControlMode::TotalTxCount => 3,
            PerObjectCongestionControlMode::TotalGasBudget => 4_000_000,
        };

        let mut shared_object_congestion_tracker =
            SharedObjectCongestionTracker::new(mode, min_free_execution_slot_assigned);

        let max_gas_price = ProtocolConfig::get_for_max_version_UNSAFE().max_gas_price();
        let mut suggested_gas_price_calculator = SuggestedGasPriceCalculator::new(
            Some(max_execution_duration_per_commit),
            min_free_execution_slot_assigned,
            REFERENCE_GAS_PRICE,
            max_gas_price,
        );

        let object_1 = ObjectID::random();
        let object_2 = ObjectID::random();

        // Construct a certificate that touches the following shared objects:
        // - `object_1` by mutable reference,
        // - `object_2` by immutable reference.
        let objects_1 = vec![(object_1, true), (object_2, false)];
        let gas_budget_1 = 1_010_000;
        let gas_price_1 = 1_100;
        let certificate_1 = build_transaction(&objects_1, gas_budget_1, gas_price_1);
        let estimated_execution_duration_1 =
            shared_object_congestion_tracker.get_estimated_execution_duration(&certificate_1);
        // Try sequencing this certificate
        let shared_input_objects_1: Vec<_> = certificate_1.shared_input_objects().collect();
        shared_object_congestion_tracker.initialize_object_execution_slots(&shared_input_objects_1);
        let sequencing_result_1 = shared_object_congestion_tracker.try_schedule(
            &certificate_1,
            max_execution_duration_per_commit,
            &previously_deferred_tx_digests,
            commit_round,
        );
        // Shared object transactions allocations should look as follows:
        // |----------------------
        // | object 1 | object 2 |
        // |----------|----------|
        // |          |          |
        // |----------|----------|
        // |          |          |
        // |----------|----------|
        // | cert. 1  |          |
        // |----------------------
        if let SequencingResult::Schedule(start_time) = sequencing_result_1 {
            shared_object_congestion_tracker
                .bump_object_execution_slots(&certificate_1, start_time);
            suggested_gas_price_calculator
                .update_congestion_info(&certificate_1, estimated_execution_duration_1);
        } else {
            panic!("Certificate 1 must be scheduled");
        }

        // Construct a certificate that touches the following shared objects:
        // - `object_1` by immutable reference,
        // - `object_2` by mutable reference.
        let objects_2 = vec![(object_1, false), (object_2, true)];
        let gas_budget_2 = 1_010_000;
        let gas_price_2 = 1_008;
        let certificate_2 = build_transaction(&objects_2, gas_budget_2, gas_price_2);
        let estimated_execution_duration_2 =
            shared_object_congestion_tracker.get_estimated_execution_duration(&certificate_2);
        // Try sequencing this certificate
        let shared_input_objects_2: Vec<_> = certificate_2.shared_input_objects().collect();
        shared_object_congestion_tracker.initialize_object_execution_slots(&shared_input_objects_2);
        let sequencing_result_2 = shared_object_congestion_tracker.try_schedule(
            &certificate_2,
            max_execution_duration_per_commit,
            &previously_deferred_tx_digests,
            commit_round,
        );
        // Shared object transactions allocations should look as follows:
        // |----------------------
        // | object 1 | object 2 |
        // |----------|----------|
        // |          |          |
        // |----------|----------|
        // |          | cert. 2  |
        // |----------|----------|
        // | cert. 1  |          |
        // |----------------------
        if let SequencingResult::Schedule(start_time) = sequencing_result_2 {
            shared_object_congestion_tracker
                .bump_object_execution_slots(&certificate_2, start_time);
            suggested_gas_price_calculator
                .update_congestion_info(&certificate_2, estimated_execution_duration_2);
        } else {
            panic!("Certificate 2 must be scheduled");
        }

        // Construct a certificate that touches the following shared objects:
        // - `object_1` by immutable reference,
        // - `object_2` by mutable reference.
        let objects_3 = vec![(object_1, false), (object_2, true)];
        let gas_budget_3 = 1_010_000;
        let gas_price_3 = 1_006;
        let certificate_3 = build_transaction(&objects_3, gas_budget_3, gas_price_3);
        let estimated_execution_duration_3 =
            shared_object_congestion_tracker.get_estimated_execution_duration(&certificate_3);
        // Try sequencing this certificate
        let shared_input_objects_3: Vec<_> = certificate_3.shared_input_objects().collect();
        shared_object_congestion_tracker.initialize_object_execution_slots(&shared_input_objects_3);
        let sequencing_result_3 = shared_object_congestion_tracker.try_schedule(
            &certificate_3,
            max_execution_duration_per_commit,
            &previously_deferred_tx_digests,
            commit_round,
        );
        // Shared object transactions allocations should look as follows:
        // |----------------------
        // | object 1 | object 2 |
        // |----------|----------|
        // |          | cert. 3  |
        // |----------|----------|
        // |          | cert. 2  |
        // |----------|----------|
        // | cert. 1  |          |
        // |----------------------
        if let SequencingResult::Schedule(start_time) = sequencing_result_3 {
            shared_object_congestion_tracker
                .bump_object_execution_slots(&certificate_3, start_time);
            suggested_gas_price_calculator
                .update_congestion_info(&certificate_3, estimated_execution_duration_3);
        } else {
            panic!("Certificate 3 must be scheduled");
        }

        // Construct a certificate that touches the following shared objects:
        // - `object_2` by mutable reference.
        let objects_4 = vec![(object_2, true)];
        let gas_budget_4 = 1_010_000;
        let gas_price_4 = 1_002;
        let certificate_4 = build_transaction(&objects_4, gas_budget_4, gas_price_4);
        let estimated_execution_duration_4 =
            shared_object_congestion_tracker.get_estimated_execution_duration(&certificate_4);
        // Try sequencing this certificate
        let shared_input_objects_4: Vec<_> = certificate_4.shared_input_objects().collect();
        shared_object_congestion_tracker.initialize_object_execution_slots(&shared_input_objects_4);
        let sequencing_result_4 = shared_object_congestion_tracker.try_schedule(
            &certificate_4,
            max_execution_duration_per_commit,
            &previously_deferred_tx_digests,
            commit_round,
        );
        // If `min_free_execution_slot_assigned = true`, shared object transactions
        // allocations should look as follows:
        // |----------------------
        // | object 1 | object 2 |
        // |----------|----------|
        // |          | cert. 3  |
        // |----------|----------|
        // |          | cert. 2  |
        // |----------|----------|
        // | cert. 1  | cert. 4  |
        // |----------------------
        // If `min_free_execution_slot_assigned = false`, this certificate must be
        // deferred.
        if min_free_execution_slot_assigned {
            // ^ this corresponds the new sequencer's logic
            if let SequencingResult::Schedule(start_time) = sequencing_result_4 {
                shared_object_congestion_tracker
                    .bump_object_execution_slots(&certificate_4, start_time);
                suggested_gas_price_calculator
                    .update_congestion_info(&certificate_4, estimated_execution_duration_4);
            } else {
                panic!("Certificate 4 must be scheduled in the new sequencer");
            }
        } else {
            // ^ this corresponds the old sequencer's logic
            if let SequencingResult::Defer(_key, congested_objects) = sequencing_result_4 {
                assert_eq!(congested_objects, vec![object_2]);
                let suggested_gas_price = suggested_gas_price_calculator
                    .calculate_suggested_gas_price(&certificate_4, estimated_execution_duration_4);
                assert_eq!(suggested_gas_price, REFERENCE_GAS_PRICE);
            } else {
                panic!("Certificate 4 must be deferred in the old sequencer");
            }
        }

        // Construct a certificate that touches the following shared objects:
        // - `object_1` by mutable reference,
        // - `object_2` by mutable reference.
        let objects_5 = vec![(object_1, true), (object_2, true)];
        let gas_budget_5 = 1_010_000;
        let gas_price_5 = 1_000;
        let certificate_5 = build_transaction(&objects_5, gas_budget_5, gas_price_5);
        let estimated_execution_duration_5 =
            shared_object_congestion_tracker.get_estimated_execution_duration(&certificate_5);
        // Try sequencing this certificate
        let shared_input_objects_5: Vec<_> = certificate_5.shared_input_objects().collect();
        shared_object_congestion_tracker.initialize_object_execution_slots(&shared_input_objects_5);
        let sequencing_result_5 = shared_object_congestion_tracker.try_schedule(
            &certificate_5,
            max_execution_duration_per_commit,
            &previously_deferred_tx_digests,
            commit_round,
        );
        // Shared object transactions allocations should look as follows:
        // |----------------------
        // | object 1 | object 2 |
        // |----------|----------|
        // |          | cert. 3  |
        // |----------|----------|
        // |          | cert. 2  |
        // |----------|----------|
        // | cert. 1  | cert. 4  |
        // |----------------------
        // That is, this certificate must be deferred in both new and old sequencers.
        if let SequencingResult::Defer(_key, congested_objects) = sequencing_result_5 {
            let suggested_gas_price = suggested_gas_price_calculator
                .calculate_suggested_gas_price(&certificate_5, estimated_execution_duration_5);

            if min_free_execution_slot_assigned {
                // ^ this corresponds the new sequencer's logic
                assert_eq!(
                    congested_objects,
                    objects_5.into_iter().map(|(id, _)| id).collect::<Vec<_>>()
                );
                assert_eq!(
                    suggested_gas_price,
                    // FIX: suggested gas price must be gas_price_3 + 1!
                    gas_price_4 + 1,
                );
            } else {
                // ^ this corresponds the old sequencer's logic
                assert_eq!(congested_objects, vec![object_2]);
                assert_eq!(suggested_gas_price, REFERENCE_GAS_PRICE);
            }
        } else {
            panic!("Certificate 5 must be deferred");
        }

        // Construct a certificate that touches the following shared objects:
        // - `object_1` by mutable reference.
        let objects_6 = vec![(object_1, true)];
        let gas_budget_6 = 1_010_000;
        let gas_price_6 = 1_000;
        let certificate_6 = build_transaction(&objects_6, gas_budget_6, gas_price_6);
        let estimated_execution_duration_6 =
            shared_object_congestion_tracker.get_estimated_execution_duration(&certificate_6);
        // Try sequencing this certificate
        let shared_input_objects_6: Vec<_> = certificate_6.shared_input_objects().collect();
        shared_object_congestion_tracker.initialize_object_execution_slots(&shared_input_objects_6);
        let sequencing_result_6 = shared_object_congestion_tracker.try_schedule(
            &certificate_6,
            max_execution_duration_per_commit,
            &previously_deferred_tx_digests,
            commit_round,
        );
        // Shared object transactions allocations should look as follows:
        // |----------------------
        // | object 1 | object 2 |
        // |----------|----------|
        // |          | cert. 3  |
        // |----------|----------|
        // | cert. 6  | cert. 2  |
        // |----------|----------|
        // | cert. 1  | cert. 4  |
        // |----------------------
        if let SequencingResult::Schedule(start_time) = sequencing_result_6 {
            shared_object_congestion_tracker
                .bump_object_execution_slots(&certificate_6, start_time);
            suggested_gas_price_calculator
                .update_congestion_info(&certificate_6, estimated_execution_duration_6);
        } else {
            panic!("Certificate 6 must be scheduled");
        }

        // Construct a certificate that touches the following shared objects:
        // - `object_1` by mutable reference.
        let objects_7 = vec![(object_1, true)];
        let gas_budget_7 = 1_010_000;
        let gas_price_7 = 1_000;
        let certificate_7 = build_transaction(&objects_7, gas_budget_7, gas_price_7);
        let estimated_execution_duration_7 =
            shared_object_congestion_tracker.get_estimated_execution_duration(&certificate_7);
        // Try sequencing this certificate
        let shared_input_objects_7: Vec<_> = certificate_7.shared_input_objects().collect();
        shared_object_congestion_tracker.initialize_object_execution_slots(&shared_input_objects_7);
        let sequencing_result_7 = shared_object_congestion_tracker.try_schedule(
            &certificate_7,
            max_execution_duration_per_commit,
            &previously_deferred_tx_digests,
            commit_round,
        );
        // Shared object transactions allocations should look as follows:
        // |----------------------
        // | object 1 | object 2 |
        // |----------|----------|
        // | cert. 7  | cert. 3  |
        // |----------|----------|
        // | cert. 6  | cert. 2  |
        // |----------|----------|
        // | cert. 1  | cert. 4  |
        // |----------------------
        if let SequencingResult::Schedule(start_time) = sequencing_result_7 {
            shared_object_congestion_tracker
                .bump_object_execution_slots(&certificate_7, start_time);
            suggested_gas_price_calculator
                .update_congestion_info(&certificate_7, estimated_execution_duration_7);
        } else {
            panic!("Certificate 7 must be scheduled");
        }
    }
}
