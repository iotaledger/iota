// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{cmp::Ordering, collections::HashMap};

use iota_types::{
    base_types::{CommitRound, ObjectID},
    executable_transaction::VerifiedExecutableTransaction,
    transaction::{SharedInputObject, TransactionDataAPI},
};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::{
    authority_per_epoch_store::PreviouslyDeferredTransactions, transaction_deferral::DeferralKey,
};
use crate::authority::authority_per_epoch_store::CongestionControlParameters;

/// Represents execution slot boundaries
pub(super) type ExecutionTime = u64;
const MAX_EXECUTION_TIME: ExecutionTime = ExecutionTime::MAX;

/// Represents a sequencing result: schedule transaction, or defer it
/// due to shared object congestion. Sequencing result is returned by
/// the `try_schedule` method of the `SharedObjectCongestionTracker`.
pub(super) enum SequencingResult {
    /// Sequencing result indicating that a transaction is scheduled to be
    /// executed at start time
    Schedule(/* start_time */ ExecutionTime),

    /// Sequencing result indicating that a transaction is deferred.
    /// The list of objects are congested objects.
    Defer(DeferralKey, Vec<ObjectID>),
}

/// An execution slot represents the allocated time slot for a transaction to be
/// executed. We can only estimate the time to execute a transaction.
///
/// Execution slots must have strictly positive duration, i.e., the start time
/// must be strictly less than the end time.
///
/// Execution slots of transactions with common shared objects cannot overlap.
/// Transactions can occupy overlapping execution slots if they do not touch
/// any common shared objects.
#[derive(PartialEq, Eq, Clone, Debug, Copy)]
struct ExecutionSlot {
    start_time: ExecutionTime,
    end_time: ExecutionTime,
}

impl ExecutionSlot {
    /// Constructs a new execution slot where start_time must be strictly less
    /// than end_time.
    fn new(start_time: ExecutionTime, end_time: ExecutionTime) -> Self {
        debug_assert!(
            start_time < end_time,
            "invalid execution slot: start time must be less than end time"
        );
        Self {
            start_time,
            end_time,
        }
    }

    /// Calculates the duration of this execution slot.
    ///
    /// Panics if this slot is invalid, i.e., its `end_time` is smaller than
    /// its `start_time`, which should never happen if the `new(...)` method
    /// is used for creating an execution slot.
    fn duration(&self) -> ExecutionTime {
        debug_assert!(
            self.start_time < self.end_time,
            "invalid execution slot: start time must be less than end time"
        );

        self.end_time - self.start_time
    }

    /// Returns the intersection of this execution slot with another execution,
    /// if it exists. Otherwise, returns None
    fn intersection(&self, other: &Self) -> Option<Self> {
        let start_time = self.start_time.max(other.start_time);
        let end_time = self.end_time.min(other.end_time);
        if start_time < end_time {
            Some(Self::new(start_time, end_time))
        } else {
            None
        }
    }

    /// Returns a execution slot with maximum possible duration
    fn max_duration_slot() -> Self {
        Self::new(0, MAX_EXECUTION_TIME)
    }

    /// Returns an ordering indicating whether this execution slot contains the
    /// other execution slot. The ordering is defined as follows:
    /// - Less: the other slot is not contained by this slot and this slot's end
    ///   time is less than the other slot's end time.
    /// - Greater: the other slot is not contained by this slot and this slot's
    ///   start time is greater than the other slot's start time.
    /// - Equal: the other slot is contained by this slot.
    fn contains(&self, other: &Self) -> Ordering {
        if self.end_time < other.end_time {
            Ordering::Less
        } else if self.start_time > other.start_time {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}

/// `ObjectExecutionSlots` stores a list of free execution slots for a given
/// object. It contains a list of execution slots that are free for a
/// transaction touching that object to use. The list of execution slots is
/// sorted in ascending order of their start time with no overlap between slots.
#[derive(PartialEq, Eq, Clone, Debug)]
struct ObjectExecutionSlots(Vec<ExecutionSlot>);

impl ObjectExecutionSlots {
    /// Create a new `ObjectExecutionSlots` with a single slot of maximum
    /// duration.
    fn new() -> Self {
        Self(vec![ExecutionSlot::max_duration_slot()])
    }

    /// Returns the start time of the last free slot for a given object that can
    /// fit a transaction of duration `tx_duration`. If no such slot exists,
    /// returns None.
    fn max_object_free_slot_start_time(&self, tx_duration: ExecutionTime) -> Option<ExecutionTime> {
        if let Some(last_free_slot) = self.0.last() {
            if MAX_EXECUTION_TIME - last_free_slot.start_time >= tx_duration {
                // if the transaction will fit in the last free slot, return its start time.
                return Some(last_free_slot.start_time);
            }
        }
        None
    }

    /// Returns the maximum occupied slot end time for a given shared object.
    fn max_object_occupied_slot_end_time(&self) -> ExecutionTime {
        // the maximum free slot start time for a transaction of duration 0 will give
        // the desired result. If this returns None for a transaction of duration 0,
        // that means there are no free slots, so we should return MAX_EXECUTION_TIME.
        self.max_object_free_slot_start_time(0)
            .unwrap_or(MAX_EXECUTION_TIME)
    }

    /// Remove the occupied slot `slot_to_remove` from this
    /// `ObjectExecutionSlots`.
    fn remove(&mut self, slot_to_remove: ExecutionSlot) {
        // binary search the slot that contains the slot to be removed.
        let mut index = self
            .0
            .binary_search_by(|s| s.contains(&slot_to_remove))
            .expect("can't remove a slot that is not available");
        // if the occupied slot that we wish to remove overlaps with the free slot, we
        // split the free slot. There are 4 cases to consider.
        // case A: a free slot remains at the start.
        // (occupied_slot.start_time > free_slot.start_time && occupied_slot.end_time ==
        // free_slot.end_time)
        //      | free_slot                 |
        //   => | free_slot | occupied_slot |
        // case B: a free slot remains at the end.
        // (occupied_slot.start_time == free_slot.start_time && occupied_slot.end_time <
        // free_slot.end_time)
        //      | free_slot                 |
        //   => | occupied_slot | free_slot |
        // case AB: a free slot remains at the start and the end.
        // (occupied_slot.start_time > free_slot.start_time && occupied_slot.end_time
        // <
        // free_slot.end_time)
        //      | free_slot                             |
        //   => | free_slot | occupied_slot | free_slot |
        // case 0: the occupied slot perfectly overlaps with the free slot.
        // (occupied_slot.start_time == free_slot.start_time && occupied_slot.end_time
        // == free_slot.end_time)
        //      | free_slot     |
        //   => | occupied_slot |

        let free_slot = self.0.remove(index);
        // case A: if a part of the free slot remains at the start, create a new
        // free slot.
        if slot_to_remove.start_time > free_slot.start_time {
            self.0.insert(
                index,
                ExecutionSlot::new(free_slot.start_time, slot_to_remove.start_time),
            );
            index += 1;
        }
        // case B: if a part of the free slot remains at the end, create a new free
        // slot.
        if slot_to_remove.end_time < free_slot.end_time {
            self.0.insert(
                index,
                ExecutionSlot::new(slot_to_remove.end_time, free_slot.end_time),
            );
        }
    }
}

/// `SharedObjectCongestionTracker` stores the available and occupied execution
/// slots for the transactions within a consensus commit.
///
/// When transactions are scheduled by the consensus handler, each scheduled
/// transaction takes up an execution slot with a certain start time.
///
/// The goal of this data structure is to capture the critical path of
/// transaction execution latency on each objects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SharedObjectCongestionTracker {
    object_execution_slots: HashMap<ObjectID, ObjectExecutionSlots>,
    congestion_control_parameters: CongestionControlParameters,
}

impl SharedObjectCongestionTracker {
    /// Create a new `SharedObjectCongestionTracker` for the given
    /// `CongestionControlParameters` and taking into account
    /// `initial_object_debts`.
    pub(super) fn new(
        initial_object_debts: impl IntoIterator<Item = (ObjectID, u64)>,
        congestion_control_parameters: CongestionControlParameters,
    ) -> Self {
        let object_execution_slots = initial_object_debts
            .into_iter()
            .map(|(object_id, debt)| {
                let mut slots = ObjectExecutionSlots::new();
                if debt > 0 {
                    // If there is an initial debt, remove the occupied slot from time 0 to
                    // debt.
                    slots.remove(ExecutionSlot::new(0, debt));
                }

                (object_id, slots)
            })
            .collect::<HashMap<_, _>>();

        Self {
            object_execution_slots,
            congestion_control_parameters,
        }
    }

