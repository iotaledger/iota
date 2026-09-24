// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
    sync::Arc,
};

use iota_config::node::CongestionTrackerConfig;
use iota_metrics::monitored_scope;
use iota_types::{
    base_types::ObjectID,
    digests::TransactionDigest,
    effects::{InputSharedObject, TransactionEffects, TransactionEffectsAPI},
    execution_status::CongestedObjects,
    messages_checkpoint::{CheckpointTimestamp, VerifiedCheckpoint},
    transaction::{TransactionData, TransactionDataAPI},
};
use moka::{ops::compute::Op, sync::Cache};
use prometheus::Registry;
use serde::Deserialize;
use tracing::{debug, info};

#[cfg(feature = "gas-nn")]
use crate::model_updater::{
    InNodeModelUpdater, ModelReader, ModelUpdater, ObjectCheckpointStats, ObjectSnapshot,
    RawTxItem, build_cp_update_batch, build_train_tx_batch,
};
use crate::{
    execution_cache::TransactionCacheRead,
    gas_metrics::{GasMetrics, init_gas_metrics},
};

/// Environment variable overriding the learning rate `ogd_eta`.
pub const ETA_ENV_VAR: &str = "IOTA_OGD_ETA";

/// Environment variable overriding the loss asymmetry `ogd_alpha`.
pub const ALPHA_ENV_VAR: &str = "IOTA_OGD_ALPHA";

/// Parameters controlling congestion tracker behaviour.
///
/// Hotness weights are learned online, once per checkpoint, by gradient
/// descent on an asymmetric squared loss. For every object touched in the
/// checkpoint the checkpoint reveals a target price for that object:
///
/// 1. no transaction touching the object was cancelled: the reference price
///    sufficed;
/// 2. some transaction touching it was cancelled and some other one executed:
///    the lowest executed bid is the clearing price of the object;
/// 3. only cancellations touched it: the lowest gas price feedback among them,
///    an upper bound on the required price that is exact when the object is the
///    binding one;
/// 4. cancellations only and no feedback: the highest cancelled bid is a lower
///    bound, so the weight is only ever raised up to it.
///
/// Each object then takes one step `2 * ogd_eta * residual` toward its own
/// target, where the residual is `(reference price + hotness) - target` and is
/// multiplied by `ogd_alpha` when negative. Within one checkpoint a weight
/// cannot fall below `hotness / max_decay_factor`; increases are not capped.
/// Untouched objects decay by `max_decay_factor` and are evicted below
/// `hotness_cutoff`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CongestionTrackerParams {
    pub cache_capacity: u64,
    pub hotness_cutoff: f64,
    pub max_decay_factor: f64,
    /// Learning rate, in `(0, 0.5]`. `0.5` overwrites the weight with the
    /// latest observation; larger values overshoot the target.
    pub ogd_eta: f64,
    /// Loss asymmetry, `>= 1`: weight of under-estimating the price relative
    /// to over-estimating it. `1` is the symmetric squared loss.
    pub ogd_alpha: f64,
}

impl Default for CongestionTrackerParams {
    fn default() -> Self {
        CongestionTrackerParams::from(&CongestionTrackerConfig::default())
    }
}

impl From<&CongestionTrackerConfig> for CongestionTrackerParams {
    fn from(config: &CongestionTrackerConfig) -> Self {
        Self {
            cache_capacity: config.cache_capacity(),
            hotness_cutoff: config.hotness_cutoff(),
            max_decay_factor: config.max_decay_factor(),
            ogd_eta: config.ogd_eta(),
            ogd_alpha: config.ogd_alpha(),
        }
    }
}

impl CongestionTrackerParams {
    /// Replaces `ogd_eta` and `ogd_alpha` with the values of `IOTA_OGD_ETA`
    /// and `IOTA_OGD_ALPHA` when those are set.
    pub fn with_env_overrides(mut self) -> Result<Self, String> {
        if let Some(eta) = parse_env_f64(ETA_ENV_VAR)? {
            self.ogd_eta = eta;
        }
        if let Some(alpha) = parse_env_f64(ALPHA_ENV_VAR)? {
            self.ogd_alpha = alpha;
        }
        Ok(self)
    }

    /// Validates `0 < ogd_eta <= 0.5` and `ogd_alpha >= 1`.
    pub fn validate(&self) -> Result<(), String> {
        let eta = self.ogd_eta;
        if !(eta.is_finite() && eta > 0.0 && eta <= 0.5) {
            return Err(format!("ogd_eta must be in (0, 0.5], got {eta}"));
        }
        let alpha = self.ogd_alpha;
        if !(alpha.is_finite() && alpha >= 1.0) {
            return Err(format!("ogd_alpha must be >= 1, got {alpha}"));
        }
        Ok(())
    }
}

fn parse_env_f64(name: &str) -> Result<Option<f64>, String> {
    match std::env::var(name) {
        Ok(value) => value
            .trim()
            .parse::<f64>()
            .map(Some)
            .map_err(|e| format!("{name}={value:?} is not a number: {e}")),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(e) => Err(format!("{name}: {e}")),
    }
}

