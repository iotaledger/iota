// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::{HashMap, hash_map::Entry};

use iota_types::{
    base_types::ObjectID,
    effects::{InputSharedObject, TransactionEffects, TransactionEffectsAPI},
    execution_status::CongestedObjects,
    messages_checkpoint::{CheckpointTimestamp, VerifiedCheckpoint},
    transaction::{TransactionData, TransactionDataAPI},
};
use moka::{ops::compute::Op, sync::Cache};

use crate::execution_cache::TransactionCacheRead;

#[derive(Clone, Copy, Debug)]
pub struct CongestionInfo {
    pub last_cancellation_time: CheckpointTimestamp,
    pub highest_cancelled_gas_price: u64,
    pub last_success_time: Option<CheckpointTimestamp>,
    pub lowest_executed_gas_price: Option<u64>,
    pub hotness: f64, /* The hotness of an object corresponds to the expected tip to pay for a
                       * successful execution */
}

const LEARNING_RATE: f64 = 0.2;

impl CongestionInfo {
    /// Update the congestion info with the latest congestion info from a new
    /// checkpoint
    fn update_for_new_checkpoint(&mut self, new: &CongestionInfo) {
        // If there are more recent cancellations, we need to know the latest highest
        // cancelled price
        if new.last_cancellation_time > self.last_cancellation_time {
            self.last_cancellation_time = new.last_cancellation_time;
            self.highest_cancelled_gas_price = new.highest_cancelled_gas_price;
        }
        // If there are more recent successful transactions, we need to know the latest
        // lowest executed price
        if new.last_success_time > self.last_success_time {
            self.last_success_time = new.last_success_time;
            self.lowest_executed_gas_price = new.lowest_executed_gas_price;
        }
    }

    fn update_hotness(
        &mut self,
        new: &CongestionInfo,
        number_congested_transactions: usize,
        _number_cleared_transactions: usize,
    ) {
        // Update hotness per object based on the congestion events according to the
        // formula: hotness(i) -= SUM(tx)[hotness(i) - gas_price_feedback(tx)] *
        // LEARNING_RATE / number_congested_transactions
        self.hotness -= new.hotness * LEARNING_RATE / number_congested_transactions as f64;
        self.hotness = self.hotness.max(0.0); // Ensure hotness is non-negative
    }

    fn update_hotness_for_new_object(
        &mut self,
        new: &CongestionInfo,
        number_congested_transactions: usize,
        _number_cleared_transactions: usize,
    ) {
        self.hotness = -new.hotness * LEARNING_RATE / number_congested_transactions as f64;
        self.hotness = self.hotness.max(0.0); // Ensure hotness is non-negative
    }

    fn update_for_cancellation(&mut self, now: CheckpointTimestamp, gas_price: u64) {
        self.last_cancellation_time = now;
        self.highest_cancelled_gas_price =
            std::cmp::max(self.highest_cancelled_gas_price, gas_price);
    }

    fn update_for_success(&mut self, now: CheckpointTimestamp, gas_price: u64) {
        self.last_success_time = Some(now);
        self.lowest_executed_gas_price = Some(match self.lowest_executed_gas_price {
            Some(current_min) => std::cmp::min(current_min, gas_price),
            None => gas_price,
        });
    }
}

pub struct CongestionTracker {
    pub reference_gas_price: u64,
    pub congestion_clearing_prices: Cache<ObjectID, CongestionInfo>,
}

impl CongestionTracker {
    pub fn new(reference_gas_price: u64) -> Self {
        Self {
            reference_gas_price,
            congestion_clearing_prices: Cache::new(10_000),
        }
    }

