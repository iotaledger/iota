// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use iota_protocol_config::PerObjectCongestionControlMode;
use iota_types::{
    base_types::{CommitRound, ObjectID, TransactionDigest},
    executable_transaction::VerifiedExecutableTransaction,
    transaction::SharedInputObject,
};

use crate::authority::transaction_deferral::DeferralKey;
pub enum SequencingResult {
    Schedule(u64),
    Defer(DeferralKey, Vec<ObjectID>),
}

// An execution slot represents the allocated time slot for a transaction to be
// executed. We can only estimate the time to execute a transaction, so
// we represent this expected time with a "cost". Execution slots of
// non-concurrent transactions cannot overlap. Transactions can occupy
// overlapping execution slots if they do not touch any common shared objects.
#[derive(PartialEq, Eq, Clone, Debug, Copy)]
pub struct ExecutionSlot {
    start_cost: u64,
    end_cost: u64,
    scheduled: bool,
}

impl ExecutionSlot {
    fn new(start_cost: u64, end_cost: u64, scheduled: bool) -> Self {
        Self {
            start_cost,
            end_cost,
            scheduled,
        }
    }

    pub fn height(&self) -> u64 {
        if self.end_cost > self.start_cost {
            self.end_cost - self.start_cost
        } else {
            0
        }
    }

    pub fn longest_overlap(&self, other: &ExecutionSlot) -> ExecutionSlot {
        ExecutionSlot {
            start_cost: self.start_cost.max(other.start_cost),
            end_cost: self.end_cost.min(other.end_cost),
            scheduled: false,
        }
    }

    pub fn overlaps(&self, other: &ExecutionSlot) -> bool {
        self.start_cost < other.end_cost && self.end_cost > other.start_cost
    }
}

// SharedObjectCongestionTracker stores the available and occupied execution
// slots for the transactions within a consensus commit.
//
// Cost is an indication of transaction execution latency. When transactions are
// scheduled by the consensus handler, each scheduled transaction takes up an
// execution slot with a certain cost.
//
// The goal of this data structure is to capture the critical path of
// transaction execution latency on each objects.
//
// The mode field determines how the cost is calculated. The cost can be
// calculated based on the total gas budget, or total number of transaction
// count.
//
// The min_free_execution_slot field determines how the start cost of a
// transaction should be assigned. If true, the tracker will assign the start
// cost according to the minimum free execution slot for a transaction over all
// its shared objects. If false, the tracker will assign the start cost
// according to the maximum end cost of of the occupied execution slots for a
// transaction over all its shared objects.
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct SharedObjectCongestionTracker {
    object_execution_slots: HashMap<ObjectID, Vec<ExecutionSlot>>,
    mode: PerObjectCongestionControlMode,
    min_free_execution_slot: bool,
}

impl SharedObjectCongestionTracker {
    pub fn new(mode: PerObjectCongestionControlMode, min_free_execution_slot: bool) -> Self {
        Self {
            object_execution_slots: HashMap::new(),
            mode,
            min_free_execution_slot: min_free_execution_slot,
        }
    }

    // Given a list of shared input objects and the cost of a transaction that
    // operates on these objects, returns the starting cost of the transaction
    //
    // Starting cost is a proxy for the starting time of the transaction. It is
    // determined by all the input shared objects' last write.
    pub fn compute_tx_start_cost(
        &mut self,
        shared_input_objects: &[SharedInputObject],
        tx_cost: u64,
    ) -> u64 {
        // initialise the free execution slots for the objects that are not in the
        // tracker.
        for obj in shared_input_objects {
            self.object_execution_slots
                .entry(obj.id)
                .or_insert(vec![ExecutionSlot::new(0, u64::MAX, false)]);
        }
        if self.min_free_execution_slot {
            // If min_free_execution_slot is true, we assign the transaction start cost
            // based on the lowest free execution slot that can accommodates the
            // transaction. We start the search from the full range of the slots
            // available with no constraints from previous objects.
            let initial_free_slot = ExecutionSlot::new(0, u64::MAX, false);
            self.compute_min_free_execution_slot(&shared_input_objects, tx_cost, initial_free_slot)
                .unwrap_or(u64::MAX)
        } else {
            // If min_free_execution_slot is false, we assign the transaction start cost
            // based on the maximum end cost of the occupied execution slots for the
            // transaction over all its shared objects.
            shared_input_objects
                .iter()
                .map(|obj| {
                    self.object_execution_slots
                        .get(&obj.id)
                        .expect("object should have been inserted at the start of this function.")
                })
                .map(|slots| max_object_free_slot_start_cost(slots))
                .max()
                .expect("There must be at least one object in shared_input_objects.")
        }
    }