/// Struct to hold data about a given transaction
struct TxData {
    checkpoint: CheckpointTimestamp,
    digest: TransactionDigest,
    /// Congested objects of a cancelled transaction, or the mutated shared
    /// inputs of an executed one.
    objects: Vec<ObjectID>,
    /// All mutable shared inputs of the transaction, for `prediction.csv`.
    mutable_shared_inputs: Vec<ObjectID>,
    gas_price: u64,
    gas_price_feedback: Option<u64>,
    sui_prediction: u64,
    ogd_prediction: u64,
    nn_prediction: u64,
    cleread: bool,
}

/// What one checkpoint reveals about one object.
#[derive(Clone, Copy, Debug, Default)]
struct CheckpointObservation {
    /// Some transaction touching the object was cancelled in this checkpoint.
    congested: bool,
    /// Highest bid among the cancelled transactions touching the object.
    highest_bid: u64,
    /// Lowest gas price feedback among the cancelled transactions touching the
    /// object that carry one.
    min_feedback: Option<u64>,
    /// Lowest bid among the executed transactions touching the object.
    lowest_clearing: Option<u64>,
}

impl CheckpointObservation {
    fn record_congested_tx(&mut self, gas_price: u64, gas_price_feedback: Option<u64>) {
        self.congested = true;
        self.highest_bid = self.highest_bid.max(gas_price);
        if let Some(feedback) = gas_price_feedback {
            self.min_feedback = Some(self.min_feedback.map_or(feedback, |f| f.min(feedback)));
        }
    }

    fn record_clearing_tx(&mut self, gas_price: u64) {
        self.lowest_clearing = Some(
            self.lowest_clearing
                .map_or(gas_price, |current| current.min(gas_price)),
        );
    }
}

/// Which case of the update determined the target price; the discriminant is
/// the `case` column of `updates.csv`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TargetCase {
    /// No cancellation, the reference price sufficed.
    ReferencePrice = 1,
    /// Cancellations and executions, the clearing price of the object.
    ClearingPrice = 2,
    /// Cancellations only, the lowest gas price feedback.
    Feedback = 3,
    /// Cancellations only and no feedback, the highest bid as a lower bound.
    LowerBound = 4,
}

/// One row of `updates.csv`.
#[derive(Clone, Copy, Debug)]
struct HotnessUpdate {
    object_id: ObjectID,
    case: TargetCase,
    target: f64,
    hotness_before: f64,
    hotness_after: f64,
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
    /// A not yet tracked object, before its first observation is recorded.
    fn untracked() -> Self {
        Self {
            latest_congestion_time: 0,
            highest_congestion_gas_price: 0,
            latest_clearing_time: None,
            lowest_clearing_gas_price: None,
            hotness: 0.0,
        }
    }

    /// Record the congestion and clearing events of the checkpoint at `time`.
    fn record(&mut self, time: CheckpointTimestamp, observation: &CheckpointObservation) {
        if observation.congested {
            self.latest_congestion_time = time;
            self.highest_congestion_gas_price = observation.highest_bid;
        }
        if let Some(lowest_clearing) = observation.lowest_clearing {
            self.latest_clearing_time = Some(time);
            self.lowest_clearing_gas_price = Some(lowest_clearing);
        }
    }

    /// Move the hotness one step toward the target price revealed by
    /// `observation`; returns the case and the target for logging.
    fn update_hotness(
        &mut self,
        observation: &CheckpointObservation,
        reference_gas_price: u64,
        params: &CongestionTrackerParams,
    ) -> (TargetCase, f64) {
        let predicted = reference_gas_price as f64 + self.hotness;
        let (case, target) = if !observation.congested {
            (TargetCase::ReferencePrice, reference_gas_price as f64)
        } else if let Some(clearing) = observation.lowest_clearing {
            (TargetCase::ClearingPrice, clearing as f64)
        } else if let Some(feedback) = observation.min_feedback {
            (TargetCase::Feedback, feedback as f64)
        } else {
            (
                TargetCase::LowerBound,
                predicted.max(observation.highest_bid as f64),
            )
        };

        let mut residual = predicted - target;
        if residual < 0.0 {
            residual *= params.ogd_alpha;
        }
        let step = 2.0 * params.ogd_eta * residual;
        self.hotness = (self.hotness - step)
            .max(self.hotness / params.max_decay_factor)
            .max(0.0);

        (case, target)
    }
}

/// `CongestionTracker` tracks objects' congestion info.
/// The info is then used to calculated a suggested gas price.
pub struct CongestionTracker {
    reference_gas_price: u64,
    params: CongestionTrackerParams,
    /// Key-value cache for storing congestion info of objects.
    object_congestion_info: Cache<ObjectID, CongestionInfo>,
    /// HTTP client for posting model updates/training batches.
    #[cfg(feature = "gas-nn")]
    model_updater: InNodeModelUpdater,
    /// Lock-free reader for per-object tip snapshot.
    #[cfg(feature = "gas-nn")]
    model_reader: ModelReader,
    /// Metrics handle
    metrics: Arc<GasMetrics>,
}

