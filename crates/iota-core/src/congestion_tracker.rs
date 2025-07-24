// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashMap, hash_map::Entry},
    sync::{Arc, RwLock},
};

use iota_common::{debug_fatal, fatal};
use iota_types::{
    base_types::ObjectID,
    effects::{InputSharedObject, TransactionEffects, TransactionEffectsAPI},
    execution_status::CongestedObjects,
    messages_checkpoint::{CheckpointTimestamp, VerifiedCheckpoint},
    transaction::{TransactionData, TransactionDataAPI},
};
use moka::{ops::compute::Op, sync::Cache};
use reqwest::Client;
use tracing::info;

use crate::execution_cache::TransactionCacheRead;

/// Holds congestion-related information for a single object
#[derive(Clone, Copy, Debug)]
pub struct CongestionInfo {
    pub last_cancellation_time: CheckpointTimestamp,
    pub highest_cancelled_gas_price: u64,
    pub last_success_time: Option<CheckpointTimestamp>,
    pub lowest_executed_gas_price: Option<u64>,
}

impl CongestionInfo {
    /// Updates this object with newer congestion info from a newer checkpoint
    fn update_for_new_checkpoint(&mut self, new: &CongestionInfo) {
        if new.last_cancellation_time > self.last_cancellation_time {
            self.last_cancellation_time = new.last_cancellation_time;
            self.highest_cancelled_gas_price = new.highest_cancelled_gas_price;
        }
        if new.last_success_time > self.last_success_time {
            self.last_success_time = new.last_success_time;
            self.lowest_executed_gas_price = new.lowest_executed_gas_price;
        }
    }

    /// Records a cancellation event at a given checkpoint
    fn update_for_cancellation(&mut self, now: CheckpointTimestamp, gas_price: u64) {
        self.last_cancellation_time = now;
        self.highest_cancelled_gas_price = std::cmp::max(self.highest_cancelled_gas_price, gas_price);
    }

    /// Records a successful execution at a given checkpoint
    fn update_for_success(&mut self, now: CheckpointTimestamp, gas_price: u64) {
        self.last_success_time = Some(now);
        self.lowest_executed_gas_price = Some(match self.lowest_executed_gas_price {
            Some(current_min) => std::cmp::min(current_min, gas_price),
            None => gas_price,
        });
    }
}

/// Main congestion tracker responsible for recording and serving congestion data
pub struct CongestionTracker {
    pub congestion_clearing_prices: Cache<ObjectID, CongestionInfo>,
    object_to_index: RwLock<BTreeMap<ObjectID, u64>>, // Maps object to feature vector index
    next_index: RwLock<u64>, // Counter for next available index
}

impl Default for CongestionTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl CongestionTracker {
    /// Creates a new congestion tracker instance
    pub fn new() -> Self {
        Self {
            congestion_clearing_prices: Cache::new(10_000),
            object_to_index: RwLock::new(BTreeMap::new()),
            next_index: RwLock::new(0),
        }
    }

    /// Processes a checkpoint by collecting features and sending a training batch to the model server
    pub async fn process_checkpoint_effects(
        &self,
        transaction_cache_reader: &dyn TransactionCacheRead,
        checkpoint: &VerifiedCheckpoint,
        effects: &[TransactionEffects],
    ) {
        let mut training_data = Vec::new();

        for effect in effects {
            // Extract the gas price from the transaction
            let gas_price = transaction_cache_reader
                .get_transaction_block(effect.transaction_digest())
                .unwrap()
                .unwrap()
                .transaction_data()
                .gas_price();

            // Collect all mutated shared object IDs for this transaction
            let object_ids: Vec<ObjectID> = effect
                .input_shared_objects()
                .iter()
                .filter_map(|obj| match obj {
                    InputSharedObject::Mutate((id, _, _)) => Some(*id),
                    _ => None,
                })
                .collect();

            // Build feature vector from congestion info of each object
            let mut features = vec![];
            for object_id in &object_ids {
                if let Some(info) = self.get_congestion_info(*object_id) {
                    features.push(serde_json::json!({
                        "tx_count": 1, // Placeholder — currently hardcoded
                        "cancel_count": if info.last_cancellation_time >= checkpoint.timestamp_ms { 1 } else { 0 },
                        "avg_gas_price": info.highest_cancelled_gas_price,
                    }));
                }
            }

            // Add to training batch if we have any features
            if !features.is_empty() {
                training_data.push(serde_json::json!({
                    "features": features,
                    "label": gas_price,
                }));
            }
        }

        // Send batch to model server if we collected any training points
        if !training_data.is_empty() {
            let client = Client::new();
            let payload = serde_json::json!({ "batch": training_data });

            let res = client
                .post("http://localhost:8000/batch_train")
                .json(&payload)
                .send()
                .await;

            match res {
                Ok(r) => info!("Sent training batch, response: {:?}", r),
                Err(e) => info!("Failed to send training batch: {:?}", e),
            }
        }
    }

    /// Queries the model server to get a predicted gas price for the given transaction
    pub async fn get_suggested_gas_price(&self, tx: &TransactionData) -> Option<u64> {
        // Extract mutable object IDs as strings
        let object_ids: Vec<String> = tx
            .shared_input_objects()
            .into_iter()
            .filter(|o| o.mutable)
            .map(|o| o.id.to_string())
            .collect();

        let client = Client::new();

        let response = client
            .post("http://localhost:8000/predict")
            .json(&serde_json::json!({"objects": object_ids}))
            .send()
            .await
            .ok()?;

        let json: serde_json::Value = response.json().await.ok()?;
        let score = json.get("suggested_gas_price")?.as_u64()?;
        Some(score)
    }

    /// Retrieves or assigns a unique index to a given object (used for vectorization)
    fn get_or_assign_index(&self, object: ObjectID) -> u64 {
        let mut map = self.object_to_index.write().unwrap();
        if let Some(&idx) = map.get(&object) {
            idx
        } else {
            let mut next_idx = self.next_index.write().unwrap();
            let idx = *next_idx;
            map.insert(object, idx);
            *next_idx += 1;
            idx
        }
    }

    /// Retrieves cached congestion info for a given object, if it exists
    fn get_congestion_info(&self, object_id: ObjectID) -> Option<CongestionInfo> {
        self.congestion_clearing_prices.get(&object_id)
    }
}