    /// Get congestion control parameters used in the tracker.
    pub(super) fn congestion_control_parameters(&self) -> &CongestionControlParameters {
        &self.congestion_control_parameters
    }

    /// Initialize the free execution slots for the objects that are not in the
    /// tracker.
    pub(super) fn initialize_object_execution_slots(
        &mut self,
        shared_input_objects: &[SharedInputObject],
    ) {
        for obj in shared_input_objects {
            self.object_execution_slots
                .entry(obj.id)
                .or_insert(ObjectExecutionSlots::new());
        }
    }

    /// Given a list of shared input objects and the estimated execution
    /// duration of a transaction that operates on these objects, returns
    /// the starting time of the transaction if the transaction can be
    /// scheduled. Otherwise, returns None.
    ///
    /// Starting time is determined by all the input shared objects' last write.
    ///
    /// Before calling this function, the caller should ensure that the tracker
    /// is initialized for all objects in the transaction by first calling
    /// `initialize_object_execution_slots`.
    #[instrument(level = "trace", skip_all)]
    fn compute_tx_start_time(
        &self,
        shared_input_objects: &[SharedInputObject],
        tx_duration: ExecutionTime,
    ) -> Option<ExecutionTime> {
        if self
            .congestion_control_parameters
            .congestion_control_min_free_execution_slot()
        {
            // If `congestion_control_min_free_execution_slot` is true, we assign the
            // transaction start time based on the lowest free execution slot that
            // can accommodate the transaction. We start the search from the full
            // range of the slots available with no constraints from previous objects.
            let _span = tracing::trace_span!("compute_min_free_execution_slot").entered();
            let initial_free_slot = ExecutionSlot::max_duration_slot();
            self.compute_min_free_execution_slot(
                shared_input_objects,
                tx_duration,
                initial_free_slot,
            )
        } else {
            // If `congestion_control_min_free_execution_slot` is false, we assign the
            // transaction start time based on the maximum start time of free execution
            // slots for the transaction over all its shared objects.
            let _span = tracing::trace_span!("max_object_free_slot_start_time").entered();
            shared_input_objects
                .iter()
                .map(|obj| {
                    // `start_time`
                    self.object_execution_slots
                        .get(&obj.id)
                        .expect("object should have been inserted at the start of this function.")
                        .max_object_free_slot_start_time(tx_duration)
                })
                // If any `start_time` is `None` (i.e., the corresponding object
                // does not have a free slot), the collect will return `None`
                .collect::<Option<Vec<_>>>()
                .and_then(|object_start_times| object_start_times.into_iter().max())
        }
    }

    /// A recursive function that tries to find the lowest free slot for a
    /// transaction. If a slot is found that fits the transaction, the function
    /// returns the slot. Otherwise, it returns None.
    /// lookup_interval is the range of the slot that the transaction can fit in
    /// given the objects that have been checked so far.
    fn compute_min_free_execution_slot(
        &self,
        shared_input_objects: &[SharedInputObject],
        tx_duration: ExecutionTime,
        lookup_interval: ExecutionSlot,
    ) -> Option<ExecutionTime> {
        // Take the first object from the shared input objects, and
        // set aside the remaining objects for the next recursive call.
        let (obj, remaining_objects) = shared_input_objects
            .split_first()
            .expect("shared_input_objects must not be empty.");

        for intersection_slot in self
            .object_execution_slots
            .get(&obj.id)
            .expect("object should have been inserted before.")
            .0
            .iter()
            .filter_map(|slot| slot.intersection(&lookup_interval))
        {
            // If there is no overlap that can fit the transaction, continue to the next
            // free slot.
            if intersection_slot.duration() < tx_duration {
                continue;
            }
            // if this is the last object to check, return this slot as it is the lowest
            // slot available.
            if remaining_objects.is_empty() {
                return Some(intersection_slot.start_time);
            }
            // if there are more objects to check, recursively call the function with the
            // remaining objects.
            // If the recursive call returns a start time, that means the transaction fits
            // in the slot for all remaining objects. Return the start time.
            // Otherwise, continue to check the next free slot for the current object.
            if let Some(lowest_overlap) = self.compute_min_free_execution_slot(
                remaining_objects,
                tx_duration,
                intersection_slot,
            ) {
                return Some(lowest_overlap);
            } else {
                continue;
            }
        }
        // if no slot is found for the current object given the available range, return
        // None.
        None
    }

    /// Given a transaction, returns a sequencing result. If the transaction can
    /// be scheduled, this returns a `start_time`, and if it should be deferred,
    /// this returns the deferral key and the congested objects responsible for
    /// the deferral.
    #[instrument(level = "trace", skip_all, fields(cert_digest = ?cert.digest()))]
    pub(super) fn try_schedule(
        &self,
        cert: &VerifiedExecutableTransaction,
        previously_deferred_tx_digests: &PreviouslyDeferredTransactions,
        commit_round: CommitRound,
    ) -> SequencingResult {
        let tx_duration = self
            .congestion_control_parameters
            .get_estimated_execution_duration(cert);
        if tx_duration == 0 {
            // This is a zero-duration transaction, no need to defer.
            return SequencingResult::Schedule(0);
        }

        let shared_input_objects = cert.shared_input_objects();
        if shared_input_objects.is_empty() {
            // This is an owned object only transaction. No need to defer.
            return SequencingResult::Schedule(0);
        }

        let congestion_limit = if let Some(congestion_limit) = self
            .congestion_control_parameters
            .get_effective_congestion_limit_per_commit()
        {
            congestion_limit
        } else {
            // If we don't have a congestion limit per commit, we don't need to check for
            // congestion.
            return SequencingResult::Schedule(0);
        };

        // Try to compute a scheduling start time for the transaction.
        if let Some(start_time) = self.compute_tx_start_time(&shared_input_objects, tx_duration) {
            // `compute_tx_start_time` returns None if the transaction cannot be scheduled,
            // so no need to check for overflow when adding `tx_duration` here.
            if start_time + tx_duration <= congestion_limit {
                // schedule this transaction and return the start time.
                return SequencingResult::Schedule(start_time);
            }
        }

        // The transaction cannot be scheduled. We need to defer it and return a list
        // of the IDs of shared input objects to explain the congestion reason.
        let congested_objects: Vec<ObjectID> = if self
            .congestion_control_parameters
            .congestion_control_min_free_execution_slot()
        {
            // If `congestion_control_min_free_execution_slot` is true, we return all the
            // shared input objects as no individual object can be identified as
            // the cause of congestion.
            shared_input_objects.iter().map(|obj| obj.id).collect()
        } else {
            // If `congestion_control_min_free_execution_slot` is false, we return
            // only shared objects that can be identified as the cause of congestion.
            shared_input_objects
                .iter()
                .filter(|obj| {
                    let (end_time, overflow) = self
                        .object_execution_slots
                        .get(&obj.id)
                        .expect("object should have been inserted before.")
                        .max_object_occupied_slot_end_time()
                        .overflowing_add(tx_duration);
                    overflow || end_time > congestion_limit
                })
                .map(|obj| obj.id)
                .collect()
        };

        assert!(!congested_objects.is_empty());

        let deferral_key = if let Some(previous_key_suggested_gas_price_pair) =
            previously_deferred_tx_digests.get(cert.digest())
        {
            // This transaction has been deferred in previous consensus commit. Use its
            // previous deferred_from_round.
            DeferralKey::new_for_consensus_round(
                commit_round + 1,
                previous_key_suggested_gas_price_pair
                    .0
                    .deferred_from_round(),
            )
        } else {
            // This transaction has not been deferred before. Use the current commit round
            // as the deferred_from_round.
            DeferralKey::new_for_consensus_round(commit_round + 1, commit_round)
        };
        SequencingResult::Defer(deferral_key, congested_objects)
    }