    // A recursive function that tries to find the lowest free slot for a
    // transaction. If a slot is found that fits the transaction, the function
    // returns the slot. Otherwise, it returns None.
    // lookup_interval is the range of the slot that the transaction can fit in
    // given the objects that have been checked so far.
    fn compute_min_free_execution_slot(
        &self,
        shared_input_objects: &[SharedInputObject],
        tx_cost: u64,
        lookup_interval: ExecutionSlot,
    ) -> Option<u64> {
        // take the first object from the shared input objects.
        let obj = shared_input_objects
            .first()
            .expect("shared_input_objects must not be empty.");
        // set aside the remaining objects for the next recursive call.
        let remaining_objects = if shared_input_objects.len() > 1 {
            &shared_input_objects[1..]
        } else {
            &[]
        };

        for free_slot in self.object_execution_slots.get(&obj.id).unwrap() {
            // only consider slots with no transaction assigned yet.
            if free_slot.scheduled {
                continue;
            }
            let longest_overlap = free_slot.longest_overlap(&lookup_interval);
            // If there is no overlap that can fit the transaction, continue to the next
            // free slot.
            if longest_overlap.height() < tx_cost {
                continue;
            }
            // if this is the last object to check, return this slot as it is the lowest
            // slot available.
            if remaining_objects.is_empty() {
                return Some(longest_overlap.start_cost);
            }
            // if there are more objects to check, recursively call the function with the
            // remaining objects.
            // If the recursive call returns a start cost, that means the transaction fits
            // in the slot for all remaining objects. Return the start cost.
            // Otherwise, continue to check the next free slot for the current object.
            if let Some(lowest_overlap) =
                self.compute_min_free_execution_slot(remaining_objects, tx_cost, longest_overlap)
            {
                return Some(lowest_overlap);
            } else {
                continue;
            }
        }
        // if no slot is found for the current object given the available range, return
        // None.
        None
    }

    pub fn get_tx_cost(&self, cert: &VerifiedExecutableTransaction) -> u64 {
        match self.mode {
            PerObjectCongestionControlMode::None => 0,
            PerObjectCongestionControlMode::TotalGasBudget => cert.gas_budget(),
            PerObjectCongestionControlMode::TotalTxCount => 1,
        }
    }

    // Given a transaction, returns a sequencing result. If the transactions can be
    // scheduled, this returns a start_cost, and if it should be deferred, this
    // returns the deferral key and the congested objects responsible for the
    // deferral.
    pub fn try_schedule(
        &mut self,
        cert: &VerifiedExecutableTransaction,
        max_accumulated_txn_cost_per_object_in_commit: u64,
        previously_deferred_tx_digests: &HashMap<TransactionDigest, DeferralKey>,
        commit_round: CommitRound,
    ) -> SequencingResult {
        let tx_cost = self.get_tx_cost(cert);
        if tx_cost == 0 {
            // This is a zero-cost transaction, no need to defer.
            return SequencingResult::Schedule(0);
        }

        let shared_input_objects: Vec<_> = cert.shared_input_objects().collect();
        if shared_input_objects.is_empty() {
            // This is an owned object only transaction. No need to defer.
            return SequencingResult::Schedule(0);
        }
        let start_cost = self.compute_tx_start_cost(&shared_input_objects, tx_cost);

        let (end_cost, cost_overflow) = start_cost.overflowing_add(tx_cost);
        if !cost_overflow && end_cost <= max_accumulated_txn_cost_per_object_in_commit {
            // schedule this transaction and return the start cost.
            return SequencingResult::Schedule(start_cost);
        }

        let mut congested_objects = vec![];
        for obj in shared_input_objects {
            let execution_slots = self
                .object_execution_slots
                .get(&obj.id)
                .expect("Execution slot vector should have been inserted when computing start cost or before.");
            if self.min_free_execution_slot {
                // If we are using min_free_execution_slot, we define an object as congested if
                // the lowest free slot for that object is the same as the start cost of the
                // entire transaction. This means that this shared object was the bottleneck for
                // the transaction.
                let obj_id = obj.id;
                if self
                    .compute_min_free_execution_slot(
                        &[obj],
                        tx_cost,
                        ExecutionSlot::new(0, max_accumulated_txn_cost_per_object_in_commit, false),
                    )
                    .is_none()
                {
                    congested_objects.push(obj_id);
                }
            } else {
                // If we are not using min_free_execution_slot, we define an object as congested
                // if the maximum free slot start cost is the same as the start
                // cost of the entire transaction. This means that this shared
                // object was the bottleneck for the transaction in this case.
                if start_cost == max_object_free_slot_start_cost(execution_slots) {
                    congested_objects.push(obj.id);
                }
            };
        }

        let deferral_key =
            if let Some(previous_key) = previously_deferred_tx_digests.get(cert.digest()) {
                // This transaction has been deferred in previous consensus commit. Use its
                // previous deferred_from_round.
                DeferralKey::new_for_consensus_round(
                    commit_round + 1,
                    previous_key.deferred_from_round(),
                )
            } else {
                // This transaction has not been deferred before. Use the current commit round
                // as the deferred_from_round.
                DeferralKey::new_for_consensus_round(commit_round + 1, commit_round)
            };
        SequencingResult::Defer(deferral_key, congested_objects)
    }