    pub fn process_checkpoint_effects(
        &self,
        transaction_cache_reader: &dyn TransactionCacheRead,
        checkpoint: &VerifiedCheckpoint,
        effects: &[TransactionEffects],
    ) {
        let mut congestion_events = Vec::with_capacity(effects.len());
        let mut cleared_events = Vec::with_capacity(effects.len());

        for effect in effects {
            let gas_price = transaction_cache_reader
                .get_transaction_block(effect.transaction_digest())
                .unwrap()
                .unwrap()
                .transaction_data()
                .gas_price();
            if let Some(CongestedObjects(congested_objects)) =
                effect.status().get_congested_objects()
            {
                // let gas_price_feedback = effect.status().get_suggested_gas_price();     //
                // TODO: Add getter to ExecutionStatus
                let gas_price_feedback = 1_100;
                congestion_events.push((gas_price, congested_objects.clone(), gas_price_feedback));
            } else {
                // let gas_price_feedback = effect.status().get_suggested_gas_price();     //
                // TODO: Add getter to ExecutionStatus
                let gas_price_feedback = 1_100;
                cleared_events.push((
                    gas_price,
                    effect
                        .input_shared_objects()
                        .into_iter()
                        .filter_map(|object| match object {
                            InputSharedObject::Mutate((id, _, _)) => Some(id),
                            InputSharedObject::Cancelled(_, _)
                            | InputSharedObject::ReadOnly(_)
                            | InputSharedObject::ReadDeleted(_, _)
                            | InputSharedObject::MutateDeleted(_, _) => None,
                        })
                        .collect::<Vec<_>>(),
                    gas_price_feedback,
                ));
            }
        }

        self.process_per_checkpoint_events(
            checkpoint.timestamp_ms,
            &congestion_events,
            &cleared_events,
        );
    }

    /// For all the mutable shared inputs, get the highest minimum clearing
    /// price (if any exists) and the lowest maximum cancelled price.
    pub fn get_suggested_gas_prices(&self, transaction: &TransactionData) -> Option<u64> {
        self.get_suggested_gas_price_for_objects(
            transaction
                .shared_input_objects()
                .into_iter()
                .filter(|id| id.mutable)
                .map(|id| id.id),
        )
    }

    /// For all the mutable shared inputs, sum the hotness of the objects.
    /// More sophisticated prediction can be implemented.
    pub fn get_suggested_gas_price_with_ogd(&self, transaction: TransactionData) -> Option<u64> {
        self.get_total_hotness_for_objects(
            transaction
                .shared_input_objects()
                .into_iter()
                .filter(|id| id.mutable)
                .map(|id| id.id),
        )
    }

    /// Returns a map of all objects and their hotness values.
    pub fn get_all_hotness(&self) -> HashMap<ObjectID, f64> {
        let mut hotness = HashMap::new();

        for entry in self.congestion_clearing_prices.iter() {
            hotness.insert(*entry.0, entry.1.hotness);
        }

        hotness
    }

    /// Returns the hotness of a specific object, if it exists.
    pub fn get_hotness_for_object(&self, object_id: &ObjectID) -> Option<f64> {
        self.congestion_clearing_prices
            .get(object_id)
            .map(|info| info.hotness)
    }
}

impl CongestionTracker {
    fn process_per_checkpoint_events(
        &self,
        now: CheckpointTimestamp,
        congestion_events: &[(u64, Vec<ObjectID>, u64)],
        cleared_events: &[(u64, Vec<ObjectID>, u64)],
    ) {
        let congestion_info_map =
            self.compute_per_checkpoint_congestion_info(now, congestion_events, cleared_events);
        self.process_checkpoint_congestion(
            congestion_info_map,
            congestion_events.len(),
            cleared_events.len(),
        );
    }

    fn get_suggested_gas_price_for_objects(
        &self,
        objects: impl Iterator<Item = ObjectID>,
    ) -> Option<u64> {
        let mut clearing_price = None;
        for object_id in objects {
            if let Some(info) = self.get_congestion_info(object_id) {
                let clearing_price_for_object = match info
                    .last_success_time
                    .cmp(&Some(info.last_cancellation_time))
                {
                    std::cmp::Ordering::Greater => {
                        // there were no cancellations in the most recent checkpoint,
                        // so the object is probably not congested any more
                        None
                    }
                    std::cmp::Ordering::Less => {
                        // there were no successes in the most recent checkpoint. This should be a
                        // rare case, but we know we will have to bid at
                        // least as much as the highest cancelled price.
                        Some(info.highest_cancelled_gas_price)
                    }
                    std::cmp::Ordering::Equal => {
                        // there were both successes and cancellations.
                        info.lowest_executed_gas_price
                    }
                };
                clearing_price = std::cmp::max(clearing_price, clearing_price_for_object);
            }
        }
        clearing_price
    }

    fn get_total_hotness_for_objects(
        &self,
        objects: impl Iterator<Item = ObjectID>,
    ) -> Option<u64> {
        let mut total_hotness = 0.0;

        for object_id in objects {
            if let Some(info) = self.get_congestion_info(object_id) {
                total_hotness += info.hotness;
            }
        }
        Some(total_hotness as u64)
    }

