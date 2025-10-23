// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{HashMap, hash_map::Entry},
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
};

use iota_types::{
    base_types::ObjectID,
    digests::TransactionDigest,
    effects::{InputSharedObject, TransactionEffects, TransactionEffectsAPI},
    execution_status::CongestedObjects,
    messages_checkpoint::{CheckpointTimestamp, VerifiedCheckpoint},
    transaction::{TransactionData, TransactionDataAPI},
};
use moka::{ops::compute::Op, sync::Cache};
use serde::Deserialize;
use crate::model_updater::{
    build_cp_update_batch, build_train_tx_batch, HttpModelUpdater, ObjectCheckpointStats,
    ObjectSnapshot, RawTxItem, ModelUpdater,
};
use tracing::info;

use crate::execution_cache::TransactionCacheRead;

/// Capacity of the congestion tracker's cache.
const CONGESTION_TRACKER_CACHE_CAPACITY: u64 = 10_000;

/// Threshold for hotness below which an object is considered cold.
/// Values should be > 0.0. If HOTNESS_CUTOFF = 0.0, then no pruning will
/// happen.
const HOTNESS_CUTOFF: f64 = 1.0;

/// Controls how quickly congestion tracker updates object hotness.
/// Values should be > 0.0. Higher values mean faster adjustments.
const HOTNESS_ADJUSTMENT_FACTOR: f64 = 1.0;

/// Controls how quickly hotness decays for objects not seen in congestion.
/// Values should be >= 1.0: set to > 1.0 for decay, or 1.0 for no decay.
const MAX_DECAY_FACTOR: f64 = 2.0;

/// Alias for type holding congestion info per checkpoint.
type CongestionInfoMap = HashMap<ObjectID, CongestionInfo>;

/// Struct to hold data about a given transaction
struct TxData {
    checkpoint: CheckpointTimestamp,
    digest: TransactionDigest,
    objects: Vec<ObjectID>,
    gas_price: u64,
    gas_price_feedback: Option<u64>,
    sui_prediction: u64,
    ogd_prediction: u64,
    cleread: bool,
}

/// Holds tracked per-object congestion info.
#[derive(Clone, Copy, Debug, Deserialize, serde::Serialize)]
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
    /// successful execution. Values should be >= 0.0.
    hotness: f64,
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

    fn update_hotness(&mut self, new: &CongestionInfo, number_transactions: usize, is_new: bool) {
        if number_transactions > 0 {
            // Compute hotness adjustment
            let hotness_adjustment =
                new.hotness * HOTNESS_ADJUSTMENT_FACTOR / number_transactions as f64;

            // Apply hotness change depending on whether the object is new
            let updated_hotness = if is_new {
                -hotness_adjustment
            } else {
                (self.hotness - hotness_adjustment).max(self.hotness / MAX_DECAY_FACTOR)
            };

            // Ensure hotness is non-negative
            self.hotness = updated_hotness.max(0.0);
        }
    }

    /// Update the highest gas price and the latest time with the data from a
    /// congested transaction.
    fn update_for_congested_tx(&mut self, time: CheckpointTimestamp, gas_price: u64) {
        self.latest_congestion_time = time;
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
    reference_gas_price: u64,
    /// Key-value cache for storing congestion info of objects.
    object_congestion_info: Cache<ObjectID, CongestionInfo>,
    /// HTTP client for posting model updates/training batches.
    model_updater: HttpModelUpdater,
}