    /// Update shared objects' execution slots used in `cert` using `cert`'s
    /// estimated execution duration. This is called when `cert` is scheduled
    /// for execution.
    ///
    /// `start_time` provides the start time of the execution slot assigned to
    /// `cert`.
    ///
    /// Returns `Some(BumpObjectExecutionSlotsResult)` if `cert`'s estimated
    /// execution duration is non-zero, else returns `None`.
    pub(super) fn bump_object_execution_slots(
        &mut self,
        cert: &VerifiedExecutableTransaction,
        start_time: ExecutionTime,
    ) -> Option<BumpObjectExecutionSlotsResult> {
        let estimated_execution_duration = self
            .congestion_control_parameters
            .get_estimated_execution_duration(cert);

        if estimated_execution_duration == 0 {
            return None;
        }

        let end_time = start_time.saturating_add(estimated_execution_duration);
        let occupied_slot = ExecutionSlot::new(start_time, end_time);

        // Find IDs of shared objects for which execution slots should be bumped.
        let object_ids = cert
            .shared_input_objects()
            .into_iter()
            .filter_map(|obj| obj.mutable.then_some(obj.id))
            .collect::<Vec<_>>();

        object_ids.iter().for_each(|obj_id| {
            self.object_execution_slots
                .get_mut(obj_id)
                .expect("object execution slot should have been initialized before.")
                .remove(occupied_slot);
        });

        Some(BumpObjectExecutionSlotsResult::new(
            object_ids,
            start_time,
            estimated_execution_duration,
            cert.transaction_data().gas_price(),
        ))
    }

    /// Returns the maximum occupied slot end time over all shared objects.
    pub(super) fn max_occupied_slot_end_time(&self) -> ExecutionTime {
        self.object_execution_slots
            .values()
            .map(|slots| slots.max_object_occupied_slot_end_time())
            .max()
            .unwrap_or(0)
    }

    /// Returns accumulated debts for objects whose budgets have been exceeded
    /// over the course of the commit. Consumes the tracker object, since
    /// this should only be called once after all txs have been processed.
    pub(super) fn accumulated_debts(
        self,
        max_execution_duration_per_commit: u64,
    ) -> Vec<(ObjectID, u64)> {
        self.object_execution_slots
            .into_iter()
            .filter_map(|(obj_id, slots)| {
                let debt = slots
                    .max_object_occupied_slot_end_time()
                    .saturating_sub(max_execution_duration_per_commit);
                if debt > 0 { Some((obj_id, debt)) } else { None }
            })
            .collect()
    }
}

/// Stores per-object debts from a given consensus commit.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) enum CongestionPerObjectDebt {
    V1(CommitRound, u64),
}

impl CongestionPerObjectDebt {
    pub(super) fn new(round: CommitRound, debt: u64) -> Self {
        Self::V1(round, debt)
    }

    pub(super) fn into_v1(self) -> (CommitRound, u64) {
        match self {
            Self::V1(round, debt) => (round, debt),
        }
    }
}

/// Stores a result of the [`bump_object_execution_slots`] method
/// of `SharedObjectCongestionTracker` for a single scheduled transaction.
/// The result is then intended to be used in `SuggestedGasPriceCalculator`.
pub(super) struct BumpObjectExecutionSlotsResult {
    /// List of IDs of shared objects for which execution slots
    /// were bumped. Usually this includes shared objects accessed
    /// by a mutable reference in a transaction.
    object_ids: Vec<ObjectID>,

    /// Start time at which the shared-object transaction has been scheduled.
    execution_start_time: ExecutionTime,

    /// Estimated execution duration of the scheduled shared-object transaction.
    estimated_execution_duration: ExecutionTime,

    /// Gas price of the scheduled shared-object transaction.
    gas_price: u64,
}

impl BumpObjectExecutionSlotsResult {
    /// Create a new `BumpObjectExecutionSlotsResult`.
    fn new(
        object_ids: Vec<ObjectID>,
        execution_start_time: ExecutionTime,
        estimated_execution_duration: ExecutionTime,
        gas_price: u64,
    ) -> Self {
        Self {
            object_ids,
            execution_start_time,
            estimated_execution_duration,
            gas_price,
        }
    }

    /// Get the list of IDs of shared objects for which execution slots
    /// were bumped.
    pub(super) fn object_ids(&self) -> &[ObjectID] {
        &self.object_ids
    }

    /// Get start time at which the shared-object transaction has been
    /// scheduled.
    pub(super) fn execution_start_time(&self) -> ExecutionTime {
        self.execution_start_time
    }

    /// Get estimated execution duration of the scheduled shared-object
    /// transaction.
    pub(super) fn estimated_execution_duration(&self) -> ExecutionTime {
        self.estimated_execution_duration
    }

    /// Get gas price of the scheduled shared-object transaction.
    pub(super) fn gas_price(&self) -> u64 {
        self.gas_price
    }

    /// Create a new `BumpObjectExecutionSlotsResult` for test.
    #[cfg(test)]
    pub(super) fn new_for_test(
        object_ids: Vec<ObjectID>,
        execution_start_time: ExecutionTime,
        estimated_execution_duration: ExecutionTime,
        gas_price: u64,
    ) -> Self {
        Self {
            object_ids,
            execution_start_time,
            estimated_execution_duration,
            gas_price,
        }
    }
}

#[cfg(test)]
mod execution_slot_tests {
    use std::cmp::Ordering;

    use super::ExecutionSlot;

    #[test]
    fn test_execution_slot_new_and_duration() {
        // Creating a slot with `start_time`  < `end_time`
        let slot = ExecutionSlot::new(1, 3);
        assert_eq!(slot.duration(), 2);
    }

    #[test]
    #[should_panic]
    fn test_execution_slot_new_zero_duration() {
        // Creating a slot with `start_time`  == `end_time` should panic.
        ExecutionSlot::new(1, 1);
    }

    #[test]
    #[should_panic]
    fn test_execution_slot_new_negative_duration() {
        // Creating a slot with `start_time`  > `end_time` should panic.
        ExecutionSlot::new(3, 1);
    }

    #[test]
    fn test_execution_slot_intersection() {
        // Test intersection of two identical slots
        let slot_1 = ExecutionSlot::new(1, 3);
        let slot_2 = ExecutionSlot::new(1, 3);
        if let Some(intersection) = slot_1.intersection(&slot_2) {
            assert_eq!(intersection, ExecutionSlot::new(1, 3));
            assert_eq!(intersection.duration(), 2);
        } else {
            panic!("Expected intersection to be Some");
        }

        // Test intersection of two non-overlapping slots
        let slot_1 = ExecutionSlot::new(1, 3);
        let slot_2 = ExecutionSlot::new(4, 5);
        let intersection = slot_1.intersection(&slot_2);
        assert!(intersection.is_none());

        // Test intersection of non-overlapping slots, with slot 2 being after slot 1
        let slot_1 = ExecutionSlot::new(1, 3);
        let slot_2 = ExecutionSlot::new(3, 5);
        let intersection = slot_1.intersection(&slot_2);
        assert!(intersection.is_none());

        // Test intersection of non-overlapping slots, with slot 2 being before slot 1
        // and end time of one slot equal to the other's start time.
        let slot_1 = ExecutionSlot::new(3, 5);
        let slot_2 = ExecutionSlot::new(1, 3);
        let intersection = slot_1.intersection(&slot_2);
        assert!(intersection.is_none());

        // Test intersection of non-overlapping slots, with slot 2 being after slot 1
        // and end time of one slot equal to the other's start time.
        let slot_1 = ExecutionSlot::new(1, 3);
        let slot_2 = ExecutionSlot::new(3, 5);
        let intersection = slot_1.intersection(&slot_2);
        assert!(intersection.is_none());

        // Test intersection of overlapping slots, with slot 2 starting later than slot
        // 1 starts
        let slot_1 = ExecutionSlot::new(1, 5);
        let slot_2 = ExecutionSlot::new(3, 9);
        if let Some(intersection) = slot_1.intersection(&slot_2) {
            assert_eq!(intersection, ExecutionSlot::new(3, 5));
            assert_eq!(intersection.duration(), 2);
        } else {
            panic!("Expected intersection to be Some");
        }

        // Test intersection of overlapping slots, with slot 2 before slot 1 starts
        let slot_1 = ExecutionSlot::new(4, 9);
        let slot_2 = ExecutionSlot::new(1, 9);
        if let Some(intersection) = slot_1.intersection(&slot_2) {
            assert_eq!(intersection, ExecutionSlot::new(4, 9));
            assert_eq!(intersection.duration(), 5);
        } else {
            panic!("Expected intersection to be Some");
        }

        // Test intersection of non-overlapping slots with a gap between them
        let slot_1 = ExecutionSlot::new(1, 3);
        let slot_2 = ExecutionSlot::new(5, 9);
        assert!(slot_1.intersection(&slot_2).is_none());
    }

