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

/// Holds tracked per-object congestion info.
#[derive(Clone, Copy, Debug)]
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
}

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

    /// Update the highest congestion gas price with the new `gas_price`.
    fn update_highest_congestion_gas_price(&mut self, gas_price: u64) {
        self.highest_congestion_gas_price = self.highest_congestion_gas_price.max(gas_price);
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
    pub fn process_checkpoint_effects(
        &self,
        transaction_cache_reader: &dyn TransactionCacheRead,
        checkpoint: &VerifiedCheckpoint,
        effects: &[TransactionEffects],
    ) {
        // Containers for checkpoint's congested and cleared transactions info.
        let mut congested_txs_info: Vec<TransactionGasPriceMutSharedObjectsPair> =
            Vec::with_capacity(effects.len());
        let mut cleared_txs_info: Vec<TransactionGasPriceMutSharedObjectsPair> =
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
                congested_txs_info.push((gas_price, congested_objects.clone()));
            } else {
                cleared_txs_info.push((
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

        self.process_per_checkpoint_events(
            checkpoint.timestamp_ms,
            &congested_txs_info,
            &cleared_txs_info,
        );
    }

    /// For all the mutable input shared objects accessed by `transaction`,
    /// get the highest minimum clearing price, if any exists. The 'clearing'
    /// gas price means the underlying transaction was not cancelled due
    /// congestion.
    pub fn get_suggested_gas_price(&self, transaction: &TransactionData) -> Option<u64> {
        self.get_suggested_gas_price_for_objects(
            transaction
                .shared_input_objects()
                .into_iter()
                .filter(|obj| obj.mutable)
                .map(|obj| obj.id),
        )
    }
}

impl CongestionTracker {
    fn process_per_checkpoint_events(
        &self,
        now: CheckpointTimestamp,
        congestion_events: &[TransactionGasPriceMutSharedObjectsPair],
        cleared_events: &[TransactionGasPriceMutSharedObjectsPair],
    ) {
        let congestion_info_map =
            self.compute_per_checkpoint_congestion_info(now, congestion_events, cleared_events);
        self.process_checkpoint_congestion(congestion_info_map);
    }

    fn get_suggested_gas_price_for_objects(
        &self,
        objects: impl Iterator<Item = ObjectID>,
    ) -> Option<u64> {
        let mut clearing_price = None;

        for object_id in objects {
            if let Some(info) = self.get_congestion_info(object_id) {
                let clearing_price_for_object = match info
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
                clearing_price = std::cmp::max(clearing_price, clearing_price_for_object);
            }
        }

        clearing_price
    }

    fn compute_per_checkpoint_congestion_info(
        &self,
        now: CheckpointTimestamp,
        congestion_events: &[TransactionGasPriceMutSharedObjectsPair],
        cleared_events: &[TransactionGasPriceMutSharedObjectsPair],
    ) -> HashMap<ObjectID, CongestionInfo> {
        let mut congestion_info_map: HashMap<ObjectID, CongestionInfo> = HashMap::new();

        for (gas_price, objects) in congestion_events {
            for object in objects {
                match congestion_info_map.entry(*object) {
                    Entry::Occupied(entry) => {
                        entry
                            .into_mut()
                            .update_highest_congestion_gas_price(*gas_price);
                    }
                    Entry::Vacant(entry) => {
                        let info = CongestionInfo {
                            latest_congestion_time: now,
                            highest_congestion_gas_price: *gas_price,
                            latest_clearing_time: None,
                            lowest_clearing_gas_price: None,
                        };

                        entry.insert(info);
                    }
                }
            }
        }

        for (gas_price, objects) in cleared_events {
            for object in objects {
                // We only record clearing prices if the object has experienced congestion
                // recently
                match congestion_info_map.entry(*object) {
                    Entry::Occupied(entry) => {
                        entry.into_mut().update_for_clearing_tx(now, *gas_price);
                    }
                    Entry::Vacant(entry) => {
                        if let Some(prev) = self.get_congestion_info(*object) {
                            let info = CongestionInfo {
                                latest_congestion_time: prev.latest_congestion_time,
                                highest_congestion_gas_price: prev.highest_congestion_gas_price,
                                latest_clearing_time: Some(now),
                                lowest_clearing_gas_price: Some(*gas_price),
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
    ) {
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
                    }
                });
        }
    }

    fn get_congestion_info(&self, object_id: ObjectID) -> Option<CongestionInfo> {
        self.object_congestion_info.get(&object_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_events_new_congestion() {
        let tracker = CongestionTracker::new();
        let obj1 = ObjectID::random();
        let obj2 = ObjectID::random();
        let now = 1000;

        tracker.process_per_checkpoint_events(now, &[(100, vec![obj1]), (200, vec![obj2])], &[]);

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
        let tracker = CongestionTracker::new();
        let obj = ObjectID::random();

        // Cancellations only, no successes. Highest cancelled price is used.
        tracker.process_per_checkpoint_events(1000, &[(100, vec![obj]), (75, vec![obj])], &[]);
        assert_eq!(
            tracker.get_suggested_gas_price_for_objects(vec![obj].into_iter()),
            Some(100)
        );

        // No cancellations in last checkpoint, so no congestion
        tracker.process_per_checkpoint_events(2000, &[], &[(150, vec![obj])]);
        assert_eq!(
            tracker.get_suggested_gas_price_for_objects(vec![obj].into_iter()),
            None,
        );

        // next checkpoint has cancellations and successes, so the lowest success price
        // is used.
        tracker.process_per_checkpoint_events(
            3000,
            &[(100, vec![obj])],
            &[(175, vec![obj]), (125, vec![obj])],
        );
        assert_eq!(
            tracker.get_suggested_gas_price_for_objects(vec![obj].into_iter()),
            Some(125)
        );
    }

    #[test]
    fn test_get_suggested_gas_price_multiple_objects() {
        let tracker = CongestionTracker::new();
        let obj1 = ObjectID::random();
        let obj2 = ObjectID::random();

        // Process different congestion events
        tracker.process_per_checkpoint_events(1000, &[(100, vec![obj1]), (200, vec![obj2])], &[]);

        // Should suggest highest congestion price
        assert_eq!(
            tracker.get_suggested_gas_price_for_objects(vec![obj1, obj2].into_iter()),
            Some(200)
        );

        // Process different congestion events
        tracker.process_per_checkpoint_events(
            2000,
            &[(100, vec![obj1]), (200, vec![obj2])],
            &[(100, vec![obj1]), (150, vec![obj2])],
        );
        // Should suggest the highest lowest success price
        assert_eq!(
            tracker.get_suggested_gas_price_for_objects(vec![obj1, obj2].into_iter()),
            Some(150)
        );
    }
}