    fn compute_per_checkpoint_congestion_info(
        &self,
        now: CheckpointTimestamp,
        congestion_events: &[(u64, Vec<ObjectID>, u64)],
        cleared_events: &[(u64, Vec<ObjectID>, u64)],
    ) -> HashMap<ObjectID, CongestionInfo> {
        let mut congestion_info_map: HashMap<ObjectID, CongestionInfo> = HashMap::new();
        let mut object_id_per_tx: Vec<ObjectID> = Vec::new();

        for (gas_price, objects, gas_price_feedback) in congestion_events {
            let mut hotness_per_tx = 0.0;
            object_id_per_tx.clear();
            for object in objects {
                match congestion_info_map.entry(*object) {
                    Entry::Occupied(mut entry) => {
                        let info = entry.get_mut();
                        info.update_for_cancellation(now, *gas_price);
                        hotness_per_tx += self.get_hotness_for_object(object).unwrap_or(0.0);
                    }
                    Entry::Vacant(entry) => {
                        let info = CongestionInfo {
                            last_cancellation_time: now,
                            highest_cancelled_gas_price: *gas_price,
                            last_success_time: None,
                            lowest_executed_gas_price: None,
                            hotness: 0.0,
                        };
                        entry.insert(info);
                    }
                }
                object_id_per_tx.push(*object);
            }
            // We create an auxiliary map of objects summing object hotness minus gas price
            // feedback
            for object in &object_id_per_tx {
                let hotness_adjustment =
                    hotness_per_tx - *gas_price_feedback as f64 + self.reference_gas_price as f64;

                if let Some(info) = congestion_info_map.get_mut(object) {
                    info.hotness += hotness_adjustment;
                }
            }
        }

        for (gas_price, objects, _) in cleared_events {
            for object in objects {
                // We only record clearing prices if the object has observed cancellations
                // recently
                match congestion_info_map.entry(*object) {
                    Entry::Occupied(entry) => {
                        entry.into_mut().update_for_success(now, *gas_price);
                    }
                    Entry::Vacant(entry) => {
                        if let Some(prev) = self.get_congestion_info(*object) {
                            let info = CongestionInfo {
                                last_cancellation_time: prev.last_cancellation_time,
                                highest_cancelled_gas_price: prev.highest_cancelled_gas_price,
                                last_success_time: Some(now),
                                lowest_executed_gas_price: Some(*gas_price),
                                hotness: 0.0,
                            };
                            entry.insert(info);
                        }
                    }
                }
            }
        }

        congestion_info_map
    }

    fn process_checkpoint_congestion(
        &self,
        congestion_info_map: HashMap<ObjectID, CongestionInfo>,
        number_congested_transactions: usize,
        number_cleared_transactions: usize,
    ) {
        for (object_id, info) in congestion_info_map {
            self.congestion_clearing_prices
                .entry(object_id)
                .and_compute_with(|maybe_entry| {
                    if let Some(e) = maybe_entry {
                        let mut e = e.into_value();
                        e.update_for_new_checkpoint(&info);
                        e.update_hotness(
                            &info,
                            number_congested_transactions,
                            number_cleared_transactions,
                        );
                        Op::Put(e)
                    } else {
                        let mut new_info = info;
                        new_info.update_hotness_for_new_object(
                            &info,
                            number_congested_transactions,
                            number_cleared_transactions,
                        );
                        Op::Put(new_info)
                    }
                });
        }
    }