impl CongestionTracker {
    /// Compose and send model update and training batches for the given checkpoint.
    fn inform_model(
        &self,
        checkpoint: &VerifiedCheckpoint,
        congestion_txs_data: &[TxData],
        clearing_txs_data: &[TxData],
    ) {
        // 1) Build touched set
        let mut touched: std::collections::HashSet<ObjectID> = std::collections::HashSet::new();
        for tx in congestion_txs_data {
            touched.extend(tx.objects.iter().cloned());
        }
        for tx in clearing_txs_data {
            touched.extend(tx.objects.iter().cloned());
        }

        // 2) Build snapshots from current cache
        let mut snapshots: HashMap<ObjectID, ObjectSnapshot> = HashMap::new();
        for oid in &touched {
            if let Some(info) = self.get_congestion_info(*oid) {
                snapshots.insert(
                    *oid,
                    ObjectSnapshot {
                        latest_congestion_time: Some(info.latest_congestion_time),
                        highest_congestion_gas_price: info.highest_congestion_gas_price,
                        latest_clearing_time: info.latest_clearing_time,
                        lowest_clearing_gas_price: info.lowest_clearing_gas_price.unwrap_or(0),
                        hotness: info.hotness,
                    },
                );
            }
        }

        // 3) Build per-checkpoint stats
        let mut stats: HashMap<ObjectID, ObjectCheckpointStats> = HashMap::new();
        for tx in congestion_txs_data {
            for oid in &tx.objects {
                let entry = stats.entry(*oid).or_default();
                entry.was_congested = true;
                entry.congested_tx_count += 1;
            }
        }
        for tx in clearing_txs_data {
            for oid in &tx.objects {
                let entry = stats.entry(*oid).or_default();
                entry.was_cleared = true;
                entry.clearing_tx_count += 1;
            }
        }

        // 4) Post update batch
        let update_batch = build_cp_update_batch(
            checkpoint.timestamp_ms,
            self.reference_gas_price,
            touched.iter().cloned(),
            &snapshots,
            &stats,
        );
        self.model_updater.post_update(update_batch);

        // 5) Build raw tx items and per-object min clearing for training
        let mut raw_txs: Vec<RawTxItem> = Vec::with_capacity(
            congestion_txs_data.len() + clearing_txs_data.len(),
        );
        let mut per_obj_min_clearing: HashMap<ObjectID, u64> = HashMap::new();
        for tx in congestion_txs_data {
            raw_txs.push(RawTxItem {
                tx_digest: tx.digest.to_string(),
                is_congested: true,
                gas_price: tx.gas_price,
                gas_price_feedback: tx.gas_price_feedback,
                touched_objects: tx.objects.clone(),
            });
        }
        for tx in clearing_txs_data {
            raw_txs.push(RawTxItem {
                tx_digest: tx.digest.to_string(),
                is_congested: false,
                gas_price: tx.gas_price,
                gas_price_feedback: None,
                touched_objects: tx.objects.clone(),
            });
            for oid in &tx.objects {
                per_obj_min_clearing
                    .entry(*oid)
                    .and_modify(|m| *m = (*m).min(tx.gas_price))
                    .or_insert(tx.gas_price);
            }
        }
        if let Some(train_batch) = build_train_tx_batch(
            checkpoint.timestamp_ms,
            self.reference_gas_price,
            &raw_txs,
            &per_obj_min_clearing,
        ) {
            self.model_updater.post_train_tx(train_batch);
        }
    }
    /// Create a new `CongestionTracker`. The cache capacity will be
    /// set to `CONGESTION_TRACKER_CACHE_CAPACITY`, which is `10_000`.
    pub fn new(reference_gas_price: u64) -> Self {
        // Remove and recreate the results folder
        let mut results_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        results_path.push("src");
        results_path.push("results");

        if results_path.exists() {
            let _ = std::fs::remove_dir_all(&results_path);
        }
        let _ = std::fs::create_dir_all(&results_path);
        Self {
            reference_gas_price,
            object_congestion_info: Cache::new(CONGESTION_TRACKER_CACHE_CAPACITY),
            model_updater: HttpModelUpdater::default(),
        }
    }

    /// Process effects of all transactions included in a certain checkpoint.
    pub fn process_checkpoint_effects(
        &self,
        transaction_cache_reader: &dyn TransactionCacheRead,
        checkpoint: &VerifiedCheckpoint,
        effects: &[TransactionEffects],
    ) {
        // Containers for checkpoint's congestion and clearing transactions data.
        let mut congestion_txs_data: Vec<TxData> = Vec::with_capacity(effects.len());
        let mut clearing_txs_data: Vec<TxData> = Vec::with_capacity(effects.len());

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

            let block = transaction_cache_reader
                .get_transaction_block(effects.transaction_digest())
                .unwrap_or_else(|| {
                    panic!("block not found in transaction cache");
                });

            let tx_data = block.transaction_data();
            let sui_prediction = self
                .get_prediction_suggested_gas_price(tx_data)
                .unwrap_or(self.reference_gas_price);
            let ogd_prediction = self.get_suggested_gas_price_with_ogd(tx_data);

            // Skip system transactions
            if gas_price == 1 {
                continue;
            }

            if let Some(CongestedObjects(congested_objects)) =
                effects.status().get_congested_objects()
            {
                congestion_txs_data.push(TxData {
                    checkpoint: checkpoint.sequence_number,
                    digest: *effects.transaction_digest(),
                    objects: congested_objects.clone(),
                    gas_price,
                    gas_price_feedback: Some(
                        effects
                            .status()
                            .get_feedback_suggested_gas_price()
                            .unwrap_or(self.reference_gas_price),
                    ),
                    sui_prediction: sui_prediction,
                    ogd_prediction: ogd_prediction,
                    cleread: false,
                });
            } else {
                let mutated_objects: Vec<ObjectID> = effects
                    .input_shared_objects()
                    .into_iter()
                    .filter_map(|object| match object {
                        InputSharedObject::Mutate((id, _, _)) => Some(id),
                        _ => None,
                    })
                    .collect();

                // Only push to clearing_txs_data if there are mutated objects
                if !mutated_objects.is_empty() {
                    clearing_txs_data.push(TxData {
                        checkpoint: checkpoint.sequence_number,
                        digest: *effects.transaction_digest(),
                        objects: mutated_objects,
                        gas_price,
                        gas_price_feedback: None,
                        sui_prediction,
                        ogd_prediction,
                        cleread: true,
                    });
                }
            }
        }

