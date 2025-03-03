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

// An execution slot is a time slot in which a transaction is executed.
// Transactions can occupy overlapping execution slots if they do not touch any
// common shared objects.
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

    pub fn lowest_overlap(&self, other: &ExecutionSlot) -> ExecutionSlot {
        ExecutionSlot {
            start_cost: self.start_cost.max(other.start_cost),
            end_cost: self.end_cost.min(other.end_cost),
            scheduled: false,
        }
    }
}

// SharedObjectCongestionTracker stores the accumulated cost of executing
// transactions on an object, for all transactions in a consensus commit.
//
// Cost is an indication of transaction execution latency. When transactions are
// scheduled by the consensus handler, each scheduled transaction adds cost
// (execution latency) to all the objects it reads or writes.
//
// The goal of this data structure is to capture the critical path of
// transaction execution latency on each objects.
//
// The mode field determines how the cost is calculated. The cost can be
// calculated based on the total gas budget, or total number of transaction
// count.
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct SharedObjectCongestionTracker {
    object_execution_cost: HashMap<ObjectID, Vec<ExecutionSlot>>,
    mode: PerObjectCongestionControlMode,
    shelf_stacking: bool,
}

impl SharedObjectCongestionTracker {
    pub fn new(mode: PerObjectCongestionControlMode, shelf_stacking: bool) -> Self {
        Self {
            object_execution_cost: HashMap::new(),
            mode,
            shelf_stacking,
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
        // initalise the free execution slots for the objects that are not in the
        // tracker.
        for obj in shared_input_objects {
            self.object_execution_cost
                .entry(obj.id)
                .or_insert(vec![ExecutionSlot::new(0, u64::MAX, false)]);
        }
        if self.shelf_stacking {
            // begin with the full range of the slots available with no contstraints from
            // previous objects.
            let available_range = ExecutionSlot::new(0, u64::MAX, false);
            self.compute_lowest_available_execution_slot(
                &shared_input_objects,
                tx_cost,
                available_range,
            )
            .unwrap_or(u64::MAX)
        } else {
            // find the maximum start cost of free slots for the shared input objects.
            shared_input_objects
                .iter()
                .map(|obj| {
                    self.object_execution_cost
                        .get(&obj.id)
                        .expect("object should have been inserted at the start of this function.")
                })
                .map(|slots| max_free_slot_start_cost(slots))
                .max()
                .expect("There must be at least one object in shared_input_objects.")
        }
    }
    // A recursive function that tries to find the lowest free slot for a
    // transaction. If a slot is found that fits the transaction, the function
    // returns the slot. Otherwise, it returns None.
    // available_range is the range of the slot that the transaction can fit in
    // given the objects that have been checked so far.
    fn compute_lowest_available_execution_slot(
        &self,
        shared_input_objects: &[SharedInputObject],
        tx_cost: u64,
        available_range: ExecutionSlot,
    ) -> Option<u64> {
        // take the first object from the shared input objects.
        let obj = shared_input_objects.first().unwrap();
        // set aside the remaining objects for the next recursive call.
        let remaining_objects = if shared_input_objects.len() > 1 {
            &shared_input_objects[1..]
        } else {
            &[]
        };

        for free_slot in self.object_execution_cost.get(&obj.id).unwrap() {
            // only consider slots with no transaction assigned yet.
            if free_slot.scheduled {
                continue;
            }
            let lowest_overlap = free_slot.lowest_overlap(&available_range);
            // if there is no overlap, height will be 0.
            if lowest_overlap.height() < tx_cost {
                continue;
            }
            // if this is the last object to check, return this slot as it is the lowest
            // slot available.
            if remaining_objects.is_empty() {
                return Some(lowest_overlap.start_cost);
            }
            // if there are more objects to check, recursively call the function with the
            // remaining objects.
            // If the recursive call returns a start cost, that means the transaction fits
            // in the slot for all remaining objects. Return the start cost.
            // Otherwise, continue to check the next free slot for the current object.
            if let Some(lowest_overlap) = self.compute_lowest_available_execution_slot(
                remaining_objects,
                tx_cost,
                lowest_overlap,
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

    pub fn get_tx_cost(&self, cert: &VerifiedExecutableTransaction) -> u64 {
        match self.mode {
            PerObjectCongestionControlMode::None => 0,
            PerObjectCongestionControlMode::TotalGasBudget => cert.gas_budget(),
            PerObjectCongestionControlMode::TotalTxCount => 1,
        }
    }

    // Given a transaction, returns the deferral key and the congested objects if
    // the transaction should be deferred.
    pub fn should_defer_due_to_object_congestion(
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
                .object_execution_cost
                .get(&obj.id)
                .expect("scheduled object must have execution cost");
            let min_start_cost = if self.shelf_stacking {
                min_free_slot_start_cost(execution_slots)
            } else {
                max_free_slot_start_cost(execution_slots)
            };
            if start_cost == min_start_cost {
                congested_objects.push(obj.id);
            }
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

    // Update shared objects' execution cost used in `cert` using `cert`'s execution
    // cost. This is called when `cert` is scheduled for execution.
    pub fn bump_object_execution_cost(
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
                    .object_execution_cost
                    .get(&obj.id)
                    .unwrap_or(&mut vec![])
                    .iter()
                    .enumerate()
                {
                    // if the occupied slot overlaps with the free slot, split the free slot.
                    if occupied_slot.start_cost >= free_slot.start_cost
                        && occupied_slot.start_cost < free_slot.end_cost
                    {
                        old_slot_index = Some(index);
                        // if a part of the free slot remains after the occupied slot, add it to the
                        // new slots.
                        if occupied_slot.end_cost < free_slot.end_cost {
                            new_slots.push(ExecutionSlot::new(
                                occupied_slot.end_cost,
                                free_slot.end_cost,
                                false,
                            ));
                        }
                        // if a part of the free slot remains before the occupied slot, add it to
                        // the new slots.
                        if free_slot.start_cost < occupied_slot.start_cost {
                            new_slots.push(ExecutionSlot::new(
                                free_slot.start_cost,
                                occupied_slot.start_cost,
                                false,
                            ));
                        }
                        break;
                    }
                }
                // remove the old slot and add the new slots.
                let slots = self.object_execution_cost.get_mut(&obj.id).unwrap();
                if old_slot_index.is_some() {
                    slots.remove(old_slot_index.unwrap());
                }
                slots.push(occupied_slot);
                slots.extend(new_slots);
                slots.sort_by(|a, b| a.start_cost.cmp(&b.start_cost));
            }
        }
    }

    // Returns the maximum cost of all objects.
    pub fn max_cost(&self) -> u64 {
        self.object_execution_cost
            .values()
            .map(|slots| max_free_slot_start_cost(slots))
            .max()
            .unwrap_or(0)
    }
}

fn min_free_slot_start_cost(slots: &Vec<ExecutionSlot>) -> u64 {
    slots
        .iter()
        .filter(|slot| !slot.scheduled)
        .map(|slot| slot.start_cost)
        .min()
        .unwrap_or(u64::MAX)
}

fn max_free_slot_start_cost(slots: &Vec<ExecutionSlot>) -> u64 {
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
        shelf_stacking: bool,
    ) -> SharedObjectCongestionTracker {
        let mut shared_object_congestion_tracker =
            SharedObjectCongestionTracker::new(mode, shelf_stacking);
        // add inital values for each transaction
        for (object_id, cost) in init_values {
            match mode {
                PerObjectCongestionControlMode::None => {}
                PerObjectCongestionControlMode::TotalGasBudget => {
                    let transaction = build_transaction(&[(*object_id, true)], *cost);
                    let shared_input_objects: Vec<_> = transaction.shared_input_objects().collect();
                    let start_cost = shared_object_congestion_tracker
                        .compute_tx_start_cost(&shared_input_objects, *cost);
                    shared_object_congestion_tracker
                        .bump_object_execution_cost(&transaction, start_cost);
                }
                PerObjectCongestionControlMode::TotalTxCount => {
                    for _ in 0..*cost {
                        let transaction = build_transaction(&[(*object_id, true)], 1);
                        let shared_input_objects: Vec<_> =
                            transaction.shared_input_objects().collect();
                        let start_cost = shared_object_congestion_tracker
                            .compute_tx_start_cost(&shared_input_objects, 1);
                        shared_object_congestion_tracker
                            .bump_object_execution_cost(&transaction, start_cost);
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

    #[test]
    fn test_compute_tx_start_at_cost() {
        let object_id_0 = ObjectID::random();
        let object_id_1 = ObjectID::random();
        let object_id_2 = ObjectID::random();

        let mut shared_object_congestion_tracker =
            new_congestion_tracker_with_initial_value_for_test(
                &[(object_id_0, 5), (object_id_1, 10)],
                PerObjectCongestionControlMode::TotalGasBudget,
                false,
            );

        let shared_input_objects = construct_shared_input_objects(&[(object_id_0, false)]);
        assert_eq!(
            shared_object_congestion_tracker.compute_tx_start_cost(&shared_input_objects, 5),
            5
        );

        let shared_input_objects = construct_shared_input_objects(&[(object_id_1, true)]);
        assert_eq!(
            shared_object_congestion_tracker.compute_tx_start_cost(&shared_input_objects, 5),
            10
        );

        let shared_input_objects =
            construct_shared_input_objects(&[(object_id_0, false), (object_id_1, false)]);
        assert_eq!(
            shared_object_congestion_tracker.compute_tx_start_cost(&shared_input_objects, 5),
            10
        );

        let shared_input_objects =
            construct_shared_input_objects(&[(object_id_0, true), (object_id_1, true)]);
        assert_eq!(
            shared_object_congestion_tracker.compute_tx_start_cost(&shared_input_objects, 5),
            10
        );

        // Test tx that touch object for the first time, which should start from 0.
        let shared_input_objects = construct_shared_input_objects(&[(object_id_2, true)]);
        assert_eq!(
            shared_object_congestion_tracker.compute_tx_start_cost(&shared_input_objects, 5),
            0
        );
    }

    #[rstest]
    fn test_should_defer_return_correct_congested_objects(
        #[values(
            PerObjectCongestionControlMode::TotalGasBudget,
            PerObjectCongestionControlMode::TotalTxCount
        )]
        mode: PerObjectCongestionControlMode,
    ) {
        // Creates two shared objects and three transactions that operate on these
        // objects.
        let shared_obj_0 = ObjectID::random();
        let shared_obj_1 = ObjectID::random();

        let tx_gas_budget = 100;

        // Set max_accumulated_txn_cost_per_object_in_commit to only allow 1 transaction
        // to go through.
        let max_accumulated_txn_cost_per_object_in_commit = match mode {
            PerObjectCongestionControlMode::None => unreachable!(),
            PerObjectCongestionControlMode::TotalGasBudget => tx_gas_budget + 1,
            PerObjectCongestionControlMode::TotalTxCount => 2,
        };

        let mut shared_object_congestion_tracker = match mode {
            PerObjectCongestionControlMode::None => unreachable!(),
            PerObjectCongestionControlMode::TotalGasBudget => {
                // Construct object execution cost as following
                //                1     10
                // object 0:            |
                // object 1:      |
                new_congestion_tracker_with_initial_value_for_test(
                    &[(shared_obj_0, 10), (shared_obj_1, 1)],
                    mode,
                    false,
                )
            }
            PerObjectCongestionControlMode::TotalTxCount => {
                // Construct object execution cost as following
                //                1     2
                // object 0:            |
                // object 1:      |
                new_congestion_tracker_with_initial_value_for_test(
                    &[(shared_obj_0, 2), (shared_obj_1, 1)],
                    mode,
                    false,
                )
            }
        };

        // Read/write to object 0 should be deferred.
        for mutable in [true, false].iter() {
            let tx = build_transaction(&[(shared_obj_0, *mutable)], tx_gas_budget);
            if let SequencingResult::Defer(_, congested_objects) = shared_object_congestion_tracker
                .should_defer_due_to_object_congestion(
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

        // Read/write to object 1 should go through.
        for mutable in [true, false].iter() {
            let tx = build_transaction(&[(shared_obj_1, *mutable)], tx_gas_budget);
            matches!(
                shared_object_congestion_tracker.should_defer_due_to_object_congestion(
                    &tx,
                    max_accumulated_txn_cost_per_object_in_commit,
                    &HashMap::new(),
                    0,
                ),
                SequencingResult::Schedule(_)
            );
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
                    shared_object_congestion_tracker.should_defer_due_to_object_congestion(
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
        }
    }

    #[rstest]
    fn test_should_defer_return_correct_deferral_key(
        #[values(
            PerObjectCongestionControlMode::TotalGasBudget,
            PerObjectCongestionControlMode::TotalTxCount
        )]
        mode: PerObjectCongestionControlMode,
    ) {
        let shared_obj_0 = ObjectID::random();
        let tx = build_transaction(&[(shared_obj_0, true)], 100);
        // Make should_defer_due_to_object_congestion always defer transactions.
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
        ) = shared_object_congestion_tracker.should_defer_due_to_object_congestion(
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
        ) = shared_object_congestion_tracker.should_defer_due_to_object_congestion(
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
        ) = shared_object_congestion_tracker.should_defer_due_to_object_congestion(
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
    fn test_bump_object_execution_cost(
        #[values(
            PerObjectCongestionControlMode::TotalGasBudget,
            PerObjectCongestionControlMode::TotalTxCount
        )]
        mode: PerObjectCongestionControlMode,
    ) {
        let object_id_0 = ObjectID::random();
        let object_id_1 = ObjectID::random();
        let object_id_2 = ObjectID::random();

        let mut shared_object_congestion_tracker =
            new_congestion_tracker_with_initial_value_for_test(
                &[(object_id_0, 5), (object_id_1, 10)],
                mode,
                false,
            );
        assert_eq!(shared_object_congestion_tracker.max_cost(), 10);

        // Read two objects should not change the object execution cost.
        let cert = build_transaction(&[(object_id_0, false), (object_id_1, false)], 10);
        let shared_input_objects: Vec<_> = cert.shared_input_objects().collect();
        let cert_cost = shared_object_congestion_tracker.get_tx_cost(&cert);
        let start_cost = shared_object_congestion_tracker
            .compute_tx_start_cost(&shared_input_objects, cert_cost);

        shared_object_congestion_tracker.bump_object_execution_cost(&cert, start_cost);
        assert_eq!(
            shared_object_congestion_tracker,
            new_congestion_tracker_with_initial_value_for_test(
                &[(object_id_0, 5), (object_id_1, 10)],
                mode,
                false,
            )
        );
        assert_eq!(shared_object_congestion_tracker.max_cost(), 10);

        // Write to object 0 should only bump object 0's execution cost. The start cost
        // should be object 1's cost.
        let cert = build_transaction(&[(object_id_0, true), (object_id_1, false)], 10);
        let shared_input_objects: Vec<_> = cert.shared_input_objects().collect();
        let cert_cost = shared_object_congestion_tracker.get_tx_cost(&cert);
        let start_cost = shared_object_congestion_tracker
            .compute_tx_start_cost(&shared_input_objects, cert_cost);
        shared_object_congestion_tracker.bump_object_execution_cost(&cert, start_cost);
        let expected_object_0_cost = match mode {
            PerObjectCongestionControlMode::None => unreachable!(),
            PerObjectCongestionControlMode::TotalGasBudget => 20,
            PerObjectCongestionControlMode::TotalTxCount => 11,
        };
        assert_eq!(
            max_free_slot_start_cost(
                shared_object_congestion_tracker
                    .object_execution_cost
                    .get(&object_id_0)
                    .unwrap()
            ),
            expected_object_0_cost
        );
        assert_eq!(
            max_free_slot_start_cost(
                shared_object_congestion_tracker
                    .object_execution_cost
                    .get(&object_id_1)
                    .unwrap()
            ),
            10
        );
        assert_eq!(
            shared_object_congestion_tracker.max_cost(),
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
        shared_object_congestion_tracker.bump_object_execution_cost(&cert, start_cost);
        assert_eq!(
            max_free_slot_start_cost(
                shared_object_congestion_tracker
                    .object_execution_cost
                    .get(&object_id_0)
                    .unwrap()
            ),
            expected_object_cost
        );
        assert_eq!(
            max_free_slot_start_cost(
                shared_object_congestion_tracker
                    .object_execution_cost
                    .get(&object_id_1)
                    .unwrap()
            ),
            expected_object_cost
        );
        assert_eq!(
            max_free_slot_start_cost(
                shared_object_congestion_tracker
                    .object_execution_cost
                    .get(&object_id_2)
                    .unwrap()
            ),
            expected_object_cost
        );
        assert_eq!(
            shared_object_congestion_tracker.max_cost(),
            expected_object_cost
        );
    }

    #[test]
    fn test_cost_overflow() {
        let object_id_0 = ObjectID::random();
        let object_id_1 = ObjectID::random();
        let object_id_2 = ObjectID::random();
        // edge case: max value is saturated
        let max_accumulated_txn_cost_per_object_in_commit = u64::MAX;

        // case 1: large initial cost, small tx cost
        let mut shared_object_congestion_tracker =
            new_congestion_tracker_with_initial_value_for_test(
                &[(object_id_0, u64::MAX - 1), (object_id_1, u64::MAX - 1)],
                PerObjectCongestionControlMode::TotalGasBudget,
                false,
            );

        let tx = build_transaction(&[(object_id_0, true)], 1);
        if let SequencingResult::Schedule(start_cost) = shared_object_congestion_tracker
            .should_defer_due_to_object_congestion(
                &tx,
                max_accumulated_txn_cost_per_object_in_commit,
                &HashMap::new(),
                0,
            )
        {
            println!("start_cost: {}", start_cost);
            shared_object_congestion_tracker.bump_object_execution_cost(&tx, start_cost);
            assert_eq!(
                max_free_slot_start_cost(
                    shared_object_congestion_tracker
                        .object_execution_cost
                        .get(&object_id_0)
                        .unwrap()
                ),
                u64::MAX
            );
            assert_eq!(
                max_free_slot_start_cost(
                    shared_object_congestion_tracker
                        .object_execution_cost
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
            .should_defer_due_to_object_congestion(
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
        shared_object_congestion_tracker.bump_object_execution_cost(&tx, start_cost);
        assert_eq!(
            max_free_slot_start_cost(
                shared_object_congestion_tracker
                    .object_execution_cost
                    .get(&object_id_0)
                    .unwrap()
            ),
            u64::MAX
        );
        assert_eq!(
            max_free_slot_start_cost(
                shared_object_congestion_tracker
                    .object_execution_cost
                    .get(&object_id_1)
                    .unwrap()
            ),
            u64::MAX
        );

        if let SequencingResult::Defer(_, congested_objects) = shared_object_congestion_tracker
            .should_defer_due_to_object_congestion(
                &tx,
                max_accumulated_txn_cost_per_object_in_commit,
                &HashMap::new(),
                0,
            )
        {
            assert_eq!(congested_objects.len(), 2);
            assert_eq!(congested_objects[0], object_id_0);
            assert_eq!(congested_objects[1], object_id_1);
        } else {
            panic!("objects 0 and 1 are congesting, should defer");
        }

        let shared_input_objects: Vec<_> = tx.shared_input_objects().collect();
        let cert_cost = shared_object_congestion_tracker.get_tx_cost(&tx);
        let start_cost = shared_object_congestion_tracker
            .compute_tx_start_cost(&shared_input_objects, cert_cost);
        shared_object_congestion_tracker.bump_object_execution_cost(&tx, start_cost);
        assert_eq!(
            max_free_slot_start_cost(
                shared_object_congestion_tracker
                    .object_execution_cost
                    .get(&object_id_0)
                    .unwrap()
            ),
            u64::MAX
        );
        assert_eq!(
            max_free_slot_start_cost(
                shared_object_congestion_tracker
                    .object_execution_cost
                    .get(&object_id_1)
                    .unwrap()
            ),
            u64::MAX
        );

        // case 2: small initial cost, large tx cost
        let mut shared_object_congestion_tracker =
            new_congestion_tracker_with_initial_value_for_test(
                &[(object_id_0, 0), (object_id_1, 1), (object_id_2, 2)],
                PerObjectCongestionControlMode::TotalGasBudget,
                false,
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
            .should_defer_due_to_object_congestion(
                &tx,
                max_accumulated_txn_cost_per_object_in_commit,
                &HashMap::new(),
                0,
            )
        {
            assert_eq!(congested_objects.len(), 1);
            assert_eq!(congested_objects[0], object_id_2);
        } else {
            panic!("case 2: object 2 is congested, should defer");
        }

        let shared_input_objects: Vec<_> = tx.shared_input_objects().collect();
        let cert_cost = shared_object_congestion_tracker.get_tx_cost(&tx);
        let start_cost = shared_object_congestion_tracker
            .compute_tx_start_cost(&shared_input_objects, cert_cost);
        shared_object_congestion_tracker.bump_object_execution_cost(&tx, start_cost);
        assert_eq!(
            max_free_slot_start_cost(
                shared_object_congestion_tracker
                    .object_execution_cost
                    .get(&object_id_0)
                    .unwrap()
            ),
            u64::MAX
        );
        assert_eq!(
            max_free_slot_start_cost(
                shared_object_congestion_tracker
                    .object_execution_cost
                    .get(&object_id_1)
                    .unwrap()
            ),
            u64::MAX
        );
        assert_eq!(
            max_free_slot_start_cost(
                shared_object_congestion_tracker
                    .object_execution_cost
                    .get(&object_id_2)
                    .unwrap()
            ),
            u64::MAX
        );

        // case 3: max initial cost, max tx cost
        let mut shared_object_congestion_tracker =
            new_congestion_tracker_with_initial_value_for_test(
                &[(object_id_0, u64::MAX)],
                PerObjectCongestionControlMode::TotalGasBudget,
                false,
            );

        let tx = build_transaction(&[(object_id_0, true)], u64::MAX);
        if let SequencingResult::Defer(_, congested_objects) = shared_object_congestion_tracker
            .should_defer_due_to_object_congestion(
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
        shared_object_congestion_tracker.bump_object_execution_cost(&tx, start_cost);
        assert_eq!(
            max_free_slot_start_cost(
                shared_object_congestion_tracker
                    .object_execution_cost
                    .get(&object_id_0)
                    .unwrap()
            ),
            u64::MAX
        );
    }
}