    #[test]
    fn test_execution_slot_contains() {
        // Test case where slot_1 contains slot_2
        let slot_1 = ExecutionSlot::new(1, 5);
        let slot_2 = ExecutionSlot::new(2, 3);
        assert_eq!(slot_1.contains(&slot_2), Ordering::Equal);

        // Test case where part of slot_2 is greater than slot_1
        let slot_1 = ExecutionSlot::new(1, 5);
        let slot_2 = ExecutionSlot::new(0, 3);
        assert_eq!(slot_1.contains(&slot_2), Ordering::Greater);

        // Test case where all of slot_2 is greater than slot_1
        let slot_1 = ExecutionSlot::new(2, 5);
        let slot_2 = ExecutionSlot::new(0, 1);
        assert_eq!(slot_1.contains(&slot_2), Ordering::Greater);

        // Test case where part of slot_2 is less than slot_1
        let slot_1 = ExecutionSlot::new(1, 5);
        let slot_2 = ExecutionSlot::new(3, 6);
        assert_eq!(slot_1.contains(&slot_2), Ordering::Less);

        // Test case where all of slot_2 is less than slot_1
        let slot_1 = ExecutionSlot::new(1, 5);
        let slot_2 = ExecutionSlot::new(6, 7);
        assert_eq!(slot_1.contains(&slot_2), Ordering::Less);

        // Test case where slot_1 is equal to slot_2
        let slot_1 = ExecutionSlot::new(1, 5);
        let slot_2 = ExecutionSlot::new(1, 5);
        assert_eq!(slot_1.contains(&slot_2), Ordering::Equal);
    }
}

#[cfg(test)]
pub mod shared_object_test_utils {
    use iota_test_transaction_builder::TestTransactionBuilder;
    use iota_types::{
        base_types::{ObjectID, SequenceNumber, random_object_ref},
        crypto::{AccountKeyPair, get_key_pair},
        executable_transaction::VerifiedExecutableTransaction,
        transaction::{CallArg, ObjectArg, VerifiedTransaction},
    };

    use super::*;

    pub const TEST_ONLY_GAS_PRICE: u64 = 1_000;

    /// Builds a certificate with a list of shared objects and their mutability.
    /// The certificate is only used to test the
    /// `SharedObjectCongestionTracker` functions, therefore the content
    /// other than shared inputs, gas budget and gas price are not
    /// important.
    pub fn build_transaction(
        objects: &[(ObjectID, bool)],
        gas_budget: u64,
        gas_price: u64,
    ) -> VerifiedExecutableTransaction {
        let (sender, keypair): (_, AccountKeyPair) = get_key_pair();
        let gas_object = random_object_ref();
        VerifiedExecutableTransaction::new_system(
            VerifiedTransaction::new_unchecked(
                TestTransactionBuilder::new(sender, gas_object, gas_price)
                    .with_gas_budget(gas_budget)
                    .move_call(
                        ObjectID::random(),
                        "unimportant_module",
                        "unimportant_function",
                        objects
                            .iter()
                            .map(|(id, mutable)| {
                                CallArg::Object(ObjectArg::SharedObject {
                                    id: *id,
                                    initial_shared_version: SequenceNumber::new(),
                                    mutable: *mutable,
                                })
                            })
                            .collect(),
                    )
                    .build_and_sign(&keypair),
            ),
            0,
        )
    }

    pub(crate) fn initialize_tracker_and_compute_tx_start_time(
        shared_object_congestion_tracker: &mut SharedObjectCongestionTracker,
        shared_input_objects: &[SharedInputObject],
        tx_duration: ExecutionTime,
    ) -> Option<ExecutionTime> {
        shared_object_congestion_tracker.initialize_object_execution_slots(shared_input_objects);
        shared_object_congestion_tracker.compute_tx_start_time(shared_input_objects, tx_duration)
    }

    pub(super) fn initialize_tracker_and_try_schedule(
        shared_object_congestion_tracker: &mut SharedObjectCongestionTracker,
        cert: &VerifiedExecutableTransaction,
        previously_deferred_tx_digests: &PreviouslyDeferredTransactions,
        commit_round: CommitRound,
    ) -> SequencingResult {
        let shared_input_objects = cert.shared_input_objects();
        shared_object_congestion_tracker.initialize_object_execution_slots(&shared_input_objects);
        shared_object_congestion_tracker.try_schedule(
            cert,
            previously_deferred_tx_digests,
            commit_round,
        )
    }

    pub(crate) fn new_congestion_tracker_with_initial_value_for_test(
        init_values: &[(ObjectID, ExecutionTime)],
        congestion_control_parameters: CongestionControlParameters,
    ) -> SharedObjectCongestionTracker {
        SharedObjectCongestionTracker::new(
            init_values.iter().map(|(id, debt)| (*id, *debt)),
            congestion_control_parameters,
        )
    }

    pub fn construct_shared_input_objects(objects: &[(ObjectID, bool)]) -> Vec<SharedInputObject> {
        objects
            .iter()
            .map(|(id, mutable)| SharedInputObject {
                id: *id,
                initial_shared_version: SequenceNumber::new(),
                mutable: *mutable,
            })
            .collect()
    }
}

#[cfg(test)]
mod object_cost_tests {
    use iota_protocol_config::PerObjectCongestionControlMode;
    use iota_types::digests::TransactionDigest;
    use rstest::rstest;

    use super::{shared_object_test_utils::*, *};