    // Update shared objects' execution slots used in `cert` using `cert`'s
    // execution cost. This is called when `cert` is scheduled for execution.
    //
    // `start_cost` provides the start cost of the execution slot assigned to
    // `cert`.
    pub fn bump_object_execution_slots(
        &mut self,
        cert: &VerifiedExecutableTransaction,
        start_cost: u64,
    ) {
        let tx_cost = self.get_tx_cost(cert);
        if tx_cost == 0 {
            return;
        }
        let shared_input_objects: Vec<_> = cert.shared_input_objects().collect();
        let end_cost = start_cost.saturating_add(tx_cost);
        let occupied_slot = ExecutionSlot::new(start_cost, end_cost, true);
        for obj in shared_input_objects {
            if obj.mutable {
                let mut old_slot_index: Option<usize> = None;
                let mut new_slots = Vec::new();
                // iterate through the free slots of the object to find the slot that
                // overlaps with the transaction slot.
                for (index, free_slot) in self
                    .object_execution_slots
                    .get(&obj.id)
                    .unwrap_or(&mut vec![])
                    .iter()
                    .enumerate()
                {
                    // if the occupied slot overlaps with the free slot, we split the free slot.
                    // There are 4 cases to consider.
                    // case A: a free slot remains at the start.
                    // (occupied_slot.start_cost > free_slot.start_cost && occupied_slot.end_cost ==
                    // free_slot.end_cost)
                    //      | free_slot                 |
                    //   => | free_slot | occupied_slot |
                    // case B: a free slot remains at the end.
                    // (occupied_slot.start_cost == free_slot.start_cost && occupied_slot.end_cost <
                    // free_slot.end_cost)
                    //      | free_slot                 |
                    //   => | occupied_slot | free_slot |
                    // case AB: a free slot remains at the start and the end.
                    // (occupied_slot.start_cost > free_slot.start_cost && occupied_slot.end_cost <
                    // free_slot.end_cost)
                    //      | free_slot                             |
                    //   => | free_slot | occupied_slot | free_slot |
                    // case 0: the occupied slot perfectly overlaps with the free slot.
                    // (occupied_slot.start_cost == free_slot.start_cost && occupied_slot.end_cost
                    // == free_slot.end_cost)
                    //      | free_slot     |
                    //   => | occupied_slot |
                    if occupied_slot.overlaps(free_slot) {
                        // The occupied slot must be within the free slot or the assigned slot is
                        // not correct.
                        assert!(
                            occupied_slot.start_cost >= free_slot.start_cost
                                && occupied_slot.end_cost <= free_slot.end_cost
                        );
                        // store the index of the old slot to remove it later.
                        old_slot_index = Some(index);
                        // case A: if a part of the free slot remains at the start, create a new
                        // free slot.
                        if occupied_slot.start_cost > free_slot.start_cost {
                            new_slots.push(ExecutionSlot::new(
                                free_slot.start_cost,
                                occupied_slot.start_cost,
                                false,
                            ));
                        }
                        // case B: if a part of the free slot remains at the end, create a new free
                        // slot.
                        if occupied_slot.end_cost < free_slot.end_cost {
                            new_slots.push(ExecutionSlot::new(
                                occupied_slot.end_cost,
                                free_slot.end_cost,
                                false,
                            ));
                        }
                        break;
                    }
                }
                // remove the old slot and add the new slots.
                let slots = self.object_execution_slots.get_mut(&obj.id).unwrap();
                if old_slot_index.is_some() {
                    slots.remove(old_slot_index.unwrap());
                }
                slots.push(occupied_slot);
                slots.extend(new_slots);
                slots.sort_by(|a, b| a.start_cost.cmp(&b.start_cost));
            }
        }
    }

    // Returns the maximum occupied slot end cost over all shared objects.
    pub fn max_occupied_slot_end_cost(&self) -> u64 {
        self.object_execution_slots
            .values()
            .map(|slots| max_object_free_slot_start_cost(slots))
            .max()
            .unwrap_or(0)
    }
}

fn max_object_free_slot_start_cost(slots: &Vec<ExecutionSlot>) -> u64 {
    if slots.is_empty() {
        return 0;
    }
    let last_free_slot = slots.last().unwrap();
    if last_free_slot.scheduled {
        u64::MAX
    } else {
        last_free_slot.start_cost
    }
}

#[cfg(test)]
pub mod shared_object_test_utils {
    use iota_protocol_config::PerObjectCongestionControlMode;
    use iota_test_transaction_builder::TestTransactionBuilder;
    use iota_types::{
        base_types::{ObjectID, SequenceNumber, random_object_ref},
        crypto::{AccountKeyPair, get_key_pair},
        executable_transaction::VerifiedExecutableTransaction,
        transaction::{CallArg, ObjectArg, VerifiedTransaction},
    };

    use super::*;