    fn get_congestion_info(&self, object_id: ObjectID) -> Option<CongestionInfo> {
        self.congestion_clearing_prices.get(&object_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_events_new_congestion() {
        let rgp_test = 1000;
        let tracker = CongestionTracker::new(rgp_test);
        let obj1 = ObjectID::random();
        let obj2 = ObjectID::random();
        let now = 1000;

        tracker.process_per_checkpoint_events(
            now,
            &[(100, vec![obj1], 1000), (200, vec![obj2], 1000)],
            &[],
        );

        assert_eq!(
            tracker.get_suggested_gas_price_for_objects(vec![obj1].into_iter()),
            Some(100)
        );
        assert_eq!(
            tracker.get_suggested_gas_price_for_objects(vec![obj2].into_iter()),
            Some(200)
        );
    }

    #[test]

    fn test_process_events_congestion_then_success() {
        let rgp_test = 1000;
        let tracker = CongestionTracker::new(rgp_test);
        let obj = ObjectID::random();

        // Cancellations only, no successes. Highest cancelled price is used.
        tracker.process_per_checkpoint_events(
            1000,
            &[(100, vec![obj], 1000), (75, vec![obj], 1000)],
            &[],
        );
        assert_eq!(
            tracker.get_suggested_gas_price_for_objects(vec![obj].into_iter()),
            Some(100)
        );

        // No cancellations in last checkpoint, so no congestion
        tracker.process_per_checkpoint_events(2000, &[], &[(150, vec![obj], 1000)]);
        assert_eq!(
            tracker.get_suggested_gas_price_for_objects(vec![obj].into_iter()),
            None,
        );

        // next checkpoint has cancellations and successes, so the lowest success price
        // is used.
        tracker.process_per_checkpoint_events(
            3000,
            &[(100, vec![obj], 1000)],
            &[(175, vec![obj], 1000), (125, vec![obj], 1000)],
        );
        assert_eq!(
            tracker.get_suggested_gas_price_for_objects(vec![obj].into_iter()),
            Some(125)
        );
    }

    #[test]
    fn test_get_suggested_gas_price_multiple_objects() {
        let rgp_test = 1000;
        let tracker = CongestionTracker::new(rgp_test);
        let obj1 = ObjectID::random();
        let obj2 = ObjectID::random();

        // Process different congestion events
        tracker.process_per_checkpoint_events(
            1000,
            &[(100, vec![obj1], 1000), (200, vec![obj2], 1000)],
            &[],
        );

        // Should suggest highest congestion price
        assert_eq!(
            tracker.get_suggested_gas_price_for_objects(vec![obj1, obj2].into_iter()),
            Some(200)
        );

        // Process different congestion events
        tracker.process_per_checkpoint_events(
            2000,
            &[(100, vec![obj1], 1000), (200, vec![obj2], 1000)],
            &[(100, vec![obj1], 1000), (150, vec![obj2], 1000)],
        );
        // Should suggest the highest lowest success price
        assert_eq!(
            tracker.get_suggested_gas_price_for_objects(vec![obj1, obj2].into_iter()),
            Some(150)
        );
    }

    #[test]
    fn test_compute_per_checkpoint_congestion_info_hotness_update() {
        let rgp_test = 1000;
        let tracker = CongestionTracker::new(rgp_test);
        let obj1 = ObjectID::random();
        let obj2 = ObjectID::random();
        let obj3 = ObjectID::random();

        let now = 1000;

        // Congestion events: both objects are congested with different gas price
        // feedback
        let congestion_events = vec![
            (1000, vec![obj1], 900),  // should result in unchanged hotness for obj1
            (1000, vec![obj2], 1200), // should result in positive hotness adjustment for obj2
            (1000, vec![obj2], 1200),
            (1000, vec![obj2, obj3], 1600), /* should result in positive hotness adjustment for
                                             * obj3 */
        ];

        let cleared_events = vec![]; // no clearing in this round

        tracker.process_per_checkpoint_events(now, &congestion_events, &cleared_events);

        // New hotness values should be 0 (obj1), 50 (obj2) and 30 (obj3)
        // For obj3, this is calculated as:
        // LEARNING_RATE * [hotness (1600) - gas_price_feedback (1000)] / num_txs (4)
        assert!(
            tracker.get_hotness_for_object(&obj1).unwrap() == 0.0,
            "obj1 should have unchanged hotness"
        );
        assert!(
            tracker.get_hotness_for_object(&obj2).unwrap() == LEARNING_RATE * 250.0,
            "obj2 should have increased hotness"
        );
        assert!(
            tracker.get_hotness_for_object(&obj3).unwrap() == LEARNING_RATE * 150.0,
            "obj3 should have increased hotness"
        );
    }

    #[test]
    fn test_repeated_congestion_across_checkpoints() {
        let rgp_test = 1000;
        let tracker = CongestionTracker::new(rgp_test);
        let obj1 = ObjectID::random();
        let obj2 = ObjectID::random();

        // First checkpoint
        tracker.process_per_checkpoint_events(1000, &[(100, vec![obj1], 1500)], &[]);

        // Second checkpoint, touches same object and new one
        tracker.process_per_checkpoint_events(1100, &[(100, vec![obj1, obj2], 1700)], &[]);

        let hotness1 = tracker.get_hotness_for_object(&obj1).unwrap();
        let hotness2 = tracker.get_hotness_for_object(&obj2).unwrap();
        assert!(hotness1 == 240.0, "obj1 should have unchanged hotness");
        assert!(hotness2 == 140.0, "obj2 should have increased hotness");
    }
}