        self.process_congestion_and_clearing_txs_data(
            checkpoint.timestamp_ms,
            &congestion_txs_data,
            &clearing_txs_data,
        );

        if !clearing_txs_data.is_empty() {
            for tx in &mut clearing_txs_data {
                let block = transaction_cache_reader
                    .get_transaction_block(&tx.digest)
                    .unwrap_or_else(|| {
                        panic!("block not found in transaction cache");
                    });

                let tx_data = block.transaction_data();

                tx.gas_price_feedback = self.get_prediction_suggested_gas_price(tx_data);
                info!(
                    "Clearing tx gas price feedback updated: {:?}",
                    tx.gas_price_feedback
                );
            }
        }

        for tx in congestion_txs_data.iter().chain(clearing_txs_data.iter()) {
            info!(
                "Checkpoint: {} | Digest: {:?} | Gas price: {} | Feedback: {} | Prediction (Sui): {:?} | Prediction (IOTA): {:?} |  Cleared: {}",
                tx.checkpoint,
                tx.digest,
                tx.gas_price,
                tx.gas_price_feedback.unwrap_or(1000),
                tx.sui_prediction,
                tx.ogd_prediction,
                tx.cleread,
            );

            let _ = self.dump_prediction_to_csv(
                "prediction.csv",
                tx.checkpoint,
                &tx.digest,
                tx.gas_price,
                tx.gas_price_feedback.unwrap_or(1000),
                tx.sui_prediction,
                tx.ogd_prediction,
                tx.cleread,
            );
        }

        // Inform model right before dumping hotness CSV
        self.inform_model(checkpoint, &congestion_txs_data, &clearing_txs_data);