    // Builds a certificate with a list of shared objects and their mutability. The
    // certificate is only used to test the SharedObjectCongestionTracker
    // functions, therefore the content other than shared inputs and gas budget
    // are not important.
    pub fn build_transaction(
        objects: &[(ObjectID, bool)],
        gas_budget: u64,
    ) -> VerifiedExecutableTransaction {
        let (sender, keypair): (_, AccountKeyPair) = get_key_pair();
        let gas_object = random_object_ref();
        VerifiedExecutableTransaction::new_system(
            VerifiedTransaction::new_unchecked(
                TestTransactionBuilder::new(sender, gas_object, 1000)
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

    pub fn new_congestion_tracker_with_initial_value_for_test(
        init_values: &[(ObjectID, u64)],
        mode: PerObjectCongestionControlMode,
        min_free_execution_slot: bool,
    ) -> SharedObjectCongestionTracker {
        let mut shared_object_congestion_tracker =
            SharedObjectCongestionTracker::new(mode, min_free_execution_slot);
        // add initial values for each transaction
        for (object_id, cost) in init_values {
            match mode {
                PerObjectCongestionControlMode::None => {}
                PerObjectCongestionControlMode::TotalGasBudget => {
                    let transaction = build_transaction(&[(*object_id, true)], *cost);
                    let shared_input_objects: Vec<_> = transaction.shared_input_objects().collect();
                    let start_cost = shared_object_congestion_tracker
                        .compute_tx_start_cost(&shared_input_objects, *cost);
                    shared_object_congestion_tracker
                        .bump_object_execution_slots(&transaction, start_cost);
                }
                PerObjectCongestionControlMode::TotalTxCount => {
                    for _ in 0..*cost {
                        let transaction = build_transaction(&[(*object_id, true)], 1);
                        let shared_input_objects: Vec<_> =
                            transaction.shared_input_objects().collect();
                        let start_cost = shared_object_congestion_tracker
                            .compute_tx_start_cost(&shared_input_objects, 1);
                        shared_object_congestion_tracker
                            .bump_object_execution_slots(&transaction, start_cost);
                    }
                }
            }
        }
        shared_object_congestion_tracker
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
    use rstest::rstest;

    use super::{shared_object_test_utils::*, *};

    #[rstest]
    fn test_compute_tx_start_at_cost(#[values(true, false)] min_free_execution_slot: bool) {
        let object_id_0 = ObjectID::random();
        let object_id_1 = ObjectID::random();
        let object_id_2 = ObjectID::random();
        let object_id_3 = ObjectID::random();

        // initialise a new shared object congestion tracker.
        let mut shared_object_congestion_tracker =
            new_congestion_tracker_with_initial_value_for_test(
                &[(object_id_0, 5), (object_id_1, 9)],
                PerObjectCongestionControlMode::TotalGasBudget,
                min_free_execution_slot,
            );

        // The tracker has the following object execution cost:
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

        // a transactiont that writes to objects 0, 1 and 2 should have start_cost 9.
        let objects = &[
            (object_id_0, true),
            (object_id_1, true),
            (object_id_2, true),
        ];
        let shared_input_objects = construct_shared_input_objects(objects);
        assert_eq!(
            shared_object_congestion_tracker.compute_tx_start_cost(&shared_input_objects, 10),
            9
        );
        // now add this transaction to the tracker.
        let tx = build_transaction(objects, 1);
        shared_object_congestion_tracker.bump_object_execution_slots(&tx, 9);

        // That tracker now has the following object execution cost:
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

        // a transaction with cost 4 that reads object 0 should have start_cost 5 with
        // min_free_execution_slot or 10 without min_free_execution_slot.
        let shared_input_objects = construct_shared_input_objects(&[(object_id_0, false)]);
        assert_eq!(
            shared_object_congestion_tracker.compute_tx_start_cost(&shared_input_objects, 4),
            if min_free_execution_slot { 5 } else { 10 }
        );
        // a transaction with cost 5 that reads object 0 should have start_cost 10 with
        // or without min_free_execution_slot.
        assert_eq!(
            shared_object_congestion_tracker.compute_tx_start_cost(&shared_input_objects, 5),
            10
        );

        // a transaction with cost 5 that writes object 1 should have start_cost 10 with
        // or without min_free_execution_slot.
        let shared_input_objects = construct_shared_input_objects(&[(object_id_1, true)]);
        assert_eq!(
            shared_object_congestion_tracker.compute_tx_start_cost(&shared_input_objects, 5),
            10
        );

        // a transaction with cost 5 that reads objects 0 and 1 should have start_cost
        // 10 with or without min_free_execution_slot.
        let shared_input_objects =
            construct_shared_input_objects(&[(object_id_0, false), (object_id_1, false)]);
        assert_eq!(
            shared_object_congestion_tracker.compute_tx_start_cost(&shared_input_objects, 5),
            10
        );

        // a transaction with cost 5 that writes objects 0 and 1 should have start_cost
        // 10 with or without min_free_execution_slot.
        let shared_input_objects =
            construct_shared_input_objects(&[(object_id_0, true), (object_id_1, true)]);
        assert_eq!(
            shared_object_congestion_tracker.compute_tx_start_cost(&shared_input_objects, 5),
            10
        );

        // a transaction with cost 5 that writes object 2 should have start_cost 0 with
        // min_free_execution_slot or 10 without min_free_execution_slot.
        let shared_input_objects = construct_shared_input_objects(&[(object_id_2, true)]);
        assert_eq!(
            shared_object_congestion_tracker.compute_tx_start_cost(&shared_input_objects, 5),
            if min_free_execution_slot { 0 } else { 10 }
        );

        // a transaction with cost 5 that writes to the previously untouched object 3
        // should have start_cost 0 with or without min_free_execution_slot.
        let shared_input_objects = construct_shared_input_objects(&[(object_id_3, true)]);
        assert_eq!(
            shared_object_congestion_tracker.compute_tx_start_cost(&shared_input_objects, 5),
            0
        );

        // a transaction with cost 3 that reads objects 0 and 2 should have start_cost
        // 5 with min_free_execution_slot or 10 without min_free_execution_slot.
        let shared_input_objects =
            construct_shared_input_objects(&[(object_id_0, false), (object_id_2, false)]);
        assert_eq!(
            shared_object_congestion_tracker.compute_tx_start_cost(&shared_input_objects, 3),
            if min_free_execution_slot { 5 } else { 10 }
        );
    }

    #[rstest]
    fn test_try_schedule_return_correct_congested_objects(
        #[values(
            PerObjectCongestionControlMode::TotalGasBudget,
            PerObjectCongestionControlMode::TotalTxCount
        )]
        mode: PerObjectCongestionControlMode,
        #[values(true, false)] min_free_execution_slot: bool,
    ) {
        // Creates two shared objects and three transactions that operate on these
        // objects.
        let shared_obj_0 = ObjectID::random();
        let shared_obj_1 = ObjectID::random();

        let tx_gas_budget = 5;

        // Set max_accumulated_txn_cost_per_object_in_commit to only allow 1 transaction
        // to go through.
        let max_accumulated_txn_cost_per_object_in_commit = match mode {
            PerObjectCongestionControlMode::None => unreachable!(),
            PerObjectCongestionControlMode::TotalGasBudget => 12,
            PerObjectCongestionControlMode::TotalTxCount => 3,
        };

        let mut shared_object_congestion_tracker = match mode {
            PerObjectCongestionControlMode::None => unreachable!(),
            PerObjectCongestionControlMode::TotalGasBudget => {
                // Construct object execution cost as follows
                //    object 0       object 1
                // 0| xxxxxxxx     | xxxxxxxx
                // 1| xxxxxxxx     |
                // ::::::::::::::::::::::::::
                // 8| xxxxxxxx     |
                // 9|              |
                new_congestion_tracker_with_initial_value_for_test(
                    &[(shared_obj_0, 9), (shared_obj_1, 1)],
                    mode,
                    min_free_execution_slot,
                )
            }
            PerObjectCongestionControlMode::TotalTxCount => {
                // Construct object execution cost as follows
                //    object 0       object 1
                // 0| xxxxxxxx     | xxxxxxxx
                // 1| xxxxxxxx     |
                // 2|              |
                new_congestion_tracker_with_initial_value_for_test(
                    &[(shared_obj_0, 2), (shared_obj_1, 1)],
                    mode,
                    min_free_execution_slot,
                )
            }
        };
        // add a transaction that writes to object 0 and 1.
        let tx = build_transaction(&[(shared_obj_0, true), (shared_obj_1, true)], 1);
        shared_object_congestion_tracker.bump_object_execution_slots(
            &tx,
            match mode {
                PerObjectCongestionControlMode::None => unreachable!(),
                // in gas budget mode, the object execution cost becomes:
                //    object 0       object 1
                // 0| xxxxxxxx     | xxxxxxxx
                // 1| xxxxxxxx     |
                // ::::::::::::::::::::::::::
                // 8| xxxxxxxx     |
                // 9| xxxxxxxx     | xxxxxxxx
                PerObjectCongestionControlMode::TotalGasBudget => 10,
                // in tx count mode, the object execution cost becomes:
                //    object 0       object 1
                // 0| xxxxxxxx     | xxxxxxxx
                // 1| xxxxxxxx     |
                // 2| xxxxxxxx     | xxxxxxxx
                PerObjectCongestionControlMode::TotalTxCount => 2,
            },
        );

        // Read/write to object 0 should be deferred.
        for mutable in [true, false].iter() {
            let tx = build_transaction(&[(shared_obj_0, *mutable)], tx_gas_budget);
            if let SequencingResult::Defer(_, congested_objects) = shared_object_congestion_tracker
                .try_schedule(
                    &tx,
                    max_accumulated_txn_cost_per_object_in_commit,
                    &HashMap::new(),
                    0,
                )
            {
                assert_eq!(congested_objects.len(), 1);
                assert_eq!(congested_objects[0], shared_obj_0);
            } else {
                panic!("should defer");
            }
        }

        // Read/write to object 1 should be scheduled with start_cost 1 with
        // min_free_execution_slot and deferred otherwise.
        for mutable in [true, false].iter() {
            let tx = build_transaction(&[(shared_obj_1, *mutable)], tx_gas_budget);
            let sequencing_result = shared_object_congestion_tracker.try_schedule(
                &tx,
                max_accumulated_txn_cost_per_object_in_commit,
                &HashMap::new(),
                0,
            );
            if min_free_execution_slot {
                matches!(sequencing_result, SequencingResult::Schedule(1));
            } else {
                if let SequencingResult::Defer(_, congested_objects) = sequencing_result {
                    assert_eq!(congested_objects.len(), 1);
                    assert_eq!(congested_objects[0], shared_obj_1);
                } else {
                    panic!("should defer");
                }
            }
        }

        // Transactions touching both objects should be deferred, with object 0 as the
        // congested object.
        for mutable_0 in [true, false].iter() {
            for mutable_1 in [true, false].iter() {
                let tx = build_transaction(
                    &[(shared_obj_0, *mutable_0), (shared_obj_1, *mutable_1)],
                    tx_gas_budget,
                );
                if let SequencingResult::Defer(_, congested_objects) =
                    shared_object_congestion_tracker.try_schedule(
                        &tx,
                        max_accumulated_txn_cost_per_object_in_commit,
                        &HashMap::new(),
                        0,
                    )
                {
                    // with min_free_execution_slot, only object 0 is congested.
                    // without min_free_execution_slot, both objects are congested.
                    assert_eq!(
                        congested_objects.len(),
                        if min_free_execution_slot { 1 } else { 2 }
                    );
                    assert_eq!(congested_objects[0], shared_obj_0);
                    if !min_free_execution_slot {
                        assert_eq!(congested_objects[1], shared_obj_1);
                    }
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
        let tx = build_transaction(&[(shared_obj_0, true)], 100);
        // Make try_schedule always defers transactions.
        let max_accumulated_txn_cost_per_object_in_commit = 0;
        let mut shared_object_congestion_tracker = SharedObjectCongestionTracker::new(mode, false);

        // Insert a random pre-existing transaction.
        let mut previously_deferred_tx_digests = HashMap::new();
        previously_deferred_tx_digests.insert(
            TransactionDigest::random(),
            DeferralKey::ConsensusRound {
                future_round: 10,
                deferred_from_round: 5,
            },
        );

        // Test deferral key for a transaction that has not been deferred before.
        if let SequencingResult::Defer(
            DeferralKey::ConsensusRound {
                future_round,
                deferred_from_round,
            },
            _,
        ) = shared_object_congestion_tracker.try_schedule(
            &tx,
            max_accumulated_txn_cost_per_object_in_commit,
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
            DeferralKey::Randomness {
                deferred_from_round: 4,
            },
        );

        // New deferral key should have deferred_from_round equal to the deferred
        // randomness round.
        if let SequencingResult::Defer(
            DeferralKey::ConsensusRound {
                future_round,
                deferred_from_round,
            },
            _,
        ) = shared_object_congestion_tracker.try_schedule(
            &tx,
            max_accumulated_txn_cost_per_object_in_commit,
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
            DeferralKey::ConsensusRound {
                future_round: 10,
                deferred_from_round: 5,
            },
        );

        // New deferral key should have deferred_from_round equal to the one in the old
        // deferral key.
        if let SequencingResult::Defer(
            DeferralKey::ConsensusRound {
                future_round,
                deferred_from_round,
            },
            _,
        ) = shared_object_congestion_tracker.try_schedule(
            &tx,
            max_accumulated_txn_cost_per_object_in_commit,
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
        #[values(true, false)] min_free_execution_slot: bool,
    ) {
        let object_id_0 = ObjectID::random();
        let object_id_1 = ObjectID::random();
        let object_id_2 = ObjectID::random();

        let mut shared_object_congestion_tracker =
            new_congestion_tracker_with_initial_value_for_test(
                &[(object_id_0, 5), (object_id_1, 10)],
                mode,
                min_free_execution_slot,
            );
        assert_eq!(
            shared_object_congestion_tracker.max_occupied_slot_end_cost(),
            10
        );

        // Read two objects should not change the object execution cost.
        let cert = build_transaction(&[(object_id_0, false), (object_id_1, false)], 10);
        let shared_input_objects: Vec<_> = cert.shared_input_objects().collect();
        let cert_cost = shared_object_congestion_tracker.get_tx_cost(&cert);
        let start_cost = shared_object_congestion_tracker
            .compute_tx_start_cost(&shared_input_objects, cert_cost);

        shared_object_congestion_tracker.bump_object_execution_slots(&cert, start_cost);
        assert_eq!(
            shared_object_congestion_tracker,
            new_congestion_tracker_with_initial_value_for_test(
                &[(object_id_0, 5), (object_id_1, 10)],
                mode,
                min_free_execution_slot,
            )
        );
        assert_eq!(
            shared_object_congestion_tracker.max_occupied_slot_end_cost(),
            10
        );

        // Write to object 0 should only bump object 0's execution slots. The start cost
        // should be object 1's cost.
        let cert = build_transaction(&[(object_id_0, true), (object_id_1, false)], 10);
        let shared_input_objects: Vec<_> = cert.shared_input_objects().collect();
        let cert_cost = shared_object_congestion_tracker.get_tx_cost(&cert);
        let start_cost = shared_object_congestion_tracker
            .compute_tx_start_cost(&shared_input_objects, cert_cost);
        shared_object_congestion_tracker.bump_object_execution_slots(&cert, start_cost);
        let expected_object_0_cost = match mode {
            PerObjectCongestionControlMode::None => unreachable!(),
            PerObjectCongestionControlMode::TotalGasBudget => 20,
            PerObjectCongestionControlMode::TotalTxCount => 11,
        };
        assert_eq!(
            max_object_free_slot_start_cost(
                shared_object_congestion_tracker
                    .object_execution_slots
                    .get(&object_id_0)
                    .unwrap()
            ),
            expected_object_0_cost
        );
        assert_eq!(
            max_object_free_slot_start_cost(
                shared_object_congestion_tracker
                    .object_execution_slots
                    .get(&object_id_1)
                    .unwrap()
            ),
            10
        );
        assert_eq!(
            shared_object_congestion_tracker.max_occupied_slot_end_cost(),
            expected_object_0_cost
        );

        // Write to all objects should bump all objects' execution cost, including
        // objects that are seen for the first time.
        let cert = build_transaction(
            &[
                (object_id_0, true),
                (object_id_1, true),
                (object_id_2, true),
            ],
            10,
        );
        let expected_object_cost = match mode {
            PerObjectCongestionControlMode::None => unreachable!(),
            PerObjectCongestionControlMode::TotalGasBudget => 30,
            PerObjectCongestionControlMode::TotalTxCount => 12,
        };
        let shared_input_objects: Vec<_> = cert.shared_input_objects().collect();
        let cert_cost = shared_object_congestion_tracker.get_tx_cost(&cert);
        let start_cost = shared_object_congestion_tracker
            .compute_tx_start_cost(&shared_input_objects, cert_cost);
        shared_object_congestion_tracker.bump_object_execution_slots(&cert, start_cost);
        assert_eq!(
            max_object_free_slot_start_cost(
                shared_object_congestion_tracker
                    .object_execution_slots
                    .get(&object_id_0)
                    .unwrap()
            ),
            expected_object_cost
        );
        assert_eq!(
            max_object_free_slot_start_cost(
                shared_object_congestion_tracker
                    .object_execution_slots
                    .get(&object_id_1)
                    .unwrap()
            ),
            expected_object_cost
        );
        assert_eq!(
            max_object_free_slot_start_cost(
                shared_object_congestion_tracker
                    .object_execution_slots
                    .get(&object_id_2)
                    .unwrap()
            ),
            expected_object_cost
        );
        assert_eq!(
            shared_object_congestion_tracker.max_occupied_slot_end_cost(),
            expected_object_cost
        );
    }

    #[rstest]
    fn test_cost_overflow(#[values(true, false)] min_free_execution_slot: bool) {
        let object_id_0 = ObjectID::random();
        let object_id_1 = ObjectID::random();
        let object_id_2 = ObjectID::random();
        // edge case: max value is saturated
        let max_accumulated_txn_cost_per_object_in_commit = u64::MAX;

        // case 1: large initial cost, small tx cost
        // the initial object execution cost is as follows:
        //               object 0       object 1
        //            0| xxxxxxxx     | xxxxxxxx
        //            1| xxxxxxxx     | xxxxxxxx
        // :::::::::::::::::::::::::::::::::::::
        // u64::MAX - 2| xxxxxxxx     | xxxxxxxx
        // u64::MAX - 1|              |

        let mut shared_object_congestion_tracker =
            new_congestion_tracker_with_initial_value_for_test(
                &[(object_id_0, u64::MAX - 1), (object_id_1, u64::MAX - 1)],
                PerObjectCongestionControlMode::TotalGasBudget,
                min_free_execution_slot,
            );

        let tx = build_transaction(&[(object_id_0, true)], 1);
        if let SequencingResult::Schedule(start_cost) = shared_object_congestion_tracker
            .try_schedule(
                &tx,
                max_accumulated_txn_cost_per_object_in_commit,
                &HashMap::new(),
                0,
            )
        {
            // add the small transaction to the tracker
            // the object execution cost becomes:
            //               object 0       object 1
            //            0| xxxxxxxx     | xxxxxxxx
            //            1| xxxxxxxx     | xxxxxxxx
            // :::::::::::::::::::::::::::::::::::::
            // u64::MAX - 2| xxxxxxxx     | xxxxxxxx
            // u64::MAX - 1| xxxxxxxx     |
            shared_object_congestion_tracker.bump_object_execution_slots(&tx, start_cost);
            assert_eq!(
                max_object_free_slot_start_cost(
                    shared_object_congestion_tracker
                        .object_execution_slots
                        .get(&object_id_0)
                        .unwrap()
                ),
                u64::MAX
            );
            assert_eq!(
                max_object_free_slot_start_cost(
                    shared_object_congestion_tracker
                        .object_execution_slots
                        .get(&object_id_1)
                        .unwrap()
                ),
                u64::MAX - 1
            );
        } else {
            panic!("transaction is not congesting, should not defer");
        }

        let tx = build_transaction(&[(object_id_0, true), (object_id_1, true)], 1);
        if let SequencingResult::Defer(_, congested_objects) = shared_object_congestion_tracker
            .try_schedule(
                &tx,
                max_accumulated_txn_cost_per_object_in_commit,
                &HashMap::new(),
                0,
            )
        {
            assert_eq!(congested_objects.len(), 1);
            assert_eq!(congested_objects[0], object_id_0);
        } else {
            panic!("object 0 is congesting, should defer");
        }
        let shared_input_objects: Vec<_> = tx.shared_input_objects().collect();
        let cert_cost = shared_object_congestion_tracker.get_tx_cost(&tx);
        let start_cost = shared_object_congestion_tracker
            .compute_tx_start_cost(&shared_input_objects, cert_cost);
        println!("start_cost: {}", start_cost);
        shared_object_congestion_tracker.bump_object_execution_slots(&tx, start_cost);
        assert_eq!(
            max_object_free_slot_start_cost(
                shared_object_congestion_tracker
                    .object_execution_slots
                    .get(&object_id_0)
                    .unwrap()
            ),
            u64::MAX
        );
        assert_eq!(
            max_object_free_slot_start_cost(
                shared_object_congestion_tracker
                    .object_execution_slots
                    .get(&object_id_1)
                    .unwrap()
            ),
            u64::MAX
        );

        if let SequencingResult::Defer(_, congested_objects) = shared_object_congestion_tracker
            .try_schedule(
                &tx,
                max_accumulated_txn_cost_per_object_in_commit,
                &HashMap::new(),
                0,
            )
        {
            // with min_free_execution_slot, only object 0 is cause of congestion.
            // without min_free_execution_slot, both objects are congested.
            assert_eq!(
                congested_objects.len(),
                if min_free_execution_slot { 1 } else { 2 }
            );
            assert_eq!(congested_objects[0], object_id_0);
            if !min_free_execution_slot {
                assert_eq!(congested_objects[1], object_id_1);
            }
        } else {
            panic!("objects 0 and 1 are congesting, should defer");
        }

        let shared_input_objects: Vec<_> = tx.shared_input_objects().collect();
        let cert_cost = shared_object_congestion_tracker.get_tx_cost(&tx);
        let start_cost = shared_object_congestion_tracker
            .compute_tx_start_cost(&shared_input_objects, cert_cost);
        shared_object_congestion_tracker.bump_object_execution_slots(&tx, start_cost);
        assert_eq!(
            max_object_free_slot_start_cost(
                shared_object_congestion_tracker
                    .object_execution_slots
                    .get(&object_id_0)
                    .unwrap()
            ),
            u64::MAX
        );
        assert_eq!(
            max_object_free_slot_start_cost(
                shared_object_congestion_tracker
                    .object_execution_slots
                    .get(&object_id_1)
                    .unwrap()
            ),
            u64::MAX
        );

        // case 2: small initial cost, large tx cost
        // the initial object execution cost is as follows:
        //     object 0       object 1       object 2
        //  0|              | xxxxxxxx     | xxxxxxxx
        //  1|              |              | xxxxxxxx
        //  2|              |              |
        let mut shared_object_congestion_tracker =
            new_congestion_tracker_with_initial_value_for_test(
                &[(object_id_0, 0), (object_id_1, 1), (object_id_2, 2)],
                PerObjectCongestionControlMode::TotalGasBudget,
                min_free_execution_slot,
            );

        let tx = build_transaction(
            &[
                (object_id_0, true),
                (object_id_1, true),
                (object_id_2, true),
            ],
            u64::MAX - 1,
        );
        if let SequencingResult::Defer(_, congested_objects) = shared_object_congestion_tracker
            .try_schedule(
                &tx,
                max_accumulated_txn_cost_per_object_in_commit,
                &HashMap::new(),
                0,
            )
        {
            // object 2 is the cause of congestion.
            assert_eq!(congested_objects.len(), 1);
            assert_eq!(congested_objects[0], object_id_2);
        } else {
            panic!("case 2: object 2 is congested, should defer");
        }

        let shared_input_objects: Vec<_> = tx.shared_input_objects().collect();
        let cert_cost = shared_object_congestion_tracker.get_tx_cost(&tx);
        let start_cost = shared_object_congestion_tracker
            .compute_tx_start_cost(&shared_input_objects, cert_cost);
        shared_object_congestion_tracker.bump_object_execution_slots(&tx, start_cost);
        assert_eq!(
            max_object_free_slot_start_cost(
                shared_object_congestion_tracker
                    .object_execution_slots
                    .get(&object_id_0)
                    .unwrap()
            ),
            u64::MAX
        );
        assert_eq!(
            max_object_free_slot_start_cost(
                shared_object_congestion_tracker
                    .object_execution_slots
                    .get(&object_id_1)
                    .unwrap()
            ),
            u64::MAX
        );
        assert_eq!(
            max_object_free_slot_start_cost(
                shared_object_congestion_tracker
                    .object_execution_slots
                    .get(&object_id_2)
                    .unwrap()
            ),
            u64::MAX
        );

        // case 3: max initial cost, max tx cost
        // the initial object execution cost is as follows:
        //               object 0
        //            0| xxxxxxxx
        //            1| xxxxxxxx
        // :::::::::::::
        // u64::MAX - 1| xxxxxxxx
        let mut shared_object_congestion_tracker =
            new_congestion_tracker_with_initial_value_for_test(
                &[(object_id_0, u64::MAX)],
                PerObjectCongestionControlMode::TotalGasBudget,
                min_free_execution_slot,
            );

        let tx = build_transaction(&[(object_id_0, true)], u64::MAX);
        if let SequencingResult::Defer(_, congested_objects) = shared_object_congestion_tracker
            .try_schedule(
                &tx,
                max_accumulated_txn_cost_per_object_in_commit,
                &HashMap::new(),
                0,
            )
        {
            assert_eq!(congested_objects.len(), 1);
            assert_eq!(congested_objects[0], object_id_0);
        } else {
            panic!("case 3: object 0 is congested, should defer");
        }

        let shared_input_objects: Vec<_> = tx.shared_input_objects().collect();
        let cert_cost = shared_object_congestion_tracker.get_tx_cost(&tx);
        let start_cost = shared_object_congestion_tracker
            .compute_tx_start_cost(&shared_input_objects, cert_cost);
        shared_object_congestion_tracker.bump_object_execution_slots(&tx, start_cost);
        assert_eq!(
            max_object_free_slot_start_cost(
                shared_object_congestion_tracker
                    .object_execution_slots
                    .get(&object_id_0)
                    .unwrap()
            ),
            u64::MAX
        );
    }
}
