use std::collections::{BTreeSet, HashMap};

use iota_types::{
    base_types::ObjectID, executable_transaction::VerifiedExecutableTransaction,
    transaction::TransactionDataAPI,
};
use rayon::iter::{
    IntoParallelIterator, IntoParallelRefIterator, ParallelBridge, ParallelExtend, ParallelIterator,
};
use tracing::instrument;

use super::shared_object_congestion_tracker::ExecutionTime;

/// Holds shared object congestion info for a single scheduled shared-object
/// transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScheduledTransactionCongestionInfo {
    /// Gas price of a scheduled shared-object transaction.
    gas_price: u64,

    /// Execution start time of scheduled shared-object transaction.
    execution_start_time: ExecutionTime,

    /// Estimated execution duration of a scheduled shared-object transaction.
    estimated_execution_duration: ExecutionTime,
}

impl ScheduledTransactionCongestionInfo {
    /// Create a new congestion info for scheduled shared-object transaction
    /// with gas price `gas_price` execution start time `execution_start_time`,
    /// and estimated execution duration `estimated_execution_duration`.
    fn new(
        gas_price: u64,
        execution_start_time: ExecutionTime,
        estimated_execution_duration: ExecutionTime,
    ) -> Self {
        Self {
            gas_price,
            execution_start_time,
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
    /// Per-commit congestion info
    congestion_info: PerCommitCongestionInfo,

    /// Maximum execution duration per shared object per commit.
    max_execution_duration_per_commit: Option<ExecutionTime>,

    /// Flag indicating where the minimum free execution slot to schedule
    /// execution of a transaction is assigned in the shared object congestion
    /// tracker (sequencer). If `false`, this corresponds to the old Sui's
    /// canonical sequencer logic.
    min_free_execution_slot_assigned: bool,

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
            execution_start_time,
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
            debug_assert!(
                estimated_execution_duration <= max_execution_duration_per_commit,
                "This certificate alone has estimated execution duration of \
                {estimated_execution_duration}, which is larger than the maximum execution \
                duration per commit {max_execution_duration_per_commit}, so the certificate \
                cannot be scheduled regardless of suggested gas price. It is likely that \
                {max_execution_duration_per_commit} was set too low in the protocol config, \
                such that a commit cannot accomodate a single certificate."
            );

            let possible_start_times = self.find_possible_start_times(
                certificate,
                estimated_execution_duration,
                max_execution_duration_per_commit,
            );

            let passing_gas_price = if self.min_free_execution_slot_assigned {
                // ^ This corresponds to the new sequencer's logic.

                possible_start_times
                    .into_par_iter()
                    .map(|start_time| {
                        self.find_passing_gas_price_at_start_time(
                            certificate,
                            start_time,
                            estimated_execution_duration,
                        )
                    })
                    // Take the minimum across possible start times, since the new sequencer
                    // might schedule a certificate with lower gas prices at lower execution
                    // start times.
                    .min()
                    .unwrap_or_else(|| {
                        panic!(
                            "This certificate alone has estimated execution duration of \
                            {estimated_execution_duration}, which is larger than the maximum \
                            execution duration per commit {max_execution_duration_per_commit}, \
                            so the certificate cannot be scheduled regardless of suggested gas \
                            price."
                        );
                    })
            } else {
                // ^ This corresponds to the old Sui's canonical sequencer logic.

                let start_time = *possible_start_times
                    .last()
                    .expect("There must be at least one possible start time, which is always 0.");

                self.find_passing_gas_price_at_start_time(
                    certificate,
                    start_time,
                    estimated_execution_duration,
                )
            };

            // Suggested gas price equals passing_gas_price + 1. We add 1 to make this
            // transaction would be scheduled if the same commit structure was repeated.
            let suggested_gas_price = passing_gas_price + 1;

            // Make sure suggested gas price is not larger than the maximum possible gas
            // price.
            suggested_gas_price.min(self.max_gas_price)
        } else {
            // ^ If we don't have a max execution duration, suggest the reference gas price.

            self.reference_gas_price
        }
    }

