// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
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
use tracing::info;

use crate::execution_cache::TransactionCacheRead;

/// Capacity of the congestion tracker's cache.
const CONGESTION_TRACKER_CACHE_CAPACITY: u64 = 10_000;

/// Alias for type holding congestion info per checkpoint.
type CongestionInfoMap = HashMap<ObjectID, CongestionInfo>;

/// Holds tracked per-object congestion info.
#[derive(Clone, Copy, Debug)]
pub struct CongestionInfo {
    /// Timestamp of the latest checkpoint which contains transaction(s)
    /// with this object being congested.
    latest_congestion_time: CheckpointTimestamp,

    /// Highest gas price of transaction(s) in which the accessed
    /// object has been congested.
    highest_congestion_gas_price: u64,

    /// Timestamp of the latest checkpoint which contains transaction(s)
    /// with this object being not congested (cleared).
    latest_clearing_time: Option<CheckpointTimestamp>,

    /// Lowest gas price of clearing transaction(s) accessing the object.
    lowest_clearing_gas_price: Option<u64>,

    /// The hotness of an object corresponds to the expected tip to pay for a
    /// successful execution.
    pub hotness: f64,
}

const LEARNING_RATE: f64 = 1.0;
const HOTNESS_THRESHOLD: f64 = 1.0;

impl CongestionInfo {
    /// Update this congestion info with the congestion info from a new
    /// checkpoint.
    fn update_with_new_congestion_info(&mut self, new_congestion_info: &CongestionInfo) {
        // If there is recent congestion, we need to update the latest highest
        // gas price of transactions with congested objects, as well as the latest
        // congestion time.
        if new_congestion_info.latest_congestion_time > self.latest_congestion_time {
            self.latest_congestion_time = new_congestion_info.latest_congestion_time;
            self.highest_congestion_gas_price = new_congestion_info.highest_congestion_gas_price;
        }

        // If there are more recent clearing transactions, we need to update
        // the latest time and lowest gas price of such transactions.
        if new_congestion_info.latest_clearing_time > self.latest_clearing_time {
            self.latest_clearing_time = new_congestion_info.latest_clearing_time;
            self.lowest_clearing_gas_price = new_congestion_info.lowest_clearing_gas_price;
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
        self.latest_congestion_time = now;
        self.highest_congestion_gas_price =
            std::cmp::max(self.highest_congestion_gas_price, gas_price);
    }

    /// Update the lowest gas price and the latest time with the data from a
    /// clearing transaction.
    fn update_for_clearing_tx(&mut self, time: CheckpointTimestamp, gas_price: u64) {
        self.latest_clearing_time = Some(time);
        self.lowest_clearing_gas_price = Some(match self.lowest_clearing_gas_price {
            Some(current_lowest) => current_lowest.min(gas_price),
            None => gas_price,
        });
    }
}

/// `CongestionTracker` tracks objects' congestion info.
/// The info is then used to calculated a suggested gas price.
pub struct CongestionTracker {
    pub reference_gas_price: u64,
    pub object_congestion_info: Cache<ObjectID, CongestionInfo>,
}

impl CongestionTracker {
    /// Create a new `CongestionTracker`. The cache capacity will be
    /// set to `CONGESTION_TRACKER_CACHE_CAPACITY`, which is `10_000`.
    pub fn new(reference_gas_price: u64) -> Self {
        Self {
            reference_gas_price,
            object_congestion_info: Cache::new(CONGESTION_TRACKER_CACHE_CAPACITY),
        }
    }

