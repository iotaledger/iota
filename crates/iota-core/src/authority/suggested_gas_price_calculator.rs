// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::{BTreeMap, HashMap};

use iota_types::{
    base_types::ObjectID, executable_transaction::VerifiedExecutableTransaction,
    transaction::TransactionDataAPI,
};
use tracing::instrument;

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
///    - compute its (imaginary) execution start time as
///      `max_execution_duration_per_commit` minus its estimated execution
///      duration;
///    - for each input shared object, get the maximum gas price over scheduled
///      certificates whose end execution time is larger than our imaginary
///      start time;
///    - take the maximum over the values obtained in the previous step;
///    - the suggested gas price equals the maximum value obtained in the
///      previous step plus 1, but such that it does not become larger than the
///      maximum gas price set in the protocol.
///
/// Note that if `max_execution_duration_per_commit` is set to `None`,
/// which means there is no shared object congestion control mechanism,
/// the calculator will suggest the reference gas price.
#[derive(Debug)]
pub(crate) struct SuggestedGasPriceCalculator {
    /// Per-commit congestion info
    congestion_info: PerCommitCongestionInfo,

    /// Maximum execution duration per shared object per commit.
    max_execution_duration_per_commit: Option<ExecutionTime>,

    /// The reference gas price, which will be suggested if
    /// `max_execution_duration_per_commit` is set to `None`.
    reference_gas_price: u64,

    /// Maximum gas price that can be set in transactions. This is
    /// used to prevent suggesting feedback gas price larger than
    /// this maximum value set in the protocol config.
    max_gas_price: u64,
}

impl SuggestedGasPriceCalculator {
    /// Create a new `SuggestedGasPriceCalculator` with empty shared
    /// object congestion data.
    pub fn new(
        max_execution_duration_per_commit: Option<ExecutionTime>,
        reference_gas_price: u64,
        max_gas_price: u64,
    ) -> Self {
        Self {
            congestion_info: PerCommitCongestionInfo::new(),
            max_execution_duration_per_commit,
            reference_gas_price,
            max_gas_price,
        }
    }

