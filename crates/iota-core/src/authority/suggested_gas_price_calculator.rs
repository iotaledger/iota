// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::{BTreeMap, HashMap};

use iota_types::{base_types::ObjectID, executable_transaction::VerifiedExecutableTransaction};
use tracing::instrument;

use super::shared_object_congestion_tracker::ExecutionTime;
use crate::authority::{
    authority_per_epoch_store::CongestionControlParameters,
    shared_object_congestion_tracker::BumpObjectExecutionSlotsResult,
};

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
    /// with `gas_price` and `estimated_execution_duration`.
    fn new(gas_price: u64, estimated_execution_duration: ExecutionTime) -> Self {
        Self {
            gas_price,
            estimated_execution_duration,
        }
    }
}

/// Holds shared object congestion info for a single shared object,
/// keyed by transaction execution start time.
type PerObjectCongestionInfo = BTreeMap<ExecutionTime, ScheduledTransactionCongestionInfo>;

/// Holds shared object congestion data for a single consensus commit round.
type PerCommitCongestionInfo = HashMap<ObjectID, PerObjectCongestionInfo>;

/// `SuggestedGasPriceCalculator` calculates suggested gas prices for
/// deferred/cancelled shared-object transactions, using congestion
/// info from a single consensus commit.
///
/// The congestion info stored by the calculator should only be updated
/// for scheduled certificates. In contrast, calculations of the suggested
/// gas price should only be invoked for deferred/cancelled certificates.
///
/// Roughly speaking, the suggested gas price calculator works as follows:
/// 1. For every scheduled certificate, obtain its reference gas price,
///    execution start time and estimated execution duration.
/// 2. For every input shared object accessed mutably by the scheduled
///    transaction, keep and update a map, ordered by execution start time
///    (key), whose values store scheduled certificate's gas price and estimated
///    execution duration.
/// 3. For every deferred/cancelled certificate, obtain its estimated execution
///    duration, as well as all input shared objects.
/// 4. Calculate a suggested gas price for the deferred/cancelled certificate as
///    follows:
///    - compute its (imaginary) execution start time as congestion limit per
///      commit minus its estimated execution duration;
///    - for each input shared object, get the maximum gas price over scheduled
///      certificates whose end execution time is larger than our imaginary
///      start time;
///    - take the maximum over the values obtained in the previous step;
///    - the suggested gas price equals the maximum value obtained in the
///      previous step plus 1, but such that it does not become larger than the
///      maximum gas price set in the protocol.
///
/// Note that if shared-object congestion control is disabled, the calculator
/// will suggest the reference gas price.
#[derive(Debug)]
pub(crate) struct SuggestedGasPriceCalculator {
    /// Per-commit congestion info.
    congestion_info: PerCommitCongestionInfo,

    /// A set of congestion control parameters.
    congestion_control_parameters: CongestionControlParameters,

    /// The reference gas price, which will be suggested if
    /// shared-object congestion control is disabled.
    reference_gas_price: u64,
}

impl SuggestedGasPriceCalculator {
    /// Create a new `SuggestedGasPriceCalculator` with empty shared
    /// object congestion data.
    pub(super) fn new(
        congestion_control_parameters: CongestionControlParameters,
        reference_gas_price: u64,
    ) -> Self {
        Self {
            congestion_info: PerCommitCongestionInfo::new(),
            congestion_control_parameters,
            reference_gas_price,
        }
    }

    /// Create a new `SuggestedGasPriceCalculator` with empty shared
    /// object congestion data for testing.
    #[cfg(test)]
    fn new_for_test(
        congestion_control_parameters: CongestionControlParameters,
        reference_gas_price: u64,
    ) -> Self {
        Self {
            congestion_info: PerCommitCongestionInfo::new(),
            congestion_control_parameters,
            reference_gas_price,
        }
    }

    /// Update per-commit congestion info for a single certificate. This should
    /// only be called for scheduled certificates that contain shared object(s);
    /// otherwise, the calculator might wrongly calculate suggested gas price.
    /// `bump_object_execution_slots_result` is the outcome of the
    /// [`bump_object_execution_slots`] of `SharedObjectCongestionTracker`.
    pub(super) fn update_congestion_info(
        &mut self,
        bump_object_execution_slots_result: Option<BumpObjectExecutionSlotsResult>,
    ) {
        // If we don't have a `BumpObjectExecutionSlotsResult`, we don't need
        // to update the congestion info.
        if let Some(res) = bump_object_execution_slots_result {
            let scheduled_transaction_congestion_info = ScheduledTransactionCongestionInfo::new(
                res.gas_price(),
                res.estimated_execution_duration(),
            );

            for obj_id in res.object_ids() {
                let prev_info = self.congestion_info.entry(*obj_id).or_default().insert(
                    res.execution_start_time(),
                    scheduled_transaction_congestion_info,
                );

                if self
                    .congestion_control_parameters
                    .use_separate_gas_price_feedback_mechanism_for_randomness()
                {
                    // The sequencer should not schedule multiple transactions with the same
                    // execution start time for the same shared object, but just in case,
                    // check this during development/testing. Note that we only have this
                    // check if `use_separate_gas_price_feedback_mechanism_for_randomness`
                    // if `true`, since otherwise it is quite possible to have a `prev_info`
                    // when a single `SuggestedGasPriceCalculator` is used for regular
                    // shared-object transactions and transactions using randomness.
                    debug_assert!(
                        prev_info.is_none(),
                        "Multiple transactions were scheduled at the same execution start time {} \
                        for the same shared object {obj_id:?}",
                        res.execution_start_time(),
                    );
                }
            }
        }
    }

    /// Calculate a suggested gas price for a deferred/cancelled `certificate`
    /// using the single-commit congestion info held by the calculator. This
    /// should only be called for certificates deferred/cancelled due to
    /// shared object congestion; otherwise, there is a risk of panic.
    #[instrument(level = "trace", skip_all)]
    pub(super) fn calculate_suggested_gas_price(
        &self,
        certificate: &VerifiedExecutableTransaction,
    ) -> u64 {
        if let Some(congestion_limit_per_commit) = self.get_effective_congestion_limit_per_commit()
        {
            let clearing_gas_price =
                self.find_clearing_gas_price(certificate, congestion_limit_per_commit);

            // Suggested gas price equals `clearing_gas_price + 1`. We add 1 to make this
            // transaction would be scheduled if the same commit structure was repeated.
            let suggested_gas_price =
                clearing_gas_price.map_or(self.reference_gas_price, |p| p.saturating_add(1));

            // Make sure suggested gas price is not larger than the maximum possible gas
            // price.
            suggested_gas_price.min(self.congestion_control_parameters.max_gas_price())
        } else {
            // ^ If we don't have congestion limit per commit, suggest the reference gas
            // price.

            self.reference_gas_price
        }
    }

    /// Get effective congestion limit per commit: that is, this will take
    /// overshoot into account if the calculator uses the overshoot feature.
    /// If the overshoot feature is not used in the calculator, this will
    /// return maximum execution duration per commit (i.e., without overshoot).
    /// Returns `None` if shared-object congestion control is disabled.
    fn get_effective_congestion_limit_per_commit(&self) -> Option<ExecutionTime> {
        if self
            .congestion_control_parameters
            .use_congestion_limit_overshoot_in_gas_price_feedback_mechanism()
        {
            self.congestion_control_parameters
                .get_effective_congestion_limit_per_commit()
        } else {
            self.congestion_control_parameters
                .max_execution_duration_per_commit()
        }
    }

    /// Find the gas price for which a deferred/scheduled certificate would be
    /// scheduled if (i) that gas price was paid, and (ii) if exactly the same
    /// set of transactions appeared in a commit.
    fn find_clearing_gas_price(
        &self,
        certificate: &VerifiedExecutableTransaction,
        congestion_limit_per_commit: ExecutionTime,
    ) -> Option<u64> {
        // Imaginary start time of the deferred/cancelled certificate. We consider
        // only the highest possible (but sufficient for scheduling) start time as
        // it is very likely that scheduled certificates with lower gas prices
        // appear have higher start times. If a transaction with its estimated
        // execution duration cannot fit within `congestion_limit_per_commit`,
        // set its imaginary start time to 0.
        let start_time_of_deferred_cert = congestion_limit_per_commit.saturating_sub(
            self.congestion_control_parameters
                .get_estimated_execution_duration(certificate),
        );

        certificate
            .shared_input_objects()
            .into_iter()
            .filter_map(|object| {
                self.congestion_info
                    .get(&object.id)
                    .and_then(|per_object_congestion_info| {
                        per_object_congestion_info
                            .iter()
                            .filter_map(|(execution_start_time, tx_congestion_info)| {
                                let end_time_of_scheduled_cert = execution_start_time
                                    .saturating_add(
                                        tx_congestion_info.estimated_execution_duration,
                                    );

                                if end_time_of_scheduled_cert > start_time_of_deferred_cert {
                                    // Store gas price of that scheduled certificate
                                    Some(tx_congestion_info.gas_price)
                                } else {
                                    None
                                }
                            })
                            // Take the maximum over all found gas prices of scheduled certificates
                            // whose execution end time is larger than the imaginary start time
                            // of the deferred/cancelled transaction. It has to be maximum here
                            // since otherwise the suggested gas price will be insufficient to
                            // guarantee scheduling if the same set of certificates was repeated
                            // again in a commit.
                            .max()
                    })
            })
            // Take the maximum over all input shared objects, as we need to consider the
            // "worst-case" (most congested) object; otherwise, the suggested gas price
            // will be insufficient to guarantee scheduling if the same set of certificates
            // was repeated again in a commit.
            .max()
    }
}

#[cfg(test)]
pub mod suggested_gas_price_calculator_test_utils {
    use iota_protocol_config::PerObjectCongestionControlMode;
    use iota_types::base_types::ObjectID;

    use super::SuggestedGasPriceCalculator;
    use crate::authority::{
        authority_per_epoch_store::CongestionControlParameters,
        shared_object_congestion_tracker::{
            ExecutionTime, SharedObjectCongestionTracker,
            shared_object_test_utils::{
                build_transaction, initialize_tracker_and_compute_tx_start_time,
            },
        },
    };

