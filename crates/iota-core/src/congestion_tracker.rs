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

use crate::execution_cache::TransactionCacheRead;

/// Capacity of the congestion tracker's cache.
const CONGESTION_TRACKER_CACHE_CAPACITY: u64 = 10_000;

/// Alias type for holding transaction's gas price and mutable (or
/// congested) shared objects.
type TransactionGasPriceMutSharedObjectsPair = (u64, Vec<ObjectID>);

/// Alias for type holding congestion info per checkpoint.
type CongestionInfoMap = HashMap<ObjectID, CongestionInfo>;

/// Holds tracked per-object congestion info.
#[derive(Clone, Copy, Debug)]
<<<<<<< HEAD
pub struct CongestionInfo {
    pub last_cancellation_time: CheckpointTimestamp,
    pub highest_cancelled_gas_price: u64,
    pub last_success_time: Option<CheckpointTimestamp>,
    pub lowest_executed_gas_price: Option<u64>,
    pub hotness: f64, /* The hotness of an object corresponds to the expected tip to pay for a
                       * successful execution */
=======
struct CongestionInfo {
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
>>>>>>> protocol-research/import-congestion-tracker
}

const LEARNING_RATE: f64 = 0.2;

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

<<<<<<< HEAD
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
=======
    /// Update the highest congestion gas price with the new `gas_price`.
    fn update_highest_congestion_gas_price(&mut self, gas_price: u64) {
        self.highest_congestion_gas_price = self.highest_congestion_gas_price.max(gas_price);
>>>>>>> protocol-research/import-congestion-tracker
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
<<<<<<< HEAD
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

=======
    /// Key-value-based cache storing congestion info of objects.
    object_congestion_info: Cache<ObjectID, CongestionInfo>,
}

impl Default for CongestionTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl CongestionTracker {
    /// Create a new `CongestionTracker`. The cache capacity will be
    /// set to `CONGESTION_TRACKER_CACHE_CAPACITY`, which is `10_000`.
    pub fn new() -> Self {
        Self {
            object_congestion_info: Cache::new(CONGESTION_TRACKER_CACHE_CAPACITY),
        }
    }