    pub fn process_checkpoint_effects(
        &self,
        transaction_cache_reader: &dyn TransactionCacheRead,
        checkpoint: &VerifiedCheckpoint,
        effects: &[TransactionEffects],
    ) {
        // Containers for checkpoint's congestion and clearing transactions data.
        let mut congestion_txs_data: Vec<(u64, Vec<ObjectID>, u64)> =
            Vec::with_capacity(effects.len());
        let mut clearing_txs_data: Vec<(u64, Vec<ObjectID>)> = Vec::with_capacity(effects.len());

        for effects in effects {
            let gas_price = transaction_cache_reader
                .get_transaction_block(effects.transaction_digest())
                .unwrap_or_else(|| {
                    panic!(
                        "Could not get transaction block {} from transaction cache reader.",
                        effects.transaction_digest()
                    )
                })
                .transaction_data()
                .gas_price();

            if let Some(CongestedObjects(congested_objects)) =
                effects.status().get_congested_objects()
            {
                let gas_price_feedback = effects
                    .status()
                    .get_feedback_suggested_gas_price()
                    .unwrap_or(self.reference_gas_price);
                congestion_txs_data.push((
                    gas_price,
                    congested_objects.clone(),
                    gas_price_feedback,
                ));
                let block = transaction_cache_reader
                    .get_transaction_block(effects.transaction_digest())
                    .unwrap_or_else(|| {
                        panic!("block not found in transaction cache");
                    });

                let tx_data = block.transaction_data();

                info!(
                    "Checkpoint: {} | Gas price: {} | Feedback: {} | Prediction (Sui): {:?} | Prediction (IOTA): {:?}",
                    checkpoint.sequence_number,
                    gas_price,
                    gas_price_feedback,
                    self.get_prediction_suggested_gas_price(&tx_data)
                        .unwrap_or(0),
                    self.get_suggested_gas_price_with_ogd(&tx_data).unwrap_or(0)
                );
            } else {
                clearing_txs_data.push((
                    gas_price,
                    effects
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
                ));
            }
        }

        self.process_congestion_and_clearing_txs_data(
            checkpoint.timestamp_ms,
            &congestion_txs_data,
            &clearing_txs_data,
        );

        if !self.get_all_hotness().is_empty() {
            info!(
                "Hotness after checkpoint {}: {:?}",
                checkpoint.sequence_number,
                self.get_all_hotness()
            );
        }
    }

    /// For all the mutable input shared objects accessed by `transaction`,
    /// get the highest minimum clearing price, if any exists. The 'clearing'
    /// gas price means the underlying transaction was not cancelled due
    /// congestion.
    pub fn get_prediction_suggested_gas_price(&self, transaction: &TransactionData) -> Option<u64> {
        self.get_suggested_gas_price_for_objects(
            transaction
                .shared_input_objects()
                .into_iter()
                .filter(|obj| obj.mutable)
                .map(|obj| obj.id),
        )
    }

    /// For all the mutable shared inputs, sum the hotness of the objects.
    /// More sophisticated prediction can be implemented.
    pub fn get_suggested_gas_price_with_ogd(&self, transaction: &TransactionData) -> Option<u64> {
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

        for entry in self.object_congestion_info.iter() {
            hotness.insert(*entry.0, entry.1.hotness);
        }

        hotness
    }

    /// Returns the hotness of a specific object, if it exists.
    pub fn get_hotness_for_object(&self, object_id: &ObjectID) -> Option<f64> {
        self.object_congestion_info
            .get(object_id)
            .map(|info| info.hotness)
    }
}

impl CongestionTracker {
    /// Process checkpoint's congestion and clearing transactions info.
    fn process_congestion_and_clearing_txs_data(
        &self,
        time: CheckpointTimestamp,
        congestion_txs_data: &[(u64, Vec<ObjectID>, u64)],
        clearing_txs_data: &[(u64, Vec<ObjectID>)],
    ) {
        let congestion_info_map = self.compute_per_checkpoint_congestion_info(
            time,
            congestion_txs_data,
            clearing_txs_data,
        );
        self.update_congestion_info_cache(
            congestion_info_map,
            congestion_txs_data.len(),
            clearing_txs_data.len(),
        );
    }