    pub(crate) fn new_suggested_gas_price_calculator_with_initial_values_for_test(
        init_values: &[(ObjectID, ExecutionTime, u64)],
        congestion_control_parameters: CongestionControlParameters,
        reference_gas_price: u64,
    ) -> SuggestedGasPriceCalculator {
        let mut shared_object_congestion_tracker =
            SharedObjectCongestionTracker::new(vec![], congestion_control_parameters.clone());

        let mut suggested_gas_price_calculator = SuggestedGasPriceCalculator::new(
            congestion_control_parameters.clone(),
            reference_gas_price,
        );

        for (object_id, duration, gas_price) in init_values {
            match congestion_control_parameters.per_object_congestion_control_mode_for_test() {
                PerObjectCongestionControlMode::None => {}
                PerObjectCongestionControlMode::TotalGasBudget => {
                    let certificate =
                        build_transaction(&[(*object_id, true)], *duration, *gas_price);

                    let execution_start_time = initialize_tracker_and_compute_tx_start_time(
                        &mut shared_object_congestion_tracker,
                        &certificate.shared_input_objects(),
                        *duration,
                    )
                    .expect(
                        "initial value should fit within the available range of slots in the \
                                tracker",
                    );

                    let bump_result = shared_object_congestion_tracker
                        .bump_object_execution_slots(&certificate, execution_start_time);

                    suggested_gas_price_calculator.update_congestion_info(bump_result);
                }
                PerObjectCongestionControlMode::TotalTxCount => {
                    let tx_duration = 1; // since this is TotalTxCount mode
                    for _ in 0..*duration {
                        let certificate =
                            build_transaction(&[(*object_id, true)], tx_duration, *gas_price);

                        let execution_start_time = initialize_tracker_and_compute_tx_start_time(
                            &mut shared_object_congestion_tracker,
                            &certificate.shared_input_objects(),
                            tx_duration,
                        )
                        .expect(
                            "initial value should fit within the available range of slots in \
                                    the tracker",
                        );

                        let bump_result = shared_object_congestion_tracker
                            .bump_object_execution_slots(&certificate, execution_start_time);

                        suggested_gas_price_calculator.update_congestion_info(bump_result);
                    }
                }
            }
        }

        suggested_gas_price_calculator
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use iota_protocol_config::{PerObjectCongestionControlMode, ProtocolConfig};
    use iota_types::{base_types::ObjectID, executable_transaction::VerifiedExecutableTransaction};
    use rstest::rstest;

    use super::SuggestedGasPriceCalculator;
    use crate::authority::{
        authority_per_epoch_store::CongestionControlParameters,
        shared_object_congestion_tracker::{
            BumpObjectExecutionSlotsResult, ExecutionTime, SequencingResult,
            SharedObjectCongestionTracker, shared_object_test_utils::build_transaction,
        },
        suggested_gas_price_calculator::{
            PerCommitCongestionInfo, PerObjectCongestionInfo, ScheduledTransactionCongestionInfo,
        },
    };

    const REFERENCE_GAS_PRICE: u64 = 1_000;

    /// Helper data structure to store transaction data used for sequencing.
    #[derive(Debug)]
    struct TransactionData {
        /// Index of transaction in the set ordered by gas price in
        /// descending order. Used for debugging purposes.
        order_idx: usize,
        gas_price: u64,
        gas_budget: u64,
        input_shared_objects: Vec<(ObjectID, /* mutability */ bool)>,
    }

    /// Build a set of `TransactionData` with two shared objects for tests.
    fn build_transactions_data_for_test(
        maxgp: u64,
        object_1: ObjectID,
        object_2: ObjectID,
    ) -> Vec<TransactionData> {
        [
            // (gas price, gas budget, input shared objects)
            (maxgp, 3_000_000, vec![(object_1, true), (object_2, false)]), //  0
            (9_000, 1_000_000, vec![(object_1, false), (object_2, true)]), //  1
            (8_000, 4_000_000, vec![(object_1, false), (object_2, true)]), //  2
            (7_000, 2_000_000, vec![(object_2, true)]),                    //  3
            (7_000, 1_000_001, vec![(object_2, false)]),                   //  4
            (7_000, 5_000_000, vec![(object_2, true)]),                    //  5
            (7_000, 5_000_001, vec![(object_1, true), (object_2, true)]),  //  6
            (7_000, 8_000_000, vec![(object_1, true), (object_2, true)]),  //  7
            (6_000, 4_000_000, vec![(object_1, true)]),                    //  8
            (5_000, 2_000_000, vec![(object_1, true)]),                    //  9
            (5_000, 1_000_001, vec![(object_1, false), (object_2, false)]), // 10
            (5_000, 5_000_001, vec![(object_1, true), (object_2, false)]), //  11
            (5_000, 9_000_000, vec![(object_1, false), (object_2, true)]), //  12
        ]
        .into_iter()
        .enumerate()
        .map(|(idx, (price, budget, objects))| TransactionData {
            order_idx: idx,
            gas_price: price,
            gas_budget: budget,
            input_shared_objects: objects,
        })
        .collect()
    }

    /// Helper function for tests to build a certificate with `tx_data` and
    /// then try sequencing it by `shared_object_congestion_tracker`.
    /// Returns the certificate itself and a result of its sequencing.
    fn build_and_try_sequencing_certificate(
        tx_data: &TransactionData,
        shared_object_congestion_tracker: &mut SharedObjectCongestionTracker,
    ) -> (VerifiedExecutableTransaction, SequencingResult) {
        let certificate = build_transaction(
            &tx_data.input_shared_objects,
            tx_data.gas_budget,
            tx_data.gas_price,
        );
        let shared_input_objects = certificate.shared_input_objects();
        shared_object_congestion_tracker.initialize_object_execution_slots(&shared_input_objects);

        let sequencing_result = shared_object_congestion_tracker.try_schedule(
            &certificate,
            // The remaining inputs are not important for these tests
            &HashMap::new(),
            0,
        );

        (certificate, sequencing_result)
    }

    /// Helper function for tests to update data internally stored by
    /// `shared_object_congestion_tracker` and `suggested_gas_price_calculator`
    /// for a `certificate` scheduled at `execution_start_time`.
    fn update_data_for_scheduled_certificate(
        certificate: &VerifiedExecutableTransaction,
        execution_start_time: ExecutionTime,
        shared_object_congestion_tracker: &mut SharedObjectCongestionTracker,
        suggested_gas_price_calculator: &mut SuggestedGasPriceCalculator,
    ) {
        let bump_result = shared_object_congestion_tracker
            .bump_object_execution_slots(certificate, execution_start_time);
        suggested_gas_price_calculator.update_congestion_info(bump_result);
    }

    /// Helper function to test if a certificate with and `tx_data` is
    /// scheduled. Returns execution start time of the certificate if
    /// it is scheduled, otherwise returns `None`.
    fn try_schedule(
        tx_data: &TransactionData,
        shared_object_congestion_tracker: &mut SharedObjectCongestionTracker,
        suggested_gas_price_calculator: &mut SuggestedGasPriceCalculator,
    ) -> Option<ExecutionTime> {
        let (certificate, sequencing_result) =
            build_and_try_sequencing_certificate(tx_data, shared_object_congestion_tracker);
        if let SequencingResult::Schedule(execution_start_time) = sequencing_result {
            update_data_for_scheduled_certificate(
                &certificate,
                execution_start_time,
                shared_object_congestion_tracker,
                suggested_gas_price_calculator,
            );

            Some(execution_start_time)
        } else {
            None
        }
    }

    /// Helper function to test if a certificate with and `tx_data` is
    /// deferred. Returns congested objects and suggested gas price if
    /// the certificate is deferred, otherwise returns `None`.
    fn try_defer(
        tx_data: &TransactionData,
        shared_object_congestion_tracker: &mut SharedObjectCongestionTracker,
        suggested_gas_price_calculator: &mut SuggestedGasPriceCalculator,
    ) -> Option<(Vec<ObjectID>, u64)> {
        let (certificate, sequencing_result) =
            build_and_try_sequencing_certificate(tx_data, shared_object_congestion_tracker);
        if let SequencingResult::Defer(_key, congested_objects) = sequencing_result {
            Some((
                congested_objects,
                suggested_gas_price_calculator.calculate_suggested_gas_price(&certificate),
            ))
        } else {
            None
        }
    }

    #[rstest]
    fn update_congestion_info(
        #[values(
            None,
            Some(10), // the value is not important in this test
        )]
        max_execution_duration_per_commit: Option<ExecutionTime>,
    ) {
        let mut suggested_gas_price_calculator = SuggestedGasPriceCalculator::new_for_test(
            // NOTE: congestion control parameters (except `max_execution_duration_per_commit`)
            // are not important in this test
            CongestionControlParameters::new_for_test(
                PerObjectCongestionControlMode::TotalTxCount,
                false,
                max_execution_duration_per_commit,
                None,
                ProtocolConfig::get_for_max_version_UNSAFE().max_gas_price(),
                false,
                true,
            ),
            REFERENCE_GAS_PRICE,
        );

        let object_1 = ObjectID::random();
        let object_2 = ObjectID::random();
        let object_3 = ObjectID::random();
        let object_4 = ObjectID::random();
        let object_5 = ObjectID::random();

        // Construct the first certificate that touches shared objects:
        // - `object_1` by mutable reference,
        // - `object_2` by immutable reference.
        let objects_1 = [(object_1, true), (object_2, false)];
        let gas_price_1 = 1_003;
        let execution_start_time_1 = 0;
        let estimated_execution_duration_1 = 3;
        // Update the calculator's congestion info for this certificate.
        suggested_gas_price_calculator.update_congestion_info(
            max_execution_duration_per_commit.map(|_| {
                BumpObjectExecutionSlotsResult::new_for_test(
                    objects_1
                        .iter()
                        .filter_map(|(obj_id, mutable)| mutable.then_some(*obj_id))
                        .collect(),
                    execution_start_time_1,
                    estimated_execution_duration_1,
                    gas_price_1,
                )
            }),
        );
        //
        if let Some(_max_execution_duration_per_commit) = max_execution_duration_per_commit {
            // Note that `object_2` should not appear because it is accessed immutably.
            let object_1_expected_congestion_info = PerObjectCongestionInfo::from([(
                execution_start_time_1,
                ScheduledTransactionCongestionInfo::new(
                    gas_price_1,
                    estimated_execution_duration_1,
                ),
            )]);
            assert_eq!(
                suggested_gas_price_calculator.congestion_info,
                PerCommitCongestionInfo::from([(object_1, object_1_expected_congestion_info)]),
            );
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
        let objects_2 = [(object_2, true), (object_3, false), (object_4, true)];
        let gas_price_2 = 1_002;
        let execution_start_time_2 = 1;
        let estimated_execution_duration_2 = 2;
        // Update the calculator's congestion info for this certificate.
        suggested_gas_price_calculator.update_congestion_info(
            max_execution_duration_per_commit.map(|_| {
                BumpObjectExecutionSlotsResult::new_for_test(
                    objects_2
                        .iter()
                        .filter_map(|(obj_id, mutable)| mutable.then_some(*obj_id))
                        .collect(),
                    execution_start_time_2,
                    estimated_execution_duration_2,
                    gas_price_2,
                )
            }),
        );
        //
        if let Some(_max_execution_duration_per_commit) = max_execution_duration_per_commit {
            // Note that `object_3` should not appear because it is accessed immutably.
            let object_1_expected_congestion_info = PerObjectCongestionInfo::from([(
                execution_start_time_1,
                ScheduledTransactionCongestionInfo::new(
                    gas_price_1,
                    estimated_execution_duration_1,
                ),
            )]);
            let object_2_expected_congestion_info = PerObjectCongestionInfo::from([(
                execution_start_time_2,
                ScheduledTransactionCongestionInfo::new(
                    gas_price_2,
                    estimated_execution_duration_2,
                ),
            )]);
            let object_4_expected_congestion_info = PerObjectCongestionInfo::from([(
                execution_start_time_2,
                ScheduledTransactionCongestionInfo::new(
                    gas_price_2,
                    estimated_execution_duration_2,
                ),
            )]);
            assert_eq!(
                suggested_gas_price_calculator.congestion_info,
                PerCommitCongestionInfo::from([
                    (object_1, object_1_expected_congestion_info),
                    (object_2, object_2_expected_congestion_info),
                    (object_4, object_4_expected_congestion_info),
                ]),
            );
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
        let objects_3 = [(object_4, false), (object_5, true)];
        let gas_price_3 = 1_001;
        let execution_start_time_3 = 2;
        let estimated_execution_duration_3 = 1;
        // Update the calculator's congestion info for this certificate.
        suggested_gas_price_calculator.update_congestion_info(
            max_execution_duration_per_commit.map(|_| {
                BumpObjectExecutionSlotsResult::new_for_test(
                    objects_3
                        .iter()
                        .filter_map(|(obj_id, mutable)| mutable.then_some(*obj_id))
                        .collect(),
                    execution_start_time_3,
                    estimated_execution_duration_3,
                    gas_price_3,
                )
            }),
        );
        //
        if let Some(_max_execution_duration_per_commit) = max_execution_duration_per_commit {
            // Note that `object_3` should not appear because it is accessed immutably.
            let object_1_expected_congestion_info = PerObjectCongestionInfo::from([(
                execution_start_time_1,
                ScheduledTransactionCongestionInfo::new(
                    gas_price_1,
                    estimated_execution_duration_1,
                ),
            )]);
            let object_2_expected_congestion_info = PerObjectCongestionInfo::from([(
                execution_start_time_2,
                ScheduledTransactionCongestionInfo::new(
                    gas_price_2,
                    estimated_execution_duration_2,
                ),
            )]);
            let object_4_expected_congestion_info = PerObjectCongestionInfo::from([(
                execution_start_time_2,
                ScheduledTransactionCongestionInfo::new(
                    gas_price_2,
                    estimated_execution_duration_2,
                ),
            )]);
            let object_5_expected_congestion_info = PerObjectCongestionInfo::from([(
                execution_start_time_3,
                ScheduledTransactionCongestionInfo::new(
                    gas_price_3,
                    estimated_execution_duration_3,
                ),
            )]);
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
            // We don't have max execution duration per commit, so there is no need
            // in updating the calculator's congestion info.
            assert_eq!(
                suggested_gas_price_calculator.congestion_info,
                PerCommitCongestionInfo::new()
            );
        }
    }

    // Test `SuggestedGasPriceCalculator::calculate_suggested_gas_price`
    // in the `PerObjectCongestionControlMode::TotalTxCount` mode without
    // congestion limit overshoot.
    #[rstest]
    fn calculate_suggested_gas_price_in_tx_count_mode_without_overshoot(
        #[values(false, true)] assign_min_free_exec_slot: bool,
        // Whether to use congestion limit overshoot in the gas price feedback
        // mechanism, i.e., this is only used in `SuggestedGasPriceCalculator`.
        // This is used to test that `SuggestedGasPriceCalculator` behaves in
        // the same way regardless of `use_congestion_limit_overshoot` values
        // if `max_congestion_limit_overshoot_per_commit` is `None`.
        #[values(false, true)] use_congestion_limit_overshoot: bool,
    ) {
        let object_1 = ObjectID::random();
        let object_2 = ObjectID::random();

        // Congestion control and other parameters used in
        // `SharedObjectCongestionTracker` and `SuggestedGasPriceCalculator`
        let max_gas_price = ProtocolConfig::get_for_max_version_UNSAFE().max_gas_price();
        let congestion_control_parameters = CongestionControlParameters::new_for_test(
            PerObjectCongestionControlMode::TotalTxCount,
            assign_min_free_exec_slot,
            Some(3),       // max_execution_duration_per_commit
            None,          // max_congestion_limit_overshoot_per_commit
            max_gas_price, // max_gas_price
            use_congestion_limit_overshoot,
            true,
        );

        // Initialize `SharedObjectCongestionTracker` and `SuggestedGasPriceCalculator`
        let mut shared_object_congestion_tracker = SharedObjectCongestionTracker::new(
            [], // initial_object_debts
            congestion_control_parameters.clone(),
        );
        let mut suggested_gas_price_calculator = SuggestedGasPriceCalculator::new_for_test(
            congestion_control_parameters,
            REFERENCE_GAS_PRICE,
        );

        // Create some data for transactions and process each for scheduling.
        let txs_data = build_transactions_data_for_test(max_gas_price, object_1, object_2);

        // Transactions
        // 0:  (100K, 3_000_000, [object_1: mut, object_2: imm]),
        // 1:  (9000, 1_000_000, [object_1: imm, object_2: mut]),
        // 2:  (8000, 4_000_000, [object_1: imm, object_2: mut])
        // should be scheduled, after which allocations of mutably
        // accessed shared objects being as follows:
        // |-------------------------------------|------------|
        // |     object_1     |     object_2     | start time |
        // |__________________|__________________|____________|
        // |------------------|------------------|---- 3 -----|
        // |                  | cert. 2 (g=8000) |            |
        // |                  |------------------|---- 2      |
        // |                  | cert. 1 (g=9000) |            |
        // |------------------|------------------|---- 1      |
        // | cert. 0 (g=100K) |                  |            |
        // |-------------------------------------|---- 0 -----|
        (0..=2).for_each(|i| {
            let tx_data = &txs_data[i];
            if let Some(execution_start_time) = try_schedule(
                tx_data,
                &mut shared_object_congestion_tracker,
                &mut suggested_gas_price_calculator,
            ) {
                assert_eq!(execution_start_time, i as u64);
            } else {
                panic!(
                    "Transaction {} must be scheduled:\n{tx_data:#?}",
                    tx_data.order_idx
                );
            }
        });

        // If `assign_min_free_exec_slot` is `true`, transaction
        // 3:  (7000, 2_000_000, [object_2: mut])
        // must be scheduled, in which case allocations of mutably
        // accessed shared object should look as follows:
        // |-------------------------------------|------------|
        // |     object_1     |     object_2     | start time |
        // |__________________|__________________|____________|
        // |------------------|------------------|---- 3 -----|
        // |                  | cert. 2 (g=8000) |            |
        // |                  |------------------|---- 2      |
        // |                  | cert. 1 (g=9000) |            |
        // |------------------|------------------|---- 1      |
        // | cert. 0 (g=100K) | cert. 3 (g=7000) |            |
        // |-------------------------------------|---- 0 -----|
        // If `assign_min_free_exec_slot` is `false`, transaction 3 must be deferred,
        // in which case object 2 must be labeled as congested and suggested gas price
        // must be equals to that of transaction 2 plus one.
        let tx_data = &txs_data[3];
        if assign_min_free_exec_slot {
            if let Some(execution_start_time) = try_schedule(
                tx_data,
                &mut shared_object_congestion_tracker,
                &mut suggested_gas_price_calculator,
            ) {
                assert_eq!(execution_start_time, 0);
            } else {
                panic!(
                    "Transaction {} must be scheduled:\n{tx_data:#?}",
                    tx_data.order_idx
                );
            }
        } else {
            let (congested_objects, suggested_gas_price) = try_defer(
                tx_data,
                &mut shared_object_congestion_tracker,
                &mut suggested_gas_price_calculator,
            )
            .unwrap_or_else(|| {
                panic!(
                    "Transaction {} must be deferred:\n{tx_data:#?}",
                    tx_data.order_idx
                );
            });
            assert_eq!(
                congested_objects,
                vec![object_2], // expected congested objects
                "Calculated congested objects do not match expected for transaction {}:\n{:#?}",
                tx_data.order_idx,
                tx_data,
            );
            assert_eq!(
                suggested_gas_price,
                txs_data[2].gas_price + 1, // expected suggested gas price
                "Calculated suggested gas price does not match expected for transaction {}:\n{:#?}",
                tx_data.order_idx,
                tx_data,
            );
        }

        // Transactions
        // 4:  (7000, 1_000_001, [object_2: imm]),
        // 5:  (7000, 5_000_000, [object_2: mut])
        // must be deferred, with object 2 being labeled congested and
        // suggested gas price being equal that of transaction 2 plus one.
        (4..=5).for_each(|i| {
            let tx_data = &txs_data[i];
            let (congested_objects, suggested_gas_price) = try_defer(
                tx_data,
                &mut shared_object_congestion_tracker,
                &mut suggested_gas_price_calculator,
            )
            .unwrap_or_else(|| {
                panic!(
                    "Transaction {} must be deferred:\n{tx_data:#?}",
                    tx_data.order_idx
                );
            });
            assert_eq!(
                congested_objects,
                vec![object_2], // expected congested objects
                "Calculated congested objects do not match expected for transaction {}:\n{:#?}",
                tx_data.order_idx,
                tx_data,
            );
            assert_eq!(
                suggested_gas_price,
                txs_data[2].gas_price + 1, // expected suggested gas price
                "Calculated suggested gas price does not match expected for transaction {}:\n{:#?}",
                tx_data.order_idx,
                tx_data,
            );
        });

        // Transactions
        // 6:  (7000, 5_000_001, [object_1: mut, object_2, mut]),
        // 7:  (7000, 8_000_000, [object_1: mut, object_2, mut])
        // must be deferred, with objects 1 and 2 being
        // labeled congested if `assign_min_free_exec_slot` is `true` and
        // object 2 if `assign_min_free_exec_slot` is false and suggested
        // gas price being equal that of transaction 2 plus one.
        (6..=7).for_each(|i| {
            let tx_data = &txs_data[i];
            let (congested_objects, suggested_gas_price) = try_defer(
                tx_data,
                &mut shared_object_congestion_tracker,
                &mut suggested_gas_price_calculator,
            )
            .unwrap_or_else(|| {
                panic!(
                    "Transaction {} must be deferred:\n{tx_data:#?}",
                    tx_data.order_idx
                );
            });
            assert_eq!(
                congested_objects,
                if assign_min_free_exec_slot {
                    vec![object_1, object_2]
                } else {
                    vec![object_2]
                }, // expected congested objects
                "Calculated congested objects do not match expected for transaction {}:\n{:#?}",
                tx_data.order_idx,
                tx_data,
            );
            assert_eq!(
                suggested_gas_price,
                txs_data[2].gas_price + 1, // expected suggested gas price
                "Calculated suggested gas price does not match expected for transaction {}:\n{:#?}",
                tx_data.order_idx,
                tx_data,
            );
        });

        // Transactions
        // 8:  (6000, 4_000_000, [object_1: mut]),
        // 9:  (5000, 2_000_000, [object_1: mut])
        // should be scheduled, after which allocations of mutably
        // accessed shared objects being as follows:
        // |-------------------------------------|------------|
        // |     object_1     |     object_2     | start time |
        // |__________________|__________________|____________|
        // |------------------|------------------|---- 3 -----|
        // | cert. 9 (g=5000) | cert. 2 (g=8000) |            |
        // |------------------|------------------|---- 2      |
        // | cert. 8 (g=6000) | cert. 1 (g=9000) |            |
        // |------------------|------------------|---- 1      |
        // | cert. 0 (g=100K) | cert. 3 (g=7000) |            |
        // |-------------------------------------|---- 0 -----|
        // NOTE: certificate 3 will only be scheduled if
        // `assign_min_free_exec_slot` is `true`.
        (8..=9).for_each(|i| {
            let tx_data = &txs_data[i];
            if let Some(execution_start_time) = try_schedule(
                tx_data,
                &mut shared_object_congestion_tracker,
                &mut suggested_gas_price_calculator,
            ) {
                assert_eq!(execution_start_time, i as u64 - 7);
            } else {
                panic!(
                    "Transaction {} must be scheduled:\n{tx_data:#?}",
                    tx_data.order_idx
                );
            }
        });

        // Transactions
        // 10: (5000, 1_000_001, [object_1: imm, object_2, imm]),
        // 11: (5000, 5_000_001, [object_1: mut, object_2, imm]),
        // 12: (5000, 9_000_000, [object_1: imm, object_2, mut])
        // must be deferred, with objects 1 and 2 being labeled congested
        // if `assign_min_free_exec_slot` is `true` and object 2 if
        // `assign_min_free_exec_slot` is false and suggested gas price
        // being equal that of transaction 2 plus one.
        (10..=12).for_each(|i| {
            let tx_data = &txs_data[i];
            let (congested_objects, suggested_gas_price) = try_defer(
                tx_data,
                &mut shared_object_congestion_tracker,
                &mut suggested_gas_price_calculator,
            )
            .unwrap_or_else(|| {
                panic!(
                    "Transaction {} must be deferred:\n{tx_data:#?}",
                    tx_data.order_idx
                );
            });
            assert_eq!(
                congested_objects,
                vec![object_1, object_2], // expected congested objects
                "Calculated congested objects do not match expected for transaction {}:\n{:#?}",
                tx_data.order_idx,
                tx_data,
            );
            assert_eq!(
                suggested_gas_price,
                txs_data[2].gas_price + 1, // expected suggested gas price
                "Calculated suggested gas price does not match expected for transaction {}:\n{:#?}",
                tx_data.order_idx,
                tx_data,
            );
        });
    }

    // Test `SuggestedGasPriceCalculator::calculate_suggested_gas_price`
    // in the `PerObjectCongestionControlMode::TotalGasBudget` mode mode
    // without congestion limit overshoot.
    #[rstest]
    fn calculate_suggested_gas_price_in_gas_budget_mode_without_overshoot(
        #[values(false, true)] assign_min_free_exec_slot: bool,
        // Whether to use congestion limit overshoot in the gas price feedback
        // mechanism, i.e., this is only used in `SuggestedGasPriceCalculator`.
        // This is used to test that `SuggestedGasPriceCalculator` behaves in
        // the same way regardless of `use_congestion_limit_overshoot` values
        // if `max_congestion_limit_overshoot_per_commit` is `None`.
        #[values(false, true)] use_congestion_limit_overshoot: bool,
    ) {
        let object_1 = ObjectID::random();
        let object_2 = ObjectID::random();

        // Congestion control and other parameters used in
        // `SharedObjectCongestionTracker` and `SuggestedGasPriceCalculator`
        let max_gas_price = ProtocolConfig::get_for_max_version_UNSAFE().max_gas_price();
        let congestion_control_parameters = CongestionControlParameters::new_for_test(
            PerObjectCongestionControlMode::TotalGasBudget,
            assign_min_free_exec_slot,
            Some(9_000_000), // max_execution_duration_per_commit
            None,            // max_congestion_limit_overshoot_per_commit
            max_gas_price,   // max_gas_price
            use_congestion_limit_overshoot,
            true,
        );

        // Initialize `SharedObjectCongestionTracker` and `SuggestedGasPriceCalculator`
        let mut shared_object_congestion_tracker = SharedObjectCongestionTracker::new(
            [], // initial_object_debts
            congestion_control_parameters.clone(),
        );
        let mut suggested_gas_price_calculator = SuggestedGasPriceCalculator::new_for_test(
            congestion_control_parameters,
            REFERENCE_GAS_PRICE,
        );

        // Create some data for transactions and process each for scheduling
        let txs_data = build_transactions_data_for_test(max_gas_price, object_1, object_2);

        // Transactions
        // 0:  (100K, 3_000_000, [object_1: mut, object_2: imm]),
        // 1:  (9000, 1_000_000, [object_1: imm, object_2: mut]),
        // 2:  (8000, 4_000_000, [object_1: imm, object_2: mut])
        // should be scheduled, after which allocations of mutably
        // accessed shared objects being as follows:
        // |-------------------------------------------------|------------|
        // |        object_1        |        object_2        | start time |
        // |________________________|________________________|____________|
        // |------------------------|------------------------|---- 9M ----|
        // |                        |                        |            |
        // |                        |------------------------|---- 8M     |
        // |                        |                        |            |
        // |                        |                        |---- 7M     |
        // |                        |                        |            |
        // |                        | cert. 2 (g=8000, d=4M) |---- 6M     |
        // |                        |                        |            |
        // |                        |                        |---- 5M     |
        // |                        |                        |            |
        // |                        |------------------------|---- 4M     |
        // |                        | cert. 1 (g=9000, d=1M) |            |
        // |------------------------|------------------------|---- 3M     |
        // |                        |                        |            |
        // |                        |                        |---- 2M     |
        // | cert. 0 (g=100K, d=3M) |                        |            |
        // |                        |                        |---- 1M     |
        // |                        |                        |            |
        // |-------------------------------------------------|---- 0 -----|
        // 0:
        let tx_data = &txs_data[0];
        if let Some(execution_start_time) = try_schedule(
            tx_data,
            &mut shared_object_congestion_tracker,
            &mut suggested_gas_price_calculator,
        ) {
            assert_eq!(execution_start_time, 0);
        } else {
            panic!(
                "Transaction {} must be scheduled:\n{tx_data:#?}",
                tx_data.order_idx
            );
        }
        // 1:
        let tx_data = &txs_data[1];
        if let Some(execution_start_time) = try_schedule(
            tx_data,
            &mut shared_object_congestion_tracker,
            &mut suggested_gas_price_calculator,
        ) {
            assert_eq!(execution_start_time, 3_000_000);
        } else {
            panic!(
                "Transaction {} must be scheduled:\n{tx_data:#?}",
                tx_data.order_idx
            );
        }
        // 2:
        let tx_data = &txs_data[2];
        if let Some(execution_start_time) = try_schedule(
            tx_data,
            &mut shared_object_congestion_tracker,
            &mut suggested_gas_price_calculator,
        ) {
            assert_eq!(execution_start_time, 4_000_000);
        } else {
            panic!(
                "Transaction {} must be scheduled:\n{tx_data:#?}",
                tx_data.order_idx
            );
        }

        // If `assign_min_free_exec_slot` is `true`, transaction
        // 3:  (7000, 2_000_000, [object_2: mut])
        // must be scheduled, in which case allocations of mutably accessed
        // shared object should look as follows:
        // |-------------------------------------------------|------------|
        // |        object_1        |        object_2        | start time |
        // |________________________|________________________|____________|
        // |------------------------|------------------------|---- 9M ----|
        // |                        |                        |            |
        // |                        |------------------------|---- 8M     |
        // |                        |                        |            |
        // |                        |                        |---- 7M     |
        // |                        |                        |            |
        // |                        | cert. 2 (g=8000, d=4M) |---- 6M     |
        // |                        |                        |            |
        // |                        |                        |---- 5M     |
        // |                        |                        |            |
        // |                        |------------------------|---- 4M     |
        // |                        | cert. 1 (g=9000, d=1M) |            |
        // |------------------------|------------------------|---- 3M     |
        // |                        |                        |            |
        // |                        |------------------------|---- 2M     |
        // | cert. 0 (g=100K, d=3M) |                        |            |
        // |                        | cert. 3 (g=7000, d=2M) |---- 1M     |
        // |                        |                        |            |
        // |-------------------------------------------------|---- 0 -----|
        // If `assign_min_free_exec_slot` is `false`, transaction 3 must be deferred,
        // in which case object 2 must be labeled as congested and suggested gas price
        // must be equals to that of transaction 2 plus one.
        let tx_data = &txs_data[3];
        if assign_min_free_exec_slot {
            if let Some(execution_start_time) = try_schedule(
                tx_data,
                &mut shared_object_congestion_tracker,
                &mut suggested_gas_price_calculator,
            ) {
                assert_eq!(execution_start_time, 0);
            } else {
                panic!(
                    "Transaction {} must be scheduled:\n{tx_data:#?}",
                    tx_data.order_idx
                );
            }
        } else {
            let (congested_objects, suggested_gas_price) = try_defer(
                tx_data,
                &mut shared_object_congestion_tracker,
                &mut suggested_gas_price_calculator,
            )
            .unwrap_or_else(|| {
                panic!(
                    "Transaction {} must be deferred:\n{tx_data:#?}",
                    tx_data.order_idx
                );
            });
            assert_eq!(
                congested_objects,
                vec![object_2], // expected congested objects
                "Calculated congested objects do not match expected for transaction {}:\n{:#?}",
                tx_data.order_idx,
                tx_data,
            );
            assert_eq!(
                suggested_gas_price,
                txs_data[2].gas_price + 1, // expected suggested gas price
                "Calculated suggested gas price does not match expected for transaction {}:\n{:#?}",
                tx_data.order_idx,
                tx_data,
            );
        }

        // Transactions
        // 4:  (7000, 1_000_001, [object_2: imm]),
        // 5:  (7000, 5_000_000, [object_2: mut])
        // must be deferred, with object 2 being labeled congested and
        // suggested gas price being equal that of transaction 2 plus one.
        (4..=5).for_each(|i| {
            let tx_data = &txs_data[i];
            let (congested_objects, suggested_gas_price) = try_defer(
                tx_data,
                &mut shared_object_congestion_tracker,
                &mut suggested_gas_price_calculator,
            )
            .unwrap_or_else(|| {
                panic!(
                    "Transaction {} must be deferred:\n{tx_data:#?}",
                    tx_data.order_idx
                );
            });
            assert_eq!(
                congested_objects,
                vec![object_2], // expected congested objects
                "Calculated congested objects do not match expected for transaction {}:\n{:#?}",
                tx_data.order_idx,
                tx_data,
            );
            assert_eq!(
                suggested_gas_price,
                txs_data[2].gas_price + 1, // expected suggested gas price
                "Calculated suggested gas price does not match expected for transaction {}:\n{:#?}",
                tx_data.order_idx,
                tx_data,
            );
        });

        // Transaction
        // 6:  (7000, 5_000_001, [object_1: mut, object_2, mut]),
        // must be deferred, with objects 1 and 2 being labeled congested
        // if `assign_min_free_exec_slot` is `true` and object 2 if
        // `assign_min_free_exec_slot` is false and suggested gas price
        // being equal that of transaction 1 plus one.
        let tx_data = &txs_data[6];
        let (congested_objects, suggested_gas_price) = try_defer(
            tx_data,
            &mut shared_object_congestion_tracker,
            &mut suggested_gas_price_calculator,
        )
        .unwrap_or_else(|| {
            panic!(
                "Transaction {} must be deferred:\n{tx_data:#?}",
                tx_data.order_idx
            );
        });
        assert_eq!(
            congested_objects,
            if assign_min_free_exec_slot {
                vec![object_1, object_2]
            } else {
                vec![object_2]
            }, // expected congested objects
            "Calculated congested objects do not match expected for transaction {}:\n{:#?}",
            tx_data.order_idx,
            tx_data,
        );
        assert_eq!(
            suggested_gas_price,
            txs_data[1].gas_price + 1, // expected suggested gas price
            "Calculated suggested gas price does not match expected for transaction {}:\n{:#?}",
            tx_data.order_idx,
            tx_data,
        );

        // Transaction
        // 7:  (7000, 8_000_000, [object_1: mut, object_2, mut])
        // must be deferred, with objects 1 and 2 being labeled congested
        // and suggested gas price being equal that of transaction 0,
        // i.e., max gas price.
        let tx_data = &txs_data[7];
        let (congested_objects, suggested_gas_price) = try_defer(
            tx_data,
            &mut shared_object_congestion_tracker,
            &mut suggested_gas_price_calculator,
        )
        .unwrap_or_else(|| {
            panic!(
                "Transaction {} must be deferred:\n{tx_data:#?}",
                tx_data.order_idx
            );
        });
        assert_eq!(
            congested_objects,
            vec![object_1, object_2], // expected congested objects
            "Calculated congested objects do not match expected for transaction {}:\n{:#?}",
            tx_data.order_idx,
            tx_data,
        );
        assert_eq!(
            suggested_gas_price,
            txs_data[0].gas_price, // expected suggested gas price
            "Calculated suggested gas price does not match expected for transaction {}:\n{:#?}",
            tx_data.order_idx,
            tx_data,
        );

        // Transactions
        // 8:  (6000, 4_000_000, [object_1: mut]),
        // 9:  (5000, 2_000_000, [object_1: mut])
        // should be scheduled, after which allocations of mutably accessed
        // shared objects being as follows:
        // |-------------------------------------------------|------------|
        // |        object_1        |        object_2        | start time |
        // |________________________|________________________|____________|
        // |------------------------|------------------------|---- 9M ----|
        // |                        |                        |            |
        // | cert. 9 (g=5000, d=2M) |------------------------|---- 8M     |
        // |                        |                        |            |
        // |------------------------|                        |---- 7M     |
        // |                        |                        |            |
        // |                        | cert. 2 (g=8000, d=4M) |---- 6M     |
        // |                        |                        |            |
        // | cert. 8 (g=6000, d=4M) |                        |---- 5M     |
        // |                        |                        |            |
        // |                        |------------------------|---- 4M     |
        // |                        | cert. 1 (g=9000, d=1M) |            |
        // |------------------------|------------------------|---- 3M     |
        // |                        |                        |            |
        // |                        |------------------------|---- 2M     |
        // | cert. 0 (g=100K, d=3M) |                        |            |
        // |                        | cert. 3 (g=7000, d=2M) |---- 1M     |
        // |                        |                        |            |
        // |-------------------------------------------------|---- 0 -----|
        // NOTE: certificate 3 will only be scheduled if
        // `assign_min_free_exec_slot` is `true`.
        // 8:
        let tx_data = &txs_data[8];
        if let Some(execution_start_time) = try_schedule(
            tx_data,
            &mut shared_object_congestion_tracker,
            &mut suggested_gas_price_calculator,
        ) {
            assert_eq!(execution_start_time, 3_000_000);
        } else {
            panic!(
                "Transaction {} must be scheduled:\n{tx_data:#?}",
                tx_data.order_idx
            );
        }
        // 9:
        let tx_data = &txs_data[9];
        if let Some(execution_start_time) = try_schedule(
            tx_data,
            &mut shared_object_congestion_tracker,
            &mut suggested_gas_price_calculator,
        ) {
            assert_eq!(execution_start_time, 7_000_000);
        } else {
            panic!(
                "Transaction {} must be scheduled:\n{tx_data:#?}",
                tx_data.order_idx
            );
        }

        // Transaction
        // 10: (5000, 1_000_001, [object_1: imm, object_2, imm])
        // must be deferred, with objects 1 and 2 being labeled congested
        // and suggested gas price being equal that of transaction 2 plus one.
        let tx_data = &txs_data[10];
        let (congested_objects, suggested_gas_price) = try_defer(
            tx_data,
            &mut shared_object_congestion_tracker,
            &mut suggested_gas_price_calculator,
        )
        .unwrap_or_else(|| {
            panic!(
                "Transaction {} must be deferred:\n{tx_data:#?}",
                tx_data.order_idx
            );
        });
        assert_eq!(
            congested_objects,
            vec![object_1, object_2], // expected congested objects
            "Calculated congested objects do not match expected for transaction {}:\n{:#?}",
            tx_data.order_idx,
            tx_data,
        );
        assert_eq!(
            suggested_gas_price,
            txs_data[2].gas_price + 1, // expected suggested gas price
            "Calculated suggested gas price does not match expected for transaction {}:\n{:#?}",
            tx_data.order_idx,
            tx_data,
        );

        // Transaction
        // 11: (5000, 5_000_001, [object_1: mut, object_2, imm])
        // must be deferred, with objects 1 and 2 being labeled congested
        // and suggested gas price being equal that of transaction 1 plus one.
        let tx_data = &txs_data[11];
        let (congested_objects, suggested_gas_price) = try_defer(
            tx_data,
            &mut shared_object_congestion_tracker,
            &mut suggested_gas_price_calculator,
        )
        .unwrap_or_else(|| {
            panic!(
                "Transaction {} must be deferred:\n{tx_data:#?}",
                tx_data.order_idx
            );
        });
        assert_eq!(
            congested_objects,
            vec![object_1, object_2], // expected congested objects
            "Calculated congested objects do not match expected for transaction {}:\n{:#?}",
            tx_data.order_idx,
            tx_data,
        );
        assert_eq!(
            suggested_gas_price,
            txs_data[1].gas_price + 1, // expected suggested gas price
            "Calculated suggested gas price does not match expected for transaction {}:\n{:#?}",
            tx_data.order_idx,
            tx_data,
        );

        // Transaction
        // 12: (5000, 9_000_000, [object_1: imm, object_2, mut])
        // must be deferred, with objects 1 and 2 being labeled congested
        // and suggested gas price being equal that of transaction 0, i.e.,
        // max gas price.
        let tx_data = &txs_data[12];
        let (congested_objects, suggested_gas_price) = try_defer(
            tx_data,
            &mut shared_object_congestion_tracker,
            &mut suggested_gas_price_calculator,
        )
        .unwrap_or_else(|| {
            panic!(
                "Transaction {} must be deferred:\n{tx_data:#?}",
                tx_data.order_idx
            );
        });
        assert_eq!(
            congested_objects,
            vec![object_1, object_2], // expected congested objects
            "Calculated congested objects do not match expected for transaction {}:\n{:#?}",
            tx_data.order_idx,
            tx_data,
        );
        assert_eq!(
            suggested_gas_price,
            txs_data[0].gas_price, // expected suggested gas price
            "Calculated suggested gas price does not match expected for transaction {}:\n{:#?}",
            tx_data.order_idx,
            tx_data,
        );
    }

    // Test `SuggestedGasPriceCalculator::calculate_suggested_gas_price`
    // in the `PerObjectCongestionControlMode::TotalTxCount` mode with
    // congestion limit overshoot.
    #[rstest]
    fn calculate_suggested_gas_price_in_tx_count_mode_with_overshoot(
        #[values(false, true)] assign_min_free_exec_slot: bool,
        // Whether to use congestion limit overshoot in the gas price feedback
        // mechanism, i.e., this is only used in `SuggestedGasPriceCalculator`.
        // This is used to test that `SuggestedGasPriceCalculator` behaves
        // differently depending on `use_congestion_limit_overshoot` values
        // while congestion limit overshoot is always enabled in
        // `SharedObjectCongestionTracker`.
        #[values(false, true)] use_congestion_limit_overshoot: bool,
    ) {
        let object_1 = ObjectID::random();
        let object_2 = ObjectID::random();

        // Congestion control and other parameters used in
        // `SharedObjectCongestionTracker` and `SuggestedGasPriceCalculator`
        let max_gas_price = ProtocolConfig::get_for_max_version_UNSAFE().max_gas_price();
        let congestion_control_parameters = CongestionControlParameters::new_for_test(
            PerObjectCongestionControlMode::TotalTxCount,
            assign_min_free_exec_slot,
            Some(3),       // max_execution_duration_per_commit
            Some(2),       // max_congestion_limit_overshoot_per_commit
            max_gas_price, // max_gas_price
            use_congestion_limit_overshoot,
            true,
        );

        // Initialize `SharedObjectCongestionTracker` and `SuggestedGasPriceCalculator`
        let mut shared_object_congestion_tracker = SharedObjectCongestionTracker::new(
            [(object_1, 1), (object_2, 2)], // initial_object_debts
            congestion_control_parameters.clone(),
        );
        let mut suggested_gas_price_calculator = SuggestedGasPriceCalculator::new_for_test(
            congestion_control_parameters,
            REFERENCE_GAS_PRICE,
        );

        // Create some data for transactions and process each for scheduling.
        let txs_data = build_transactions_data_for_test(max_gas_price, object_1, object_2);

        // Transactions
        // 0:  (100K, 3_000_000, [object_1: mut, object_2: imm]),
        // 1:  (9000, 1_000_000, [object_1: imm, object_2: mut]),
        // 2:  (8000, 4_000_000, [object_1: imm, object_2: mut]),
        // should be scheduled, after which allocations of mutably
        // accessed shared objects being as follows:
        // |-------------------------------------|------------|
        // |     object_1     |     object_2     | start time |
        // |__________________|__________________|____________|
        // |------------------|------------------|---- 5 -----|
        // |                  | cert. 2 (g=8000) |            |
        // |                  |------------------|---- 4      |
        // |                  | cert. 1 (g=9000) |            |
        // |------------------|------------------|---- 3 -----|
        // | cert. 0 (g=100K) |                  |            |
        // |------------------|------------------|---- 2      |
        // |                  | init. obj. debts |            |
        // |------------------| init. obj. debts |---- 1      |
        // | init. obj. debts | init. obj. debts |            |
        // |-------------------------------------|---- 0 -----|
        (0..=2).for_each(|i| {
            let tx_data = &txs_data[i];
            if let Some(execution_start_time) = try_schedule(
                tx_data,
                &mut shared_object_congestion_tracker,
                &mut suggested_gas_price_calculator,
            ) {
                assert_eq!(execution_start_time, i as u64 + 2);
            } else {
                panic!(
                    "Transaction {} must be scheduled:\n{tx_data:#?}",
                    tx_data.order_idx
                );
            }
        });

        // If `assign_min_free_exec_slot` is `true`, transaction
        // 3:  (7000, 2_000_000, [object_2: mut])
        // must be scheduled, in which case allocations of mutably
        // accessed shared object should look as follows:
        // |-------------------------------------|------------|
        // |     object_1     |     object_2     | start time |
        // |__________________|__________________|____________|
        // |------------------|------------------|---- 5 -----|
        // |                  | cert. 2 (g=8000) |            |
        // |                  |------------------|---- 4      |
        // |                  | cert. 1 (g=9000) |            |
        // |------------------|------------------|---- 3 -----|
        // | cert. 0 (g=100K) | cert. 3 (g=7000) |            |
        // |------------------|------------------|---- 2      |
        // |                  | init. obj. debts |            |
        // |------------------| init. obj. debts |---- 1      |
        // | init. obj. debts | init. obj. debts |            |
        // |-------------------------------------|---- 0 -----|
        // If `assign_min_free_exec_slot` is `false`, transaction 3 must be deferred,
        // in which case object 2 must be labeled as congested and suggested gas price
        // must be equals to that of transaction
        // 2: if `use_congestion_limit_overshoot` is `true`,
        // 1: if `use_congestion_limit_overshoot` is `false`
        // plus one.
        let tx_data = &txs_data[3];
        if assign_min_free_exec_slot {
            if let Some(execution_start_time) = try_schedule(
                tx_data,
                &mut shared_object_congestion_tracker,
                &mut suggested_gas_price_calculator,
            ) {
                assert_eq!(execution_start_time, 2);
            } else {
                panic!(
                    "Transaction {} must be scheduled:\n{tx_data:#?}",
                    tx_data.order_idx
                );
            }
        } else {
            let (congested_objects, suggested_gas_price) = try_defer(
                tx_data,
                &mut shared_object_congestion_tracker,
                &mut suggested_gas_price_calculator,
            )
            .unwrap_or_else(|| {
                panic!(
                    "Transaction {} must be deferred:\n{tx_data:#?}",
                    tx_data.order_idx
                );
            });
            assert_eq!(
                congested_objects,
                vec![object_2], // expected congested objects
                "Calculated congested objects do not match expected for transaction {}:\n{:#?}",
                tx_data.order_idx,
                tx_data,
            );
            assert_eq!(
                suggested_gas_price,
                // expected suggested gas price
                txs_data[if use_congestion_limit_overshoot { 2 } else { 1 }].gas_price + 1,
                "Calculated suggested gas price does not match expected for transaction {}:\n{:#?}",
                tx_data.order_idx,
                tx_data,
            );
        }

        // Transactions
        // 4:  (7000, 1_000_001, [object_2: imm]),
        // 5:  (7000, 5_000_000, [object_2: mut])
        // must be deferred, with object 2 being labeled congested and
        // suggested gas price being equal that of transaction
        // 2: if `use_congestion_limit_overshoot` is `true`,
        // 1: if `use_congestion_limit_overshoot` is `false`
        // plus one.
        (4..=5).for_each(|i| {
            let tx_data = &txs_data[i];
            let (congested_objects, suggested_gas_price) = try_defer(
                tx_data,
                &mut shared_object_congestion_tracker,
                &mut suggested_gas_price_calculator,
            )
            .unwrap_or_else(|| {
                panic!(
                    "Transaction {} must be deferred:\n{tx_data:#?}",
                    tx_data.order_idx
                );
            });
            assert_eq!(
                congested_objects,
                vec![object_2], // expected congested objects
                "Calculated congested objects do not match expected for transaction {}:\n{:#?}",
                tx_data.order_idx,
                tx_data,
            );
            assert_eq!(
                suggested_gas_price,
                // expected suggested gas price
                txs_data[if use_congestion_limit_overshoot { 2 } else { 1 }].gas_price + 1,
                "Calculated suggested gas price does not match expected for transaction {}:\n{:#?}",
                tx_data.order_idx,
                tx_data,
            );
        });

        // Transactions
        // 6:  (7000, 5_000_001, [object_1: mut, object_2, mut]),
        // 7:  (7000, 8_000_000, [object_1: mut, object_2, mut])
        // must be deferred, with objects 1 and 2 being
        // labeled congested if `assign_min_free_exec_slot` is `true` and
        // object 2 if `assign_min_free_exec_slot` is false and suggested
        // gas price being equal that of transaction
        // 2: if `use_congestion_limit_overshoot` is `true` plus one,
        // 0: if `use_congestion_limit_overshoot` is `false`.
        (6..=7).for_each(|i| {
            let tx_data = &txs_data[i];
            let (congested_objects, suggested_gas_price) = try_defer(
                tx_data,
                &mut shared_object_congestion_tracker,
                &mut suggested_gas_price_calculator,
            )
            .unwrap_or_else(|| {
                panic!(
                    "Transaction {} must be deferred:\n{tx_data:#?}",
                    tx_data.order_idx
                );
            });
            assert_eq!(
                congested_objects,
                if assign_min_free_exec_slot {
                    vec![object_1, object_2]
                } else {
                    vec![object_2]
                }, // expected congested objects
                "Calculated congested objects do not match expected for transaction {}:\n{:#?}",
                tx_data.order_idx,
                tx_data,
            );
            assert_eq!(
                suggested_gas_price,
                // expected suggested gas price
                if use_congestion_limit_overshoot {
                    txs_data[2].gas_price + 1
                } else {
                    txs_data[0].gas_price
                },
                "Calculated suggested gas price does not match expected for transaction {}:\n{:#?}",
                tx_data.order_idx,
                tx_data,
            );
        });

        // Transactions
        // 8:  (6000, 4_000_000, [object_1: mut]),
        // 9:  (5000, 2_000_000, [object_1: mut])
        // should be scheduled, after which allocations of mutably
        // accessed shared objects being as follows if
        // `assign_min_free_exec_slot` is `true`:
        // |-------------------------------------|------------|
        // |     object_1     |     object_2     | start time |
        // |__________________|__________________|____________|
        // |------------------|------------------|---- 5 -----|
        // |                  | cert. 2 (g=8000) |            |
        // |------------------|------------------|---- 4      |
        // | cert. 9 (g=5000) | cert. 1 (g=9000) |            |
        // |------------------|------------------|---- 3 -----|
        // | cert. 0 (g=100K) | cert. 3 (g=7000) |            |
        // |------------------|------------------|---- 2      |
        // | cert. 8 (g=6000) | init. obj. debts |            |
        // |------------------| init. obj. debts |---- 1      |
        // | init. obj. debts | init. obj. debts |            |
        // |-------------------------------------|---- 0 -----|
        // NOTE: certificate 3 will only be scheduled if
        // `assign_min_free_exec_slot` is `true`.
        // Tx 8:
        let tx_data = &txs_data[8];
        if let Some(execution_start_time) = try_schedule(
            tx_data,
            &mut shared_object_congestion_tracker,
            &mut suggested_gas_price_calculator,
        ) {
            assert_eq!(
                execution_start_time,
                if assign_min_free_exec_slot { 1 } else { 3 }
            );
        } else {
            panic!(
                "Transaction {} must be scheduled:\n{tx_data:#?}",
                tx_data.order_idx
            );
        }
        // Tx 9:
        let tx_data = &txs_data[9];
        if let Some(execution_start_time) = try_schedule(
            tx_data,
            &mut shared_object_congestion_tracker,
            &mut suggested_gas_price_calculator,
        ) {
            assert_eq!(
                execution_start_time,
                if assign_min_free_exec_slot { 3 } else { 4 }
            );
        } else {
            panic!(
                "Transaction {} must be scheduled:\n{tx_data:#?}",
                tx_data.order_idx
            );
        }

        // Transactions
        // 10: (5000, 1_000_001, [object_1: imm, object_2, imm]),
        // 11: (5000, 5_000_001, [object_1: mut, object_2, imm]),
        // 12: (5000, 9_000_000, [object_1: imm, object_2, mut])
        // must be deferred, with objects 1 and 2 being labeled congested
        // if `assign_min_free_exec_slot` is `true` and object 2 if
        // `assign_min_free_exec_slot` is false and suggested gas price
        // being equal that of transaction
        // 2: if `use_congestion_limit_overshoot` is `true` plus one,
        // 0: if `use_congestion_limit_overshoot` is `false`.
        (10..=12).for_each(|i| {
            let tx_data = &txs_data[i];
            let (congested_objects, suggested_gas_price) = try_defer(
                tx_data,
                &mut shared_object_congestion_tracker,
                &mut suggested_gas_price_calculator,
            )
            .unwrap_or_else(|| {
                panic!(
                    "Transaction {} must be deferred:\n{tx_data:#?}",
                    tx_data.order_idx
                );
            });
            assert_eq!(
                congested_objects,
                vec![object_1, object_2], // expected congested objects
                "Calculated congested objects do not match expected for transaction {}:\n{:#?}",
                tx_data.order_idx,
                tx_data,
            );
            assert_eq!(
                suggested_gas_price,
                // expected suggested gas price
                if use_congestion_limit_overshoot {
                    txs_data[2].gas_price + 1
                } else {
                    txs_data[0].gas_price
                },
                "Calculated suggested gas price does not match expected for transaction {}:\n{:#?}",
                tx_data.order_idx,
                tx_data,
            );
        });
    }

    // Test `SuggestedGasPriceCalculator::calculate_suggested_gas_price`
    // in the `PerObjectCongestionControlMode::TotalGasBudget` mode mode
    // with congestion limit overshoot.
    #[rstest]
    fn calculate_suggested_gas_price_in_gas_budget_mode_with_overshoot(
        #[values(false, true)] assign_min_free_exec_slot: bool,
        // Whether to use congestion limit overshoot in the gas price feedback
        // mechanism, i.e., this is only used in `SuggestedGasPriceCalculator`.
        // This is used to test that `SuggestedGasPriceCalculator` behaves
        // differently depending on `use_congestion_limit_overshoot` values
        // while congestion limit overshoot is always enabled in
        // `SharedObjectCongestionTracker`.
        #[values(false, true)] use_congestion_limit_overshoot: bool,
    ) {
        let object_1 = ObjectID::random();
        let object_2 = ObjectID::random();

        // Congestion control and other parameters used in
        // `SharedObjectCongestionTracker` and `SuggestedGasPriceCalculator`
        let max_gas_price = ProtocolConfig::get_for_max_version_UNSAFE().max_gas_price();
        let congestion_control_parameters = CongestionControlParameters::new_for_test(
            PerObjectCongestionControlMode::TotalGasBudget,
            assign_min_free_exec_slot,
            Some(9_000_000), // max_execution_duration_per_commit
            Some(2_000_000), // max_congestion_limit_overshoot_per_commit
            max_gas_price,   // max_gas_price
            use_congestion_limit_overshoot,
            true,
        );

        // Initialize `SharedObjectCongestionTracker` and `SuggestedGasPriceCalculator`
        let mut shared_object_congestion_tracker = SharedObjectCongestionTracker::new(
            [(object_1, 2_000_000), (object_2, 1_000_000)], // initial_object_debts
            congestion_control_parameters.clone(),
        );
        let mut suggested_gas_price_calculator = SuggestedGasPriceCalculator::new_for_test(
            congestion_control_parameters,
            REFERENCE_GAS_PRICE,
        );

        // Create some data for transactions and process each for scheduling
        let txs_data = build_transactions_data_for_test(max_gas_price, object_1, object_2);

        // Transactions
        // 0:  (100K, 3_000_000, [object_1: mut, object_2: imm]),
        // 1:  (9000, 1_000_000, [object_1: imm, object_2: mut]),
        // 2:  (8000, 4_000_000, [object_1: imm, object_2: mut])
        // should be scheduled, after which allocations of mutably
        // accessed shared objects being as follows:
        // |-------------------------------------------------|------------|
        // |        object_1        |        object_2        | start time |
        // |________________________|________________________|____________|
        // |------------------------|------------------------|---- 11M ---|
        // |                        |                        |            |
        // |                        |------------------------|---- 10M    |
        // |                        |                        |            |
        // |                        |                        |---- 9M ----|
        // |                        |                        |            |
        // |                        | cert. 2 (g=8000, d=4M) |---- 8M     |
        // |                        |                        |            |
        // |                        |                        |---- 7M     |
        // |                        |                        |            |
        // |                        |------------------------|---- 6M     |
        // |                        | cert. 1 (g=9000, d=1M) |            |
        // |------------------------|------------------------|---- 5M     |
        // |                        |                        |            |
        // |                        |                        |---- 4M     |
        // | cert. 0 (g=100K, d=3M) |                        |            |
        // |                        |                        |---- 3M     |
        // |                        |                        |            |
        // |------------------------|                        |---- 2M     |
        // | initial object debts   |                        |            |
        // | initial object debts   |------------------------|---- 1M     |
        // | initial object debts   | initial object debts   |            |
        // |-------------------------------------------------|---- 0 -----|
        // 0:
        let tx_data = &txs_data[0];
        if let Some(execution_start_time) = try_schedule(
            tx_data,
            &mut shared_object_congestion_tracker,
            &mut suggested_gas_price_calculator,
        ) {
            assert_eq!(execution_start_time, 2_000_000);
        } else {
            panic!(
                "Transaction {} must be scheduled:\n{tx_data:#?}",
                tx_data.order_idx
            );
        }
        // 1:
        let tx_data = &txs_data[1];
        if let Some(execution_start_time) = try_schedule(
            tx_data,
            &mut shared_object_congestion_tracker,
            &mut suggested_gas_price_calculator,
        ) {
            assert_eq!(execution_start_time, 5_000_000);
        } else {
            panic!(
                "Transaction {} must be scheduled:\n{tx_data:#?}",
                tx_data.order_idx
            );
        }
        // 2:
        let tx_data = &txs_data[2];
        if let Some(execution_start_time) = try_schedule(
            tx_data,
            &mut shared_object_congestion_tracker,
            &mut suggested_gas_price_calculator,
        ) {
            assert_eq!(execution_start_time, 6_000_000);
        } else {
            panic!(
                "Transaction {} must be scheduled:\n{tx_data:#?}",
                tx_data.order_idx
            );
        }

        // If `assign_min_free_exec_slot` is `true`, transactions
        // 3:  (7000, 2_000_000, [object_2: mut])
        // 4:  (7000, 1_000_001, [object_2: imm]),
        // must be scheduled, in which case allocations of mutably accessed
        // shared object should look as follows:
        // |-------------------------------------------------|------------|
        // |        object_1        |        object_2        | start time |
        // |________________________|________________________|____________|
        // |------------------------|------------------------|---- 11M ---|
        // |                        |                        |            |
        // |                        |------------------------|---- 10M    |
        // |                        |                        |            |
        // |                        |                        |---- 9M ----|
        // |                        |                        |            |
        // |                        | cert. 2 (g=8000, d=4M) |---- 8M     |
        // |                        |                        |            |
        // |                        |                        |---- 7M     |
        // |                        |                        |            |
        // |                        |------------------------|---- 6M     |
        // |                        | cert. 1 (g=9000, d=1M) |            |
        // |------------------------|------------------------|---- 5M     |
        // |                        |                        |            |
        // |                        |------------------------|---- 4M     |
        // | cert. 0 (g=100K, d=3M) | cert. 4 (g=7000, ~d=1M)|            |
        // |                        |------------------------|---- 3M     |
        // |                        |                        |            |
        // |------------------------| cert. 3 (g=7000, d=2M) |---- 2M ----|
        // | initial object debts   |                        |            |
        // | initial object debts   |------------------------|---- 1M     |
        // | initial object debts   | initial object debts   |            |
        // |-------------------------------------------------|---- 0 -----|
        // If `assign_min_free_exec_slot` is `false`, transactions 3 and 4 must be
        // deferred, in which case object 2 must be labeled as congested and
        // suggested gas price must be equals to that of transaction 2 plus one.
        (3..=4).for_each(|i| {
            let tx_data = &txs_data[i];
            if assign_min_free_exec_slot {
                if let Some(execution_start_time) = try_schedule(
                    tx_data,
                    &mut shared_object_congestion_tracker,
                    &mut suggested_gas_price_calculator,
                ) {
                    assert_eq!(
                        execution_start_time,
                        if i == 3 {
                            1_000_000
                        } else if i == 4 {
                            3_000_000
                        } else {
                            unreachable!()
                        }
                    );
                } else {
                    panic!(
                        "Transaction {} must be scheduled:\n{tx_data:#?}",
                        tx_data.order_idx
                    );
                }
            } else {
                let (congested_objects, suggested_gas_price) = try_defer(
                    tx_data,
                    &mut shared_object_congestion_tracker,
                    &mut suggested_gas_price_calculator,
                )
                .unwrap_or_else(|| {
                    panic!(
                        "Transaction {} must be deferred:\n{tx_data:#?}",
                        tx_data.order_idx
                    );
                });
                assert_eq!(
                    congested_objects,
                    vec![object_2], // expected congested objects
                    "Calculated congested objects do not match expected for transaction {}:\n{:#?}",
                    tx_data.order_idx,
                    tx_data,
                );
                assert_eq!(
                    suggested_gas_price,
                    txs_data[2].gas_price + 1, // expected suggested gas price
                    "Calculated suggested gas price does not match expected for transaction {}:\
                        \n{:#?}",
                    tx_data.order_idx,
                    tx_data,
                );
            }
        });

        // Transaction
        // 5:  (7000, 5_000_000, [object_2: mut])
        // must be deferred, with object 2 being labeled congested and
        // suggested gas price being equal that of transaction
        // 2: if `use_congestion_limit_overshoot` is `true`,
        // 1: if `use_congestion_limit_overshoot` is `false`
        // plus one.
        let tx_data = &txs_data[5];
        let (congested_objects, suggested_gas_price) = try_defer(
            tx_data,
            &mut shared_object_congestion_tracker,
            &mut suggested_gas_price_calculator,
        )
        .unwrap_or_else(|| {
            panic!(
                "Transaction {} must be deferred:\n{tx_data:#?}",
                tx_data.order_idx
            );
        });
        assert_eq!(
            congested_objects,
            vec![object_2], // expected congested objects
            "Calculated congested objects do not match expected for transaction {}:\n{:#?}",
            tx_data.order_idx,
            tx_data,
        );
        assert_eq!(
            suggested_gas_price,
            // expected suggested gas price
            txs_data[if use_congestion_limit_overshoot { 2 } else { 1 }].gas_price + 1,
            "Calculated suggested gas price does not match expected for transaction {}:\n{:#?}",
            tx_data.order_idx,
            tx_data,
        );

        // Transaction
        // 6:  (7000, 5_000_001, [object_1: mut, object_2, mut]),
        // must be deferred, with objects 1 and 2 being labeled congested
        // if `assign_min_free_exec_slot` is `true` and object 2 if
        // `assign_min_free_exec_slot` is false and suggested gas price
        // being equal that of transaction
        // 1: if `use_congestion_limit_overshoot` is `true` plus one,
        // 0: if `use_congestion_limit_overshoot` is `false`.
        let tx_data = &txs_data[6];
        let (congested_objects, suggested_gas_price) = try_defer(
            tx_data,
            &mut shared_object_congestion_tracker,
            &mut suggested_gas_price_calculator,
        )
        .unwrap_or_else(|| {
            panic!(
                "Transaction {} must be deferred:\n{tx_data:#?}",
                tx_data.order_idx
            );
        });
        assert_eq!(
            congested_objects,
            if assign_min_free_exec_slot {
                vec![object_1, object_2]
            } else {
                vec![object_2]
            }, // expected congested objects
            "Calculated congested objects do not match expected for transaction {}:\n{:#?}",
            tx_data.order_idx,
            tx_data,
        );
        assert_eq!(
            suggested_gas_price,
            // expected suggested gas price
            if use_congestion_limit_overshoot {
                txs_data[1].gas_price + 1
            } else {
                txs_data[0].gas_price
            },
            "Calculated suggested gas price does not match expected for transaction {}:\n{:#?}",
            tx_data.order_idx,
            tx_data,
        );

        // Transaction
        // 7:  (7000, 8_000_000, [object_1: mut, object_2, mut])
        // must be deferred, with objects 1 and 2 being labeled congested
        // and suggested gas price being equal that of transaction 0,
        // i.e., max gas price.
        let tx_data = &txs_data[7];
        let (congested_objects, suggested_gas_price) = try_defer(
            tx_data,
            &mut shared_object_congestion_tracker,
            &mut suggested_gas_price_calculator,
        )
        .unwrap_or_else(|| {
            panic!(
                "Transaction {} must be deferred:\n{tx_data:#?}",
                tx_data.order_idx
            );
        });
        assert_eq!(
            congested_objects,
            vec![object_1, object_2], // expected congested objects
            "Calculated congested objects do not match expected for transaction {}:\n{:#?}",
            tx_data.order_idx,
            tx_data,
        );
        assert_eq!(
            suggested_gas_price,
            txs_data[0].gas_price, // expected suggested gas price
            "Calculated suggested gas price does not match expected for transaction {}:\n{:#?}",
            tx_data.order_idx,
            tx_data,
        );

        // Transactions
        // 8:  (6000, 4_000_000, [object_1: mut]),
        // 9:  (5000, 2_000_000, [object_1: mut])
        // should be scheduled, after which allocations of mutably accessed
        // shared objects being as follows:
        // |-------------------------------------------------|------------|
        // |        object_1        |        object_2        | start time |
        // |________________________|________________________|____________|
        // |------------------------|------------------------|---- 11M ---|
        // |                        |                        |            |
        // | cert. 9 (g=5000, d=2M) |------------------------|---- 10M    |
        // |                        |                        |            |
        // |------------------------|                        |---- 9M ----|
        // |                        |                        |            |
        // |                        | cert. 2 (g=8000, d=4M) |---- 8M     |
        // |                        |                        |            |
        // | cert. 8 (g=6000, d=4M) |                        |---- 7M     |
        // |                        |                        |            |
        // |                        |------------------------|---- 6M     |
        // |                        | cert. 1 (g=9000, d=1M) |            |
        // |------------------------|------------------------|---- 5M     |
        // |                        |                        |            |
        // |                        |------------------------|---- 4M     |
        // | cert. 0 (g=100K, d=3M) | cert. 4 (g=7000, ~d=1M)|            |
        // |                        |------------------------|---- 3M     |
        // |                        |                        |            |
        // |------------------------| cert. 3 (g=7000, d=2M) |---- 2M ----|
        // | initial object debts   |                        |            |
        // | initial object debts   |------------------------|---- 1M     |
        // | initial object debts   | initial object debts   |            |
        // |-------------------------------------------------|---- 0 -----|
        // NOTE: certificates 3 and 4 will only be scheduled if
        // `assign_min_free_exec_slot` is `true`.
        // 8:
        let tx_data = &txs_data[8];
        if let Some(execution_start_time) = try_schedule(
            tx_data,
            &mut shared_object_congestion_tracker,
            &mut suggested_gas_price_calculator,
        ) {
            assert_eq!(execution_start_time, 5_000_000);
        } else {
            panic!(
                "Transaction {} must be scheduled:\n{tx_data:#?}",
                tx_data.order_idx
            );
        }
        // 9:
        let tx_data = &txs_data[9];
        if let Some(execution_start_time) = try_schedule(
            tx_data,
            &mut shared_object_congestion_tracker,
            &mut suggested_gas_price_calculator,
        ) {
            assert_eq!(execution_start_time, 9_000_000);
        } else {
            panic!(
                "Transaction {} must be scheduled:\n{tx_data:#?}",
                tx_data.order_idx
            );
        }

        // Transaction
        // 10: (5000, 1_000_001, [object_1: imm, object_2, imm])
        // must be deferred, with objects 1 and 2 being labeled congested
        // and suggested gas price being equal that of transaction 2 plus one.
        let tx_data = &txs_data[10];
        let (congested_objects, suggested_gas_price) = try_defer(
            tx_data,
            &mut shared_object_congestion_tracker,
            &mut suggested_gas_price_calculator,
        )
        .unwrap_or_else(|| {
            panic!(
                "Transaction {} must be deferred:\n{tx_data:#?}",
                tx_data.order_idx
            );
        });
        assert_eq!(
            congested_objects,
            vec![object_1, object_2], // expected congested objects
            "Calculated congested objects do not match expected for transaction {}:\n{:#?}",
            tx_data.order_idx,
            tx_data,
        );
        assert_eq!(
            suggested_gas_price,
            txs_data[2].gas_price + 1, // expected suggested gas price
            "Calculated suggested gas price does not match expected for transaction {}:\n{:#?}",
            tx_data.order_idx,
            tx_data,
        );

        // Transaction
        // 11: (5000, 5_000_001, [object_1: mut, object_2, imm])
        // must be deferred, with objects 1 and 2 being labeled congested
        // and suggested gas price being equal that of transaction
        // 1: if `use_congestion_limit_overshoot` is `true` plus one,
        // 0: if `use_congestion_limit_overshoot` is `false`.
        let tx_data = &txs_data[11];
        let (congested_objects, suggested_gas_price) = try_defer(
            tx_data,
            &mut shared_object_congestion_tracker,
            &mut suggested_gas_price_calculator,
        )
        .unwrap_or_else(|| {
            panic!(
                "Transaction {} must be deferred:\n{tx_data:#?}",
                tx_data.order_idx
            );
        });
        assert_eq!(
            congested_objects,
            vec![object_1, object_2], // expected congested objects
            "Calculated congested objects do not match expected for transaction {}:\n{:#?}",
            tx_data.order_idx,
            tx_data,
        );
        assert_eq!(
            suggested_gas_price,
            // expected suggested gas price
            if use_congestion_limit_overshoot {
                txs_data[1].gas_price + 1
            } else {
                txs_data[0].gas_price
            },
            "Calculated suggested gas price does not match expected for transaction {}:\n{:#?}",
            tx_data.order_idx,
            tx_data,
        );

        // Transaction
        // 12: (5000, 9_000_000, [object_1: imm, object_2, mut])
        // must be deferred, with objects 1 and 2 being labeled congested
        // and suggested gas price being equal that of transaction 0, i.e.,
        // max gas price.
        let tx_data = &txs_data[12];
        let (congested_objects, suggested_gas_price) = try_defer(
            tx_data,
            &mut shared_object_congestion_tracker,
            &mut suggested_gas_price_calculator,
        )
        .unwrap_or_else(|| {
            panic!(
                "Transaction {} must be deferred:\n{tx_data:#?}",
                tx_data.order_idx
            );
        });
        assert_eq!(
            congested_objects,
            vec![object_1, object_2], // expected congested objects
            "Calculated congested objects do not match expected for transaction {}:\n{:#?}",
            tx_data.order_idx,
            tx_data,
        );
        assert_eq!(
            suggested_gas_price,
            txs_data[0].gas_price, // expected suggested gas price
            "Calculated suggested gas price does not match expected for transaction {}:\n{:#?}",
            tx_data.order_idx,
            tx_data,
        );
    }

    // Test `SuggestedGasPriceCalculator::calculate_suggested_gas_price`
    // with congestion limit overshoot disabled in the tracker for a
    // transaction whose estimated execution duration is larger than
    // congestion limit per commit.
    #[rstest]
    fn calculate_suggested_gas_price_for_unschedulable_transaction_without_overshoot(
        #[values(false, true)] assign_min_free_exec_slot: bool,
        // Whether to use congestion limit overshoot in the gas price feedback
        // mechanism, i.e., this is only used in `SuggestedGasPriceCalculator`.
        // This is used to test that `SuggestedGasPriceCalculator` behaves in
        // the same way regardless of `use_congestion_limit_overshoot` values
        // if `max_congestion_limit_overshoot_per_commit` is `None`.
        #[values(false, true)] use_congestion_limit_overshoot: bool,
        #[values(
            PerObjectCongestionControlMode::TotalTxCount,
            PerObjectCongestionControlMode::TotalGasBudget
        )]
        per_object_congestion_control_mode: PerObjectCongestionControlMode,
    ) {
        let object_1 = ObjectID::random();
        let object_2 = ObjectID::random();

        // Congestion control and other parameters used in
        // `SharedObjectCongestionTracker` and `SuggestedGasPriceCalculator`
        let max_gas_price = ProtocolConfig::get_for_max_version_UNSAFE().max_gas_price();
        let max_execution_duration_per_commit = match per_object_congestion_control_mode {
            PerObjectCongestionControlMode::None => unreachable!(),
            PerObjectCongestionControlMode::TotalTxCount => 0,
            PerObjectCongestionControlMode::TotalGasBudget => 2_999_999,
        };
        let congestion_control_parameters = CongestionControlParameters::new_for_test(
            per_object_congestion_control_mode,
            assign_min_free_exec_slot,
            Some(max_execution_duration_per_commit),
            None, // max_congestion_limit_overshoot_per_commit
            max_gas_price,
            use_congestion_limit_overshoot,
            true,
        );

        // Initialize `SharedObjectCongestionTracker` and `SuggestedGasPriceCalculator`
        let mut shared_object_congestion_tracker = SharedObjectCongestionTracker::new(
            [], // initial_object_debts
            congestion_control_parameters.clone(),
        );
        let mut suggested_gas_price_calculator = SuggestedGasPriceCalculator::new_for_test(
            congestion_control_parameters,
            REFERENCE_GAS_PRICE,
        );

        // Create some data for transactions and process each for scheduling
        let txs_data = build_transactions_data_for_test(max_gas_price, object_1, object_2);

        // Transaction
        // 0: (maxgp, 3_000_000, vec![(object_1, true), (object_2, false)])
        // must be deferred, with objects 1 and 2 being labeled congested
        // and suggested gas price being equal the reference gas price.
        let tx_data = &txs_data[0];
        let (congested_objects, suggested_gas_price) = try_defer(
            tx_data,
            &mut shared_object_congestion_tracker,
            &mut suggested_gas_price_calculator,
        )
        .unwrap_or_else(|| {
            panic!(
                "Transaction {} must be deferred:\n{tx_data:#?}",
                tx_data.order_idx
            );
        });
        assert_eq!(
            congested_objects,
            vec![object_1, object_2], // expected congested objects
            "Calculated congested objects do not match expected for transaction {}:\n{:#?}",
            tx_data.order_idx,
            tx_data,
        );
        assert_eq!(
            suggested_gas_price,
            REFERENCE_GAS_PRICE, // expected suggested gas price
            "Calculated suggested gas price does not match expected for transaction {}:\n{:#?}",
            tx_data.order_idx,
            tx_data,
        );
    }

    // Test `SuggestedGasPriceCalculator::calculate_suggested_gas_price`
    // with congestion limit overshoot enabled in the tracker for a
    // transaction whose estimated execution duration is larger than
    // congestion limit per commit.
    #[rstest]
    fn calculate_suggested_gas_price_for_unschedulable_transaction_with_overshoot(
        #[values(false, true)] assign_min_free_exec_slot: bool,
        // Whether to use congestion limit overshoot in the gas price feedback
        // mechanism, i.e., this is only used in `SuggestedGasPriceCalculator`.
        #[values(false, true)] use_congestion_limit_overshoot: bool,
        #[values(
            PerObjectCongestionControlMode::TotalTxCount,
            PerObjectCongestionControlMode::TotalGasBudget
        )]
        per_object_congestion_control_mode: PerObjectCongestionControlMode,
    ) {
        let object_1 = ObjectID::random();
        let object_2 = ObjectID::random();

        // Congestion control and other parameters used in
        // `SharedObjectCongestionTracker` and `SuggestedGasPriceCalculator`
        let max_gas_price = ProtocolConfig::get_for_max_version_UNSAFE().max_gas_price();
        let (max_execution_duration_per_commit, max_congestion_limit_overshoot_per_commit) =
            match per_object_congestion_control_mode {
                PerObjectCongestionControlMode::None => unreachable!(),
                PerObjectCongestionControlMode::TotalTxCount => (1, 2),
                PerObjectCongestionControlMode::TotalGasBudget => (1_000_000, 2_000_000),
            };
        let congestion_control_parameters = CongestionControlParameters::new_for_test(
            per_object_congestion_control_mode,
            assign_min_free_exec_slot,
            Some(max_execution_duration_per_commit),
            Some(max_congestion_limit_overshoot_per_commit),
            max_gas_price,
            use_congestion_limit_overshoot,
            true,
        );

        // Initialize `SharedObjectCongestionTracker` and `SuggestedGasPriceCalculator`
        let mut shared_object_congestion_tracker = SharedObjectCongestionTracker::new(
            [(object_1, 3), (object_2, 3)], // initial_object_debts
            congestion_control_parameters.clone(),
        );
        let mut suggested_gas_price_calculator = SuggestedGasPriceCalculator::new_for_test(
            congestion_control_parameters,
            REFERENCE_GAS_PRICE,
        );

        // Create some data for transactions and process each for scheduling
        let txs_data = build_transactions_data_for_test(max_gas_price, object_1, object_2);

        // Transaction
        // 0: (maxgp, 3_000_000, vec![(object_1, true), (object_2, false)])
        // must be deferred, with objects 1 and 2 being labeled congested
        // and suggested gas price being equal the reference gas price.
        let tx_data = &txs_data[0];
        let (congested_objects, suggested_gas_price) = try_defer(
            tx_data,
            &mut shared_object_congestion_tracker,
            &mut suggested_gas_price_calculator,
        )
        .unwrap_or_else(|| {
            panic!(
                "Transaction {} must be deferred:\n{tx_data:#?}",
                tx_data.order_idx
            );
        });
        assert_eq!(
            congested_objects,
            vec![object_1, object_2], // expected congested objects
            "Calculated congested objects do not match expected for transaction {}:\n{:#?}",
            tx_data.order_idx,
            tx_data,
        );
        assert_eq!(
            suggested_gas_price,
            REFERENCE_GAS_PRICE, // expected suggested gas price
            "Calculated suggested gas price does not match expected for transaction {}:\n{:#?}",
            tx_data.order_idx,
            tx_data,
        );
    }
}