    /// Process effects of all transactions included in a certain checkpoint.
>>>>>>> protocol-research/import-congestion-tracker
    pub fn process_checkpoint_effects(
        &self,
        transaction_cache_reader: &dyn TransactionCacheRead,
        checkpoint: &VerifiedCheckpoint,
        effects: &[TransactionEffects],
    ) {
        // Containers for checkpoint's congestion and clearing transactions data.
        let mut congestion_txs_data: Vec<TransactionGasPriceMutSharedObjectsPair> =
            Vec::with_capacity(effects.len());
        let mut clearing_txs_data: Vec<TransactionGasPriceMutSharedObjectsPair> =
            Vec::with_capacity(effects.len());

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
<<<<<<< HEAD
                // let gas_price_feedback = effect.status().get_suggested_gas_price();     //
                // TODO: Add getter to ExecutionStatus
                let gas_price_feedback = 1_100;
                congestion_events.push((gas_price, congested_objects.clone(), gas_price_feedback));
            } else {
                // let gas_price_feedback = effect.status().get_suggested_gas_price();     //
                // TODO: Add getter to ExecutionStatus
                let gas_price_feedback = 1_100;
                cleared_events.push((
=======
                congestion_txs_data.push((gas_price, congested_objects.clone()));
            } else {
                clearing_txs_data.push((
>>>>>>> protocol-research/import-congestion-tracker
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
                    gas_price_feedback,
                ));
            }
        }

        self.process_congestion_and_clearing_txs_data(
            checkpoint.timestamp_ms,
<<<<<<< HEAD
            &congestion_events,
            &cleared_events,
=======
            &congestion_txs_data,
            &clearing_txs_data,
>>>>>>> protocol-research/import-congestion-tracker
        );
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
<<<<<<< HEAD
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
=======
    /// Process checkpoint's congestion and clearing transactions info.
    fn process_congestion_and_clearing_txs_data(
        &self,
        time: CheckpointTimestamp,
        congestion_txs_data: &[TransactionGasPriceMutSharedObjectsPair],
        clearing_txs_data: &[TransactionGasPriceMutSharedObjectsPair],
    ) {
        let congestion_info_map =
            self.compute_congestion_info_map(time, congestion_txs_data, clearing_txs_data);
        self.update_congestion_info_cache(congestion_info_map);
>>>>>>> protocol-research/import-congestion-tracker
    }

    /// Get the highest minimum clearing price, if any exists, for a list of
    /// (input shared) objects.
    fn get_suggested_gas_price_for_objects(
        &self,
        objects: impl Iterator<Item = ObjectID>,
    ) -> Option<u64> {
        let mut clearing_gas_price = None;

        for object_id in objects {
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

<<<<<<< HEAD
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
=======
    /// Compute a congestion info map from checkpoint's congestion and
    /// clearing transactions data.
    fn compute_congestion_info_map(
        &self,
        time: CheckpointTimestamp,
        congestion_txs_data: &[TransactionGasPriceMutSharedObjectsPair],
        clearing_txs_data: &[TransactionGasPriceMutSharedObjectsPair],
    ) -> CongestionInfoMap {
        let mut congestion_info_map = CongestionInfoMap::new();

        for (gas_price, objects) in congestion_txs_data {
            objects.iter().for_each(|object_id| {
                congestion_info_map
                    .entry(*object_id)
                    .and_modify(|entry| entry.update_highest_congestion_gas_price(*gas_price))
                    .or_insert(CongestionInfo {
                        latest_congestion_time: time,
                        highest_congestion_gas_price: *gas_price,
                        latest_clearing_time: None,
                        lowest_clearing_gas_price: None,
                    });
            });
        }

        for (gas_price, objects) in clearing_txs_data {
            objects.iter().for_each(|object_id| {
                match congestion_info_map.entry(*object_id) {
>>>>>>> protocol-research/import-congestion-tracker
                    Entry::Occupied(entry) => {
                        entry.into_mut().update_for_clearing_tx(time, *gas_price);
                    }
                    Entry::Vacant(entry) => {
<<<<<<< HEAD
                        if let Some(prev) = self.get_congestion_info(*object) {
                            let info = CongestionInfo {
                                last_cancellation_time: prev.last_cancellation_time,
                                highest_cancelled_gas_price: prev.highest_cancelled_gas_price,
                                last_success_time: Some(now),
                                lowest_executed_gas_price: Some(*gas_price),
                                hotness: 0.0,
                            };
                            entry.insert(info);
=======
                        // We only record clearing prices if the object has experienced
                        // congestion recently.
                        if let Some(prev) = self.get_congestion_info(*object_id) {
                            entry.insert(CongestionInfo {
                                latest_congestion_time: prev.latest_congestion_time,
                                highest_congestion_gas_price: prev.highest_congestion_gas_price,
                                latest_clearing_time: Some(time),
                                lowest_clearing_gas_price: Some(*gas_price),
                            });
>>>>>>> protocol-research/import-congestion-tracker
                        }
                    }
                }
            });
        }

        congestion_info_map
    }

<<<<<<< HEAD
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
=======
    /// Update tracker's congestion info cache using checkpoint's congestion
    /// info map.
    fn update_congestion_info_cache(&self, congestion_info_map: CongestionInfoMap) {
        for (object_id, new_congestion_info) in congestion_info_map {
            self.object_congestion_info
                .entry(object_id)
                .and_compute_with(|maybe_entry| {
                    if let Some(entry) = maybe_entry {
                        let mut congestion_info = entry.into_value();
                        congestion_info.update_with_new_congestion_info(&new_congestion_info);

                        Op::Put(congestion_info)
                    } else {
                        Op::Put(new_congestion_info)
>>>>>>> protocol-research/import-congestion-tracker
                    }
                });
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
<<<<<<< HEAD
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
=======
    fn process_checkpoint_congestion_and_clearing_txs_data_for_new_congestion() {
        let tracker = CongestionTracker::new();
        let object_1 = ObjectID::random();
        let object_2 = ObjectID::random();

        let time = 1_000;
        let congestion_txs_data = vec![(100, vec![object_1]), (200, vec![object_2])];
        let clearing_txs_data = vec![];

        tracker.process_congestion_and_clearing_txs_data(
            time,
            &congestion_txs_data,
            &clearing_txs_data,
>>>>>>> protocol-research/import-congestion-tracker
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
<<<<<<< HEAD

    fn test_process_events_congestion_then_success() {
        let rgp_test = 1000;
        let tracker = CongestionTracker::new(rgp_test);
        let obj = ObjectID::random();

        // Cancellations only, no successes. Highest cancelled price is used.
        tracker.process_per_checkpoint_events(
            1000,
            &[(100, vec![obj], 1000), (75, vec![obj], 1000)],
            &[],
=======
    fn process_checkpoint_congestion_and_clearing_txs_data_for_congestion_then_success() {
        let tracker = CongestionTracker::new();
        let object = ObjectID::random();

        // Congestion transactions only, no clearing ones. The highest congestion
        // gas price should be used.
        let time = 1_000;
        let congestion_txs_data = vec![(100, vec![object]), (75, vec![object])];
        let clearing_txs_data = vec![];
        tracker.process_congestion_and_clearing_txs_data(
            time,
            &congestion_txs_data,
            &clearing_txs_data,
>>>>>>> protocol-research/import-congestion-tracker
        );
        assert_eq!(
            tracker.get_suggested_gas_price_for_objects(vec![object].into_iter()),
            Some(100)
        );

<<<<<<< HEAD
        // No cancellations in last checkpoint, so no congestion
        tracker.process_per_checkpoint_events(2000, &[], &[(150, vec![obj], 1000)]);
=======
        // No congestion transactions data in last checkpoint, so no congestion.
        let time = 2_000;
        let congestion_txs_data = vec![];
        let clearing_txs_data = vec![(150, vec![object])];
        tracker.process_congestion_and_clearing_txs_data(
            time,
            &congestion_txs_data,
            &clearing_txs_data,
        );
>>>>>>> protocol-research/import-congestion-tracker
        assert_eq!(
            tracker.get_suggested_gas_price_for_objects(vec![object].into_iter()),
            None,
        );

<<<<<<< HEAD
        // next checkpoint has cancellations and successes, so the lowest success price
        // is used.
        tracker.process_per_checkpoint_events(
            3000,
            &[(100, vec![obj], 1000)],
            &[(175, vec![obj], 1000), (125, vec![obj], 1000)],
=======
        // Next checkpoint has both congestion and clearing transactions,
        // so the lowest clearing gas price should be used.
        let time = 3_000;
        let congestion_txs_data = vec![(100, vec![object])];
        let clearing_txs_data = vec![(175, vec![object]), (125, vec![object])];
        tracker.process_congestion_and_clearing_txs_data(
            time,
            &congestion_txs_data,
            &clearing_txs_data,
>>>>>>> protocol-research/import-congestion-tracker
        );
        assert_eq!(
            tracker.get_suggested_gas_price_for_objects(vec![object].into_iter()),
            Some(125)
        );
    }

    #[test]
<<<<<<< HEAD
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
=======
    fn get_suggested_gas_price_for_multiple_objects() {
        let tracker = CongestionTracker::new();
        let object_1 = ObjectID::random();
        let object_2 = ObjectID::random();

        let time = 1_000;
        let congestion_txs_data = vec![(100, vec![object_1]), (200, vec![object_2])];
        let clearing_txs_data = vec![];
        tracker.process_congestion_and_clearing_txs_data(
            time,
            &congestion_txs_data,
            &clearing_txs_data,
        );
        // Should suggest the highest congestion gas price
>>>>>>> protocol-research/import-congestion-tracker
        assert_eq!(
            tracker.get_suggested_gas_price_for_objects(vec![object_1, object_2].into_iter()),
            Some(200)
        );

<<<<<<< HEAD
        // Process different congestion events
        tracker.process_per_checkpoint_events(
            2000,
            &[(100, vec![obj1], 1000), (200, vec![obj2], 1000)],
            &[(100, vec![obj1], 1000), (150, vec![obj2], 1000)],
        );
        // Should suggest the highest lowest success price
=======
        let time = 2_000;
        let congestion_txs_data = vec![(100, vec![object_1]), (200, vec![object_2])];
        let clearing_txs_data = vec![(100, vec![object_1]), (150, vec![object_2])];
        tracker.process_congestion_and_clearing_txs_data(
            time,
            &congestion_txs_data,
            &clearing_txs_data,
        );
        // Should suggest the maximum (over objects) lowest clearing gas price
>>>>>>> protocol-research/import-congestion-tracker
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