    #[rstest]
    fn test_compute_tx_start_at_time(#[values(true, false)] assign_min_free_execution_slot: bool) {
        let object_id_0 = ObjectID::random();
        let object_id_1 = ObjectID::random();
        let object_id_2 = ObjectID::random();
        let object_id_3 = ObjectID::random();

        // initialise a new shared object congestion tracker.
        let mut shared_object_congestion_tracker =
            new_congestion_tracker_with_initial_value_for_test(
                &[(object_id_0, 5), (object_id_1, 9)],
                CongestionControlParameters::new_for_test(
                    PerObjectCongestionControlMode::TotalGasBudget,
                    assign_min_free_execution_slot,
                    None,  // not important in this test
                    None,  // not important in this test
                    0,     // not important in this test
                    false, // not important in this test
                    true,  // not important in this test
                ),
            );

        // The tracker has the following object execution slots:
        //
        //    object_id_0:       object_id_1:       object_id_2:       object_id_3:
        // 0| xxxxxxxxxxxx     | xxxxxxxxxxxx     |                  |
        // 1| xxxxxxxxxxxx     | xxxxxxxxxxxx     |                  |
        // 2| xxxxxxxxxxxx     | xxxxxxxxxxxx     |                  |
        // 3| xxxxxxxxxxxx     | xxxxxxxxxxxx     |                  |
        // 4| xxxxxxxxxxxx     | xxxxxxxxxxxx     |                  |
        // 5|                  | xxxxxxxxxxxx     |                  |
        // 6|                  | xxxxxxxxxxxx     |                  |
        // 7|                  | xxxxxxxxxxxx     |                  |
        // 8|                  | xxxxxxxxxxxx     |                  |
        // 9|                  |                  |                  |

        // a transaction that writes to objects 0, 1 and 2 should have start_time 9.
        let objects = &[
            (object_id_0, true),
            (object_id_1, true),
            (object_id_2, true),
        ];
        let shared_input_objects = construct_shared_input_objects(objects);
        assert_eq!(
            initialize_tracker_and_compute_tx_start_time(
                &mut shared_object_congestion_tracker,
                &shared_input_objects,
                10
            ),
            Some(9)
        );
        // now add this transaction to the tracker.
        let tx = build_transaction(objects, 1, TEST_ONLY_GAS_PRICE);
        shared_object_congestion_tracker.bump_object_execution_slots(&tx, 9);

        // That tracker now has the following object execution slots:
        //
        //    object_id_0:       object_id_1:       object_id_2:       object_id_3:
        // 0| xxxxxxxxxxxx     | xxxxxxxxxxxx     |                  |
        // 1| xxxxxxxxxxxx     | xxxxxxxxxxxx     |                  |
        // 2| xxxxxxxxxxxx     | xxxxxxxxxxxx     |                  |
        // 3| xxxxxxxxxxxx     | xxxxxxxxxxxx     |                  |
        // 4| xxxxxxxxxxxx     | xxxxxxxxxxxx     |                  |
        // 5|                  | xxxxxxxxxxxx     |                  |
        // 6|                  | xxxxxxxxxxxx     |                  |
        // 7|                  | xxxxxxxxxxxx     |                  |
        // 8|                  | xxxxxxxxxxxx     |                  |
        // 9| xxxxxxxxxxxx     | xxxxxxxxxxxx     | xxxxxxxxxxxx     |

        // a transaction with duration 4 that reads object 0 should have start_time 5
        // with `assign_min_free_execution_slot` or 10 without
        // `assign_min_free_execution_slot`.
        let shared_input_objects = construct_shared_input_objects(&[(object_id_0, false)]);
        assert_eq!(
            initialize_tracker_and_compute_tx_start_time(
                &mut shared_object_congestion_tracker,
                &shared_input_objects,
                4
            ),
            if assign_min_free_execution_slot {
                Some(5)
            } else {
                Some(10)
            }
        );
        // a transaction with duration 5 that reads object 0 should have start_time 10
        // with or without `assign_min_free_execution_slot`.
        assert_eq!(
            initialize_tracker_and_compute_tx_start_time(
                &mut shared_object_congestion_tracker,
                &shared_input_objects,
                5
            ),
            Some(10)
        );

        // a transaction with duration 5 that writes object 1 should have start_time 10
        // with or without `assign_min_free_execution_slot`.
        let shared_input_objects = construct_shared_input_objects(&[(object_id_1, true)]);
        assert_eq!(
            initialize_tracker_and_compute_tx_start_time(
                &mut shared_object_congestion_tracker,
                &shared_input_objects,
                5
            ),
            Some(10)
        );

        // a transaction with duration 5 that reads objects 0 and 1 should have
        // start_time 10 with or without `assign_min_free_execution_slot`.
        let shared_input_objects =
            construct_shared_input_objects(&[(object_id_0, false), (object_id_1, false)]);
        assert_eq!(
            initialize_tracker_and_compute_tx_start_time(
                &mut shared_object_congestion_tracker,
                &shared_input_objects,
                5
            ),
            Some(10)
        );

        // a transaction with duration 5 that writes objects 0 and 1 should have
        // start_time 10 with or without `assign_min_free_execution_slot`.
        let shared_input_objects =
            construct_shared_input_objects(&[(object_id_0, true), (object_id_1, true)]);
        assert_eq!(
            initialize_tracker_and_compute_tx_start_time(
                &mut shared_object_congestion_tracker,
                &shared_input_objects,
                5
            ),
            Some(10)
        );

        // a transaction with duration 5 that writes object 2 should have start_time 0
        // with `assign_min_free_execution_slot` or 10 without
        // `assign_min_free_execution_slot`.
        let shared_input_objects = construct_shared_input_objects(&[(object_id_2, true)]);
        assert_eq!(
            initialize_tracker_and_compute_tx_start_time(
                &mut shared_object_congestion_tracker,
                &shared_input_objects,
                5
            ),
            if assign_min_free_execution_slot {
                Some(0)
            } else {
                Some(10)
            }
        );

        // a transaction with duration 5 that writes to the previously untouched object
        // 3 should have start_time 0 with or without
        // `assign_min_free_execution_slot`.
        let shared_input_objects = construct_shared_input_objects(&[(object_id_3, true)]);
        assert_eq!(
            initialize_tracker_and_compute_tx_start_time(
                &mut shared_object_congestion_tracker,
                &shared_input_objects,
                5
            ),
            Some(0)
        );

        // a transaction with duration 3 that reads objects 0 and 2 should have
        // start_time 5 with `assign_min_free_execution_slot` or 10 without
        // `assign_min_free_execution_slot`.
        let shared_input_objects =
            construct_shared_input_objects(&[(object_id_0, false), (object_id_2, false)]);
        assert_eq!(
            initialize_tracker_and_compute_tx_start_time(
                &mut shared_object_congestion_tracker,
                &shared_input_objects,
                3
            ),
            if assign_min_free_execution_slot {
                Some(5)
            } else {
                Some(10)
            }
        );
    }