impl CongestionTracker {
    /// Compose and send model update and training batches for the given
    /// checkpoint.
    #[cfg(feature = "gas-nn")]
    fn inform_model(
        &self,
        checkpoint: &VerifiedCheckpoint,
        congestion_txs_data: &[TxData],
        clearing_txs_data: &[TxData],
    ) {
        let _scope = monitored_scope("CongestionTracker::inform_model");
        let h = self.metrics.latency_component("congestion.inform_model");
        let _t = h.start_timer();
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
        let mut raw_txs: Vec<RawTxItem> =
            Vec::with_capacity(congestion_txs_data.len() + clearing_txs_data.len());
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
        // Hardware sample after composing and enqueueing
        self.metrics.record_hw_sample("congestion.inform_model");
    }

    /// Fallback when gas-nn is disabled: still record metrics, but skip NN
    /// work.
    #[cfg(not(feature = "gas-nn"))]
    fn inform_model(
        &self,
        _checkpoint: &VerifiedCheckpoint,
        _congestion_txs_data: &[TxData],
        _clearing_txs_data: &[TxData],
    ) {
        let _scope = monitored_scope("CongestionTracker::inform_model");
        let h = self.metrics.latency_component("congestion.inform_model");
        let _t = h.start_timer();
        self.metrics.record_hw_sample("congestion.inform_model");
    }
    /// Create a new `CongestionTracker` with default parameters.
    pub fn new(reference_gas_price: u64, registry: &Registry) -> Self {
        Self::new_with_params(
            reference_gas_price,
            registry,
            CongestionTrackerParams::default(),
        )
    }

    /// Create a new `CongestionTracker` with the provided parameters, after
    /// applying the `IOTA_OGD_ETA` / `IOTA_OGD_ALPHA` environment overrides.
    ///
    /// # Panics
    ///
    /// Panics when an override is not a number or the effective parameters
    /// fail validation: a malformed override is an operator error, and failing
    /// at startup beats running an experiment with the wrong parameters.
    pub fn new_with_params(
        reference_gas_price: u64,
        registry: &Registry,
        params: CongestionTrackerParams,
    ) -> Self {
        let params = params
            .with_env_overrides()
            .expect("congestion tracker environment overrides should be numbers");
        params
            .validate()
            .expect("congestion tracker parameters should be valid");
        Self::build(reference_gas_price, registry, params)
    }

    #[cfg(test)]
    fn new_for_test(reference_gas_price: u64, params: CongestionTrackerParams) -> Self {
        params.validate().unwrap();
        Self::build(reference_gas_price, &Registry::new(), params)
    }

    fn build(
        reference_gas_price: u64,
        registry: &Registry,
        params: CongestionTrackerParams,
    ) -> Self {
        debug!(
            cache_capacity = params.cache_capacity,
            hotness_cutoff = params.hotness_cutoff,
            max_decay_factor = params.max_decay_factor,
            ogd_eta = params.ogd_eta,
            ogd_alpha = params.ogd_alpha,
            "Initializing CongestionTracker with parameters",
        );
        // Remove and recreate the results folder
        let mut results_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        results_path.push("src");
        results_path.push("results");

        if results_path.exists() {
            let _ = std::fs::remove_dir_all(&results_path);
        }
        let _ = std::fs::create_dir_all(&results_path);
        let metrics = init_gas_metrics(registry);
        #[cfg(feature = "gas-nn")]
        let model_updater = InNodeModelUpdater::new(metrics.clone());
        #[cfg(feature = "gas-nn")]
        let model_reader = model_updater.reader();
        Self {
            reference_gas_price,
            params,
            object_congestion_info: Cache::new(params.cache_capacity),
            #[cfg(feature = "gas-nn")]
            model_updater,
            #[cfg(feature = "gas-nn")]
            model_reader,
            metrics,
        }
    }

