// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{cmp::Ordering, collections::HashMap};

use iota_sdk_types::{ObjectId, SharedObjectReference};
use iota_types::{
    base_types::CommitRound,
    executable_transaction::VerifiedExecutableTransaction,
    transaction::{SenderSignedTransactionAPI, TransactionAPI},
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
    Defer(DeferralKey, Vec<ObjectId>),
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

/// A contiguous interval `[start_time, end_time)` during which `worker_count`
/// transactions are scheduled to occupy an execution worker concurrently.
#[derive(PartialEq, Eq, Clone, Debug, Copy)]
struct WorkerSlot {
    start_time: ExecutionTime,
    end_time: ExecutionTime,
    worker_count: u16,
}

/// `WorkerSlots` models the execution-worker pool as a concurrency profile
/// over the per-commit timeline: a sparse, sorted list of contiguous busy
/// slots (gaps between slots have worker count `0`). It mirrors
/// `ObjectExecutionSlots` but tracks a worker count (multiplicity) per slot
/// instead of a single free/busy lane, so it can enforce "at most `N`
/// transactions overlapping at any instant". The representation is sparse —
/// at most two breakpoints per scheduled transaction — so it is suitable for
/// `TotalGasBudget` mode where durations are large.
#[derive(PartialEq, Eq, Clone, Debug)]
struct WorkerSlots(Vec<WorkerSlot>);

impl WorkerSlots {
    #[cfg(test)]
    fn new() -> Self {
        Self(Vec::new())
    }

    /// Appends `[start, end)` with `count` to a slot list, coalescing with
    /// the previous slot when they are adjacent and share a count, and
    /// dropping empty or zero-count pieces. Inputs must arrive in ascending
    /// time order.
    fn push_slot(
        slots: &mut Vec<WorkerSlot>,
        start: ExecutionTime,
        end: ExecutionTime,
        count: u16,
    ) {
        if count == 0 || start >= end {
            return;
        }
        if let Some(last) = slots.last_mut() {
            if last.end_time == start && last.worker_count == count {
                last.end_time = end;
                return;
            }
        }
        slots.push(WorkerSlot {
            start_time: start,
            end_time: end,
            worker_count: count,
        });
    }

    /// Returns the free execution slots in which a new transaction can be
    /// scheduled without exceeding `n` concurrent workers, i.e. the intervals
    /// of `[0, MAX)` where the worker count is strictly below `n`. The result
    /// is a valid free-list (sorted, non-overlapping) and can be
    /// intersected with object free-lists during scheduling.
    ///
    /// Single pass over the (sorted, disjoint) slots: the free-list is the
    /// complement of the saturated (`count >= n`) slots within `[0, MAX)`.
    fn slots_with_worker_available(&self, max_concurrent_workers: u16) -> ObjectExecutionSlots {
        let mut free_slots = Vec::new();
        let mut cursor = 0;
        for slot in &self.0 {
            // Slots below the cap (and the implicit gaps between slots)
            // remain free, so only saturated slots break the free region.
            if slot.worker_count >= max_concurrent_workers {
                if cursor < slot.start_time {
                    free_slots.push(ExecutionSlot::new(cursor, slot.start_time));
                }
                cursor = slot.end_time;
            }
        }
        if cursor < MAX_EXECUTION_TIME {
            free_slots.push(ExecutionSlot::new(cursor, MAX_EXECUTION_TIME));
        }
        ObjectExecutionSlots(free_slots)
    }

    /// Returns the end time of the last slot in which a worker is occupied, or
    /// `0` if no transaction has been scheduled. The slots are sorted and
    /// zero-count slots are never stored, so this is the last slot's end time.
    fn max_occupied_end_time(&self) -> ExecutionTime {
        self.0.last().map_or(0, |slot| slot.end_time)
    }

    /// Increments the worker count over `[start_time, start_time + duration)`,
    /// maintaining the invariant (sorted, disjoint, adjacent-equal-count
    /// slots merged, no zero-count slots).
    ///
    /// Single pass over the existing (sorted, disjoint) slots: each is split
    /// into its before-, within- and after-`[start, end)` portions (the within
    /// portion getting `+1`), and gaps inside `[start, end)` are emitted with
    /// count `1`. `filled` tracks how far the `[start, end)` region has been
    /// covered so the inter-slot gaps can be filled in order.
    fn occupy(&mut self, start_time: ExecutionTime, duration: ExecutionTime) {
        let end_time = start_time.saturating_add(duration);
        if start_time >= end_time {
            return;
        }

        let old = std::mem::take(&mut self.0);
        let mut merged: Vec<WorkerSlot> = Vec::with_capacity(old.len() + 2);
        // Next position within `[start_time, end_time)` not yet covered, so any
        // gap between slots inside the range can be emitted with count 1.
        let mut filled = start_time;

        for slot in old {
            let WorkerSlot {
                start_time: a,
                end_time: b,
                worker_count: c,
            } = slot;

            // Gap inside `[start_time, end_time)` preceding this slot.
            if a > filled && filled < end_time {
                let gap_end = a.min(end_time);
                Self::push_slot(&mut merged, filled, gap_end, 1);
                filled = filled.max(gap_end);
            }
            // Portion before the occupied range: count unchanged.
            Self::push_slot(&mut merged, a, b.min(start_time), c);
            // Portion within the occupied range: count + 1.
            let within_start = a.max(start_time);
            let within_end = b.min(end_time);
            if within_start < within_end {
                Self::push_slot(&mut merged, within_start, within_end, c.saturating_add(1));
                filled = filled.max(within_end);
            }
            // Portion after the occupied range: count unchanged.
            Self::push_slot(&mut merged, a.max(end_time), b, c);
        }
        // Trailing gap inside `[start_time, end_time)` after the last slot.
        Self::push_slot(&mut merged, filled, end_time, 1);

        self.0 = merged;
    }

    /// Reconstructs a profile from a stored debt (`(start, end, count)`
    /// slots). The input is assumed to already satisfy the invariant
    /// (sorted, disjoint, merged), as produced by [`Self::overshoot`] and
    /// [`Self::decay`].
    fn from_debt(slots: Vec<(ExecutionTime, ExecutionTime, u16)>) -> Self {
        Self(
            slots
                .into_iter()
                .map(|(start_time, end_time, worker_count)| WorkerSlot {
                    start_time,
                    end_time,
                    worker_count,
                })
                .collect(),
        )
    }

    /// Returns the worker slots that extend past
    /// `max_execution_duration_per_commit`, shifted left so that
    /// `max_execution_duration_per_commit` becomes time `0`. This is the
    /// worker work still "running" at the start of the next commit
    /// and is carried over as its initial worker slots.
    fn overshoot(
        &self,
        max_execution_duration_per_commit: ExecutionTime,
    ) -> Vec<(ExecutionTime, ExecutionTime, u16)> {
        self.0
            .iter()
            .filter_map(|s| {
                let start = s.start_time.max(max_execution_duration_per_commit);
                (start < s.end_time).then(|| {
                    (
                        start - max_execution_duration_per_commit,
                        s.end_time - max_execution_duration_per_commit,
                        s.worker_count,
                    )
                })
            })
            .collect()
    }

    /// Shifts a debt left by `shift`, dropping the portion that falls below
    /// time `0`. Used to age a stored debt by the budget of the commits
    /// that elapsed since it was recorded.
    fn decay(
        slots: Vec<(ExecutionTime, ExecutionTime, u16)>,
        shift: ExecutionTime,
    ) -> Vec<(ExecutionTime, ExecutionTime, u16)> {
        slots
            .into_iter()
            .filter_map(|(start, end, count)| {
                let end = end.saturating_sub(shift);
                (end > 0).then(|| (start.saturating_sub(shift), end, count))
            })
            .collect()
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
    object_execution_slots: HashMap<ObjectId, ObjectExecutionSlots>,
    /// Concurrency profile of the execution-worker pool. `Some` only when
    /// execution-worker congestion control is active (see
    /// `CongestionControlParameters::max_concurrent_execution_workers`), in
    /// which case every scheduled transaction — including owned-object-only
    /// ones — occupies a worker over its execution interval.
    worker_slots: Option<WorkerSlots>,
    congestion_control_parameters: CongestionControlParameters,
}

impl SharedObjectCongestionTracker {
    /// Create a new `SharedObjectCongestionTracker` for the given
    /// `CongestionControlParameters`, taking into account the per-object debts
    /// (`initial_object_debts`) and the execution-worker debt
    /// (`initial_worker_debt`) carried over from prior commits. The worker debt
    /// is ignored when execution-worker congestion control is inactive.
    pub(super) fn new(
        initial_object_debts: impl IntoIterator<Item = (ObjectId, u64)>,
        initial_worker_debt: Vec<(ExecutionTime, ExecutionTime, u16)>,
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

        let worker_slots = congestion_control_parameters
            .max_concurrent_execution_workers()
            .map(|_| WorkerSlots::from_debt(initial_worker_debt));

        Self {
            object_execution_slots,
            worker_slots,
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
        shared_input_objects: &[SharedObjectReference],
    ) {
        for obj in shared_input_objects {
            self.object_execution_slots
                .entry(obj.object_id)
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
        shared_input_objects: &[SharedObjectReference],
        tx_duration: ExecutionTime,
        check_worker_limit: bool,
    ) -> Option<ExecutionTime> {
        // Collect the free-list of every resource the transaction must fit in:
        // one per shared input object, plus the execution-worker pool when
        // worker congestion control is active and `check_worker_limit` is set.
        let mut resources: Vec<&ObjectExecutionSlots> = shared_input_objects
            .iter()
            .map(|obj| {
                self.object_execution_slots
                    .get(&obj.object_id)
                    .expect("object should have been inserted at the start of this function.")
            })
            .collect();
        let worker_free_slots = if check_worker_limit {
            self.worker_slots
                .as_ref()
                .zip(
                    self.congestion_control_parameters
                        .max_concurrent_execution_workers(),
                )
                .map(|(worker_slots, n)| worker_slots.slots_with_worker_available(n))
        } else {
            None
        };
        if let Some(worker_free_slots) = &worker_free_slots {
            resources.push(worker_free_slots);
        }
        if resources.is_empty() {
            // No constraining resources (e.g. an owned-object-only transaction
            // when worker congestion control is disabled): schedule at time 0.
            return Some(0);
        }

        if self
            .congestion_control_parameters
            .congestion_control_min_free_execution_slot()
        {
            // If `congestion_control_min_free_execution_slot` is true, we assign the
            // transaction start time based on the lowest free execution slot that
            // can accommodate the transaction across all resources. We start the
            // search from the full range with no constraints from previous resources.
            let _span = tracing::trace_span!("compute_min_free_execution_slot").entered();
            let initial_free_slot = ExecutionSlot::max_duration_slot();
            Self::compute_min_free_execution_slot(&resources, tx_duration, initial_free_slot)
        } else {
            // If `congestion_control_min_free_execution_slot` is false, we assign the
            // transaction start time based on the maximum start time of free execution
            // slots for the transaction over all its resources.
            let _span = tracing::trace_span!("max_object_free_slot_start_time").entered();
            resources
                .iter()
                .map(|slots| slots.max_object_free_slot_start_time(tx_duration))
                // If any `start_time` is `None` (i.e., the corresponding resource
                // does not have a free slot), the collect will return `None`
                .collect::<Option<Vec<_>>>()
                .and_then(|resource_start_times| resource_start_times.into_iter().max())
        }
    }

    /// A recursive function that tries to find the lowest free slot for a
    /// transaction across all `resources`. If a slot is found that fits the
    /// transaction in every resource simultaneously, returns its start time;
    /// otherwise returns None.
    /// lookup_interval is the range of the slot that the transaction can fit in
    /// given the resources that have been checked so far.
    fn compute_min_free_execution_slot(
        resources: &[&ObjectExecutionSlots],
        tx_duration: ExecutionTime,
        lookup_interval: ExecutionSlot,
    ) -> Option<ExecutionTime> {
        // Take the first resource, and set aside the remaining ones for the
        // next recursive call.
        let (resource, remaining_resources) = resources
            .split_first()
            .expect("resources must not be empty.");

        for intersection_slot in resource
            .0
            .iter()
            .filter_map(|slot| slot.intersection(&lookup_interval))
        {
            // If there is no overlap that can fit the transaction, continue to the next
            // free slot.
            if intersection_slot.duration() < tx_duration {
                continue;
            }
            // if this is the last resource to check, return this slot as it is the lowest
            // slot available.
            if remaining_resources.is_empty() {
                return Some(intersection_slot.start_time);
            }
            // if there are more resources to check, recursively call the function with the
            // remaining resources.
            // If the recursive call returns a start time, that means the transaction fits
            // in the slot for all remaining resources. Return the start time.
            // Otherwise, continue to check the next free slot for the current resource.
            if let Some(lowest_overlap) = Self::compute_min_free_execution_slot(
                remaining_resources,
                tx_duration,
                intersection_slot,
            ) {
                return Some(lowest_overlap);
            } else {
                continue;
            }
        }
        // if no slot is found for the current resource given the available range,
        // return None.
        None
    }

    /// Given a transaction, returns a sequencing result. If the transaction can
    /// be scheduled, this returns a `start_time`, and if it should be deferred,
    /// this returns the deferral key and the congested objects responsible for
    /// the deferral.
    #[instrument(level = "trace", skip_all, fields(tx_digest = ?transaction.digest()))]
    pub(super) fn try_schedule(
        &self,
        transaction: &VerifiedExecutableTransaction,
        previously_deferred_tx_digests: &PreviouslyDeferredTransactions,
        commit_round: CommitRound,
    ) -> SequencingResult {
        let tx_duration = self
            .congestion_control_parameters
            .get_estimated_execution_duration(transaction);
        if tx_duration == 0 {
            // This is a zero-duration transaction, no need to defer.
            return SequencingResult::Schedule(0);
        }

        let shared_input_objects = transaction.shared_input_objects();
        if shared_input_objects.is_empty() && self.worker_slots.is_none() {
            // This is an owned-object-only transaction and execution-worker
            // congestion control is disabled. No need to defer.
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

        // Try to compute a scheduling start time that fits both the shared
        // objects and (when active) the execution-worker pool.
        if let Some(start_time) =
            self.compute_tx_start_time(&shared_input_objects, tx_duration, true)
        {
            // `compute_tx_start_time` returns None if the transaction cannot be scheduled,
            // so no need to check for overflow when adding `tx_duration` here.
            if start_time + tx_duration <= congestion_limit {
                // schedule this transaction and return the start time.
                return SequencingResult::Schedule(start_time);
            }
        }

        // The transaction cannot be scheduled. Determine whether the shared
        // objects are the bottleneck (so we can report them as congested), or
        // whether the transaction fits the objects but is shed by the
        // execution-worker pool (reported with an empty object list).
        let objects_fit = self
            .compute_tx_start_time(&shared_input_objects, tx_duration, false)
            .is_some_and(|start_time| start_time + tx_duration <= congestion_limit);

        let congested_objects: Vec<ObjectId> = if objects_fit {
            // The shared objects fit within the congestion limit; the
            // execution-worker pool is the bottleneck. There is no specific
            // congested object to report.
            Vec::new()
        } else if self
            .congestion_control_parameters
            .congestion_control_min_free_execution_slot()
        {
            // If `congestion_control_min_free_execution_slot` is true, we return all the
            // shared input objects as no individual object can be identified as
            // the cause of congestion.
            shared_input_objects
                .iter()
                .map(|obj| obj.object_id)
                .collect()
        } else {
            // If `congestion_control_min_free_execution_slot` is false, we return
            // only shared objects that can be identified as the cause of congestion.
            shared_input_objects
                .iter()
                .filter(|obj| {
                    let (end_time, overflow) = self
                        .object_execution_slots
                        .get(&obj.object_id)
                        .expect("object should have been inserted before.")
                        .max_object_occupied_slot_end_time()
                        .overflowing_add(tx_duration);
                    overflow || end_time > congestion_limit
                })
                .map(|obj| obj.object_id)
                .collect()
        };

        let deferral_key = if let Some(previous_key_suggested_gas_price_pair) =
            previously_deferred_tx_digests.get(transaction.digest())
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

    /// Update shared objects' execution slots used in `transaction` using
    /// `transaction`'s estimated execution duration. This is called when
    /// `transaction` is scheduled for execution.
    ///
    /// `start_time` provides the start time of the execution slot assigned to
    /// `transaction`.
    ///
    /// Returns `Some(BumpObjectExecutionSlotsResult)` if `transaction`'s
    /// estimated execution duration is non-zero, else returns `None`.
    pub(super) fn bump_object_execution_slots(
        &mut self,
        transaction: &VerifiedExecutableTransaction,
        start_time: ExecutionTime,
    ) -> Option<BumpObjectExecutionSlotsResult> {
        let estimated_execution_duration = self
            .congestion_control_parameters
            .get_estimated_execution_duration(transaction);

        if estimated_execution_duration == 0 {
            return None;
        }

        let end_time = start_time.saturating_add(estimated_execution_duration);
        let occupied_slot = ExecutionSlot::new(start_time, end_time);

        // Find IDs of shared objects for which execution slots should be bumped.
        let object_ids = transaction
            .shared_input_objects()
            .into_iter()
            .filter_map(|obj| obj.mutable.then_some(obj.object_id))
            .collect::<Vec<_>>();

        object_ids.iter().for_each(|obj_id| {
            self.object_execution_slots
                .get_mut(obj_id)
                .expect("object execution slot should have been initialized before.")
                .remove(occupied_slot);
        });

        // Every scheduled transaction — including owned-object-only ones —
        // occupies an execution worker over its execution interval when
        // execution-worker congestion control is active.
        if let Some(worker_slots) = self.worker_slots.as_mut() {
            worker_slots.occupy(start_time, estimated_execution_duration);
        }

        Some(BumpObjectExecutionSlotsResult::new(
            object_ids,
            start_time,
            estimated_execution_duration,
            transaction.transaction().gas_price(),
        ))
    }

    /// Returns the maximum occupied slot end time over all shared objects and,
    /// when execution-worker congestion control is active, the execution-worker
    /// pool. The pool must be included because a commit of owned-object-only
    /// transactions occupies workers without occupying any object slot.
    pub(super) fn max_occupied_slot_end_time(&self) -> ExecutionTime {
        self.object_execution_slots
            .values()
            .map(|slots| slots.max_object_occupied_slot_end_time())
            .max()
            .unwrap_or(0)
            .max(
                self.worker_slots
                    .as_ref()
                    .map_or(0, WorkerSlots::max_occupied_end_time),
            )
    }

    /// Returns accumulated debts for objects whose budgets have been exceeded
    /// over the course of the commit. Consumes the tracker object, since
    /// this should only be called once after all txs have been processed.
    pub(super) fn accumulated_object_debts(
        self,
        max_execution_duration_per_commit: u64,
    ) -> Vec<(ObjectId, u64)> {
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

    /// Returns the execution-worker slots that extend past
    /// `max_execution_duration_per_commit`, shifted to start at time `0`, to be
    /// carried over as the next commit's initial worker slots. Returns
    /// `None` when execution-worker congestion control is inactive. Borrows
    /// (unlike [`Self::accumulated_object_debts`]) so it can be called before
    /// consuming the tracker for the per-object debts.
    pub(super) fn accumulated_worker_debt(
        &self,
        max_execution_duration_per_commit: u64,
    ) -> Option<Vec<(ExecutionTime, ExecutionTime, u16)>> {
        self.worker_slots
            .as_ref()
            .map(|worker_slots| worker_slots.overshoot(max_execution_duration_per_commit))
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

/// The execution-worker debt carried over from a consensus commit: the
/// worker concurrency profile that extends past the per-commit limit, stored
/// as `(start, end, count)` slots together with the round in which it was
/// recorded (so future commits can age it by their elapsed budget).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum CongestionWorkerDebt {
    V1(CommitRound, Vec<(ExecutionTime, ExecutionTime, u16)>),
}

impl CongestionWorkerDebt {
    pub(super) fn new(round: CommitRound, slots: Vec<(ExecutionTime, ExecutionTime, u16)>) -> Self {
        Self::V1(round, slots)
    }

    pub(super) fn into_v1(self) -> (CommitRound, Vec<(ExecutionTime, ExecutionTime, u16)>) {
        match self {
            Self::V1(round, slots) => (round, slots),
        }
    }

    /// Ages the debt recorded at `stored_round` for use at `current_round`,
    /// shifting it left by the execution budget of the fully-elapsed commits
    /// (`max_execution_duration_per_commit` per commit).
    pub(super) fn decayed(
        self,
        current_round: CommitRound,
        max_execution_duration_per_commit: ExecutionTime,
    ) -> Vec<(ExecutionTime, ExecutionTime, u16)> {
        let (stored_round, slots) = self.into_v1();
        // Mirrors the per-object debt aging: the stored debt already
        // accounts for the budget of its own round, so only fully-elapsed
        // rounds since then are applied.
        let num_rounds = current_round.saturating_sub(stored_round).saturating_sub(1);
        let shift = max_execution_duration_per_commit.saturating_mul(num_rounds);
        WorkerSlots::decay(slots, shift)
    }
}

/// Stores a result of the [`bump_object_execution_slots`] method
/// of `SharedObjectCongestionTracker` for a single scheduled transaction.
/// The result is then intended to be used in `SuggestedGasPriceCalculator`.
pub(super) struct BumpObjectExecutionSlotsResult {
    /// List of IDs of shared objects for which execution slots
    /// were bumped. Usually this includes shared objects accessed
    /// by a mutable reference in a transaction.
    object_ids: Vec<ObjectId>,

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
        object_ids: Vec<ObjectId>,
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
    pub(super) fn object_ids(&self) -> &[ObjectId] {
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
        object_ids: Vec<ObjectId>,
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
    use iota_sdk_types::Version;
    use iota_test_transaction_builder::TestTransactionBuilder;
    use iota_types::{
        base_types::random_object_ref,
        crypto::{AccountKeyPair, get_key_pair},
        executable_transaction::VerifiedExecutableTransaction,
        transaction::{CallArg, VerifiedTransaction},
    };

    use super::*;

    pub const TEST_ONLY_GAS_PRICE: u64 = 1_000;

    /// Builds a transaction with a list of shared objects and their mutability.
    /// The transaction is only used to test the
    /// `SharedObjectCongestionTracker` functions, therefore the content
    /// other than shared inputs, gas budget and gas price are not
    /// important.
    pub fn build_transaction(
        objects: &[(ObjectId, bool)],
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
                        ObjectId::random(),
                        "unimportant_module",
                        "unimportant_function",
                        objects
                            .iter()
                            .map(|(id, mutable)| {
                                CallArg::Shared(SharedObjectReference::new(
                                    *id,
                                    Version::default(),
                                    *mutable,
                                ))
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
        shared_input_objects: &[SharedObjectReference],
        tx_duration: ExecutionTime,
    ) -> Option<ExecutionTime> {
        shared_object_congestion_tracker.initialize_object_execution_slots(shared_input_objects);
        shared_object_congestion_tracker.compute_tx_start_time(
            shared_input_objects,
            tx_duration,
            false,
        )
    }

    pub(super) fn initialize_tracker_and_try_schedule(
        shared_object_congestion_tracker: &mut SharedObjectCongestionTracker,
        transaction: &VerifiedExecutableTransaction,
        previously_deferred_tx_digests: &PreviouslyDeferredTransactions,
        commit_round: CommitRound,
    ) -> SequencingResult {
        let shared_input_objects = transaction.shared_input_objects();
        shared_object_congestion_tracker.initialize_object_execution_slots(&shared_input_objects);
        shared_object_congestion_tracker.try_schedule(
            transaction,
            previously_deferred_tx_digests,
            commit_round,
        )
    }

    pub(crate) fn new_congestion_tracker_with_initial_value_for_test(
        init_values: &[(ObjectId, ExecutionTime)],
        congestion_control_parameters: CongestionControlParameters,
    ) -> SharedObjectCongestionTracker {
        SharedObjectCongestionTracker::new(
            init_values.iter().map(|(id, debt)| (*id, *debt)),
            Vec::new(),
            congestion_control_parameters,
        )
    }

    pub fn construct_shared_input_objects(
        objects: &[(ObjectId, bool)],
    ) -> Vec<SharedObjectReference> {
        objects
            .iter()
            .map(|(id, mutable)| SharedObjectReference::new(*id, Version::default(), *mutable))
            .collect()
    }
}

#[cfg(test)]
mod object_cost_tests {
    use iota_protocol_config::PerObjectCongestionControlMode;
    use iota_sdk_types::TransactionDigest;
    use rstest::rstest;

    use super::{shared_object_test_utils::*, *};

    #[rstest]
    fn test_compute_tx_start_at_time(#[values(true, false)] assign_min_free_execution_slot: bool) {
        let object_id_0 = ObjectId::random();
        let object_id_1 = ObjectId::random();
        let object_id_2 = ObjectId::random();
        let object_id_3 = ObjectId::random();

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
        let shared_obj_0 = ObjectId::random();
        let shared_obj_1 = ObjectId::random();

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
        let shared_obj_0 = ObjectId::random();
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
        let object_id_0 = ObjectId::random();
        let object_id_1 = ObjectId::random();
        let object_id_2 = ObjectId::random();

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
        let transaction = build_transaction(
            &[(object_id_0, false), (object_id_1, false)],
            10,
            TEST_ONLY_GAS_PRICE,
        );
        let tx_duration = shared_object_congestion_tracker
            .congestion_control_parameters
            .get_estimated_execution_duration(&transaction);
        let start_time = initialize_tracker_and_compute_tx_start_time(
            &mut shared_object_congestion_tracker,
            &transaction.shared_input_objects(),
            tx_duration,
        )
        .expect("start time should be computable");

        shared_object_congestion_tracker.bump_object_execution_slots(&transaction, start_time);
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
        let transaction = build_transaction(
            &[(object_id_0, true), (object_id_1, false)],
            10,
            TEST_ONLY_GAS_PRICE,
        );
        let tx_duration = shared_object_congestion_tracker
            .congestion_control_parameters
            .get_estimated_execution_duration(&transaction);
        let start_time = initialize_tracker_and_compute_tx_start_time(
            &mut shared_object_congestion_tracker,
            &transaction.shared_input_objects(),
            tx_duration,
        )
        .expect("start time should be computable");
        shared_object_congestion_tracker.bump_object_execution_slots(&transaction, start_time);
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
        let transaction = build_transaction(
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
        let tx_duration = shared_object_congestion_tracker
            .congestion_control_parameters
            .get_estimated_execution_duration(&transaction);
        let start_time = initialize_tracker_and_compute_tx_start_time(
            &mut shared_object_congestion_tracker,
            &transaction.shared_input_objects(),
            tx_duration,
        )
        .expect("start time should be computable");
        shared_object_congestion_tracker.bump_object_execution_slots(&transaction, start_time);
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
        let object_id_0 = ObjectId::random();
        let object_id_1 = ObjectId::random();
        let object_id_2 = ObjectId::random();
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

        let tx_duration = shared_object_congestion_tracker
            .congestion_control_parameters
            .get_estimated_execution_duration(&tx);
        assert!(
            initialize_tracker_and_compute_tx_start_time(
                &mut shared_object_congestion_tracker,
                &tx.shared_input_objects(),
                tx_duration,
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

        let tx_duration = shared_object_congestion_tracker
            .congestion_control_parameters
            .get_estimated_execution_duration(&tx);
        assert!(
            initialize_tracker_and_compute_tx_start_time(
                &mut shared_object_congestion_tracker,
                &tx.shared_input_objects(),
                tx_duration,
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

        let tx_duration = shared_object_congestion_tracker
            .congestion_control_parameters
            .get_estimated_execution_duration(&tx);
        assert!(
            initialize_tracker_and_compute_tx_start_time(
                &mut shared_object_congestion_tracker,
                &tx.shared_input_objects(),
                tx_duration,
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
        let shared_obj_0 = ObjectId::random();
        let shared_obj_1 = ObjectId::random();

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
                    Vec::new(),
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
                    Vec::new(),
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
        let shared_obj_0 = ObjectId::random();
        let shared_obj_1 = ObjectId::random();

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
            Vec::new(),
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

        // Verify that accumulated_object_debts is empty initially.
        let accumulated_object_debts = shared_object_congestion_tracker
            .clone()
            .accumulated_object_debts(max_execution_duration_per_commit);
        assert!(accumulated_object_debts.is_empty());

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

        // Verify that accumulated_object_debts reports the debt for object 0.
        let accumulated_object_debts = shared_object_congestion_tracker
            .accumulated_object_debts(max_execution_duration_per_commit);
        assert_eq!(accumulated_object_debts.len(), 1);
        match mode {
            PerObjectCongestionControlMode::None => unreachable!(),
            PerObjectCongestionControlMode::TotalGasBudget => {
                assert_eq!(accumulated_object_debts[0], (shared_obj_0, 80)); // overshoot = initial_debt (70) + tx_duration (100) - max_execution_duration_per_commit (90) = 80
            }
            PerObjectCongestionControlMode::TotalTxCount => {
                assert_eq!(accumulated_object_debts[0], (shared_obj_0, 1)); // overshoot = initial_debt (2) + tx_duration (1) - max_execution_duration_per_commit (2) = 1
            }
        }
    }

    #[test]
    fn test_worker_slots_occupy_and_free_slots() {
        let mut worker_slots = WorkerSlots::new();

        worker_slots.occupy(0, 10); // [0, 10) -> count 1
        // With a cap of 2 workers, worker count 1 is below the cap everywhere.
        assert_eq!(
            worker_slots.slots_with_worker_available(2).0,
            vec![ExecutionSlot::new(0, MAX_EXECUTION_TIME)]
        );
        // With a cap of 1 worker, [0, 10) is saturated.
        assert_eq!(
            worker_slots.slots_with_worker_available(1).0,
            vec![ExecutionSlot::new(10, MAX_EXECUTION_TIME)]
        );

        worker_slots.occupy(0, 10); // [0, 10) -> count 2
        assert_eq!(
            worker_slots.slots_with_worker_available(2).0,
            vec![ExecutionSlot::new(10, MAX_EXECUTION_TIME)]
        );

        // Overlapping worker counts: [0, 5) -> 2, [5, 10) -> 3, [10, 15) -> 1.
        worker_slots.occupy(5, 10);
        assert_eq!(
            worker_slots.slots_with_worker_available(3).0,
            vec![
                ExecutionSlot::new(0, 5),
                ExecutionSlot::new(10, MAX_EXECUTION_TIME),
            ]
        );
    }

    #[test]
    fn test_worker_slots_gap_fill_and_coalesce() {
        let mut worker_slots = WorkerSlots::new();
        // Two busy regions separated by a gap [5, 10).
        worker_slots.occupy(0, 5); // [0, 5) -> 1
        worker_slots.occupy(10, 5); // [10, 15) -> 1

        // Occupy across the gap: the gap is filled at count 1 and the existing
        // regions rise to 2 -> [0, 5):2, [5, 10):1, [10, 15):2.
        worker_slots.occupy(0, 15);
        assert_eq!(
            worker_slots.slots_with_worker_available(2).0,
            vec![
                ExecutionSlot::new(5, 10),
                ExecutionSlot::new(15, MAX_EXECUTION_TIME),
            ]
        );

        // Raising the middle to 2 makes all of [0, 15) count 2, which must
        // coalesce into a single slot.
        worker_slots.occupy(5, 5); // [5, 10) -> 2
        assert_eq!(
            worker_slots.0,
            vec![WorkerSlot {
                start_time: 0,
                end_time: 15,
                worker_count: 2,
            }]
        );
        assert_eq!(
            worker_slots.slots_with_worker_available(2).0,
            vec![ExecutionSlot::new(15, MAX_EXECUTION_TIME)]
        );
    }

    #[test]
    fn test_worker_slots_debt_and_decay() {
        // Worker counts: [0, 5) -> 1, [5, 15) -> 2.
        let mut worker_slots = WorkerSlots::new();
        worker_slots.occupy(0, 5);
        worker_slots.occupy(5, 10);
        worker_slots.occupy(5, 10);

        // Only the part beyond the per-commit limit (10) carries over, shifted to start
        // at 0: [10, 15) -> 2 becomes [0, 5) -> 2.
        let debt = worker_slots.overshoot(10);
        assert_eq!(debt, vec![(0, 5, 2)]);

        // Aging shifts left and drops anything that reaches time 0.
        assert_eq!(WorkerSlots::decay(debt.clone(), 2), vec![(0, 3, 2)]);
        assert_eq!(WorkerSlots::decay(debt.clone(), 5), vec![]);
        assert_eq!(WorkerSlots::decay(debt, 6), vec![]);

        // `decayed` ages by the fully-elapsed commits' budget:
        // num_rounds = current - stored - 1, shift = num_rounds * limit.
        let stored = CongestionWorkerDebt::new(3, vec![(0, 5, 2)]);
        assert_eq!(stored.clone().decayed(4, 2), vec![(0, 5, 2)]); // 0 elapsed
        assert_eq!(stored.clone().decayed(5, 2), vec![(0, 3, 2)]); // 1 elapsed
        assert_eq!(stored.decayed(7, 2), vec![]); // 3 elapsed -> expired
    }

    #[test]
    fn test_worker_slots_rollover_seeds_next_commit() {
        let mut congestion_control_parameters = CongestionControlParameters::new_for_test(
            PerObjectCongestionControlMode::TotalTxCount,
            true,
            Some(1), // max_execution_duration_per_commit (the congestion limit)
            None,
            TEST_ONLY_GAS_PRICE,
            false,
            false,
        );
        congestion_control_parameters.set_max_concurrent_execution_workers_for_test(1);

        // Seed the tracker as if a prior commit left the single worker busy over
        // [0, 1) — the carried-over debt.
        let tracker = SharedObjectCongestionTracker::new(
            Vec::new(),
            vec![(0, 1, 1)],
            congestion_control_parameters,
        );
        let previously_deferred = PreviouslyDeferredTransactions::new();

        // An owned-object-only transaction cannot fit within the per-commit
        // limit of 1 because the carried-over worker debt already fills the single
        // worker on [0, 1); it is shed for worker congestion.
        let tx = build_transaction(&[], 0, TEST_ONLY_GAS_PRICE);
        match tracker.try_schedule(&tx, &previously_deferred, 0) {
            SequencingResult::Defer(_, congested_objects) => {
                assert!(congested_objects.is_empty());
            }
            SequencingResult::Schedule(_) => {
                panic!("expected the tx to be shed due to carried-over worker debt")
            }
        }
    }

    #[test]
    fn test_execution_worker_congestion_schedules_then_sheds_ooo() {
        let mut congestion_control_parameters = CongestionControlParameters::new_for_test(
            PerObjectCongestionControlMode::TotalTxCount,
            true,    // congestion_control_min_free_execution_slot
            Some(1), // max_execution_duration_per_commit (also the congestion limit)
            None,    // max_congestion_limit_overshoot_per_commit
            TEST_ONLY_GAS_PRICE,
            false,
            false,
        );
        // A single execution worker, which serializes execution.
        congestion_control_parameters.set_max_concurrent_execution_workers_for_test(1);
        let mut tracker = SharedObjectCongestionTracker::new(
            Vec::new(),
            Vec::new(),
            congestion_control_parameters,
        );
        let previously_deferred = PreviouslyDeferredTransactions::new();

        // First owned-object-only transaction: scheduled at time 0.
        let tx0 = build_transaction(&[], 0, TEST_ONLY_GAS_PRICE);
        assert!(matches!(
            tracker.try_schedule(&tx0, &previously_deferred, 0),
            SequencingResult::Schedule(0)
        ));
        tracker.bump_object_execution_slots(&tx0, 0);

        // Second owned-object-only transaction: the single worker is occupied
        // on [0, 1), so it cannot fit within the per-commit limit and is shed
        // for execution-worker congestion (no specific congested object).
        let tx1 = build_transaction(&[], 0, TEST_ONLY_GAS_PRICE);
        match tracker.try_schedule(&tx1, &previously_deferred, 0) {
            SequencingResult::Defer(_, congested_objects) => {
                assert!(
                    congested_objects.is_empty(),
                    "worker congestion should not report a congested object"
                );
            }
            SequencingResult::Schedule(_) => {
                panic!("expected the second owned-object-only tx to be shed")
            }
        }
    }

    // Owned-object-only transactions occupy execution workers without
    // occupying any object slot, so the maximum occupied slot end time has to
    // come from the worker pool rather than being reported as zero.
    #[test]
    fn test_max_occupied_slot_end_time_covers_worker_slots() {
        let mut congestion_control_parameters = CongestionControlParameters::new_for_test(
            PerObjectCongestionControlMode::TotalTxCount,
            true,    // congestion_control_min_free_execution_slot
            Some(3), // max_execution_duration_per_commit (also the congestion limit)
            None,    // max_congestion_limit_overshoot_per_commit
            TEST_ONLY_GAS_PRICE,
            false,
            false,
        );
        // A single execution worker, which serializes execution.
        congestion_control_parameters.set_max_concurrent_execution_workers_for_test(1);
        let mut tracker = SharedObjectCongestionTracker::new(
            Vec::new(),
            Vec::new(),
            congestion_control_parameters,
        );
        let previously_deferred = PreviouslyDeferredTransactions::new();

        assert_eq!(tracker.max_occupied_slot_end_time(), 0);

        // Three owned-object-only transactions run back-to-back on the single
        // worker, so each starts when the previous one ends.
        for expected_start_time in 0..3 {
            let tx = build_transaction(&[], 0, TEST_ONLY_GAS_PRICE);
            let SequencingResult::Schedule(start_time) =
                tracker.try_schedule(&tx, &previously_deferred, 0)
            else {
                panic!("owned-object-only tx {expected_start_time} should be scheduled");
            };
            assert_eq!(start_time, expected_start_time);
            tracker.bump_object_execution_slots(&tx, start_time);
            assert_eq!(
                tracker.max_occupied_slot_end_time(),
                expected_start_time + 1
            );
        }
    }

    // A commit mixing shared-object and owned-object-only transactions with a
    // single execution worker: the worker constraint binds both kinds, and a
    // shared-object transaction shed purely by the worker pool reports no
    // congested objects.
    #[test]
    fn test_execution_worker_congestion_mixed_commit() {
        let mut congestion_control_parameters = CongestionControlParameters::new_for_test(
            PerObjectCongestionControlMode::TotalTxCount,
            true,    // congestion_control_min_free_execution_slot
            Some(2), // max_execution_duration_per_commit (two tx-count slots)
            None,    // max_congestion_limit_overshoot_per_commit
            TEST_ONLY_GAS_PRICE,
            false,
            false,
        );
        congestion_control_parameters.set_max_concurrent_execution_workers_for_test(1);
        let mut tracker = SharedObjectCongestionTracker::new(
            Vec::new(),
            Vec::new(),
            congestion_control_parameters,
        );
        let previously_deferred = PreviouslyDeferredTransactions::new();
        let shared_obj_a = ObjectId::random();
        let shared_obj_b = ObjectId::random();

        // Shared-object tx on A: worker and object both free, scheduled at 0.
        let tx0 = build_transaction(&[(shared_obj_a, true)], 0, TEST_ONLY_GAS_PRICE);
        tracker.initialize_object_execution_slots(&tx0.shared_input_objects());
        assert!(matches!(
            tracker.try_schedule(&tx0, &previously_deferred, 0),
            SequencingResult::Schedule(0)
        ));
        tracker.bump_object_execution_slots(&tx0, 0);

        // Owned-object-only tx: the worker is busy on [0, 1), so it is pushed
        // to start time 1, which still fits the per-commit limit of 2.
        let tx1 = build_transaction(&[], 0, TEST_ONLY_GAS_PRICE);
        assert!(matches!(
            tracker.try_schedule(&tx1, &previously_deferred, 0),
            SequencingResult::Schedule(1)
        ));
        tracker.bump_object_execution_slots(&tx1, 1);

        // Another owned-object-only tx: the worker is now busy on [0, 2), the
        // whole per-commit limit, so it is shed for worker congestion.
        let tx2 = build_transaction(&[], 0, TEST_ONLY_GAS_PRICE);
        match tracker.try_schedule(&tx2, &previously_deferred, 0) {
            SequencingResult::Defer(_, congested_objects) => {
                assert!(congested_objects.is_empty());
            }
            SequencingResult::Schedule(_) => panic!("expected the owned-object-only tx to be shed"),
        }

        // Shared-object tx on the untouched object B: B's slots are completely
        // free, but no worker is available within the per-commit limit — the
        // worker pool is the bottleneck, so no congested object is reported.
        let tx3 = build_transaction(&[(shared_obj_b, true)], 0, TEST_ONLY_GAS_PRICE);
        tracker.initialize_object_execution_slots(&tx3.shared_input_objects());
        match tracker.try_schedule(&tx3, &previously_deferred, 0) {
            SequencingResult::Defer(_, congested_objects) => {
                assert!(
                    congested_objects.is_empty(),
                    "worker-bound shed of a shared-object tx should not report \
                    congested objects, got {congested_objects:?}"
                );
            }
            SequencingResult::Schedule(_) => panic!("expected the shared-object tx to be shed"),
        }
    }

    // The execution-worker profile filling across a commit: with two workers
    // each transaction occupies one worker for its duration, start times
    // advance once both workers are busy, and a transaction that no longer
    // fits within the per-commit limit is deferred.
    #[rstest]
    fn test_worker_slot_filling_across_commit(
        #[values(true, false)] assign_min_free_execution_slot: bool,
    ) {
        let mut congestion_control_parameters = CongestionControlParameters::new_for_test(
            PerObjectCongestionControlMode::TotalTxCount,
            assign_min_free_execution_slot,
            Some(3), // max_execution_duration_per_commit
            None,    // max_congestion_limit_overshoot_per_commit
            TEST_ONLY_GAS_PRICE,
            false,
            false,
        );
        congestion_control_parameters.set_max_concurrent_execution_workers_for_test(2);
        let mut tracker = SharedObjectCongestionTracker::new(
            Vec::new(),
            Vec::new(),
            congestion_control_parameters,
        );
        let previously_deferred = PreviouslyDeferredTransactions::new();

        // Six owned-object-only transactions fill the profile two at a time:
        //     worker count
        // 0 | 1 2
        // 1 | 1 2
        // 2 | 1 2
        // 3 |______ max_execution_duration_per_commit = 3
        for (i, expected_start_time) in [0, 0, 1, 1, 2, 2].into_iter().enumerate() {
            let tx = build_transaction(&[], 0, TEST_ONLY_GAS_PRICE);
            match tracker.try_schedule(&tx, &previously_deferred, 0) {
                SequencingResult::Schedule(start_time) => {
                    assert_eq!(start_time, expected_start_time, "transaction {i}");
                }
                SequencingResult::Defer(..) => panic!("transaction {i} should be scheduled"),
            }
            tracker.bump_object_execution_slots(&tx, expected_start_time);
        }
        assert_eq!(
            tracker.worker_slots.as_ref().unwrap().0,
            vec![WorkerSlot {
                start_time: 0,
                end_time: 3,
                worker_count: 2,
            }]
        );

        // The profile is saturated up to the limit: the seventh is deferred,
        // with no congested object to report.
        let tx = build_transaction(&[], 0, TEST_ONLY_GAS_PRICE);
        match tracker.try_schedule(&tx, &previously_deferred, 0) {
            SequencingResult::Defer(_, congested_objects) => {
                assert!(congested_objects.is_empty());
            }
            SequencingResult::Schedule(_) => panic!("the seventh transaction should be deferred"),
        }
    }

    // A worker-shed transaction gets the same deferral-key bookkeeping as an
    // object-shed one: a fresh deferral starts from the current commit round,
    // and a previously-deferred transaction keeps its original round.
    #[test]
    fn test_try_schedule_worker_deferral_key() {
        let mut congestion_control_parameters = CongestionControlParameters::new_for_test(
            PerObjectCongestionControlMode::TotalTxCount,
            true,
            Some(1), // max_execution_duration_per_commit
            None,
            TEST_ONLY_GAS_PRICE,
            false,
            false,
        );
        congestion_control_parameters.set_max_concurrent_execution_workers_for_test(1);
        let mut tracker = SharedObjectCongestionTracker::new(
            Vec::new(),
            Vec::new(),
            congestion_control_parameters,
        );
        let mut previously_deferred = PreviouslyDeferredTransactions::new();

        // Fill the single worker for the whole commit.
        let scheduled = build_transaction(&[], 0, TEST_ONLY_GAS_PRICE);
        tracker.bump_object_execution_slots(&scheduled, 0);

        let tx = build_transaction(&[], 0, TEST_ONLY_GAS_PRICE);
        match tracker.try_schedule(&tx, &previously_deferred, 10) {
            SequencingResult::Defer(
                DeferralKey::ConsensusRound {
                    future_round,
                    deferred_from_round,
                },
                _,
            ) => {
                assert_eq!(future_round, 11);
                assert_eq!(deferred_from_round, 10);
            }
            _ => panic!("should defer with a consensus-round key"),
        }

        // A transaction deferred in an earlier round keeps its original
        // deferred_from_round.
        previously_deferred.insert(
            *tx.digest(),
            (
                DeferralKey::ConsensusRound {
                    future_round: 10,
                    deferred_from_round: 5,
                },
                Some(1_000),
            ),
        );
        match tracker.try_schedule(&tx, &previously_deferred, 10) {
            SequencingResult::Defer(
                DeferralKey::ConsensusRound {
                    future_round,
                    deferred_from_round,
                },
                _,
            ) => {
                assert_eq!(future_round, 11);
                assert_eq!(deferred_from_round, 5);
            }
            _ => panic!("should defer with a consensus-round key"),
        }
    }

    // The congested-objects report distinguishes the congestion cause: a
    // worker-bound shed reports no objects, an object-bound shed reports the
    // congested objects, and a shed bound by both reports the objects.
    #[rstest]
    fn test_try_schedule_congested_objects_under_worker_congestion(
        #[values(true, false)] assign_min_free_execution_slot: bool,
    ) {
        let congested_object = ObjectId::random();
        let free_object = ObjectId::random();

        let build_parameters = |workers: u16| {
            let mut parameters = CongestionControlParameters::new_for_test(
                PerObjectCongestionControlMode::TotalTxCount,
                assign_min_free_execution_slot,
                Some(1), // max_execution_duration_per_commit
                None,
                TEST_ONLY_GAS_PRICE,
                false,
                false,
            );
            parameters.set_max_concurrent_execution_workers_for_test(workers);
            parameters
        };
        let previously_deferred = PreviouslyDeferredTransactions::new();

        // One worker, fully occupied; `congested_object` occupied for the
        // whole commit by an initial debt; `free_object` untouched.
        let mut tracker = new_congestion_tracker_with_initial_value_for_test(
            &[(congested_object, 1)],
            build_parameters(1),
        );
        let scheduled = build_transaction(&[], 0, TEST_ONLY_GAS_PRICE);
        tracker.bump_object_execution_slots(&scheduled, 0);

        // Worker-bound only: the object is free, so no object is congested.
        let tx = build_transaction(&[(free_object, true)], 0, TEST_ONLY_GAS_PRICE);
        tracker.initialize_object_execution_slots(&tx.shared_input_objects());
        match tracker.try_schedule(&tx, &previously_deferred, 0) {
            SequencingResult::Defer(_, congested_objects) => {
                assert!(
                    congested_objects.is_empty(),
                    "worker-bound shed should not report objects, got {congested_objects:?}"
                );
            }
            SequencingResult::Schedule(_) => panic!("should defer"),
        }

        // Bound by both: the congested object is reported.
        let tx = build_transaction(&[(congested_object, true)], 0, TEST_ONLY_GAS_PRICE);
        tracker.initialize_object_execution_slots(&tx.shared_input_objects());
        match tracker.try_schedule(&tx, &previously_deferred, 0) {
            SequencingResult::Defer(_, congested_objects) => {
                assert_eq!(congested_objects, vec![congested_object]);
            }
            SequencingResult::Schedule(_) => panic!("should defer"),
        }

        // Object-bound only (two workers, one free): still the object.
        let mut tracker = new_congestion_tracker_with_initial_value_for_test(
            &[(congested_object, 1)],
            build_parameters(2),
        );
        let scheduled = build_transaction(&[], 0, TEST_ONLY_GAS_PRICE);
        tracker.bump_object_execution_slots(&scheduled, 0);
        let tx = build_transaction(&[(congested_object, true)], 0, TEST_ONLY_GAS_PRICE);
        tracker.initialize_object_execution_slots(&tx.shared_input_objects());
        match tracker.try_schedule(&tx, &previously_deferred, 0) {
            SequencingResult::Defer(_, congested_objects) => {
                assert_eq!(congested_objects, vec![congested_object]);
            }
            SequencingResult::Schedule(_) => panic!("should defer"),
        }
    }
}