    #[rstest]
    fn test_try_schedule_return_correct_congested_objects(
        #[values(
            PerObjectCongestionControlMode::TotalGasBudget,
            PerObjectCongestionControlMode::TotalTxCount
        )]
        mode: PerObjectCongestionControlMode,
        #[values(true, false)] assign_min_free_execution_slot: bool,
    ) {
        // Creates two shared objects and three transactions that operate on these
        // objects.
        let shared_obj_0 = ObjectID::random();
        let shared_obj_1 = ObjectID::random();

        let (max_execution_duration_per_commit, max_overshoot_per_commit) = match mode {
            PerObjectCongestionControlMode::None => unreachable!(),
            PerObjectCongestionControlMode::TotalGasBudget => (12, 0),
            PerObjectCongestionControlMode::TotalTxCount => (3, 0),
        };

        let (initial_debt_obj_0, initial_debt_obj_1) = match mode {
            PerObjectCongestionControlMode::None => unreachable!(),
            PerObjectCongestionControlMode::TotalGasBudget => {
                // Initial debts for TotalGasBudget mode are set such that
                // the object execution slots are constructed as follows:
                //    object 0       object 1
                // 0| xxxxxxxx     | xxxxxxxx
                // 1| xxxxxxxx     |
                // ::::::::::::::::::::::::::
                // 8| xxxxxxxx     |
                // 9|              |
                (9, 1)
            }
            PerObjectCongestionControlMode::TotalTxCount => {
                // Initial debts for TotalTxCount mode are set such that
                // the object execution slots are constructed as follows:
                //    object 0       object 1
                // 0| xxxxxxxx     | xxxxxxxx
                // 1| xxxxxxxx     |
                // 2|              |
                (2, 1)
            }
        };
        let mut shared_object_congestion_tracker =
            new_congestion_tracker_with_initial_value_for_test(
                &[
                    (shared_obj_0, initial_debt_obj_0),
                    (shared_obj_1, initial_debt_obj_1),
                ],
                CongestionControlParameters::new_for_test(
                    mode,
                    assign_min_free_execution_slot,
                    Some(max_execution_duration_per_commit),
                    Some(max_overshoot_per_commit),
                    0,     // not important in this test
                    false, // not important in this test
                    true,  // not important in this test
                ),
            );
        // add a transaction with gas budget 1 that writes to object 0 and 1.
        // We don't test the scheduling result here, we just want to update the
        // tracker's object execution slots.
        let tx_gas_budget = 1;
        let tx = build_transaction(
            &[(shared_obj_0, true), (shared_obj_1, true)],
            tx_gas_budget,
            TEST_ONLY_GAS_PRICE,
        );
        shared_object_congestion_tracker.bump_object_execution_slots(
            &tx,
            match mode {
                PerObjectCongestionControlMode::None => unreachable!(),
                // in TotalGasBudget mode, the object execution slots becomes:
                //    object 0       object 1
                //  0| xxxxxxxx     | xxxxxxxx
                //  1| xxxxxxxx     |
                //  ::::::::::::::::::::::::::
                //  8| xxxxxxxx     |
                //  9| xxxxxxxx     | xxxxxxxx
                // 10|              |
                // 11|______________|____________ max_execution_duration_per_commit = 12
                // 12|              |
                // 13|              |
                PerObjectCongestionControlMode::TotalGasBudget => 9,
                // in TotalTxCount mode, the object execution slots becomes:
                //    object 0       object 1
                // 0| xxxxxxxx     | xxxxxxxx
                // 1| xxxxxxxx     |
                // 2| xxxxxxxx_____|_xxxxxxxx____ max_execution_duration_per_commit = 3
                // 3|              |
                // 4|              |
                PerObjectCongestionControlMode::TotalTxCount => 2,
            },
        );

        // Read/write to object 0 should be deferred.
        let tx_gas_budget = 5;
        for mutable in [true, false].iter() {
            let tx = build_transaction(
                &[(shared_obj_0, *mutable)],
                tx_gas_budget,
                TEST_ONLY_GAS_PRICE,
            );
            if let SequencingResult::Defer(_, congested_objects) =
                shared_object_congestion_tracker.try_schedule(&tx, &HashMap::new(), 0)
            {
                assert_eq!(congested_objects.len(), 1);
                assert_eq!(congested_objects[0], shared_obj_0);
            } else {
                panic!("should defer");
            }
        }

        // Read/write to object 1 should be scheduled with start_time 1 with
        // `assign_min_free_execution_slot` and deferred otherwise.
        for mutable in [true, false].iter() {
            let tx = build_transaction(
                &[(shared_obj_1, *mutable)],
                tx_gas_budget,
                TEST_ONLY_GAS_PRICE,
            );
            let sequencing_result = initialize_tracker_and_try_schedule(
                &mut shared_object_congestion_tracker,
                &tx,
                &HashMap::new(),
                0,
            );
            if assign_min_free_execution_slot {
                assert!(matches!(sequencing_result, SequencingResult::Schedule(1)));
            } else if let SequencingResult::Defer(_, congested_objects) = sequencing_result {
                assert_eq!(congested_objects.len(), 1);
                assert_eq!(congested_objects[0], shared_obj_1);
            } else {
                panic!("should defer");
            }
        }

        // Transactions touching both objects should be deferred, with object 0 as the
        // congested object.
        for mutable_0 in [true, false].iter() {
            for mutable_1 in [true, false].iter() {
                let tx = build_transaction(
                    &[(shared_obj_0, *mutable_0), (shared_obj_1, *mutable_1)],
                    tx_gas_budget,
                    TEST_ONLY_GAS_PRICE,
                );
                if let SequencingResult::Defer(_, congested_objects) =
                    initialize_tracker_and_try_schedule(
                        &mut shared_object_congestion_tracker,
                        &tx,
                        &HashMap::new(),
                        0,
                    )
                {
                    // both objects should be reported as congested.
                    assert_eq!(congested_objects.len(), 2);
                    assert_eq!(congested_objects[0], shared_obj_0);
                    assert_eq!(congested_objects[1], shared_obj_1);
                } else {
                    panic!("should defer");
                }
            }
        }
    }

    #[rstest]
    fn test_try_schedule_return_correct_deferral_key(
        #[values(
            PerObjectCongestionControlMode::TotalGasBudget,
            PerObjectCongestionControlMode::TotalTxCount
        )]
        mode: PerObjectCongestionControlMode,
    ) {
        let shared_obj_0 = ObjectID::random();
        let tx = build_transaction(&[(shared_obj_0, true)], 100, TEST_ONLY_GAS_PRICE);
        // Make try_schedule always defers transactions.
        let max_execution_duration_per_commit = 0;
        let max_overshoot_per_commit = 0;
        let mut shared_object_congestion_tracker =
            new_congestion_tracker_with_initial_value_for_test(
                &[],
                CongestionControlParameters::new_for_test(
                    mode,
                    false,
                    Some(max_execution_duration_per_commit),
                    Some(max_overshoot_per_commit),
                    0,     // not important in this test
                    false, // not important in this test
                    true,  // not important in this test
                ),
            );

        // Insert a random pre-existing transaction.
        let mut previously_deferred_tx_digests = PreviouslyDeferredTransactions::new();
        previously_deferred_tx_digests.insert(
            TransactionDigest::random(),
            (
                DeferralKey::ConsensusRound {
                    future_round: 10,
                    deferred_from_round: 5,
                },
                Some(1_000),
            ),
        );

        // Test deferral key for a transaction that has not been deferred before.
        if let SequencingResult::Defer(
            DeferralKey::ConsensusRound {
                future_round,
                deferred_from_round,
            },
            _,
        ) = initialize_tracker_and_try_schedule(
            &mut shared_object_congestion_tracker,
            &tx,
            &previously_deferred_tx_digests,
            10,
        ) {
            assert_eq!(future_round, 11);
            assert_eq!(deferred_from_round, 10);
        } else {
            panic!("should defer");
        }

        // Insert `tx`` as previously deferred transaction due to randomness.
        previously_deferred_tx_digests.insert(
            *tx.digest(),
            (
                DeferralKey::Randomness {
                    deferred_from_round: 4,
                },
                None,
            ),
        );

        // New deferral key should have deferred_from_round equal to the deferred
        // randomness round.
        if let SequencingResult::Defer(
            DeferralKey::ConsensusRound {
                future_round,
                deferred_from_round,
            },
            _,
        ) = initialize_tracker_and_try_schedule(
            &mut shared_object_congestion_tracker,
            &tx,
            &previously_deferred_tx_digests,
            10,
        ) {
            assert_eq!(future_round, 11);
            assert_eq!(deferred_from_round, 4);
        } else {
            panic!("should defer");
        }

        // Insert `tx`` as previously deferred consensus transaction.
        previously_deferred_tx_digests.insert(
            *tx.digest(),
            (
                DeferralKey::ConsensusRound {
                    future_round: 10,
                    deferred_from_round: 5,
                },
                Some(1_000),
            ),
        );

        // New deferral key should have deferred_from_round equal to the one in the old
        // deferral key.
        if let SequencingResult::Defer(
            DeferralKey::ConsensusRound {
                future_round,
                deferred_from_round,
            },
            _,
        ) = initialize_tracker_and_try_schedule(
            &mut shared_object_congestion_tracker,
            &tx,
            &previously_deferred_tx_digests,
            10,
        ) {
            assert_eq!(future_round, 11);
            assert_eq!(deferred_from_round, 5);
        } else {
            panic!("should defer");
        }
    }

    #[rstest]
    fn test_bump_object_execution_slots(
        #[values(
            PerObjectCongestionControlMode::TotalGasBudget,
            PerObjectCongestionControlMode::TotalTxCount
        )]
        mode: PerObjectCongestionControlMode,
        #[values(true, false)] assign_min_free_execution_slot: bool,
    ) {
        let object_id_0 = ObjectID::random();
        let object_id_1 = ObjectID::random();
        let object_id_2 = ObjectID::random();

        let congestion_control_parameters = CongestionControlParameters::new_for_test(
            mode,
            assign_min_free_execution_slot,
            None,  // not important in this test
            None,  // not important in this test
            0,     // not important in this test
            false, // not important in this test
            true,  // not important in this test
        );

        let mut shared_object_congestion_tracker =
            new_congestion_tracker_with_initial_value_for_test(
                &[(object_id_0, 5), (object_id_1, 10)],
                congestion_control_parameters.clone(),
            );
        assert_eq!(
            shared_object_congestion_tracker.max_occupied_slot_end_time(),
            10
        );

        // Read two objects should not change the object execution slots.
        let cert = build_transaction(
            &[(object_id_0, false), (object_id_1, false)],
            10,
            TEST_ONLY_GAS_PRICE,
        );
        let cert_duration = shared_object_congestion_tracker
            .congestion_control_parameters
            .get_estimated_execution_duration(&cert);
        let start_time = initialize_tracker_and_compute_tx_start_time(
            &mut shared_object_congestion_tracker,
            &cert.shared_input_objects(),
            cert_duration,
        )
        .expect("start time should be computable");

        shared_object_congestion_tracker.bump_object_execution_slots(&cert, start_time);
        assert_eq!(
            shared_object_congestion_tracker,
            new_congestion_tracker_with_initial_value_for_test(
                &[(object_id_0, 5), (object_id_1, 10)],
                congestion_control_parameters,
            )
        );
        assert_eq!(
            shared_object_congestion_tracker.max_occupied_slot_end_time(),
            10
        );

        // Write to object 0 should only bump object 0's execution slots. The start time
        // should be object 1's duration.
        let cert = build_transaction(
            &[(object_id_0, true), (object_id_1, false)],
            10,
            TEST_ONLY_GAS_PRICE,
        );
        let cert_duration = shared_object_congestion_tracker
            .congestion_control_parameters
            .get_estimated_execution_duration(&cert);
        let start_time = initialize_tracker_and_compute_tx_start_time(
            &mut shared_object_congestion_tracker,
            &cert.shared_input_objects(),
            cert_duration,
        )
        .expect("start time should be computable");
        shared_object_congestion_tracker.bump_object_execution_slots(&cert, start_time);
        let expected_object_0_duration = match mode {
            PerObjectCongestionControlMode::None => unreachable!(),
            PerObjectCongestionControlMode::TotalGasBudget => 20,
            PerObjectCongestionControlMode::TotalTxCount => 11,
        };
        assert_eq!(
            shared_object_congestion_tracker
                .object_execution_slots
                .get(&object_id_0)
                .unwrap()
                .max_object_occupied_slot_end_time(),
            expected_object_0_duration
        );
        assert_eq!(
            shared_object_congestion_tracker
                .object_execution_slots
                .get(&object_id_1)
                .unwrap()
                .max_object_occupied_slot_end_time(),
            10
        );
        assert_eq!(
            shared_object_congestion_tracker.max_occupied_slot_end_time(),
            expected_object_0_duration
        );

        // Write to all objects should bump all objects' execution durations, including
        // objects that are seen for the first time.
        let cert = build_transaction(
            &[
                (object_id_0, true),
                (object_id_1, true),
                (object_id_2, true),
            ],
            10,
            TEST_ONLY_GAS_PRICE,
        );
        let expected_object_duration = match mode {
            PerObjectCongestionControlMode::None => unreachable!(),
            PerObjectCongestionControlMode::TotalGasBudget => 30,
            PerObjectCongestionControlMode::TotalTxCount => 12,
        };
        let cert_duration = shared_object_congestion_tracker
            .congestion_control_parameters
            .get_estimated_execution_duration(&cert);
        let start_time = initialize_tracker_and_compute_tx_start_time(
            &mut shared_object_congestion_tracker,
            &cert.shared_input_objects(),
            cert_duration,
        )
        .expect("start time should be computable");
        shared_object_congestion_tracker.bump_object_execution_slots(&cert, start_time);
        assert_eq!(
            shared_object_congestion_tracker
                .object_execution_slots
                .get(&object_id_0)
                .unwrap()
                .max_object_occupied_slot_end_time(),
            expected_object_duration
        );
        assert_eq!(
            shared_object_congestion_tracker
                .object_execution_slots
                .get(&object_id_1)
                .unwrap()
                .max_object_occupied_slot_end_time(),
            expected_object_duration
        );
        assert_eq!(
            shared_object_congestion_tracker
                .object_execution_slots
                .get(&object_id_2)
                .unwrap()
                .max_object_occupied_slot_end_time(),
            expected_object_duration
        );
        assert_eq!(
            shared_object_congestion_tracker.max_occupied_slot_end_time(),
            expected_object_duration
        );
    }

    #[rstest]
    fn test_slots_overflow(#[values(true, false)] assign_min_free_execution_slot: bool) {
        let object_id_0 = ObjectID::random();
        let object_id_1 = ObjectID::random();
        let object_id_2 = ObjectID::random();
        // edge case: max value is saturated
        let max_execution_duration_per_commit = u64::MAX;
        let max_overshoot_per_commit = u64::MAX;

        let congestion_control_parameters = CongestionControlParameters::new_for_test(
            PerObjectCongestionControlMode::TotalGasBudget,
            assign_min_free_execution_slot,
            Some(max_execution_duration_per_commit),
            Some(max_overshoot_per_commit),
            0,     // not important in this test
            false, // not important in this test
            true,  // not important in this test
        );

        // case 1: large initial duration, small tx duration
        // the initial object execution slots is as follows:
        //               object 0       object 1
        //            0| xxxxxxxx     | xxxxxxxx
        //            1| xxxxxxxx     | xxxxxxxx
        // :::::::::::::::::::::::::::::::::::::
        // u64::MAX - 2| xxxxxxxx     | xxxxxxxx
        // u64::MAX - 1|              |

        let mut shared_object_congestion_tracker =
            new_congestion_tracker_with_initial_value_for_test(
                &[(object_id_0, u64::MAX - 1), (object_id_1, u64::MAX - 1)],
                congestion_control_parameters.clone(),
            );

        let tx = build_transaction(&[(object_id_0, true)], 1, TEST_ONLY_GAS_PRICE);
        if let SequencingResult::Schedule(start_time) = initialize_tracker_and_try_schedule(
            &mut shared_object_congestion_tracker,
            &tx,
            &HashMap::new(),
            0,
        ) {
            // add the small transaction to the tracker
            // the object execution slots becomes:
            //               object 0       object 1
            //            0| xxxxxxxx     | xxxxxxxx
            //            1| xxxxxxxx     | xxxxxxxx
            // :::::::::::::::::::::::::::::::::::::
            // u64::MAX - 2| xxxxxxxx     | xxxxxxxx
            // u64::MAX - 1| xxxxxxxx     |
            shared_object_congestion_tracker.bump_object_execution_slots(&tx, start_time);
            assert_eq!(
                shared_object_congestion_tracker
                    .object_execution_slots
                    .get(&object_id_0)
                    .unwrap()
                    .max_object_occupied_slot_end_time(),
                MAX_EXECUTION_TIME
            );
            assert_eq!(
                shared_object_congestion_tracker
                    .object_execution_slots
                    .get(&object_id_1)
                    .unwrap()
                    .max_object_occupied_slot_end_time(),
                MAX_EXECUTION_TIME - 1
            );
        } else {
            panic!("transaction is not congesting, should not defer");
        }

        let tx = build_transaction(
            &[(object_id_0, true), (object_id_1, true)],
            1,
            TEST_ONLY_GAS_PRICE,
        );
        if let SequencingResult::Defer(_, congested_objects) = initialize_tracker_and_try_schedule(
            &mut shared_object_congestion_tracker,
            &tx,
            &HashMap::new(),
            0,
        ) {
            // object 0 should be reported as congested in both cases.
            assert_eq!(congested_objects[0], object_id_0);
            if assign_min_free_execution_slot {
                assert_eq!(congested_objects.len(), 2);
                assert_eq!(congested_objects[1], object_id_1);
            } else {
                assert_eq!(congested_objects.len(), 1);
            }
        } else {
            panic!("transaction is congesting, should defer");
        }

        let cert_duration = shared_object_congestion_tracker
            .congestion_control_parameters
            .get_estimated_execution_duration(&tx);
        assert!(
            initialize_tracker_and_compute_tx_start_time(
                &mut shared_object_congestion_tracker,
                &tx.shared_input_objects(),
                cert_duration,
            )
            .is_none()
        );

        // case 2: small initial duration, large tx duration
        // the initial object execution slots is as follows:
        //     object 0       object 1       object 2
        //  0|              | xxxxxxxx     | xxxxxxxx
        //  1|              |              | xxxxxxxx
        //  2|              |              |
        let mut shared_object_congestion_tracker =
            new_congestion_tracker_with_initial_value_for_test(
                &[(object_id_0, 0), (object_id_1, 1), (object_id_2, 2)],
                congestion_control_parameters.clone(),
            );

        let tx = build_transaction(
            &[
                (object_id_0, true),
                (object_id_1, true),
                (object_id_2, true),
            ],
            MAX_EXECUTION_TIME - 1,
            TEST_ONLY_GAS_PRICE,
        );
        if let SequencingResult::Defer(_, congested_objects) = initialize_tracker_and_try_schedule(
            &mut shared_object_congestion_tracker,
            &tx,
            &HashMap::new(),
            0,
        ) {
            // objects 2 should be reported as congested in both cases, but 0 and 1 should
            // also be reported when `assign_min_free_execution_slot` is true.
            if assign_min_free_execution_slot {
                assert_eq!(congested_objects.len(), 3);
                assert_eq!(congested_objects[0], object_id_0);
                assert_eq!(congested_objects[1], object_id_1);
                assert_eq!(congested_objects[2], object_id_2);
            } else {
                assert_eq!(congested_objects.len(), 1);
                assert_eq!(congested_objects[0], object_id_2);
            }
        } else {
            panic!("case 2: object 2 is congested, should defer");
        }

        let cert_duration = shared_object_congestion_tracker
            .congestion_control_parameters
            .get_estimated_execution_duration(&tx);
        assert!(
            initialize_tracker_and_compute_tx_start_time(
                &mut shared_object_congestion_tracker,
                &tx.shared_input_objects(),
                cert_duration,
            )
            .is_none()
        );

        // case 3: max initial duration, max tx duration
        // the initial object execution slots is as follows:
        //               object 0
        //            0| xxxxxxxx
        //            1| xxxxxxxx
        // :::::::::::::
        // u64::MAX - 1| xxxxxxxx
        let mut shared_object_congestion_tracker =
            new_congestion_tracker_with_initial_value_for_test(
                &[(object_id_0, u64::MAX)],
                congestion_control_parameters,
            );

        let tx = build_transaction(&[(object_id_0, true)], u64::MAX, TEST_ONLY_GAS_PRICE);
        if let SequencingResult::Defer(_, congested_objects) = initialize_tracker_and_try_schedule(
            &mut shared_object_congestion_tracker,
            &tx,
            &HashMap::new(),
            0,
        ) {
            assert_eq!(congested_objects.len(), 1);
            assert_eq!(congested_objects[0], object_id_0);
        } else {
            panic!("case 3: object 0 is congested, should defer");
        }

        let cert_duration = shared_object_congestion_tracker
            .congestion_control_parameters
            .get_estimated_execution_duration(&tx);
        assert!(
            initialize_tracker_and_compute_tx_start_time(
                &mut shared_object_congestion_tracker,
                &tx.shared_input_objects(),
                cert_duration,
            )
            .is_none()
        );
    }

    #[rstest]
    fn test_try_schedule_allow_overshoot(
        #[values(
            PerObjectCongestionControlMode::TotalGasBudget,
            PerObjectCongestionControlMode::TotalTxCount
        )]
        mode: PerObjectCongestionControlMode,
        #[values(true, false)] assign_min_free_execution_slot: bool,
    ) {
        let shared_obj_0 = ObjectID::random();
        let shared_obj_1 = ObjectID::random();

        let tx_gas_budget = 100;

        let max_execution_duration_per_commit = match mode {
            PerObjectCongestionControlMode::None => unreachable!(),
            PerObjectCongestionControlMode::TotalGasBudget => 100,
            PerObjectCongestionControlMode::TotalTxCount => 2,
        };

        let max_overshoot_per_commit = match mode {
            PerObjectCongestionControlMode::None => unreachable!(),
            PerObjectCongestionControlMode::TotalGasBudget => 200,
            PerObjectCongestionControlMode::TotalTxCount => 2,
        };

        let congestion_control_parameters = CongestionControlParameters::new_for_test(
            mode,
            assign_min_free_execution_slot,
            Some(max_execution_duration_per_commit),
            Some(max_overshoot_per_commit),
            0,     // not important in this test
            false, // not important in this test
            true,  // not important in this test
        );

        // instantiate the tracker with some initial debts such that 1 transaction
        // touching object 1 can be scheduled with some overshoot, but nothing touching
        // object 0 can be scheduled.
        let shared_object_congestion_tracker = match mode {
            PerObjectCongestionControlMode::None => unreachable!(),
            PerObjectCongestionControlMode::TotalGasBudget => {
                // Construct object execution cost as following
                //          object 0    object 1
                //       0| xxxxxxxx   | xxxxxxxx
                // :::::::::::::::::::::::::::
                //      99| xxxxxxxx   | xxxxxxxx _____ max_execution_duration_per_commit = 100
                //     100| xxxxxxxx   | xxxxxxxx
                // :::::::::::::::::::::::::::
                //     198| xxxxxxxx   | xxxxxxxx
                //     199| xxxxxxxx   |
                // :::::::::::::::::::::::::::
                //     299| xxxxxxxx   |          _____ 100 + max_overshoot_per_commit = 300
                //     300| xxxxxxxx   |
                //     301|            |
                SharedObjectCongestionTracker::new(
                    [(shared_obj_0, 301), (shared_obj_1, 199)],
                    congestion_control_parameters,
                )
            }
            PerObjectCongestionControlMode::TotalTxCount => {
                // Construct object execution cost as following
                //           object 0    object 1
                //        0| xxxxxxxx   | xxxxxxxx
                //        1| xxxxxxxx   | xxxxxxxx _____ max_execution_duration_per_commit = 2
                //        2| xxxxxxxx   | xxxxxxxx
                //        3| xxxxxxxx   |          _____ 2 + max_overshoot_per_commit = 4
                //        4|            |
                SharedObjectCongestionTracker::new(
                    [(shared_obj_0, 4), (shared_obj_1, 3)],
                    congestion_control_parameters,
                )
            }
        };

        // Read/write to object 0 should be deferred.
        for mutable in [true, false].iter() {
            let tx = build_transaction(
                &[(shared_obj_0, *mutable)],
                tx_gas_budget,
                TEST_ONLY_GAS_PRICE,
            );
            if let SequencingResult::Defer(_, congested_objects) =
                shared_object_congestion_tracker.try_schedule(&tx, &HashMap::new(), 0)
            {
                assert_eq!(congested_objects.len(), 1);
                assert_eq!(congested_objects[0], shared_obj_0);
            } else {
                panic!("should defer");
            }
        }

        // Read/write to object 1 should go through even though the per-commit limit is
        // exceeded even before the cost of this tx is considered.
        for mutable in [true, false].iter() {
            let tx = build_transaction(
                &[(shared_obj_1, *mutable)],
                tx_gas_budget,
                TEST_ONLY_GAS_PRICE,
            );
            if let SequencingResult::Schedule(_) =
                shared_object_congestion_tracker.try_schedule(&tx, &HashMap::new(), 0)
            {
                // pass
            } else {
                panic!("should schedule");
            }
        }

        // Transactions touching both objects should be deferred, with object 0 as the
        // congested object, or both objects as congested when
        // `assign_min_free_execution_slot` is true.
        for mutable_0 in [true, false].iter() {
            for mutable_1 in [true, false].iter() {
                let tx = build_transaction(
                    &[(shared_obj_0, *mutable_0), (shared_obj_1, *mutable_1)],
                    tx_gas_budget,
                    1,
                );
                if let SequencingResult::Defer(_, congested_objects) =
                    shared_object_congestion_tracker.try_schedule(&tx, &HashMap::new(), 0)
                {
                    if assign_min_free_execution_slot {
                        assert_eq!(congested_objects.len(), 2);
                    } else {
                        assert_eq!(congested_objects.len(), 1);
                        assert_eq!(congested_objects[0], shared_obj_0);
                    }
                } else {
                    panic!("should defer");
                }
            }
        }
    }

    #[rstest]
    fn test_accumulated_debts(
        #[values(
            PerObjectCongestionControlMode::TotalGasBudget,
            PerObjectCongestionControlMode::TotalTxCount
        )]
        mode: PerObjectCongestionControlMode,
        #[values(true, false)] assign_min_free_execution_slot: bool,
    ) {
        // Creates two shared objects to operate on them in transactions.
        let shared_obj_0 = ObjectID::random();
        let shared_obj_1 = ObjectID::random();

        let tx_gas_budget = 100;

        // Set max_accumulated_txn_cost_per_object_in_commit  and initial_object_debt
        // such that a single transaction will cause an overshoot.
        let max_execution_duration_per_commit = match mode {
            PerObjectCongestionControlMode::None => unreachable!(),
            PerObjectCongestionControlMode::TotalGasBudget => 90,
            PerObjectCongestionControlMode::TotalTxCount => 2,
        };

        let initial_object_debt = match mode {
            PerObjectCongestionControlMode::None => unreachable!(),
            PerObjectCongestionControlMode::TotalGasBudget => 70,
            PerObjectCongestionControlMode::TotalTxCount => 2,
        };

        let mut shared_object_congestion_tracker = SharedObjectCongestionTracker::new(
            [
                (shared_obj_0, initial_object_debt),
                (shared_obj_1, initial_object_debt),
            ],
            CongestionControlParameters::new_for_test(
                mode,
                assign_min_free_execution_slot,
                Some(max_execution_duration_per_commit),
                None,  // not important in this test
                0,     // not important in this test
                false, // not important in this test
                true,  // not important in this test
            ),
        );

        // Verify that accumulated_debts is empty initially.
        let accumulated_debts = shared_object_congestion_tracker
            .clone()
            .accumulated_debts(max_execution_duration_per_commit);
        assert!(accumulated_debts.is_empty());

        // Simulate transactions on object 0 that exceed the per-commit limit,
        // taking into account the initial debt.
        // We simulate both read and write access, but the read transaction should not
        // result in any change to the tracker state.
        for mutable in [true, false].iter() {
            let tx = build_transaction(
                &[(shared_obj_0, *mutable)],
                tx_gas_budget,
                TEST_ONLY_GAS_PRICE,
            );
            shared_object_congestion_tracker.bump_object_execution_slots(&tx, initial_object_debt);
        }

        // Verify that accumulated_debts reports the debt for object 0.
        let accumulated_debts =
            shared_object_congestion_tracker.accumulated_debts(max_execution_duration_per_commit);
        assert_eq!(accumulated_debts.len(), 1);
        match mode {
            PerObjectCongestionControlMode::None => unreachable!(),
            PerObjectCongestionControlMode::TotalGasBudget => {
                assert_eq!(accumulated_debts[0], (shared_obj_0, 80)); // overshoot = initial_debt (70) + tx_duration (100) - max_execution_duration_per_commit (90) = 80
            }
            PerObjectCongestionControlMode::TotalTxCount => {
                assert_eq!(accumulated_debts[0], (shared_obj_0, 1)); // overshoot = initial_debt (2) + tx_duration (1) - max_execution_duration_per_commit (2) = 1
            }
        }
    }
}