    /// Process effects of all transactions included in a certain checkpoint.
    pub fn process_checkpoint_effects(
        &self,
        transaction_cache_reader: &dyn TransactionCacheRead,
        checkpoint: &VerifiedCheckpoint,
        effects: &[TransactionEffects],
    ) {
        let _scope = monitored_scope("CongestionTracker::process_checkpoint_effects");
        let h = self
            .metrics
            .latency_component("congestion.process_checkpoint_effects");
        let timer = h.start_timer();
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
            let ogd_prediction = self
                .get_suggested_gas_price_with_ogd(tx_data)
                .unwrap_or(self.reference_gas_price);
            let nn_prediction = {
                let this = &self;
                Some(
                    this.model_reader
                        .predict_for_tx(tx_data, this.reference_gas_price),
                )
            }
            .unwrap_or(self.reference_gas_price);

            // Skip system transactions
            if gas_price == 1 {
                continue;
            }

            let mutable_shared_inputs: Vec<ObjectID> = tx_data
                .shared_input_objects()
                .into_iter()
                .filter(|obj| obj.mutable)
                .map(|obj| obj.id)
                .collect();

            if let Some(CongestedObjects(congested_objects)) =
                effects.status().get_congested_objects()
            {
                congestion_txs_data.push(TxData {
                    checkpoint: checkpoint.sequence_number,
                    digest: *effects.transaction_digest(),
                    objects: congested_objects.clone(),
                    mutable_shared_inputs,
                    gas_price,
                    gas_price_feedback: effects.status().get_feedback_suggested_gas_price(),
                    sui_prediction,
                    ogd_prediction,
                    nn_prediction,
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
                        mutable_shared_inputs,
                        gas_price,
                        gas_price_feedback: None,
                        sui_prediction,
                        ogd_prediction,
                        nn_prediction,
                        cleread: true,
                    });
                }
            }
        }

        let updates = self.process_congestion_and_clearing_txs_data(
            checkpoint.timestamp_ms,
            &congestion_txs_data,
            &clearing_txs_data,
        );
        if let Err(e) =
            self.dump_updates_to_csv("updates.csv", checkpoint.sequence_number, &updates)
        {
            info!("Failed to write updates.csv: {e}");
        }
        // Record touched objects histogram
        let mut touched: std::collections::HashSet<ObjectID> = std::collections::HashSet::new();
        for tx in &congestion_txs_data {
            touched.extend(tx.objects.iter().cloned());
        }
        for tx in &clearing_txs_data {
            touched.extend(tx.objects.iter().cloned());
        }
        self.metrics.touched_hist().observe(touched.len() as u64);

        drop(timer);

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
                "Checkpoint: {} | Digest: {:?} | Gas price: {} | Feedback: {} | Prediction (Sui): {:?} | Prediction (IOTA): {:?} | Prediction (NN): {:?} | Cleared: {}",
                tx.checkpoint,
                tx.digest,
                tx.gas_price,
                tx.gas_price_feedback.unwrap_or(1000),
                tx.sui_prediction,
                tx.ogd_prediction,
                tx.nn_prediction,
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
                tx.nn_prediction,
                tx.cleread,
                &tx.mutable_shared_inputs,
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
    pub fn get_suggested_gas_price_with_ogd(&self, transaction: &TransactionData) -> Option<u64> {
        let (_, hotness) = self.get_max_hotness_per_tx(
            transaction
                .shared_input_objects()
                .into_iter()
                .filter(|id| id.mutable)
                .map(|id| id.id),
        )?;

        Some(self.reference_gas_price.saturating_add(hotness as u64))
    }

    /// NN disabled: no prediction.
    #[cfg(not(feature = "gas-nn"))]
    pub fn get_suggested_gas_price_with_nn(&self, _transaction: &TransactionData) -> Option<u64> {
        None
    }

    /// Alias expected by authority.rs for model prediction.
    /// Non-blocking: reads from lock-free snapshot of per-object tips.
    #[cfg(feature = "gas-nn")]
    pub fn get_suggested_gas_price_with_nn(&self, transaction: &TransactionData) -> Option<u64> {
        Some(
            self.model_reader
                .predict_for_tx(transaction, self.reference_gas_price),
        )
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
        prediction_nn: u64,
        cleared: bool,
        objects: &[ObjectID],
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
            writeln!(
                file,
                "checkpoint,digest,gasprice,feedback,sui,ogd,nn,cleared,objects"
            )?;
        }

        let objects = objects
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(";");

        // Build row
        let row = format!(
            "{},{},{},{},{},{},{},{},{}",
            checkpoint,
            digest,
            gas_price,
            gas_price_feedback,
            prediction_sui,
            prediction_ogd,
            prediction_nn,
            cleared,
            objects
        );

        // Column-count check
        if row.split(',').count() != 9 {
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

    /// Append one row per updated object of the checkpoint to `updates.csv`.
    fn dump_updates_to_csv(
        &self,
        file_name: &str,
        checkpoint: u64,
        updates: &[HotnessUpdate],
    ) -> std::io::Result<()> {
        if updates.is_empty() {
            return Ok(());
        }
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("src");
        path.push("results");
        path.push(file_name);

        std::fs::create_dir_all(path.parent().unwrap())?;
        let file_exists = path.exists();

        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;

        if !file_exists {
            writeln!(file, "checkpoint,object,case,target,w_before,w_after")?;
        }

        for update in updates {
            writeln!(
                file,
                "{},{},{},{},{},{}",
                checkpoint,
                update.object_id,
                update.case as u8,
                update.target,
                update.hotness_before,
                update.hotness_after
            )?;
        }
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

    /// Process checkpoint's congestion and clearing transactions info; returns
    /// one entry per updated object.
    fn process_congestion_and_clearing_txs_data(
        &self,
        time: CheckpointTimestamp,
        congestion_txs_data: &[TxData],
        clearing_txs_data: &[TxData],
    ) -> Vec<HotnessUpdate> {
        let _scope = monitored_scope("CongestionTracker::process_congestion_and_clearing_txs_data");
        let h = self.metrics.latency_component("congestion.process_cp_data");
        let timer = h.start_timer();
        let observations =
            self.compute_checkpoint_observations(congestion_txs_data, clearing_txs_data);
        let updates = self.update_congestion_info_cache(time, observations);
        drop(timer);
        updates
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
        let _scope = monitored_scope("CongestionTracker::get_max_hotness_per_tx");
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

    /// Collect, per object, what this checkpoint reveals. Objects touched only
    /// by executed transactions and not yet tracked are left out: their weight
    /// is 0 and their target is the reference price, so the residual is 0.
    fn compute_checkpoint_observations(
        &self,
        congestion_txs_data: &[TxData],
        clearing_txs_data: &[TxData],
    ) -> HashMap<ObjectID, CheckpointObservation> {
        let _scope = monitored_scope("CongestionTracker::compute_checkpoint_observations");
        let h = self.metrics.latency_component("congestion.compute_cp_info");
        let _timer = h.start_timer();
        let mut observations: HashMap<ObjectID, CheckpointObservation> = HashMap::new();

        for TxData {
            objects,
            gas_price,
            gas_price_feedback,
            ..
        } in congestion_txs_data
        {
            for object_id in objects {
                observations
                    .entry(*object_id)
                    .or_default()
                    .record_congested_tx(*gas_price, *gas_price_feedback);
            }
        }

        for TxData {
            objects, gas_price, ..
        } in clearing_txs_data
        {
            for object_id in objects {
                match observations.entry(*object_id) {
                    Entry::Occupied(entry) => entry.into_mut().record_clearing_tx(*gas_price),
                    Entry::Vacant(entry) => {
                        if self.object_congestion_info.contains_key(object_id) {
                            entry
                                .insert(CheckpointObservation::default())
                                .record_clearing_tx(*gas_price);
                        }
                    }
                }
            }
        }

        observations
    }

    /// Apply one hotness step per observed object, then decay and prune the
    /// untouched ones. Returns the applied updates.
    fn update_congestion_info_cache(
        &self,
        time: CheckpointTimestamp,
        observations: HashMap<ObjectID, CheckpointObservation>,
    ) -> Vec<HotnessUpdate> {
        let _scope = monitored_scope("CongestionTracker::update_congestion_info_cache");
        let h = self.metrics.latency_component("congestion.update_cache");
        let _timer = h.start_timer();
        debug!(
            cache_capacity = self.params.cache_capacity,
            hotness_cutoff = self.params.hotness_cutoff,
            max_decay_factor = self.params.max_decay_factor,
            ogd_eta = self.params.ogd_eta,
            ogd_alpha = self.params.ogd_alpha,
            observed_objects = observations.len(),
            "Updating congestion info cache with current parameters",
        );
        let touched_objects: HashSet<ObjectID> = observations.keys().copied().collect();
        let mut updates = Vec::with_capacity(observations.len());

        for (object_id, observation) in observations {
            self.object_congestion_info
                .entry(object_id)
                .and_compute_with(|maybe_entry| {
                    let mut info = maybe_entry
                        .map(|e| e.into_value())
                        .unwrap_or_else(CongestionInfo::untracked);
                    let hotness_before = info.hotness;
                    info.record(time, &observation);
                    let (case, target) =
                        info.update_hotness(&observation, self.reference_gas_price, &self.params);
                    updates.push(HotnessUpdate {
                        object_id,
                        case,
                        target,
                        hotness_before,
                        hotness_after: info.hotness,
                    });
                    Op::Put(info)
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
                            e.hotness /= self.params.max_decay_factor;
                            if e.hotness < self.params.hotness_cutoff {
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

        updates
    }

    /// Get congestion info for a given object.
    fn get_congestion_info(&self, object_id: ObjectID) -> Option<CongestionInfo> {
        self.object_congestion_info.get(&object_id)
    }

    #[cfg(test)]
    fn set_hotness_for_test(&self, object_id: ObjectID, hotness: f64) {
        let mut info = CongestionInfo::untracked();
        info.hotness = hotness;
        self.object_congestion_info.insert(object_id, info);
    }
}

#[cfg(test)]
mod tests {
    use iota_test_transaction_builder::TestTransactionBuilder;
    use iota_types::{
        base_types::{SequenceNumber, random_object_ref},
        crypto::{AccountKeyPair, get_key_pair},
        transaction::{CallArg, ObjectArg},
    };

    use super::*;

    const RGP: u64 = 1000;
    const DECAY: f64 = 1.1;

    fn params(eta: f64, alpha: f64) -> CongestionTrackerParams {
        CongestionTrackerParams {
            cache_capacity: 10_000,
            hotness_cutoff: 1.0,
            max_decay_factor: DECAY,
            ogd_eta: eta,
            ogd_alpha: alpha,
        }
    }

    fn tracker(eta: f64, alpha: f64) -> CongestionTracker {
        CongestionTracker::new_for_test(RGP, params(eta, alpha))
    }

    fn tx_data(
        objects: Vec<ObjectID>,
        gas_price: u64,
        feedback: Option<u64>,
        cleared: bool,
    ) -> TxData {
        TxData {
            checkpoint: 0,
            digest: TransactionDigest::random(),
            mutable_shared_inputs: objects.clone(),
            objects,
            gas_price,
            gas_price_feedback: feedback,
            sui_prediction: RGP,
            ogd_prediction: RGP,
            nn_prediction: RGP,
            cleread: cleared,
        }
    }

    fn cancelled(objects: Vec<ObjectID>, gas_price: u64, feedback: Option<u64>) -> TxData {
        tx_data(objects, gas_price, feedback, false)
    }

    fn executed(objects: Vec<ObjectID>, gas_price: u64) -> TxData {
        tx_data(objects, gas_price, None, true)
    }

    fn assert_hotness(tracker: &CongestionTracker, object: &ObjectID, expected: f64) {
        let actual = tracker.get_hotness_for_object(object).unwrap();
        assert!(
            (actual - expected).abs() < 1e-6,
            "hotness {actual} differs from expected {expected}"
        );
    }

    fn case_of(updates: &[HotnessUpdate], object: &ObjectID) -> TargetCase {
        updates
            .iter()
            .find(|u| u.object_id == *object)
            .expect("object should have been updated")
            .case
    }

    #[test]
    fn parameters_are_validated() {
        assert!(params(0.4, 1.0).validate().is_ok());
        assert!(params(0.5, 3.0).validate().is_ok());
        assert!(params(0.0, 1.0).validate().is_err());
        assert!(params(0.6, 1.0).validate().is_err());
        assert!(params(f64::NAN, 1.0).validate().is_err());
        assert!(params(0.4, 0.9).validate().is_err());
        assert!(params(0.4, f64::INFINITY).validate().is_err());

        let defaults = CongestionTrackerParams::default();
        assert_eq!(defaults.ogd_eta, 0.4);
        assert_eq!(defaults.ogd_alpha, 1.0);
        assert!(defaults.validate().is_ok());
    }

    #[tokio::test]
    async fn congestion_tracker_process_checkpoint_txs_data() {
        let tracker = tracker(0.4, 1.0);
        let object_1 = ObjectID::random();
        let object_2 = ObjectID::random();

        let time = 1_000;
        let congestion_txs_data = vec![
            cancelled(vec![object_1], 100, Some(1000)),
            cancelled(vec![object_2], 200, Some(1000)),
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

    #[tokio::test]
    async fn congestion_tracker_process_checkpoint_data_then_success() {
        let tracker = tracker(0.4, 1.0);
        let object = ObjectID::random();

        // Congestion transactions only, no clearing ones. The highest congestion
        // gas price should be used.
        let time = 1_000;
        let congestion_txs_data = vec![
            cancelled(vec![object], 100, Some(1000)),
            cancelled(vec![object], 75, Some(1000)),
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
        let clearing_txs_data = vec![executed(vec![object], 150)];
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
        let congestion_txs_data = vec![cancelled(vec![object], 100, Some(1000))];
        let clearing_txs_data = vec![executed(vec![object], 175), executed(vec![object], 125)];
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

    #[tokio::test]
    async fn congestion_tracker_get_suggested_gas_price_for_multiple_objects() {
        let tracker = tracker(0.4, 1.0);
        let object_1 = ObjectID::random();
        let object_2 = ObjectID::random();

        let time = 1_000;
        let congestion_txs_data = vec![
            cancelled(vec![object_1], 100, Some(1000)),
            cancelled(vec![object_2], 200, Some(1000)),
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
            cancelled(vec![object_1], 100, Some(1000)),
            cancelled(vec![object_2], 200, Some(1000)),
        ];
        let clearing_txs_data = vec![executed(vec![object_1], 100), executed(vec![object_2], 150)];
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

    #[tokio::test]
    async fn case_1_reference_price_sufficed() {
        let tracker = tracker(0.4, 1.0);
        let object = ObjectID::random();
        tracker.set_hotness_for_test(object, 500.0);

        let updates = tracker.process_congestion_and_clearing_txs_data(
            1000,
            &[],
            &[executed(vec![object], 1200), executed(vec![object], 1100)],
        );

        // Target 1000, residual +500, step 400: the bounded decrease binds.
        assert_eq!(case_of(&updates, &object), TargetCase::ReferencePrice);
        assert_hotness(&tracker, &object, 500.0 / DECAY);
    }

    #[tokio::test]
    async fn case_2_clearing_price() {
        for (alpha, expected) in [(1.0, 240.0), (2.0, 480.0)] {
            let tracker = tracker(0.4, alpha);
            let object = ObjectID::random();

            let updates = tracker.process_congestion_and_clearing_txs_data(
                1000,
                &[cancelled(vec![object], 1100, Some(1250))],
                &[executed(vec![object], 1300)],
            );

            // New object: target 1300, residual alpha * (1000 - 1300).
            assert_eq!(case_of(&updates, &object), TargetCase::ClearingPrice);
            assert_hotness(&tracker, &object, expected);
        }
    }

    #[tokio::test]
    async fn case_3_feedback() {
        for alpha in [1.0, 2.0] {
            let tracker = tracker(0.4, alpha);
            let object = ObjectID::random();
            tracker.set_hotness_for_test(object, 100.0);

            let updates = tracker.process_congestion_and_clearing_txs_data(
                1000,
                &[
                    cancelled(vec![object], 1050, Some(1500)),
                    cancelled(vec![object], 1080, Some(1400)),
                ],
                &[],
            );

            // Target is the lowest feedback 1400, residual alpha * (1100 - 1400).
            assert_eq!(case_of(&updates, &object), TargetCase::Feedback);
            assert_hotness(&tracker, &object, 100.0 + 0.8 * 300.0 * alpha);
        }
    }

    #[tokio::test]
    async fn case_4_without_feedback_is_a_lower_bound() {
        for alpha in [1.0, 2.0] {
            let tracker = tracker(0.4, alpha);
            let under = ObjectID::random();
            let over = ObjectID::random();
            tracker.set_hotness_for_test(under, 100.0);
            tracker.set_hotness_for_test(over, 400.0);

            let updates = tracker.process_congestion_and_clearing_txs_data(
                1000,
                &[
                    cancelled(vec![under, over], 1200, None),
                    cancelled(vec![under, over], 1150, None),
                ],
                &[],
            );

            assert_eq!(case_of(&updates, &under), TargetCase::LowerBound);
            assert_eq!(case_of(&updates, &over), TargetCase::LowerBound);
            // Predicted 1100 < highest bid 1200: raised toward the bound.
            assert_hotness(&tracker, &under, 100.0 + 0.8 * 100.0 * alpha);
            // Predicted 1400 >= highest bid 1200: residual 0, unchanged.
            assert_hotness(&tracker, &over, 400.0);
        }
    }

    #[tokio::test]
    async fn objects_of_one_transaction_are_updated_independently() {
        for alpha in [1.0, 2.0] {
            let tracker = tracker(0.4, alpha);
            let a = ObjectID::random();
            let b = ObjectID::random();
            tracker.set_hotness_for_test(a, 900.0);
            tracker.set_hotness_for_test(b, 100.0);

            tracker.process_congestion_and_clearing_txs_data(
                1000,
                &[cancelled(vec![a, b], 1100, Some(1500))],
                &[],
            );

            // Both move toward tip 500 from their own weight: `a` over-estimates
            // and the bounded decrease binds, `b` under-estimates and is raised.
            assert_hotness(&tracker, &a, f64::max(900.0 - 0.8 * 400.0, 900.0 / DECAY));
            assert_hotness(&tracker, &b, 100.0 + 0.8 * alpha * (500.0 - 100.0));
        }
    }

    #[tokio::test]
    async fn executed_only_untracked_objects_are_not_inserted() {
        let tracker = tracker(0.4, 1.0);
        let tracked = ObjectID::random();
        let untracked = ObjectID::random();
        tracker.set_hotness_for_test(tracked, 50.0);

        let updates = tracker.process_congestion_and_clearing_txs_data(
            1000,
            &[],
            &[executed(vec![tracked, untracked], 1300)],
        );

        assert_eq!(updates.len(), 1);
        assert!(tracker.get_hotness_for_object(&untracked).is_none());
        assert_hotness(&tracker, &tracked, 50.0 / DECAY);
    }

    #[tokio::test]
    async fn untouched_objects_decay_and_are_evicted() {
        let tracker = tracker(0.4, 1.0);
        let object = ObjectID::random();
        tracker.set_hotness_for_test(object, 1.2);

        let updates = tracker.process_congestion_and_clearing_txs_data(1000, &[], &[]);
        assert!(updates.is_empty());
        assert_hotness(&tracker, &object, 1.2 / DECAY);

        // 1.2 / 1.1^2 < hotness_cutoff: evicted.
        tracker.process_congestion_and_clearing_txs_data(1100, &[], &[]);
        assert!(tracker.get_hotness_for_object(&object).is_none());
    }

    #[tokio::test]
    async fn eta_half_overwrites_upward_and_is_clamped_downward() {
        let tracker = tracker(0.5, 1.0);
        let raised = ObjectID::random();
        let lowered = ObjectID::random();
        tracker.set_hotness_for_test(lowered, 500.0);

        tracker.process_congestion_and_clearing_txs_data(
            1000,
            &[
                cancelled(vec![raised], 1100, Some(1250)),
                cancelled(vec![lowered], 1100, Some(1250)),
            ],
            &[executed(vec![raised], 1300), executed(vec![lowered], 1100)],
        );

        // Target 1300 above predicted 1000: overwritten exactly with tip 300.
        assert_hotness(&tracker, &raised, 300.0);
        // Target 1100 below predicted 1500: the bounded decrease binds.
        assert_hotness(&tracker, &lowered, 500.0 / DECAY);
    }

    #[tokio::test]
    async fn ogd_prediction_is_reference_price_plus_max_hotness() {
        let tracker = tracker(0.4, 1.0);
        let hot = ObjectID::random();
        let warm = ObjectID::random();
        let cold = ObjectID::random();
        tracker.set_hotness_for_test(hot, 300.0);
        tracker.set_hotness_for_test(warm, 100.0);

        let (sender, _): (_, AccountKeyPair) = get_key_pair();
        let build = |objects: &[(ObjectID, bool)]| {
            TestTransactionBuilder::new(sender, random_object_ref(), RGP)
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
                .build()
        };

        let tx = build(&[(hot, true), (warm, true), (cold, true)]);
        assert_eq!(
            tracker.get_suggested_gas_price_with_ogd(&tx),
            Some(RGP + 300)
        );
        // Read-only inputs do not count.
        let tx = build(&[(hot, false), (warm, true)]);
        assert_eq!(
            tracker.get_suggested_gas_price_with_ogd(&tx),
            Some(RGP + 100)
        );
        let tx = build(&[(cold, true)]);
        assert_eq!(tracker.get_suggested_gas_price_with_ogd(&tx), Some(RGP));
    }

    #[tokio::test]
    async fn congestion_tracker_repeated_congestion_across_checkpoints() {
        let tracker = tracker(0.4, 1.0);
        let obj1 = ObjectID::random();
        let obj2 = ObjectID::random();
        let step = 0.8;

        // Case 2 for obj1: target 1600 from weight 0.
        tracker.process_congestion_and_clearing_txs_data(
            1000,
            &[cancelled(vec![obj1], 100, Some(1500))],
            &[executed(vec![obj1], 1600)],
        );
        let w1 = step * 600.0;
        assert_hotness(&tracker, &obj1, w1);

        // Case 2 for both: target 1800, each from its own weight.
        tracker.process_congestion_and_clearing_txs_data(
            1100,
            &[cancelled(vec![obj1, obj2], 100, Some(1700))],
            &[executed(vec![obj1, obj2], 1800)],
        );
        let w1 = w1 + step * (800.0 - w1);
        let w2 = step * 800.0;
        assert_hotness(&tracker, &obj1, w1);
        assert_hotness(&tracker, &obj2, w2);

        // Untouched: decay.
        tracker.process_congestion_and_clearing_txs_data(1200, &[], &[]);
        let w1 = w1 / DECAY;
        let w2 = w2 / DECAY;

        // obj1: case 1 (executed only), obj2: case 3 with feedback 1050. Both
        // over-estimate by far, so the bounded decrease binds.
        tracker.process_congestion_and_clearing_txs_data(
            1300,
            &[cancelled(vec![obj2], 100, Some(1050))],
            &[executed(vec![obj1], 1100)],
        );
        let w1 = (w1 - step * w1).max(w1 / DECAY);
        let w2 = (w2 - step * (w2 - 50.0)).max(w2 / DECAY);
        assert_hotness(&tracker, &obj1, w1);
        assert_hotness(&tracker, &obj2, w2);

        tracker.process_congestion_and_clearing_txs_data(1400, &[], &[]);
        let w1 = w1 / DECAY;
        let w2 = w2 / DECAY;

        // obj1: case 2 with clearing 1700; obj2: case 3 with feedback 1800.
        tracker.process_congestion_and_clearing_txs_data(
            1500,
            &[
                cancelled(vec![obj1, obj2], 100, Some(1800)),
                cancelled(vec![obj1], 100, Some(1750)),
            ],
            &[executed(vec![obj1], 1700)],
        );
        let w1 = w1 + step * (700.0 - w1);
        let w2 = w2 + step * (800.0 - w2);
        assert_hotness(&tracker, &obj1, w1);
        assert_hotness(&tracker, &obj2, w2);
    }

    #[tokio::test]
    async fn congestion_tracker_remove_cold_objects_from_cache() {
        let tracker = tracker(0.4, 1.0);
        let obj1 = ObjectID::random();
        let obj2 = ObjectID::random();

        // First checkpoint with two congested objects: tip 1 each.
        tracker.process_congestion_and_clearing_txs_data(
            1000,
            &[cancelled(vec![obj1, obj2], 100, Some(1001))],
            &[],
        );
        let w = 0.8;
        assert_hotness(&tracker, &obj1, w);
        assert_hotness(&tracker, &obj2, w);

        // obj1 is not congested anymore: it decays below hotness_cutoff and is
        // removed. obj2 keeps learning.
        tracker.process_congestion_and_clearing_txs_data(
            1100,
            &[cancelled(vec![obj2], 100, Some(1010))],
            &[],
        );
        assert!(tracker.get_hotness_for_object(&obj1).is_none());
        let w2 = w + 0.8 * (10.0 - w);
        assert_hotness(&tracker, &obj2, w2);

        for time in (1200..=1800).step_by(100) {
            tracker.process_congestion_and_clearing_txs_data(time, &[], &[]);
        }
        assert_hotness(&tracker, &obj2, w2 / DECAY.powi(7));
    }
}