        if !self.get_all_hotness().is_empty() {
            info!(
                "Hotness after checkpoint {}: {:?}",
                checkpoint.sequence_number,
                self.get_all_hotness()
            );
            for object in self.object_congestion_info.iter() {
                self.dump_hotness_to_csv(
                    "./hotness.csv",
                    checkpoint.sequence_number,
                    *object.0,
                    object.1.hotness,
                )
                .unwrap();
            }
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

    /// Get the largest hotness value among all mutable input shared objects
    /// accessed by `transaction`.
    pub fn get_suggested_gas_price_with_ogd(&self, transaction: &TransactionData) -> u64 {
        let (_, hotness) = self
            .get_max_hotness_per_tx(
                transaction
                    .shared_input_objects()
                    .into_iter()
                    .filter(|id| id.mutable)
                    .map(|id| id.id),
            )
            .unwrap_or((ObjectID::random(), 0.0));

        self.reference_gas_price + hotness as u64
    }

    /// Returns a map of all objects and their hotness values.
    pub fn get_all_hotness(&self) -> HashMap<ObjectID, f64> {
        self.object_congestion_info
            .iter()
            .map(|entry| (*entry.0, entry.1.hotness))
            .collect()
    }

    /// Returns the hotness of a specific object, if it exists.
    pub fn get_hotness_for_object(&self, object_id: &ObjectID) -> Option<f64> {
        self.object_congestion_info
            .get(object_id)
            .map(|info| info.hotness)
    }
}

impl CongestionTracker {
    fn dump_prediction_to_csv(
        &self,
        file_name: &str,
        checkpoint: u64,
        digest: &TransactionDigest,
        gas_price: u64,
        gas_price_feedback: u64,
        prediction_sui: u64,
        prediction_ogd: u64,
        cleared: bool,
    ) -> std::io::Result<()> {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("src");
        path.push("results");
        path.push(file_name);

        // Ensure directory exists
        std::fs::create_dir_all(path.parent().unwrap())?;
        let file_exists = path.exists();

        // Open file for appending
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;

        // Write header if the file is new
        if !file_exists {
            writeln!(file, "checkpoint,digest,gasprice,feedback,sui,ogd,cleared")?;
        }

        // Build row
        let row = format!(
            "{},{},{},{},{},{},{}",
            checkpoint,
            digest,
            gas_price,
            gas_price_feedback,
            prediction_sui,
            prediction_ogd,
            cleared
        );

        // Column-count check
        if row.split(',').count() != 7 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Malformed row: {}", row),
            ));
        }

        // Write row and flush immediately
        writeln!(file, "{}", row)?;
        file.flush()?;

        Ok(())
    }

    fn dump_hotness_to_csv(
        &self,
        file_name: &str,
        checkpoint: u64,
        object_id: ObjectID,
        hotness: f64,
    ) -> std::io::Result<()> {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("src");
        path.push("results");
        path.push(file_name);

        std::fs::create_dir_all(path.parent().unwrap())?;
        let file_exists = path.exists();

        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;

        if !file_exists {
            writeln!(file, "checkpoint,object,hotness")?;
        }

        // Build row
        let row = format!("{},{},{}", checkpoint, object_id, hotness);

        // Ensure exactly 3 columns
        if row.split(',').count() != 3 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Malformed row: {}", row),
            ));
        }

        writeln!(file, "{}", row)?;
        file.flush()?; // ensure it hits disk immediately
        Ok(())
    }

    /// Process checkpoint's congestion and clearing transactions info.
    fn process_congestion_and_clearing_txs_data(
        &self,
        time: CheckpointTimestamp,
        congestion_txs_data: &[TxData],
        clearing_txs_data: &[TxData],
    ) {
        let congestion_info_map = self.compute_per_checkpoint_congestion_info(
            time,
            congestion_txs_data,
            clearing_txs_data,
        );
        self.update_congestion_info_cache(
            congestion_info_map,
            congestion_txs_data.len() + clearing_txs_data.len(),
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

    fn get_max_hotness_per_tx(
        &self,
        mut objects: impl Iterator<Item = ObjectID>,
    ) -> Option<(ObjectID, f64)> {
        // Initialize with the first object (or return None if empty)
        let first = objects.next()?;
        let first_hotness = self
            .get_congestion_info(first)
            .map(|info| info.hotness)
            .unwrap_or(0.0);

        let mut best = (first, first_hotness);

        // Iterate through the rest
        for object_id in objects {
            let hotness = self
                .get_congestion_info(object_id)
                .map(|info| info.hotness)
                .unwrap_or(0.0);

            if hotness > best.1 {
                best = (object_id, hotness);
            }
        }

        Some(best)
    }

    fn compute_per_checkpoint_congestion_info(
        &self,
        time: CheckpointTimestamp,
        congestion_txs_data: &[TxData],
        clearing_txs_data: &[TxData],
    ) -> CongestionInfoMap {
        let mut congestion_info_map = CongestionInfoMap::new();
        let mut objects_with_mutated_hotness: Vec<ObjectID> = Vec::new();

        for TxData {
            checkpoint: _,
            digest: _,
            objects,
            gas_price,
            gas_price_feedback,
            ..
        } in congestion_txs_data
        {
            // Get the object with the maximum hotness among all objects in the transaction.
            let (max_object_id, max_hotness_per_tx) = self
                .get_max_hotness_per_tx(objects.iter().cloned())
                .unwrap_or((ObjectID::random(), 0.0));

            objects
                .iter()
                .for_each(|object_id| match congestion_info_map.entry(*object_id) {
                    Entry::Occupied(entry) => {
                        entry.into_mut().update_for_congested_tx(time, *gas_price);
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
                });

            // Adjust hotness based on the loss function comparing prediction (maximum
            // hotness of objects in the transaction + reference gas price) and actual gas
            // price feedback.
            let hotness_adjustment = max_hotness_per_tx + (self.reference_gas_price as f64)
                - (gas_price_feedback.unwrap_or(self.reference_gas_price) as f64);

            congestion_info_map
                .entry(max_object_id)
                .and_modify(|info| info.hotness += hotness_adjustment);

            objects_with_mutated_hotness.push(max_object_id);
        }

        for TxData {
            objects, gas_price, ..
        } in clearing_txs_data
        {
            objects.iter().for_each(|object_id| {
                // We only record clearing prices if the object has observed cancellations
                // recently
                match congestion_info_map.entry(*object_id) {
                    Entry::Occupied(entry) => {
                        entry.into_mut().update_for_clearing_tx(time, *gas_price);
                    }
                    Entry::Vacant(entry) => {
                        if let Some(prev) = self.get_congestion_info(*object_id) {
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
            });
        }

        for TxData { objects, .. } in clearing_txs_data {
            // Get the object with the maximum hotness among all objects in the transaction.
            let (max_object_id, max_hotness_per_tx) = self
                .get_max_hotness_per_tx(objects.iter().cloned())
                .unwrap_or((ObjectID::random(), 0.0));

            if let Some(info) = congestion_info_map.get(&max_object_id) {
                // Adjust hotness based on the loss function comparing prediction (maximum
                // hotness of objects in the transaction + reference gas price) and lowest
                // clearing gas price per object.
                let hotness_adjustment = max_hotness_per_tx + (self.reference_gas_price as f64)
                    - info
                        .lowest_clearing_gas_price
                        .unwrap_or(self.reference_gas_price) as f64;
                congestion_info_map
                    .entry(max_object_id)
                    .and_modify(|info| info.hotness += hotness_adjustment);

                objects_with_mutated_hotness.push(max_object_id);
            }
        }

        // Objects that were not updated in this checkpoint will decay as much as
        // possible, bounded by `MAX_DECAY_FACTOR` (see `update_hotness` function).
        for (object_id, info) in congestion_info_map.iter_mut() {
            if !objects_with_mutated_hotness.contains(object_id) {
                info.hotness = f64::MAX;
            }
        }

        congestion_info_map
    }

    fn update_congestion_info_cache(
        &self,
        congestion_info_map: CongestionInfoMap,
        number_transactions: usize,
    ) {
        // Store the object IDs that are touched in this checkpoint
        let touched_objects: std::collections::HashSet<_> =
            congestion_info_map.keys().cloned().collect();

        for (object_id, info) in congestion_info_map {
            self.object_congestion_info
                .entry(object_id)
                .and_compute_with(|maybe_entry| {
                    if let Some(e) = maybe_entry {
                        let mut e = e.into_value();
                        e.update_with_new_congestion_info(&info);
                        e.update_hotness(&info, number_transactions, false);
                        Op::Put(e)
                    } else {
                        let mut new_info = info;
                        new_info.update_hotness(&info, number_transactions, true);
                        Op::Put(new_info)
                    }
                });
        }

        // Decay hotness of untouched objects, and prune if too cold
        for (object_id, _) in self.object_congestion_info.iter() {
            if !touched_objects.contains(&object_id) {
                self.object_congestion_info
                    .entry(*object_id)
                    .and_compute_with(|maybe_entry| {
                        if let Some(e) = maybe_entry {
                            let mut e = e.into_value();
                            e.hotness /= MAX_DECAY_FACTOR;
                            if e.hotness < HOTNESS_CUTOFF {
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
    fn congestion_tracker_process_checkpoint_txs_data() {
        let rgp_test = 1000;
        let tracker = CongestionTracker::new(rgp_test);
        let object_1 = ObjectID::random();
        let object_2 = ObjectID::random();

        let time = 1_000;
        let congestion_txs_data = vec![
            TxData {
                checkpoint: time,
                digest: TransactionDigest::random(),
                objects: vec![object_1],
                gas_price: 100,
                gas_price_feedback: Some(1000),
                sui_prediction: 1000,
                ogd_prediction: 1000,
                cleread: false,
            },
            TxData {
                checkpoint: time,
                digest: TransactionDigest::random(),
                objects: vec![object_2],
                gas_price: 200,
                gas_price_feedback: Some(1000),
                sui_prediction: 1000,
                ogd_prediction: 1000,
                cleread: false,
            },
        ];
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
    fn congestion_tracker_process_checkpoint_data_then_success() {
        let rgp_test = 1000;
        let tracker = CongestionTracker::new(rgp_test);
        let object = ObjectID::random();

        // Congestion transactions only, no clearing ones. The highest congestion
        // gas price should be used.
        let time = 1_000;
        let congestion_txs_data = vec![
            TxData {
                checkpoint: time,
                digest: TransactionDigest::random(),
                gas_price: 100,
                objects: vec![object],
                gas_price_feedback: Some(1000),
                sui_prediction: 1000,
                ogd_prediction: 1000,
                cleread: false,
            },
            TxData {
                checkpoint: time,
                digest: TransactionDigest::random(),
                gas_price: 75,
                objects: vec![object],
                gas_price_feedback: Some(1000),
                sui_prediction: 1000,
                ogd_prediction: 1000,
                cleread: false,
            },
        ];
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
        let clearing_txs_data = vec![TxData {
            checkpoint: time,
            digest: TransactionDigest::random(),
            objects: vec![object],
            gas_price: 150,
            gas_price_feedback: None,
            sui_prediction: 1000,
            ogd_prediction: 1000,
            cleread: false,
        }];
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
        let congestion_txs_data = vec![TxData {
            objects: vec![object],
            gas_price: 100,
            gas_price_feedback: Some(1000),
            checkpoint: time,
            digest: TransactionDigest::random(),
            sui_prediction: 1000,
            ogd_prediction: 1000,
            cleread: false,
        }];
        let clearing_txs_data = vec![
            TxData {
                objects: vec![object],
                gas_price: 175,
                gas_price_feedback: None,
                checkpoint: time,
                digest: TransactionDigest::random(),
                sui_prediction: 1000,
                ogd_prediction: 1000,
                cleread: false,
            },
            TxData {
                objects: vec![object],
                gas_price: 125,
                gas_price_feedback: None,
                checkpoint: time,
                digest: TransactionDigest::random(),
                sui_prediction: 1000,
                ogd_prediction: 1000,
                cleread: false,
            },
        ];
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
    fn congestion_tracker_get_suggested_gas_price_for_multiple_objects() {
        let rgp_test = 1000;
        let tracker = CongestionTracker::new(rgp_test);
        let object_1 = ObjectID::random();
        let object_2 = ObjectID::random();

        let time = 1_000;
        let congestion_txs_data = vec![
            TxData {
                objects: vec![object_1],
                gas_price: 100,
                gas_price_feedback: Some(1000),
                checkpoint: time,
                digest: TransactionDigest::random(),
                sui_prediction: 1000,
                ogd_prediction: 1000,
                cleread: false,
            },
            TxData {
                objects: vec![object_2],
                gas_price: 200,
                gas_price_feedback: Some(1000),
                checkpoint: time,
                digest: TransactionDigest::random(),
                sui_prediction: 1000,
                ogd_prediction: 1000,
                cleread: false,
            },
        ];
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
        let congestion_txs_data = vec![
            TxData {
                objects: vec![object_1],
                gas_price: 100,
                gas_price_feedback: Some(1000),
                checkpoint: time,
                digest: TransactionDigest::random(),
                sui_prediction: 1000,
                ogd_prediction: 1000,
                cleread: false,
            },
            TxData {
                objects: vec![object_2],
                gas_price: 200,
                gas_price_feedback: Some(1000),
                checkpoint: time,
                digest: TransactionDigest::random(),
                sui_prediction: 1000,
                ogd_prediction: 1000,
                cleread: false,
            },
        ];
        let clearing_txs_data = vec![
            TxData {
                objects: vec![object_1],
                gas_price: 100,
                gas_price_feedback: None,
                checkpoint: time,
                digest: TransactionDigest::random(),
                sui_prediction: 1000,
                ogd_prediction: 1000,
                cleread: false,
            },
            TxData {
                objects: vec![object_2],
                gas_price: 150,
                gas_price_feedback: None,
                checkpoint: time,
                digest: TransactionDigest::random(),
                sui_prediction: 1000,
                ogd_prediction: 1000,
                cleread: false,
            },
        ];
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
    fn congestion_tracker_checkpoint_congestion_info_hotness_update() {
        let rgp_test = 1000;
        let tracker = CongestionTracker::new(rgp_test);
        let obj1 = ObjectID::random();
        let obj2 = ObjectID::random();
        let obj3 = ObjectID::random();

        let now = 1000;

        // Congestion events: all objects are congested with different gas price
        // feedback
        let congestion_events = vec![
            TxData {
                objects: vec![obj1],
                gas_price: 1000,
                gas_price_feedback: Some(1050),
                checkpoint: now,
                digest: TransactionDigest::random(),
                sui_prediction: 1000,
                ogd_prediction: 1000,
                cleread: false,
            },
            TxData {
                objects: vec![obj2],
                gas_price: 1000,
                gas_price_feedback: Some(1200),
                checkpoint: now,
                digest: TransactionDigest::random(),
                sui_prediction: 1000,
                ogd_prediction: 1000,
                cleread: false,
            },
            TxData {
                objects: vec![obj2],
                gas_price: 1000,
                gas_price_feedback: Some(1200),
                checkpoint: now,
                digest: TransactionDigest::random(),
                sui_prediction: 1000,
                ogd_prediction: 1000,
                cleread: false,
            },
            TxData {
                objects: vec![obj2, obj3],
                gas_price: 1000,
                checkpoint: now,
                digest: TransactionDigest::random(),
                sui_prediction: 1000,
                ogd_prediction: 1000,
                gas_price_feedback: Some(1600),
                cleread: false,
            },
        ];
        let cleared_events = vec![
            TxData {
                objects: vec![obj1],
                gas_price: 1080,
                gas_price_feedback: None,
                checkpoint: now,
                digest: TransactionDigest::random(),
                sui_prediction: 1000,
                ogd_prediction: 1000,
                cleread: true,
            },
            TxData {
                objects: vec![obj2],
                gas_price: 1300,
                gas_price_feedback: None,
                checkpoint: now,
                digest: TransactionDigest::random(),
                sui_prediction: 1000,
                ogd_prediction: 1000,
                cleread: true,
            },
            TxData {
                objects: vec![obj2],
                gas_price: 1400,
                gas_price_feedback: None,
                checkpoint: now,
                digest: TransactionDigest::random(),
                sui_prediction: 1000,
                ogd_prediction: 1000,
                cleread: true,
            },
            TxData {
                objects: vec![obj2, obj3],
                gas_price: 1200,
                gas_price_feedback: None,
                checkpoint: now,
                digest: TransactionDigest::random(),
                sui_prediction: 1000,
                ogd_prediction: 1000,
                cleread: true,
            },
        ];

        tracker.process_congestion_and_clearing_txs_data(now, &congestion_events, &cleared_events);

        assert!(
            tracker.get_hotness_for_object(&obj1).unwrap() == HOTNESS_ADJUSTMENT_FACTOR * 16.25,
            "obj1 should have positive hotness"
        );
        assert!(
            tracker.get_hotness_for_object(&obj2).unwrap() == HOTNESS_ADJUSTMENT_FACTOR * 200.0,
            "obj2 should have positive hotness"
        );
        // obj3 is included in transactions with obj2, which gets all hotness updates.
        // Remember that only the object with the largest hotness in a transaction gets
        // its hotness updated. If hotness is equal, than the first object in the
        // transaction gets updated.
        assert!(
            tracker.get_hotness_for_object(&obj3).unwrap() == 0.0,
            "obj3 should have 0 hotness"
        );
    }

    #[test]
    fn congestion_tracker_repeated_congestion_across_checkpoints() {
        let rgp_test = 1000;
        let tracker = CongestionTracker::new(rgp_test);
        let obj1 = ObjectID::random();
        let obj2 = ObjectID::random();

        // First checkpoint
        tracker.process_congestion_and_clearing_txs_data(
            1000,
            &[TxData {
                objects: vec![obj1],
                gas_price: 100,
                gas_price_feedback: Some(1500),
                checkpoint: 1000,
                digest: TransactionDigest::random(),
                sui_prediction: 1000,
                ogd_prediction: 1000,
                cleread: false,
            }],
            &[TxData {
                objects: vec![obj1],
                gas_price: 1600,
                gas_price_feedback: None,
                checkpoint: 1000,
                digest: TransactionDigest::random(),
                sui_prediction: 1000,
                ogd_prediction: 1000,
                cleread: true,
            }],
        );

        // Second checkpoint, touches same object and new one
        tracker.process_congestion_and_clearing_txs_data(
            1100,
            &[TxData {
                objects: vec![obj1, obj2],
                gas_price: 100,
                gas_price_feedback: Some(1700),
                checkpoint: 1100,
                digest: TransactionDigest::random(),
                sui_prediction: 1000,
                ogd_prediction: 1000,
                cleread: false,
            }],
            &[TxData {
                objects: vec![obj1, obj2],
                gas_price: 1800,
                gas_price_feedback: None,
                checkpoint: 1000,
                digest: TransactionDigest::random(),
                sui_prediction: 1000,
                ogd_prediction: 1000,
                cleread: true,
            }],
        );

        let hotness1 = tracker.get_hotness_for_object(&obj1).unwrap_or(0.0);
        let hotness2 = tracker.get_hotness_for_object(&obj2).unwrap_or(0.0);
        assert!(hotness1 == 750.0, "hotness for obj1 should be 750.0");
        assert!(hotness2 == 0.0, "hotness for obj2 should be 0.0");

        // Additional checkpoints
        tracker.process_congestion_and_clearing_txs_data(1200, &[], &[]);
        tracker.process_congestion_and_clearing_txs_data(
            1300,
            &[TxData {
                objects: vec![obj2],
                gas_price: 100,
                gas_price_feedback: Some(1050),
                checkpoint: 1000,
                digest: TransactionDigest::random(),
                sui_prediction: 1000,
                ogd_prediction: 1000,
                cleread: false,
            }],
            &[TxData {
                objects: vec![obj1],
                gas_price: 1100,
                gas_price_feedback: None,
                checkpoint: 1000,
                digest: TransactionDigest::random(),
                sui_prediction: 1000,
                ogd_prediction: 1000,
                cleread: true,
            }],
        );
        tracker.process_congestion_and_clearing_txs_data(1400, &[], &[]);
        tracker.process_congestion_and_clearing_txs_data(
            1500,
            &[
                TxData {
                    objects: vec![obj1, obj2],
                    gas_price: 100,
                    gas_price_feedback: Some(1150),
                    checkpoint: 1000,
                    digest: TransactionDigest::random(),
                    sui_prediction: 1000,
                    ogd_prediction: 1000,
                    cleread: false,
                },
                TxData {
                    objects: vec![obj1],
                    gas_price: 100,
                    gas_price_feedback: Some(1020),
                    checkpoint: 1000,
                    digest: TransactionDigest::random(),
                    sui_prediction: 1000,
                    ogd_prediction: 1000,
                    cleread: false,
                },
            ],
            &[TxData {
                objects: vec![obj1],
                gas_price: 1100,
                gas_price_feedback: None,
                checkpoint: 1000,
                digest: TransactionDigest::random(),
                sui_prediction: 1000,
                ogd_prediction: 1000,
                cleread: true,
            }],
        );

        let hotness1 = tracker.get_hotness_for_object(&obj1).unwrap_or(0.0);
        let hotness2 = tracker.get_hotness_for_object(&obj2).unwrap_or(0.0);
        assert!(hotness1 == 90.0, "hotness for obj1 should be approx 90.5");
        assert!(hotness2 == 6.25, "hotness for obj2 should be approx 25");
    }

    #[test]
    fn congestion_tracker_remove_cold_objects_from_cache() {
        let rgp_test = 1000;
        let tracker = CongestionTracker::new(rgp_test);
        let obj1 = ObjectID::random();
        let obj2 = ObjectID::random();

        // First checkpoint with two congested objects
        tracker.process_congestion_and_clearing_txs_data(
            1000,
            &[TxData {
                objects: vec![obj1, obj2],
                gas_price: 100,
                gas_price_feedback: Some(1010),
                checkpoint: 1000,
                digest: TransactionDigest::random(),
                sui_prediction: 1000,
                ogd_prediction: 1000,
                cleread: false,
            }],
            &[],
        );

        // obj1 is not congested anymore. Its hotness is halved for every checkpoint
        // where it is not in the congested object set.
        tracker.process_congestion_and_clearing_txs_data(
            1100,
            &[TxData {
                objects: vec![obj2],
                gas_price: 100,
                gas_price_feedback: Some(1007),
                checkpoint: 1000,
                digest: TransactionDigest::random(),
                sui_prediction: 1000,
                ogd_prediction: 1000,
                cleread: false,
            }],
            &[],
        );
        tracker.process_congestion_and_clearing_txs_data(1200, &[], &[]);
        tracker.process_congestion_and_clearing_txs_data(1300, &[], &[]);
        tracker.process_congestion_and_clearing_txs_data(1400, &[], &[]);

        // hotness for obj1 goes below 1.0 so it should be removed from cache
        assert!(
            tracker.get_hotness_for_object(&obj1).is_none(),
            "obj1 should be removed from cache"
        );
        let hotness = tracker.get_hotness_for_object(&obj2).unwrap_or(0.0);
        println!("hotness for obj2: {}", hotness);
        assert!(
            hotness == 1.25 * HOTNESS_ADJUSTMENT_FACTOR,
            "hotness for obj2 should be 1.25"
        );
    }
}