    /// Get the highest minimum clearing price, if any exists, for a list of
    /// (input shared) objects.
    fn get_suggested_gas_price_for_objects(
        &self,
        objects: impl Iterator<Item = ObjectID>,
    ) -> Option<u64> {
        let mut clearing_gas_price = None;

        for object_id in objects {
            info!(
                "Getting congestion info for object: {}",
                object_id
            );
            if let Some(info) = self.get_congestion_info(object_id) {
                let clearing_gas_price_for_object = match info
                    .latest_clearing_time
                    .cmp(&Some(info.latest_congestion_time))
                {
                    std::cmp::Ordering::Greater => {
                        // There were no congestion transactions in the most recent checkpoint,
                        // so the object is probably not congested any more
                        None
                    }
                    std::cmp::Ordering::Less => {
                        // There were no clearing transactions in the most recent checkpoint.
                        // This should be a rare case, but we know we will have to bid at least as
                        // much as the highest congestion price.
                        Some(info.highest_congestion_gas_price)
                    }
                    std::cmp::Ordering::Equal => {
                        // There were both clearing and congestion transactions.
                        info.lowest_clearing_gas_price
                    }
                };

                clearing_gas_price = clearing_gas_price_for_object.max(clearing_gas_price);
            }
        }

        clearing_gas_price
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
        Some(1000 + total_hotness as u64)
    }

    fn compute_per_checkpoint_congestion_info(
        &self,
        time: CheckpointTimestamp,
        congestion_txs_data: &[(u64, Vec<ObjectID>, u64)],
        clearing_txs_data: &[(u64, Vec<ObjectID>)],
    ) -> HashMap<ObjectID, CongestionInfo> {
        let mut congestion_info_map: HashMap<ObjectID, CongestionInfo> = HashMap::new();
        let mut object_id_per_tx: Vec<ObjectID> = Vec::new();

        for (gas_price, objects, gas_price_feedback) in congestion_txs_data {
            let mut hotness_per_tx = 0.0;
            object_id_per_tx.clear();
            for object in objects {
                match congestion_info_map.entry(*object) {
                    Entry::Occupied(entry) => {
                        entry.into_mut().update_for_cancellation(time, *gas_price);
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(CongestionInfo {
                            latest_congestion_time: time,
                            highest_congestion_gas_price: *gas_price,
                            latest_clearing_time: None,
                            lowest_clearing_gas_price: None,
                            hotness: 0.0,
                        });
                    }
                }
                hotness_per_tx += self.get_hotness_for_object(object).unwrap_or(0.0);
                object_id_per_tx.push(*object);
            }
            // Adjust hotness based on the sum of hotness of objects in the transaction
            // minus the gas price feedback
            for object in &object_id_per_tx {
                let hotness_adjustment =
                    hotness_per_tx - *gas_price_feedback as f64 + self.reference_gas_price as f64;

                if let Some(info) = congestion_info_map.get_mut(object) {
                    info.hotness += hotness_adjustment;
                }
            }
        }

        for (gas_price, objects) in clearing_txs_data {
            for object in objects {
                // We only record clearing prices if the object has observed cancellations
                // recently
                match congestion_info_map.entry(*object) {
                    Entry::Occupied(entry) => {
                        entry.into_mut().update_for_clearing_tx(time, *gas_price);
                    }
                    Entry::Vacant(entry) => {
                        if let Some(prev) = self.get_congestion_info(*object) {
                            entry.insert(CongestionInfo {
                                latest_congestion_time: prev.latest_congestion_time,
                                highest_congestion_gas_price: prev.highest_congestion_gas_price,
                                latest_clearing_time: Some(time),
                                lowest_clearing_gas_price: Some(*gas_price),
                                hotness: prev.hotness,
                            });
                        }
                    }
                }
            }
        }

        congestion_info_map
    }