    /// Find execution start times at which this deferred/cancelled certificate
    /// could be scheduled with its estimated execution duration.
    fn find_possible_start_times(
        &self,
        certificate: &VerifiedExecutableTransaction,
        estimated_execution_duration: ExecutionTime,
        max_execution_duration_per_commit: ExecutionTime,
    ) -> BTreeSet<ExecutionTime> {
        let max_possible_start_time =
            max_execution_duration_per_commit - estimated_execution_duration;

        let mut possible_start_times = BTreeSet::from([ExecutionTime::MIN]);
        possible_start_times.par_extend(
            certificate
                .shared_input_objects()
                .par_bridge()
                .filter_map(|object| {
                    self.congestion_info
                        .get(&object.id)
                        .map(|per_object_congestion_info| {
                            per_object_congestion_info.par_iter().flat_map(|tx| {
                                let end_time =
                                    tx.execution_start_time + tx.estimated_execution_duration;

                                if end_time <= max_possible_start_time {
                                    vec![tx.execution_start_time, end_time]
                                } else if tx.execution_start_time <= max_possible_start_time {
                                    vec![tx.execution_start_time]
                                } else {
                                    vec![]
                                }
                            })
                        })
                })
                .flatten(),
        );

        possible_start_times
    }

    /// Find the gas price for which a deferred/scheduled certificate would be
    /// scheduled at execution `start_time` if that gas price was payed.
    fn find_passing_gas_price_at_start_time(
        &self,
        certificate: &VerifiedExecutableTransaction,
        start_time: ExecutionTime,
        estimated_execution_duration: ExecutionTime,
    ) -> u64 {
        let end_time = start_time + estimated_execution_duration;

        certificate
            .shared_input_objects()
            .par_bridge()
            .filter_map(|object| {
                self.congestion_info
                    .get(&object.id)
                    .map(|per_object_congestion_info| {
                        per_object_congestion_info
                            .par_iter()
                            .filter_map(|tx| {
                                if (tx.execution_start_time >= start_time
                                    && tx.execution_start_time < end_time)
                                    || (tx.execution_start_time + tx.estimated_execution_duration
                                        > start_time
                                        && tx.execution_start_time
                                            + tx.estimated_execution_duration
                                            <= end_time)
                                {
                                    Some(tx.gas_price)
                                } else {
                                    None
                                }
                            })
                            .max()
                    })
            })
            .max()
            .flatten()
            .unwrap_or_else(|| {
                panic!(
                    "At least one of the shared input objects should have appeared in between \
                    execution start time of {start_time} and execution end time of {end_time}; \
                    otherwise, this deferred certificate would be scheduled by the sequencer."
                );
            })
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
        gas_price: u64,
        gas_budget: u64,
        max_execution_duration_per_commit: ExecutionTime,
        shared_object_congestion_tracker: &mut SharedObjectCongestionTracker,
    ) -> (VerifiedExecutableTransaction, SequencingResult) {
        let certificate = build_transaction(input_shared_objects, gas_budget, gas_price);
        let shared_input_objects: Vec<_> = certificate.shared_input_objects().collect();
        shared_object_congestion_tracker.initialize_object_execution_slots(&shared_input_objects);

        let sequencing_result = shared_object_congestion_tracker.try_schedule(
            &certificate,
            max_execution_duration_per_commit,
            // The next two inputs are not important for testing.
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
            let object_1_expected_congestion_info =
                PerObjectCongestionInfo::from([ScheduledTransactionCongestionInfo {
                    gas_price: gas_price_1,
                    execution_start_time: execution_start_time_1,
                    estimated_execution_duration: estimated_execution_duration_1,
                }]);
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
            let object_1_expected_congestion_info =
                PerObjectCongestionInfo::from([ScheduledTransactionCongestionInfo {
                    gas_price: gas_price_1,
                    execution_start_time: execution_start_time_1,
                    estimated_execution_duration: estimated_execution_duration_1,
                }]);
            let object_2_expected_congestion_info =
                PerObjectCongestionInfo::from([ScheduledTransactionCongestionInfo {
                    gas_price: gas_price_2,
                    execution_start_time: execution_start_time_2,
                    estimated_execution_duration: estimated_execution_duration_2,
                }]);
            let object_4_expected_congestion_info =
                PerObjectCongestionInfo::from([ScheduledTransactionCongestionInfo {
                    gas_price: gas_price_2,
                    execution_start_time: execution_start_time_2,
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
            let object_1_expected_congestion_info =
                PerObjectCongestionInfo::from([ScheduledTransactionCongestionInfo {
                    gas_price: gas_price_1,
                    execution_start_time: execution_start_time_1,
                    estimated_execution_duration: estimated_execution_duration_1,
                }]);
            let object_2_expected_congestion_info =
                PerObjectCongestionInfo::from([ScheduledTransactionCongestionInfo {
                    gas_price: gas_price_2,
                    execution_start_time: execution_start_time_2,
                    estimated_execution_duration: estimated_execution_duration_2,
                }]);
            let object_4_expected_congestion_info =
                PerObjectCongestionInfo::from([ScheduledTransactionCongestionInfo {
                    gas_price: gas_price_2,
                    execution_start_time: execution_start_time_2,
                    estimated_execution_duration: estimated_execution_duration_2,
                }]);
            let object_5_expected_congestion_info =
                PerObjectCongestionInfo::from([ScheduledTransactionCongestionInfo {
                    gas_price: gas_price_3,
                    execution_start_time: execution_start_time_3,
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
        // Allow only two transactions per shared object per commit. In the
        // `TotalGasBudget` mode, gas budget of transactions will be set
        // accordingly.
        let max_execution_duration_per_commit = match mode {
            PerObjectCongestionControlMode::None => unreachable!(),
            PerObjectCongestionControlMode::TotalTxCount => 3,
            PerObjectCongestionControlMode::TotalGasBudget => 40_000_000,
        };

        let max_gas_price = ProtocolConfig::get_for_max_version_UNSAFE().max_gas_price();

        let mut shared_object_congestion_tracker =
            SharedObjectCongestionTracker::new(mode, min_free_execution_slot_assigned);

        let mut suggested_gas_price_calculator = SuggestedGasPriceCalculator::new(
            Some(max_execution_duration_per_commit),
            min_free_execution_slot_assigned,
            REFERENCE_GAS_PRICE,
            max_gas_price,
        );

        let object_1 = ObjectID::random();
        let object_2 = ObjectID::random();

        // Gas prices (sorted in descending order) and gas budget to build transactions
        let txs_gas_data = [
            (max_gas_price, 12_000_000),
            (9_000, 10_000_000),
            (8_000, 13_000_000),
            (7_000, 11_000_000),
            (7_000, 11_000_000),
            (7_000, 14_000_000),
            (7_000, 28_000_000),
            (7_000, 28_000_001),
            (6_000, 13_000_000),
            (5_000, 15_000_000),
            (5_000, 10_000_000),
            (5_000, 20_000_000),
            (5_000, 30_000_000),
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
        let input_shared_objects = vec![(object_1, true), (object_2, false)];
        let tx_gas_data = *txs_gas_data.first().unwrap();
        let (certificate, sequencing_result) = build_and_try_sequencing_certificate(
            &input_shared_objects,
            tx_gas_data.gas_price,
            tx_gas_data.gas_budget,
            max_execution_duration_per_commit,
            &mut shared_object_congestion_tracker,
        );
        // Allocations of mutably accessed shared objects should look as follows:
        // |----------------------------------------------------
        // |        object_1         |        object_2         |
        // |_________________________|_________________________|
        // |-------------------------|-------------------------|
        // |                         |                         |
        // |-------------------------|-------------------------|
        // |                         |                         |
        // |-------------------------|-------------------------|
        // | cert. 0 (g=100K, d=12M) |                         |
        // |----------------------------------------------------
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
                tx_gas_data.global_ordering_index
            );
        }

        // Construct a certificate with some shared objects (note mutability),
        // and try scheduling it.
        let input_shared_objects = vec![(object_1, false), (object_2, true)];
        let tx_gas_data = *txs_gas_data.get(1).unwrap();
        let (certificate, sequencing_result) = build_and_try_sequencing_certificate(
            &input_shared_objects,
            tx_gas_data.gas_price,
            tx_gas_data.gas_budget,
            max_execution_duration_per_commit,
            &mut shared_object_congestion_tracker,
        );
        // Allocations of mutably accessed shared objects should look as follows:
        // |----------------------------------------------------
        // |        object_1         |        object_2         |
        // |_________________________|_________________________|
        // |-------------------------|-------------------------|
        // |                         |                         |
        // |-------------------------|-------------------------|
        // |                         | cert. 1 (g=9000, d=10M) |
        // |-------------------------|-------------------------|
        // | cert. 0 (g=100K, d=12M) |                         |
        // |----------------------------------------------------
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
                tx_gas_data.global_ordering_index
            );
        }

        // Construct a certificate with some shared objects (note mutability),
        // and try scheduling it.
        let input_shared_objects = vec![(object_1, false), (object_2, true)];
        let tx_gas_data = *txs_gas_data.get(2).unwrap();
        let (certificate, sequencing_result) = build_and_try_sequencing_certificate(
            &input_shared_objects,
            tx_gas_data.gas_price,
            tx_gas_data.gas_budget,
            max_execution_duration_per_commit,
            &mut shared_object_congestion_tracker,
        );
        // Allocations of mutably accessed shared objects should look as follows:
        // |----------------------------------------------------
        // |        object_1         |        object_2         |
        // |_________________________|_________________________|
        // |-------------------------|-------------------------|
        // |                         | cert. 2 (g=8000, d=13M) |
        // |-------------------------|-------------------------|
        // |                         | cert. 1 (g=9000, d=10M) |
        // |-------------------------|-------------------------|
        // | cert. 0 (g=100K, d=12M) |                         |
        // |----------------------------------------------------
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
                tx_gas_data.global_ordering_index
            );
        }

        // Construct a certificate with some shared objects (note mutability),
        // and try scheduling it.
        let input_shared_objects = vec![(object_2, true)];
        let tx_gas_data = *txs_gas_data.get(3).unwrap();
        let (certificate, sequencing_result) = build_and_try_sequencing_certificate(
            &input_shared_objects,
            tx_gas_data.gas_price,
            tx_gas_data.gas_budget,
            max_execution_duration_per_commit,
            &mut shared_object_congestion_tracker,
        );
        // If `min_free_execution_slot_assigned = true`, allocations of mutably
        // accessed shared objects should look as follows:
        // |----------------------------------------------------
        // |        object_1         |        object_2         |
        // |_________________________|_________________________|
        // |-------------------------|-------------------------|
        // |                         | cert. 2 (g=8000, d=13M) |
        // |-------------------------|-------------------------|
        // |                         | cert. 1 (g=9000, d=10M) |
        // |-------------------------|-------------------------|
        // | cert. 0 (g=100K, d=12M) | cert. 3 (g=7000, d=11M) |
        // |----------------------------------------------------
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
                    tx_gas_data.global_ordering_index
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
                assert_eq!(
                    suggested_gas_price,
                    txs_gas_data.get(2).unwrap().gas_price + 1
                );
            } else {
                panic!(
                    "Certificate {} must be deferred in the old sequencer",
                    tx_gas_data.global_ordering_index
                );
            }
        }

        // Construct a certificate with some shared objects (note mutability),
        // and try scheduling it.
        let input_shared_objects = vec![(object_2, false)];
        let tx_gas_data = *txs_gas_data.get(4).unwrap();
        let (certificate, sequencing_result) = build_and_try_sequencing_certificate(
            &input_shared_objects,
            tx_gas_data.gas_price,
            tx_gas_data.gas_budget,
            max_execution_duration_per_commit,
            &mut shared_object_congestion_tracker,
        );
        // If `min_free_execution_slot_assigned = true`, allocations of mutably
        // accessed shared objects should look as follows:
        // |----------------------------------------------------
        // |        object_1         |        object_2         |
        // |_________________________|_________________________|
        // |-------------------------|-------------------------|
        // |                         | cert. 2 (g=8000, d=13M) |
        // |-------------------------|-------------------------|
        // |                         | cert. 1 (g=9000, d=10M) |
        // |-------------------------|-------------------------|
        // | cert. 0 (g=100K, d=12M) | cert. 3 (g=7000, d=11M) |
        // |----------------------------------------------------
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
                assert_eq!(
                    suggested_gas_price,
                    txs_gas_data.get(3).unwrap().gas_price + 1
                );
            } else {
                // ^ this corresponds the old sequencer's logic
                assert_eq!(congested_objects, vec![object_2]);
                assert_eq!(
                    suggested_gas_price,
                    txs_gas_data.get(2).unwrap().gas_price + 1
                );
            }
        } else {
            panic!(
                "Certificate {} must be deferred",
                tx_gas_data.global_ordering_index
            );
        }

        // Construct a certificate with some shared objects (note mutability),
        // and try scheduling it.
        let input_shared_objects = vec![(object_2, true)];
        let tx_gas_data = *txs_gas_data.get(5).unwrap();
        let (certificate, sequencing_result) = build_and_try_sequencing_certificate(
            &input_shared_objects,
            tx_gas_data.gas_price,
            tx_gas_data.gas_budget,
            max_execution_duration_per_commit,
            &mut shared_object_congestion_tracker,
        );
        // If `min_free_execution_slot_assigned = true`, allocations of mutably
        // accessed shared objects should look as follows:
        // |----------------------------------------------------
        // |        object_1         |        object_2         |
        // |_________________________|_________________________|
        // |-------------------------|-------------------------|
        // |                         | cert. 2 (g=8000, d=13M) |
        // |-------------------------|-------------------------|
        // |                         | cert. 1 (g=9000, d=10M) |
        // |-------------------------|-------------------------|
        // | cert. 0 (g=100K, d=12M) | cert. 3 (g=7000, d=11M) |
        // |----------------------------------------------------
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

                match mode {
                    PerObjectCongestionControlMode::None => unreachable!(),
                    PerObjectCongestionControlMode::TotalTxCount => {
                        assert_eq!(
                            suggested_gas_price,
                            txs_gas_data.get(3).unwrap().gas_price + 1
                        );
                    }
                    PerObjectCongestionControlMode::TotalGasBudget => {
                        assert_eq!(
                            suggested_gas_price,
                            txs_gas_data.get(2).unwrap().gas_price + 1
                        );
                    }
                }
            } else {
                // ^ this corresponds the old sequencer's logic
                assert_eq!(congested_objects, vec![object_2]);
                assert_eq!(
                    suggested_gas_price,
                    txs_gas_data.get(2).unwrap().gas_price + 1
                );
            }
        } else {
            panic!(
                "Certificate {} must be deferred",
                tx_gas_data.global_ordering_index
            );
        }

        // Construct a certificate with some shared objects (note mutability),
        // and try scheduling it.
        let input_shared_objects = vec![(object_1, true), (object_2, true)];
        let tx_gas_data = *txs_gas_data.get(6).unwrap();
        let (certificate, sequencing_result) = build_and_try_sequencing_certificate(
            &input_shared_objects,
            tx_gas_data.gas_price,
            tx_gas_data.gas_budget,
            max_execution_duration_per_commit,
            &mut shared_object_congestion_tracker,
        );
        // If `min_free_execution_slot_assigned = true`, allocations of mutably
        // accessed shared objects should look as follows:
        // |----------------------------------------------------
        // |        object_1         |        object_2         |
        // |_________________________|_________________________|
        // |-------------------------|-------------------------|
        // |                         | cert. 2 (g=8000, d=13M) |
        // |-------------------------|-------------------------|
        // |                         | cert. 1 (g=9000, d=10M) |
        // |-------------------------|-------------------------|
        // | cert. 0 (g=100K, d=12M) | cert. 3 (g=7000, d=11M) |
        // |----------------------------------------------------
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
                assert_eq!(congested_objects, vec![object_2]);
            }

            match mode {
                PerObjectCongestionControlMode::None => unreachable!(),
                PerObjectCongestionControlMode::TotalTxCount => {
                    assert_eq!(
                        suggested_gas_price,
                        txs_gas_data.get(2).unwrap().gas_price + 1
                    );
                }
                PerObjectCongestionControlMode::TotalGasBudget => {
                    assert_eq!(
                        suggested_gas_price,
                        txs_gas_data.get(1).unwrap().gas_price + 1
                    );
                }
            }
        } else {
            panic!(
                "Certificate {} must be deferred",
                tx_gas_data.global_ordering_index
            );
        }

        // Construct a certificate with some shared objects (note mutability),
        // and try scheduling it.
        let input_shared_objects = vec![(object_1, true), (object_2, true)];
        let tx_gas_data = *txs_gas_data.get(7).unwrap();
        let (certificate, sequencing_result) = build_and_try_sequencing_certificate(
            &input_shared_objects,
            tx_gas_data.gas_price,
            tx_gas_data.gas_budget,
            max_execution_duration_per_commit,
            &mut shared_object_congestion_tracker,
        );
        // If `min_free_execution_slot_assigned = true`, allocations of mutably
        // accessed shared objects should look as follows:
        // |----------------------------------------------------
        // |        object_1         |        object_2         |
        // |_________________________|_________________________|
        // |-------------------------|-------------------------|
        // |                         | cert. 2 (g=8000, d=13M) |
        // |-------------------------|-------------------------|
        // |                         | cert. 1 (g=9000, d=10M) |
        // |-------------------------|-------------------------|
        // | cert. 0 (g=100K, d=12M) | cert. 3 (g=7000, d=11M) |
        // |----------------------------------------------------
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
                    assert_eq!(
                        suggested_gas_price,
                        txs_gas_data.get(2).unwrap().gas_price + 1
                    );
                }
                PerObjectCongestionControlMode::TotalGasBudget => {
                    assert_eq!(suggested_gas_price, max_gas_price);
                }
            }
        } else {
            panic!("Certificate 8 must be deferred");
        }

        // Construct a certificate with some shared objects (note mutability),
        // and try scheduling it.
        let input_shared_objects = vec![(object_1, true)];
        let tx_gas_data = *txs_gas_data.get(8).unwrap();
        let (certificate, sequencing_result) = build_and_try_sequencing_certificate(
            &input_shared_objects,
            tx_gas_data.gas_price,
            tx_gas_data.gas_budget,
            max_execution_duration_per_commit,
            &mut shared_object_congestion_tracker,
        );
        // If `min_free_execution_slot_assigned = true`, allocations of mutably
        // accessed shared objects should look as follows:
        // |----------------------------------------------------
        // |        object_1         |        object_2         |
        // |_________________________|_________________________|
        // |-------------------------|-------------------------|
        // |                         | cert. 2 (g=8000, d=13M) |
        // |-------------------------|-------------------------|
        // | cert. 8 (g=6000, d=13M) | cert. 1 (g=9000, d=10M) |
        // |-------------------------|-------------------------|
        // | cert. 0 (g=100K, d=12M) | cert. 3 (g=7000, d=11M) |
        // |----------------------------------------------------
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
                tx_gas_data.global_ordering_index
            );
        }

        // Construct a certificate with some shared objects (note mutability),
        // and try scheduling it.
        let input_shared_objects = vec![(object_1, true)];
        let tx_gas_data = *txs_gas_data.get(9).unwrap();
        let (certificate, sequencing_result) = build_and_try_sequencing_certificate(
            &input_shared_objects,
            tx_gas_data.gas_price,
            tx_gas_data.gas_budget,
            max_execution_duration_per_commit,
            &mut shared_object_congestion_tracker,
        );
        // If `min_free_execution_slot_assigned = true`, allocations of mutably
        // accessed shared objects should look as follows:
        // |----------------------------------------------------
        // |        object_1         |        object_2         |
        // |_________________________|_________________________|
        // |-------------------------|-------------------------|
        // | cert. 9 (g=5000, d=15M) | cert. 2 (g=8000, d=13M) |
        // |-------------------------|-------------------------|
        // | cert. 8 (g=6000, d=13M) | cert. 1 (g=9000, d=10M) |
        // |-------------------------|-------------------------|
        // | cert. 0 (g=100K, d=12M) | cert. 3 (g=7000, d=11M) |
        // |----------------------------------------------------
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
                tx_gas_data.global_ordering_index
            );
        }

        // Construct a certificate with some shared objects (note mutability),
        // and try scheduling it.
        let input_shared_objects = vec![(object_1, false), (object_2, false)];
        let tx_gas_data = *txs_gas_data.get(10).unwrap();
        let (certificate, sequencing_result) = build_and_try_sequencing_certificate(
            &input_shared_objects,
            tx_gas_data.gas_price,
            tx_gas_data.gas_budget,
            max_execution_duration_per_commit,
            &mut shared_object_congestion_tracker,
        );
        // If `min_free_execution_slot_assigned = true`, allocations of mutably
        // accessed shared objects should look as follows:
        // |----------------------------------------------------
        // |        object_1         |        object_2         |
        // |_________________________|_________________________|
        // |-------------------------|-------------------------|
        // | cert. 9 (g=5000, d=15M) | cert. 2 (g=8000, d=13M) |
        // |-------------------------|-------------------------|
        // | cert. 8 (g=6000, d=13M) | cert. 1 (g=9000, d=10M) |
        // |-------------------------|-------------------------|
        // | cert. 0 (g=100K, d=12M) | cert. 3 (g=7000, d=11M) |
        // |----------------------------------------------------
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

            assert_eq!(
                suggested_gas_price,
                txs_gas_data.get(2).unwrap().gas_price + 1
            );
        } else {
            panic!(
                "Certificate {} must be deferred",
                tx_gas_data.global_ordering_index
            );
        }

        // Construct a certificate with some shared objects (note mutability),
        // and try scheduling it.
        let input_shared_objects = vec![(object_1, true), (object_2, false)];
        let tx_gas_data = *txs_gas_data.get(11).unwrap();
        let (certificate, sequencing_result) = build_and_try_sequencing_certificate(
            &input_shared_objects,
            tx_gas_data.gas_price,
            tx_gas_data.gas_budget,
            max_execution_duration_per_commit,
            &mut shared_object_congestion_tracker,
        );
        // If `min_free_execution_slot_assigned = true`, allocations of mutably
        // accessed shared objects should look as follows:
        // |----------------------------------------------------
        // |        object_1         |        object_2         |
        // |_________________________|_________________________|
        // |-------------------------|-------------------------|
        // | cert. 9 (g=5000, d=15M) | cert. 2 (g=8000, d=13M) |
        // |-------------------------|-------------------------|
        // | cert. 8 (g=6000, d=13M) | cert. 1 (g=9000, d=10M) |
        // |-------------------------|-------------------------|
        // | cert. 0 (g=100K, d=12M) | cert. 3 (g=7000, d=11M) |
        // |----------------------------------------------------
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
                    assert_eq!(
                        suggested_gas_price,
                        txs_gas_data.get(2).unwrap().gas_price + 1
                    );
                }
                PerObjectCongestionControlMode::TotalGasBudget => {
                    assert_eq!(
                        suggested_gas_price,
                        txs_gas_data.get(1).unwrap().gas_price + 1
                    );
                }
            }
        } else {
            panic!(
                "Certificate {} must be deferred",
                tx_gas_data.global_ordering_index
            );
        }

        // Construct a certificate with some shared objects (note mutability),
        // and try scheduling it.
        let input_shared_objects = vec![(object_1, false), (object_2, true)];
        let tx_gas_data = *txs_gas_data.get(12).unwrap();
        let (certificate, sequencing_result) = build_and_try_sequencing_certificate(
            &input_shared_objects,
            tx_gas_data.gas_price,
            tx_gas_data.gas_budget,
            max_execution_duration_per_commit,
            &mut shared_object_congestion_tracker,
        );
        // If `min_free_execution_slot_assigned = true`, allocations of mutably
        // accessed shared objects should look as follows:
        // |----------------------------------------------------
        // |        object_1         |        object_2         |
        // |_________________________|_________________________|
        // |-------------------------|-------------------------|
        // | cert. 9 (g=5000, d=15M) | cert. 2 (g=8000, d=13M) |
        // |-------------------------|-------------------------|
        // | cert. 8 (g=6000, d=13M) | cert. 1 (g=9000, d=10M) |
        // |-------------------------|-------------------------|
        // | cert. 0 (g=100K, d=12M) | cert. 3 (g=7000, d=11M) |
        // |----------------------------------------------------
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
                    assert_eq!(
                        suggested_gas_price,
                        txs_gas_data.get(2).unwrap().gas_price + 1
                    );
                }
                PerObjectCongestionControlMode::TotalGasBudget => {
                    assert_eq!(suggested_gas_price, max_gas_price);
                }
            }
        } else {
            panic!(
                "Certificate {} must be deferred",
                tx_gas_data.global_ordering_index
            );
        }
    }
}