    /// Update per-commit congestion info for a single certificate. This should
    /// only be called for scheduled certificates that contain shared object(s);
    /// otherwise, the calculator might wrongly calculate suggested gas price.
    /// The `execution_start_time` and `estimated_execution_duration` parameters
    /// are the outcomes of the shared object congestion tracker (sequencer).
    pub fn update_congestion_info(
        &mut self,
        certificate: &VerifiedExecutableTransaction,
        execution_start_time: ExecutionTime,
        estimated_execution_duration: ExecutionTime,
    ) {
        // If we don't have a max execution duration, we don't need to update
        // the congestion info since the reference gas price will be suggested.
        if self.max_execution_duration_per_commit.is_none() {
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
                        per_object_congestion_info
                            .insert(execution_start_time, scheduled_transaction_congestion_info);
                    })
                    .or_insert(PerObjectCongestionInfo::from([(
                        execution_start_time,
                        scheduled_transaction_congestion_info,
                    )]));
            });
    }

    /// Calculate a suggested gas price for a deferred/cancelled `certificate`
    /// using the single-commit congestion info held by the calculator. This
    /// should only be called for certificates deferred/cancelled due to
    /// shared object congestion; otherwise, there is a risk of panic.
    #[instrument(level = "trace", skip_all)]
    pub fn calculate_suggested_gas_price(
        &self,
        certificate: &VerifiedExecutableTransaction,
        estimated_execution_duration: ExecutionTime,
    ) -> u64 {
        if let Some(max_execution_duration_per_commit) = self.max_execution_duration_per_commit {
            let clearing_gas_price = self.find_clearing_gas_price(
                certificate,
                estimated_execution_duration,
                max_execution_duration_per_commit,
            );

            // Suggested gas price equals `clearing_gas_price + 1`. We add 1 to make this
            // transaction would be scheduled if the same commit structure was repeated.
            let suggested_gas_price =
                clearing_gas_price.map_or(self.reference_gas_price, |p| p + 1);

            // Make sure suggested gas price is not larger than the maximum possible gas
            // price.
            suggested_gas_price.min(self.max_gas_price)
        } else {
            // ^ If we don't have a max execution duration, suggest the reference gas price.

            self.reference_gas_price
        }
    }

    /// Find the gas price for which a deferred/scheduled certificate would be
    /// scheduled if that gas price was paid and if exactly the same set of
    /// transactions appeared in a commit.
    fn find_clearing_gas_price(
        &self,
        certificate: &VerifiedExecutableTransaction,
        estimated_execution_duration: ExecutionTime,
        max_execution_duration_per_commit: ExecutionTime,
    ) -> Option<u64> {
        // Imaginary start time of the deferred/cancelled certificate. We consider
        // only the highest possible (but sufficient for scheduling) start time as
        // it is very likely that scheduled certificates with lower gas prices
        // appear have higher start times. If a transaction with its
        // `estimated_execution_duration` cannot fit within
        // `max_execution_duration_per_commit`, set its imaginary start time to 0.
        let start_time_of_deferred_cert =
            max_execution_duration_per_commit.saturating_sub(estimated_execution_duration);

        certificate
            .shared_input_objects()
            .filter_map(|object| {
                self.congestion_info
                    .get(&object.id)
                    .map(|per_object_congestion_info| {
                        per_object_congestion_info
                            .iter()
                            .filter_map(|(execution_start_time, tx_congestion_info)| {
                                let end_time_of_scheduled_cert = execution_start_time
                                    + tx_congestion_info.estimated_execution_duration;

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
            .flatten()
    }
}

#[cfg(test)]
pub mod suggested_gas_price_calculator_test_utils {
    use iota_protocol_config::PerObjectCongestionControlMode;
    use iota_types::base_types::ObjectID;

    use super::SuggestedGasPriceCalculator;
    use crate::authority::shared_object_congestion_tracker::{
        ExecutionTime, SharedObjectCongestionTracker,
        shared_object_test_utils::{
            build_transaction, initialize_tracker_and_compute_tx_start_time,
        },
    };

    pub(crate) fn new_suggested_gas_price_calculator_with_initial_values_for_test(
        init_values: &[(ObjectID, ExecutionTime, u64)],
        per_object_congestion_control_mode: PerObjectCongestionControlMode,
        max_execution_duration_per_commit: Option<ExecutionTime>,
        min_free_execution_slot_assigned: bool,
        reference_gas_price: u64,
        max_gas_price: u64,
    ) -> SuggestedGasPriceCalculator {
        let mut suggested_gas_price_calculator = SuggestedGasPriceCalculator::new(
            max_execution_duration_per_commit,
            reference_gas_price,
            max_gas_price,
        );

        let mut shared_object_congestion_tracker = SharedObjectCongestionTracker::new_for_test(
            vec![],
            per_object_congestion_control_mode,
            min_free_execution_slot_assigned,
        );

        for (object_id, duration, gas_price) in init_values {
            match per_object_congestion_control_mode {
                PerObjectCongestionControlMode::None => {}
                PerObjectCongestionControlMode::TotalGasBudget => {
                    let certificate =
                        build_transaction(&[(*object_id, true)], *duration, *gas_price);

                    let execution_start_time = initialize_tracker_and_compute_tx_start_time(
                        &mut shared_object_congestion_tracker,
                        &certificate.shared_input_objects().collect::<Vec<_>>(),
                        *duration,
                    )
                    .expect(
                        "initial value should fit within the available range of slots in the \
                                tracker",
                    );

                    shared_object_congestion_tracker
                        .bump_object_execution_slots(&certificate, execution_start_time);

                    suggested_gas_price_calculator.update_congestion_info(
                        &certificate,
                        execution_start_time,
                        *duration,
                    );
                }
                PerObjectCongestionControlMode::TotalTxCount => {
                    for _ in 0..*duration {
                        let certificate = build_transaction(&[(*object_id, true)], 1, *gas_price);

                        let execution_start_time = initialize_tracker_and_compute_tx_start_time(
                            &mut shared_object_congestion_tracker,
                            &certificate.shared_input_objects().collect::<Vec<_>>(),
                            *duration,
                        )
                        .expect(
                            "initial value should fit within the available range of slots in \
                                    the tracker",
                        );

                        shared_object_congestion_tracker
                            .bump_object_execution_slots(&certificate, execution_start_time);

                        suggested_gas_price_calculator.update_congestion_info(
                            &certificate,
                            execution_start_time,
                            1,
                        );
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
        shared_object_congestion_tracker::{
            ExecutionTime, SequencingResult, SharedObjectCongestionTracker,
            shared_object_test_utils::build_transaction,
        },
        suggested_gas_price_calculator::{
            PerCommitCongestionInfo, PerObjectCongestionInfo, ScheduledTransactionCongestionInfo,
        },
    };

    const REFERENCE_GAS_PRICE: u64 = 1_000;

    #[derive(Copy, Clone)]
    struct TxGasData {
        global_ordering_index: usize,
        gas_price: u64,
        gas_budget: u64,
    }

    fn build_and_try_sequencing_certificate(
        input_shared_objects: &[(ObjectID, bool)],
        tx_gas_data: TxGasData,
        max_execution_duration_per_commit: ExecutionTime,
        shared_object_congestion_tracker: &mut SharedObjectCongestionTracker,
    ) -> (VerifiedExecutableTransaction, SequencingResult) {
        let certificate = build_transaction(
            input_shared_objects,
            tx_gas_data.gas_budget,
            tx_gas_data.gas_price,
        );
        let shared_input_objects: Vec<_> = certificate.shared_input_objects().collect();
        shared_object_congestion_tracker.initialize_object_execution_slots(&shared_input_objects);

        let sequencing_result = shared_object_congestion_tracker.try_schedule(
            &certificate,
            max_execution_duration_per_commit,
            // The remaining inputs are not important for this test
            0,
            &HashMap::new(),
            0,
        );

        (certificate, sequencing_result)
    }

    fn update_data_for_scheduled_certificate(
        certificate: &VerifiedExecutableTransaction,
        execution_start_time: ExecutionTime,
        shared_object_congestion_tracker: &mut SharedObjectCongestionTracker,
        suggested_gas_price_calculator: &mut SuggestedGasPriceCalculator,
    ) {
        shared_object_congestion_tracker
            .bump_object_execution_slots(certificate, execution_start_time);
        suggested_gas_price_calculator.update_congestion_info(
            certificate,
            execution_start_time,
            shared_object_congestion_tracker.get_estimated_execution_duration(certificate),
        );
    }

    #[rstest]
    fn update_congestion_info(
        #[values(
            None,
            Some(10), // the value is not important in this test
        )]
        max_execution_duration_per_commit: Option<ExecutionTime>,
    ) {
        let max_gas_price = ProtocolConfig::get_for_max_version_UNSAFE().max_gas_price();
        let mut suggested_gas_price_calculator = SuggestedGasPriceCalculator::new(
            max_execution_duration_per_commit,
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
        let execution_start_time_1 = 0;
        let estimated_execution_duration_1 = 3;
        // Update the calculator's congestion info for this certificate.
        suggested_gas_price_calculator.update_congestion_info(
            &certificate_1,
            execution_start_time_1,
            estimated_execution_duration_1,
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
        let objects_2 = vec![(object_2, true), (object_3, false), (object_4, true)];
        let gas_budget_2 = 1_002_000; // not important in this test
        let gas_price_2 = 1_002;
        let certificate_2 = build_transaction(&objects_2, gas_budget_2, gas_price_2);
        let execution_start_time_2 = 1;
        let estimated_execution_duration_2 = 2;
        // Update the calculator's congestion info for this certificate.
        suggested_gas_price_calculator.update_congestion_info(
            &certificate_2,
            execution_start_time_2,
            estimated_execution_duration_2,
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
        let objects_3 = vec![(object_4, false), (object_5, true)];
        let gas_budget_3 = 1_001_000; // not important in this test
        let gas_price_3 = 1_001;
        let certificate_3 = build_transaction(&objects_3, gas_budget_3, gas_price_3);
        let execution_start_time_3 = 2;
        let estimated_execution_duration_3 = 1;
        // Update the calculator's congestion info for this certificate.
        suggested_gas_price_calculator.update_congestion_info(
            &certificate_3,
            execution_start_time_3,
            estimated_execution_duration_3,
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

    #[rstest]
    fn calculate_suggested_gas_price(
        #[values(
            PerObjectCongestionControlMode::TotalTxCount,
            PerObjectCongestionControlMode::TotalGasBudget
        )]
        mode: PerObjectCongestionControlMode,
        #[values(false, true)] min_free_execution_slot_assigned: bool,
    ) {
        // Allow only two transactions per shared object per commit. In the
        // `TotalGasBudget` mode, gas budget of transactions will be set
        // accordingly.
        let max_execution_duration_per_commit = match mode {
            PerObjectCongestionControlMode::None => unreachable!(),
            PerObjectCongestionControlMode::TotalTxCount => 3,
            PerObjectCongestionControlMode::TotalGasBudget => 9_000_000,
        };

        let max_gas_price = ProtocolConfig::get_for_max_version_UNSAFE().max_gas_price();

        let mut shared_object_congestion_tracker = SharedObjectCongestionTracker::new_for_test(
            vec![],
            mode,
            min_free_execution_slot_assigned,
        );

        let mut suggested_gas_price_calculator = SuggestedGasPriceCalculator::new(
            Some(max_execution_duration_per_commit),
            REFERENCE_GAS_PRICE,
            max_gas_price,
        );

        let object_1 = ObjectID::random();
        let object_2 = ObjectID::random();

        // Gas prices (sorted in descending order) and gas budget to build transactions
        let txs_gas_data = [
            (max_gas_price, 3_000_000), // 0
            (9_000, 1_000_000),         // 1
            (8_000, 4_000_000),         // 2
            (7_000, 2_000_000),         // 3
            (7_000, 1_000_001),         // 4
            (7_000, 5_000_000),         // 5
            (7_000, 5_000_001),         // 6
            (7_000, 8_000_000),         // 7
            (6_000, 4_000_000),         // 8
            (5_000, 2_000_000),         // 9
            (5_000, 1_000_001),         // 10
            (5_000, 5_000_001),         // 11
            (5_000, 9_000_000),         // 12
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (gas_price, gas_budget))| TxGasData {
            global_ordering_index: index,
            gas_price,
            gas_budget,
        })
        .collect::<Vec<_>>();

        // Construct a certificate with some shared objects (note mutability),
        // and try scheduling it.
        let (certificate, sequencing_result) = build_and_try_sequencing_certificate(
            &[(object_1, true), (object_2, false)],
            txs_gas_data[0],
            max_execution_duration_per_commit,
            &mut shared_object_congestion_tracker,
        );
        // Allocations of mutably accessed shared objects should look as follows:
        // |-------------------------------------------------|------------|
        // |        object_1        |        object_2        | start time |
        // |________________________|________________________|____________|
        // |------------------------|------------------------|---- 9M     |
        // |                        |                        |            |
        // |::::::::::::::::::::::::|::::::::::::::::::::::::|::::::::::::|
        // |                        |                        |            |
        // |------------------------|                        |---- 3M     |
        // |                        |                        |            |
        // |                        |                        |---- 2M     |
        // | cert. 0 (g=100K, d=3M) |                        |            |
        // |                        |                        |---- 1M     |
        // |                        |                        |            |
        // |-------------------------------------------------|---- 0 -----|
        if let SequencingResult::Schedule(execution_start_time) = sequencing_result {
            update_data_for_scheduled_certificate(
                &certificate,
                execution_start_time,
                &mut shared_object_congestion_tracker,
                &mut suggested_gas_price_calculator,
            );
        } else {
            panic!(
                "Certificate {} must be scheduled",
                txs_gas_data[0].global_ordering_index
            );
        }

        // Construct a certificate with some shared objects (note mutability),
        // and try scheduling it.
        let (certificate, sequencing_result) = build_and_try_sequencing_certificate(
            &[(object_1, false), (object_2, true)],
            txs_gas_data[1],
            max_execution_duration_per_commit,
            &mut shared_object_congestion_tracker,
        );
        // Allocations of mutably accessed shared objects should look as follows:
        // |-------------------------------------------------|------------|
        // |        object_1        |        object_2        | start time |
        // |________________________|________________________|____________|
        // |------------------------|------------------------|---- 9M     |
        // |                        |                        |            |
        // |::::::::::::::::::::::::|::::::::::::::::::::::::|::::::::::::|
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
        // Certificate 1 cannot be scheduled at start time 0 because it touches
        // object 1, even though immutably.
        if let SequencingResult::Schedule(execution_start_time) = sequencing_result {
            update_data_for_scheduled_certificate(
                &certificate,
                execution_start_time,
                &mut shared_object_congestion_tracker,
                &mut suggested_gas_price_calculator,
            );
        } else {
            panic!(
                "Certificate {} must be scheduled",
                txs_gas_data[1].global_ordering_index
            );
        }

        // Construct a certificate with some shared objects (note mutability),
        // and try scheduling it.
        let (certificate, sequencing_result) = build_and_try_sequencing_certificate(
            &[(object_1, false), (object_2, true)],
            txs_gas_data[2],
            max_execution_duration_per_commit,
            &mut shared_object_congestion_tracker,
        );
        // Allocations of mutably accessed shared objects should look as follows:
        // |-------------------------------------------------|------------|
        // |        object_1        |        object_2        | start time |
        // |________________________|________________________|____________|
        // |------------------------|------------------------|---- 9M     |
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
        if let SequencingResult::Schedule(execution_start_time) = sequencing_result {
            update_data_for_scheduled_certificate(
                &certificate,
                execution_start_time,
                &mut shared_object_congestion_tracker,
                &mut suggested_gas_price_calculator,
            );
        } else {
            panic!(
                "Certificate {} must be scheduled",
                txs_gas_data[2].global_ordering_index
            );
        }

        // Construct a certificate with some shared objects (note mutability),
        // and try scheduling it.
        let (certificate, sequencing_result) = build_and_try_sequencing_certificate(
            &[(object_2, true)],
            txs_gas_data[3],
            max_execution_duration_per_commit,
            &mut shared_object_congestion_tracker,
        );
        // If `min_free_execution_slot_assigned = true`, allocations of mutably
        // accessed shared objects should look as follows:
        // |-------------------------------------------------|------------|
        // |        object_1        |        object_2        | start time |
        // |________________________|________________________|____________|
        // |------------------------|------------------------|---- 9M     |
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
        // If `min_free_execution_slot_assigned = false` (old sequencer), this
        // certificate must be deferred.
        if min_free_execution_slot_assigned {
            // ^ This corresponds the new sequencer's logic
            if let SequencingResult::Schedule(execution_start_time) = sequencing_result {
                update_data_for_scheduled_certificate(
                    &certificate,
                    execution_start_time,
                    &mut shared_object_congestion_tracker,
                    &mut suggested_gas_price_calculator,
                );
            } else {
                panic!(
                    "Certificate {} must be scheduled in the new sequencer",
                    txs_gas_data[3].global_ordering_index
                );
            }
        } else {
            // ^ This corresponds the old sequencer's logic
            if let SequencingResult::Defer(_key, congested_objects) = sequencing_result {
                assert_eq!(congested_objects, vec![object_2]);

                let suggested_gas_price = suggested_gas_price_calculator
                    .calculate_suggested_gas_price(
                        &certificate,
                        shared_object_congestion_tracker
                            .get_estimated_execution_duration(&certificate),
                    );
                assert_eq!(suggested_gas_price, txs_gas_data[2].gas_price + 1);
            } else {
                panic!(
                    "Certificate {} must be deferred in the old sequencer",
                    txs_gas_data[3].global_ordering_index
                );
            }
        }

        // Construct a certificate with some shared objects (note mutability),
        // and try scheduling it.
        let input_shared_objects = vec![(object_2, false)];
        let (certificate, sequencing_result) = build_and_try_sequencing_certificate(
            &input_shared_objects,
            txs_gas_data[4],
            max_execution_duration_per_commit,
            &mut shared_object_congestion_tracker,
        );
        // If `min_free_execution_slot_assigned = true`, allocations of mutably
        // accessed shared objects should look as follows:
        // |-------------------------------------------------|------------|
        // |        object_1        |        object_2        | start time |
        // |________________________|________________________|____________|
        // |------------------------|------------------------|---- 9M     |
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
        // That is, this certificate must be deferred in both new and old sequencers.
        if let SequencingResult::Defer(_key, congested_objects) = sequencing_result {
            if min_free_execution_slot_assigned {
                // ^ this corresponds the new sequencer's logic
                assert_eq!(
                    congested_objects,
                    input_shared_objects
                        .into_iter()
                        .map(|(id, _)| id)
                        .collect::<Vec<_>>()
                );
            } else {
                // ^ this corresponds the old sequencer's logic
                assert_eq!(congested_objects, vec![object_2]);
            }

            let suggested_gas_price = suggested_gas_price_calculator.calculate_suggested_gas_price(
                &certificate,
                shared_object_congestion_tracker.get_estimated_execution_duration(&certificate),
            );
            assert_eq!(suggested_gas_price, txs_gas_data[2].gas_price + 1);
        } else {
            panic!(
                "Certificate {} must be deferred",
                txs_gas_data[4].global_ordering_index
            );
        }

        // Construct a certificate with some shared objects (note mutability),
        // and try scheduling it.
        let input_shared_objects = vec![(object_2, true)];
        let (certificate, sequencing_result) = build_and_try_sequencing_certificate(
            &input_shared_objects,
            txs_gas_data[5],
            max_execution_duration_per_commit,
            &mut shared_object_congestion_tracker,
        );
        // If `min_free_execution_slot_assigned = true`, allocations of mutably
        // accessed shared objects should look as follows:
        // |-------------------------------------------------|------------|
        // |        object_1        |        object_2        | start time |
        // |________________________|________________________|____________|
        // |------------------------|------------------------|---- 9M     |
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
        // That is, this certificate must be deferred in both new and old sequencers.
        if let SequencingResult::Defer(_key, congested_objects) = sequencing_result {
            if min_free_execution_slot_assigned {
                // ^ this corresponds the new sequencer's logic
                assert_eq!(
                    congested_objects,
                    input_shared_objects
                        .into_iter()
                        .map(|(id, _)| id)
                        .collect::<Vec<_>>()
                );
            } else {
                // ^ this corresponds the old sequencer's logic
                assert_eq!(congested_objects, vec![object_2]);
            }

            let suggested_gas_price = suggested_gas_price_calculator.calculate_suggested_gas_price(
                &certificate,
                shared_object_congestion_tracker.get_estimated_execution_duration(&certificate),
            );
            assert_eq!(suggested_gas_price, txs_gas_data[2].gas_price + 1);
        } else {
            panic!(
                "Certificate {} must be deferred",
                txs_gas_data[5].global_ordering_index
            );
        }

        // Construct a certificate with some shared objects (note mutability),
        // and try scheduling it.
        let input_shared_objects = vec![(object_1, true), (object_2, true)];
        let (certificate, sequencing_result) = build_and_try_sequencing_certificate(
            &input_shared_objects,
            txs_gas_data[6],
            max_execution_duration_per_commit,
            &mut shared_object_congestion_tracker,
        );
        // If `min_free_execution_slot_assigned = true`, allocations of mutably
        // accessed shared objects should look as follows:
        // |-------------------------------------------------|------------|
        // |        object_1        |        object_2        | start time |
        // |________________________|________________________|____________|
        // |------------------------|------------------------|---- 9M     |
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
        // That is, this certificate must be deferred in both new and old sequencers.
        if let SequencingResult::Defer(_key, congested_objects) = sequencing_result {
            if min_free_execution_slot_assigned {
                // ^ this corresponds the new sequencer's logic
                assert_eq!(
                    congested_objects,
                    input_shared_objects
                        .into_iter()
                        .map(|(id, _)| id)
                        .collect::<Vec<_>>()
                );
            } else {
                // ^ this corresponds the old sequencer's logic
                assert_eq!(congested_objects, vec![object_2]);
            }

            let suggested_gas_price = suggested_gas_price_calculator.calculate_suggested_gas_price(
                &certificate,
                shared_object_congestion_tracker.get_estimated_execution_duration(&certificate),
            );
            match mode {
                PerObjectCongestionControlMode::None => unreachable!(),
                PerObjectCongestionControlMode::TotalTxCount => {
                    assert_eq!(suggested_gas_price, txs_gas_data[2].gas_price + 1);
                }
                PerObjectCongestionControlMode::TotalGasBudget => {
                    assert_eq!(suggested_gas_price, txs_gas_data[1].gas_price + 1);
                }
            }
        } else {
            panic!(
                "Certificate {} must be deferred",
                txs_gas_data[6].global_ordering_index
            );
        }

        // Construct a certificate with some shared objects (note mutability),
        // and try scheduling it.
        let input_shared_objects = vec![(object_1, true), (object_2, true)];
        let (certificate, sequencing_result) = build_and_try_sequencing_certificate(
            &input_shared_objects,
            txs_gas_data[7],
            max_execution_duration_per_commit,
            &mut shared_object_congestion_tracker,
        );
        // If `min_free_execution_slot_assigned = true`, allocations of mutably
        // accessed shared objects should look as follows:
        // |-------------------------------------------------|------------|
        // |        object_1        |        object_2        | start time |
        // |________________________|________________________|____________|
        // |------------------------|------------------------|---- 9M     |
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
        // That is, this certificate must be deferred in both new and old sequencers.
        if let SequencingResult::Defer(_key, congested_objects) = sequencing_result {
            let suggested_gas_price = suggested_gas_price_calculator.calculate_suggested_gas_price(
                &certificate,
                shared_object_congestion_tracker.get_estimated_execution_duration(&certificate),
            );

            if min_free_execution_slot_assigned {
                // ^ this corresponds the new sequencer's logic
                assert_eq!(
                    congested_objects,
                    input_shared_objects
                        .into_iter()
                        .map(|(id, _)| id)
                        .collect::<Vec<_>>()
                );
            } else {
                // ^ this corresponds the old sequencer's logic
                match mode {
                    PerObjectCongestionControlMode::None => unreachable!(),
                    PerObjectCongestionControlMode::TotalTxCount => {
                        assert_eq!(congested_objects, vec![object_2]);
                    }
                    PerObjectCongestionControlMode::TotalGasBudget => {
                        assert_eq!(
                            congested_objects,
                            input_shared_objects
                                .into_iter()
                                .map(|(id, _)| id)
                                .collect::<Vec<_>>()
                        );
                    }
                }
            }

            match mode {
                PerObjectCongestionControlMode::None => unreachable!(),
                PerObjectCongestionControlMode::TotalTxCount => {
                    assert_eq!(suggested_gas_price, txs_gas_data[2].gas_price + 1);
                }
                PerObjectCongestionControlMode::TotalGasBudget => {
                    assert_eq!(suggested_gas_price, max_gas_price);
                }
            }
        } else {
            panic!(
                "Certificate {} must be deferred",
                txs_gas_data[7].global_ordering_index
            );
        }

        // Construct a certificate with some shared objects (note mutability),
        // and try scheduling it.
        let (certificate, sequencing_result) = build_and_try_sequencing_certificate(
            &[(object_1, true)],
            txs_gas_data[8],
            max_execution_duration_per_commit,
            &mut shared_object_congestion_tracker,
        );
        // If `min_free_execution_slot_assigned = true`, allocations of mutably
        // accessed shared objects should look as follows:
        // |-------------------------------------------------|------------|
        // |        object_1        |        object_2        | start time |
        // |________________________|________________________|____________|
        // |------------------------|------------------------|---- 9M     |
        // |                        |                        |            |
        // |                        |------------------------|---- 8M     |
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
        if let SequencingResult::Schedule(execution_start_time) = sequencing_result {
            update_data_for_scheduled_certificate(
                &certificate,
                execution_start_time,
                &mut shared_object_congestion_tracker,
                &mut suggested_gas_price_calculator,
            );
        } else {
            panic!(
                "Certificate {} must be scheduled",
                txs_gas_data[8].global_ordering_index
            );
        }

        // Construct a certificate with some shared objects (note mutability),
        // and try scheduling it.
        let (certificate, sequencing_result) = build_and_try_sequencing_certificate(
            &[(object_1, true)],
            txs_gas_data[9],
            max_execution_duration_per_commit,
            &mut shared_object_congestion_tracker,
        );
        // If `min_free_execution_slot_assigned = true`, allocations of mutably
        // accessed shared objects should look as follows:
        // |-------------------------------------------------|------------|
        // |        object_1        |        object_2        | start time |
        // |________________________|________________________|____________|
        // |------------------------|------------------------|---- 9M     |
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
        if let SequencingResult::Schedule(execution_start_time) = sequencing_result {
            update_data_for_scheduled_certificate(
                &certificate,
                execution_start_time,
                &mut shared_object_congestion_tracker,
                &mut suggested_gas_price_calculator,
            );
        } else {
            panic!(
                "Certificate {} must be scheduled",
                txs_gas_data[9].global_ordering_index
            );
        }

        // Construct a certificate with some shared objects (note mutability),
        // and try scheduling it.
        let input_shared_objects = vec![(object_1, false), (object_2, false)];
        let (certificate, sequencing_result) = build_and_try_sequencing_certificate(
            &input_shared_objects,
            txs_gas_data[10],
            max_execution_duration_per_commit,
            &mut shared_object_congestion_tracker,
        );
        // If `min_free_execution_slot_assigned = true`, allocations of mutably
        // accessed shared objects should look as follows:
        // |-------------------------------------------------|------------|
        // |        object_1        |        object_2        | start time |
        // |________________________|________________________|____________|
        // |------------------------|------------------------|---- 9M     |
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
        // That is, this certificate must be deferred in both new and old sequencers.
        if let SequencingResult::Defer(_key, congested_objects) = sequencing_result {
            assert_eq!(
                congested_objects,
                input_shared_objects
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect::<Vec<_>>()
            );

            let suggested_gas_price = suggested_gas_price_calculator.calculate_suggested_gas_price(
                &certificate,
                shared_object_congestion_tracker.get_estimated_execution_duration(&certificate),
            );
            assert_eq!(suggested_gas_price, txs_gas_data[2].gas_price + 1);
        } else {
            panic!(
                "Certificate {} must be deferred",
                txs_gas_data[10].global_ordering_index
            );
        }

        // Construct a certificate with some shared objects (note mutability),
        // and try scheduling it.
        let input_shared_objects = vec![(object_1, true), (object_2, false)];
        let (certificate, sequencing_result) = build_and_try_sequencing_certificate(
            &input_shared_objects,
            txs_gas_data[11],
            max_execution_duration_per_commit,
            &mut shared_object_congestion_tracker,
        );
        // If `min_free_execution_slot_assigned = true`, allocations of mutably
        // accessed shared objects should look as follows:
        // |-------------------------------------------------|------------|
        // |        object_1        |        object_2        | start time |
        // |________________________|________________________|____________|
        // |------------------------|------------------------|---- 9M     |
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
        // That is, this certificate must be deferred in both new and old sequencers.
        if let SequencingResult::Defer(_key, congested_objects) = sequencing_result {
            assert_eq!(
                congested_objects,
                input_shared_objects
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect::<Vec<_>>()
            );

            let suggested_gas_price = suggested_gas_price_calculator.calculate_suggested_gas_price(
                &certificate,
                shared_object_congestion_tracker.get_estimated_execution_duration(&certificate),
            );
            match mode {
                PerObjectCongestionControlMode::None => unreachable!(),
                PerObjectCongestionControlMode::TotalTxCount => {
                    assert_eq!(suggested_gas_price, txs_gas_data[2].gas_price + 1);
                }
                PerObjectCongestionControlMode::TotalGasBudget => {
                    assert_eq!(suggested_gas_price, txs_gas_data[1].gas_price + 1);
                }
            }
        } else {
            panic!(
                "Certificate {} must be deferred",
                txs_gas_data[11].global_ordering_index
            );
        }

        // Construct a certificate with some shared objects (note mutability),
        // and try scheduling it.
        let input_shared_objects = vec![(object_1, false), (object_2, true)];
        let (certificate, sequencing_result) = build_and_try_sequencing_certificate(
            &input_shared_objects,
            txs_gas_data[12],
            max_execution_duration_per_commit,
            &mut shared_object_congestion_tracker,
        );
        // If `min_free_execution_slot_assigned = true`, allocations of mutably
        // accessed shared objects should look as follows:
        // |-------------------------------------------------|------------|
        // |        object_1        |        object_2        | start time |
        // |________________________|________________________|____________|
        // |------------------------|------------------------|---- 9M     |
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
        // That is, this certificate must be deferred in both new and old sequencers.
        if let SequencingResult::Defer(_key, congested_objects) = sequencing_result {
            assert_eq!(
                congested_objects,
                input_shared_objects
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect::<Vec<_>>()
            );

            let suggested_gas_price = suggested_gas_price_calculator.calculate_suggested_gas_price(
                &certificate,
                shared_object_congestion_tracker.get_estimated_execution_duration(&certificate),
            );
            match mode {
                PerObjectCongestionControlMode::None => unreachable!(),
                PerObjectCongestionControlMode::TotalTxCount => {
                    assert_eq!(suggested_gas_price, txs_gas_data[2].gas_price + 1);
                }
                PerObjectCongestionControlMode::TotalGasBudget => {
                    assert_eq!(suggested_gas_price, max_gas_price);
                }
            }
        } else {
            panic!(
                "Certificate {} must be deferred",
                txs_gas_data[12].global_ordering_index
            );
        }
    }
}