    fn update_congestion_info_cache(
        &self,
        congestion_info_map: CongestionInfoMap,
        number_congested_transactions: usize,
        number_cleared_transactions: usize,
    ) {
        // Store the object IDs that are congested in this checkpoint
        let congested_objects: std::collections::HashSet<_> =
            congestion_info_map.keys().cloned().collect();

        for (object_id, info) in congestion_info_map {
            self.object_congestion_info
                .entry(object_id)
                .and_compute_with(|maybe_entry| {
                    if let Some(e) = maybe_entry {
                        let mut e = e.into_value();
                        e.update_with_new_congestion_info(&info);
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

        // Decay hotness of unaffected objects, and prune if too cold
        for (object_id, _) in self.object_congestion_info.iter() {
            if !congested_objects.contains(&object_id) {
                self.object_congestion_info
                    .entry(*object_id)
                    .and_compute_with(|maybe_entry| {
                        if let Some(e) = maybe_entry {
                            let mut e = e.into_value();
                            e.hotness /= 2.0;
                            if e.hotness < HOTNESS_THRESHOLD {
                                Op::Remove
                            } else {
                                Op::Put(e)
                            }
                        } else {
                            Op::Nop
                        }
                    });
            }
        }
    }

    /// Get congestion info for a given object.
    fn get_congestion_info(&self, object_id: ObjectID) -> Option<CongestionInfo> {
        self.object_congestion_info.get(&object_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_checkpoint_congestion_and_clearing_txs_data_for_new_congestion() {
        let rgp_test = 1000;
        let tracker = CongestionTracker::new(rgp_test);
        let object_1 = ObjectID::random();
        let object_2 = ObjectID::random();

        let time = 1_000;
        let congestion_txs_data = vec![(100, vec![object_1], 1000), (200, vec![object_2], 1000)];
        let clearing_txs_data = vec![];

        tracker.process_congestion_and_clearing_txs_data(
            time,
            &congestion_txs_data,
            &clearing_txs_data,
        );

        assert_eq!(
            tracker.get_suggested_gas_price_for_objects(vec![object_1].into_iter()),
            Some(100)
        );
        assert_eq!(
            tracker.get_suggested_gas_price_for_objects(vec![object_2].into_iter()),
            Some(200)
        );
    }

    #[test]
    fn process_checkpoint_congestion_and_clearing_txs_data_for_congestion_then_success() {
        let rgp_test = 1000;
        let tracker = CongestionTracker::new(rgp_test);
        let object = ObjectID::random();

        // Congestion transactions only, no clearing ones. The highest congestion
        // gas price should be used.
        let time = 1_000;
        let congestion_txs_data = vec![(100, vec![object], 1000), (75, vec![object], 1000)];
        let clearing_txs_data = vec![];
        tracker.process_congestion_and_clearing_txs_data(
            time,
            &congestion_txs_data,
            &clearing_txs_data,
        );
        assert_eq!(
            tracker.get_suggested_gas_price_for_objects(vec![object].into_iter()),
            Some(100)
        );

        // No congestion transactions data in last checkpoint, so no congestion.
        let time = 2_000;
        let congestion_txs_data = vec![];
        let clearing_txs_data = vec![(150, vec![object])];
        tracker.process_congestion_and_clearing_txs_data(
            time,
            &congestion_txs_data,
            &clearing_txs_data,
        );
        assert_eq!(
            tracker.get_suggested_gas_price_for_objects(vec![object].into_iter()),
            None,
        );

        // Next checkpoint has both congestion and clearing transactions,
        // so the lowest clearing gas price should be used.
        let time = 3_000;
        let congestion_txs_data = vec![(100, vec![object], 1000)];
        let clearing_txs_data = vec![(175, vec![object]), (125, vec![object])];
        tracker.process_congestion_and_clearing_txs_data(
            time,
            &congestion_txs_data,
            &clearing_txs_data,
        );
        assert_eq!(
            tracker.get_suggested_gas_price_for_objects(vec![object].into_iter()),
            Some(125)
        );
    }

    #[test]
    fn get_suggested_gas_price_for_multiple_objects() {
        let rgp_test = 1000;
        let tracker = CongestionTracker::new(rgp_test);
        let object_1 = ObjectID::random();
        let object_2 = ObjectID::random();

        let time = 1_000;
        let congestion_txs_data = vec![(100, vec![object_1], 1000), (200, vec![object_2], 1000)];
        let clearing_txs_data = vec![];
        tracker.process_congestion_and_clearing_txs_data(
            time,
            &congestion_txs_data,
            &clearing_txs_data,
        );
        // Should suggest the highest congestion gas price
        assert_eq!(
            tracker.get_suggested_gas_price_for_objects(vec![object_1, object_2].into_iter()),
            Some(200)
        );

        let time = 2_000;
        let congestion_txs_data = vec![(100, vec![object_1], 1000), (200, vec![object_2], 1000)];
        let clearing_txs_data = vec![(100, vec![object_1]), (150, vec![object_2])];
        tracker.process_congestion_and_clearing_txs_data(
            time,
            &congestion_txs_data,
            &clearing_txs_data,
        );
        // Should suggest the maximum (over objects) lowest clearing gas price
        assert_eq!(
            tracker.get_suggested_gas_price_for_objects(vec![object_1, object_2].into_iter()),
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

        tracker.process_congestion_and_clearing_txs_data(now, &congestion_events, &cleared_events);

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
        tracker.process_congestion_and_clearing_txs_data(1000, &[(100, vec![obj1], 1500)], &[]);

        // Second checkpoint, touches same object and new one
        tracker.process_congestion_and_clearing_txs_data(
            1100,
            &[(100, vec![obj1, obj2], 1700)],
            &[],
        );

        let hotness1 = tracker.get_hotness_for_object(&obj1).unwrap_or(0.0);
        let hotness2 = tracker.get_hotness_for_object(&obj2).unwrap_or(0.0);
        assert!(hotness1 == 220.0, "hotness for obj1 should be 220.0");
        assert!(hotness2 == 120.0, "hotness for obj2 should be 120.0");

        // Additional checkpoints
        tracker.process_congestion_and_clearing_txs_data(1000, &[], &[]);
        tracker.process_congestion_and_clearing_txs_data(1000, &[(100, vec![obj2], 1050)], &[]);
        tracker.process_congestion_and_clearing_txs_data(1000, &[], &[]);
        tracker.process_congestion_and_clearing_txs_data(
            1000,
            &[(100, vec![obj1, obj2], 1150), (100, vec![obj1], 1020)],
            &[],
        );

        let hotness1 = tracker.get_hotness_for_object(&obj1).unwrap_or(0.0);
        let hotness2 = tracker.get_hotness_for_object(&obj2).unwrap_or(0.0);
        assert!(hotness1 == 36.1, "hotness for obj1 should be 36.1");
        assert!(hotness2 == 38.35, "hotness for obj2 should be 38.35");
    }

    #[test]
    fn test_remove_cold_objects_from_cache() {
        let rgp_test = 1000;
        let tracker = CongestionTracker::new(rgp_test);
        let obj1 = ObjectID::random();
        let obj2 = ObjectID::random();

        // First checkpoint with two congested objects
        tracker.process_congestion_and_clearing_txs_data(
            1000,
            &[(100, vec![obj1, obj2], 1015)],
            &[],
        );

        // obj1 is not congested anymore
        tracker.process_congestion_and_clearing_txs_data(1000, &[(100, vec![obj2], 1018)], &[]);
        tracker.process_congestion_and_clearing_txs_data(1000, &[], &[]);

        // hotness for obj1 goes below 1.0 so it should be removed from cache
        assert!(
            tracker.get_hotness_for_object(&obj1).is_none(),
            "obj1 should be removed from cache"
        );
        let hotness = tracker.get_hotness_for_object(&obj2).unwrap_or(0.0);
        println!("hotness for obj2: {}", hotness);
        assert!(hotness == 3.0, "hotness for obj2 should be 3.0");
    }
}
