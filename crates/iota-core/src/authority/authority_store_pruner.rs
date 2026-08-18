// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeSet, HashMap},
    sync::{Arc, Mutex, Weak},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::anyhow;
use bincode::Options;
use iota_config::node::AuthorityStorePruningConfig;
use iota_metrics::{monitored_scope, spawn_monitored_task};
use iota_sdk_types::{
    CheckpointDigest, ObjectId, TransactionEffects, Version, checkpoint::CheckpointContents,
};
use iota_types::{
    effects::TransactionEffectsAPI,
    messages_checkpoint::{CheckpointContentsExt, CheckpointSequenceNumber, CheckpointTimestamp},
    storage::ObjectKey,
};
use once_cell::sync::Lazy;
use prometheus_filtered::{
    IntCounter, IntGauge, MetricLevel, Registry, register_int_counter_with_registry,
    register_int_gauge_with_registry,
};
use tokio::{
    sync::{
        oneshot::{self, Sender},
        watch,
    },
    time::Instant,
};
use tracing::{debug, error, info, warn};
use typed_store::{
    Map, TypedStoreError,
    rocksdb::{LiveFile, compaction_filter::Decision},
};

use super::authority_store_tables::{AuthorityPerpetualTables, AuthorityPrunerTables};
use crate::{
    authority::authority_store_types::{StoreObject, StoreObjectWrapper},
    checkpoint_progress_tracker::CheckpointProgressTracker,
    checkpoints::{CheckpointStore, CheckpointWatermark},
    rpc_indexes::RpcIndexesStore,
};

static PERIODIC_PRUNING_TABLES: Lazy<BTreeSet<String>> = Lazy::new(|| {
    [
        "objects",
        "effects",
        "transactions",
        "events_2",
        "executed_effects",
        "executed_transactions_to_checkpoint",
    ]
    .into_iter()
    .map(|cf| cf.to_string())
    .collect()
});
pub const EPOCH_DURATION_MS_FOR_TESTING: u64 = 24 * 60 * 60 * 1000;

/// Maximum number of checkpoints whose data is written in a single pruning
/// `WriteBatch`. Bounds batch memory only; it does not cap total work per run,
/// so it cannot cause the pruner to fall behind.
const MAX_CHECKPOINTS_IN_BATCH: usize = 10;
/// Maximum number of transactions whose effects are written in a single pruning
/// `WriteBatch`. Bounds batch memory only (see [`MAX_CHECKPOINTS_IN_BATCH`]).
const MAX_TRANSACTIONS_IN_BATCH: usize = 1000;

/// Chain-time backlog, in milliseconds, above which the pruner warns that it
/// has fallen behind execution.
const PRUNING_BACKLOG_WARN_THRESHOLD_MS: u64 = 60 * 60 * 1000;

/// Minimum interval between backlog warnings, so a persistently lagging pruner
/// does not warn on every drain.
const PRUNING_BACKLOG_WARN_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// While catching up (see [`PRUNING_DEBOUNCE_MIN_LAG`]), after a nudge wakes
/// the pruner it waits this long before draining so that more executed
/// checkpoints accumulate and their object deletions coalesce into larger,
/// fewer batches — which measurably improves catch-up throughput.
const PRUNING_NUDGE_DEBOUNCE: Duration = Duration::from_millis(1000);

/// The debounce above is only applied while the node is catching up, i.e. when
/// execution lags the highest synced checkpoint by more than this many
/// checkpoints. Near the tip the lag is tiny, so pruning stays prompt
/// (per-checkpoint) and does not incur the debounce delay.
const PRUNING_DEBOUNCE_MIN_LAG: u64 = 100;

/// The `AuthorityStorePruner` manages the pruning of the checkpoint data and
/// the RPC index history of the `AuthorityStore`. It includes a cancellation
/// handle that can be used to stop the pruning task.
///
/// It also owns the coordination channel between the checkpoint executor
/// (producer of new state) and the pruner task (consumer of aged-out state):
/// pruning is driven by execution progress rather than a timer — the executor
/// nudges after each checkpoint is made available, and the pruner drains fully
/// to its chain-time retention cutoff on every nudge. Pruning never blocks
/// execution; if it falls behind, the database grows temporarily and the lag
/// is surfaced via metrics and a warning (see
/// `PRUNING_BACKLOG_WARN_THRESHOLD_MS`).
pub struct AuthorityStorePruner {
    _pruner_cancel_handle: oneshot::Sender<()>,
    /// Executor -> pruner: latest executed checkpoint sequence number. Updating
    /// it both records progress and wakes the pruner task to drain.
    executed: watch::Sender<CheckpointSequenceNumber>,
}

impl AuthorityStorePruner {
    /// Called by the executor after a checkpoint has been executed and made
    /// available (watermark bumped, subscribers notified). Wakes the pruner.
    pub fn nudge(&self, executed_seq: CheckpointSequenceNumber) {
        self.executed.send_replace(executed_seq);
    }
}

/// The `AuthorityStorePruningMetrics` tracks various metrics related to the
/// pruning process of the `AuthorityStore`.
pub struct AuthorityStorePruningMetrics {
    pub last_pruned_effects_checkpoint: IntGauge,
    pub earliest_retained_indexes_epoch: IntGauge,
    pub num_epochs_to_retain_for_objects: IntGauge,
    pub num_epochs_to_retain_for_checkpoints: IntGauge,
    pub last_pruned_effects_checkpoint_timestamp_ms: IntGauge,
    pub pruning_chain_time_lag_ms: IntGauge,
}

impl AuthorityStorePruningMetrics {
    /// Initializes a new instance of `AuthorityStorePruningMetrics` with the
    /// provided registry, registering various metrics that track the pruning
    /// operations in the `AuthorityStore`.
    pub fn new(registry: &Registry) -> Arc<Self> {
        let this = Self {
            last_pruned_effects_checkpoint: register_int_gauge_with_registry!(
                "last_pruned_effects_checkpoint",
                "Last pruned effects checkpoint",
                registry;
                MetricLevel::Warn,
            )
            .unwrap(),
            earliest_retained_indexes_epoch: register_int_gauge_with_registry!(
                "earliest_retained_indexes_epoch",
                "Earliest epoch whose JSON-RPC index history is retained",
                registry;
                MetricLevel::Warn,
            )
            .unwrap(),
            num_epochs_to_retain_for_objects: register_int_gauge_with_registry!(
                "num_epochs_to_retain_for_objects",
                "Number of epochs to retain for objects",
                registry;
                MetricLevel::Warn,
            )
            .unwrap(),
            num_epochs_to_retain_for_checkpoints: register_int_gauge_with_registry!(
                "num_epochs_to_retain_for_checkpoints",
                "Number of epochs to retain for checkpoints",
                registry
            )
            .unwrap(),
            last_pruned_effects_checkpoint_timestamp_ms: register_int_gauge_with_registry!(
                "last_pruned_effects_checkpoint_timestamp_ms",
                "Timestamp of the last checkpoint whose checkpoint data was pruned",
                registry
            )
            .unwrap(),
            pruning_chain_time_lag_ms: register_int_gauge_with_registry!(
                "pruning_chain_time_lag_ms",
                "Chain time between the executed watermark and the target of the pruner's \
                 last completed drain; large values mean pruning has fallen behind execution",
                registry
            )
            .unwrap(),
        };
        Arc::new(this)
    }

    /// Creates a new instance of `AuthorityStorePruningMetrics` for testing
    /// purposes.
    pub fn new_for_test() -> Arc<Self> {
        Self::new(&Registry::new())
    }
}

impl AuthorityStorePruner {
    /// Prunes checkpoint-related data from the `AuthorityStore`, including
    /// transaction effects, executed transactions, and checkpoint contents,
    /// based on the specified checkpoint number and list of checkpoints to
    /// prune. This function removes outdated data, updates pruning metrics,
    /// and maintains database consistency by updating watermarks.
    fn prune_checkpoints(
        perpetual_db: &Arc<AuthorityPerpetualTables>,
        checkpoint_db: &Arc<CheckpointStore>,
        checkpoint_number: CheckpointSequenceNumber,
        checkpoints_to_prune: Vec<CheckpointDigest>,
        checkpoint_content_to_prune: Vec<CheckpointContents>,
        effects_to_prune: &Vec<TransactionEffects>,
        metrics: Arc<AuthorityStorePruningMetrics>,
    ) -> anyhow::Result<()> {
        let _scope = monitored_scope("EffectsLivePruner");

        let mut perpetual_batch = perpetual_db.objects.batch();
        let transactions: Vec<_> = checkpoint_content_to_prune
            .iter()
            .flat_map(|content| content.iter().map(|tx| tx.transaction))
            .collect();

        perpetual_batch.delete_batch(&perpetual_db.transactions, transactions.iter())?;
        perpetual_batch.delete_batch(&perpetual_db.executed_effects, transactions.iter())?;
        perpetual_batch.delete_batch(
            &perpetual_db.executed_transactions_to_checkpoint,
            transactions,
        )?;

        let mut effect_digests = vec![];
        for effects in effects_to_prune {
            let effects_digest = effects.digest();
            debug!("Pruning effects {:?}", effects_digest);
            effect_digests.push(effects_digest);

            if effects.events_digest().is_some() {
                perpetual_batch
                    .delete_batch(&perpetual_db.events_2, [effects.transaction_digest()])?;
            }
        }
        perpetual_batch.delete_batch(&perpetual_db.effects, effect_digests)?;

        let mut checkpoints_batch = checkpoint_db.tables.certified_checkpoints.batch();

        let checkpoint_contents_digests =
            checkpoint_content_to_prune.iter().map(|ckpt| ckpt.digest());
        checkpoints_batch.delete_batch(
            &checkpoint_db.tables.checkpoint_content,
            checkpoint_contents_digests,
        )?;

        checkpoints_batch.delete_batch(
            &checkpoint_db.tables.checkpoint_by_digest,
            checkpoints_to_prune,
        )?;

        checkpoints_batch.insert_batch(
            &checkpoint_db.tables.watermarks,
            [(
                &CheckpointWatermark::HighestPruned,
                &(checkpoint_number, CheckpointDigest::random()),
            )],
        )?;

        perpetual_batch.write()?;
        checkpoints_batch.write()?;
        metrics
            .last_pruned_effects_checkpoint
            .set(checkpoint_number as i64);
        Ok(())
    }

    /// Asynchronously prunes checkpoint data for eligible epochs based on the
    /// configuration and current state of the `AuthorityStore`. This
    /// function determines the range of checkpoints that can be pruned,
    /// taking into account retention policies, archival watermarks, and the
    /// chain-time retention cutoff. It then delegates the pruning to the
    /// `prune_for_eligible_epochs` method.
    pub async fn prune_checkpoints_for_eligible_epochs(
        perpetual_db: &Arc<AuthorityPerpetualTables>,
        checkpoint_store: &Arc<CheckpointStore>,
        config: AuthorityStorePruningConfig,
        metrics: Arc<AuthorityStorePruningMetrics>,
        epoch_duration_ms: u64,
        progress_tracker: Option<&Arc<CheckpointProgressTracker>>,
    ) -> anyhow::Result<()> {
        let _scope = monitored_scope("PruneCheckpointsForEligibleEpochs");
        let pruned_checkpoint_number = checkpoint_store
            .get_highest_pruned_checkpoint_seq_number()?
            .unwrap_or(0);
        let (max_eligible_checkpoint, last_executed_timestamp_ms) = checkpoint_store
            .get_highest_executed_checkpoint()?
            .map(|c| (c.sequence_number(), c.timestamp_ms))
            .unwrap_or_default();
        let num_epochs_to_retain = config
            .num_epochs_to_retain_for_checkpoints()
            .ok_or_else(|| anyhow!("config value not set"))?;
        let cutoff_timestamp_ms = last_executed_timestamp_ms
            .saturating_sub(num_epochs_to_retain.saturating_mul(epoch_duration_ms));
        debug!("Max eligible checkpoint {}", max_eligible_checkpoint);
        Self::prune_for_eligible_epochs(
            perpetual_db,
            checkpoint_store,
            num_epochs_to_retain,
            pruned_checkpoint_number,
            max_eligible_checkpoint,
            cutoff_timestamp_ms,
            metrics.clone(),
            progress_tracker,
        )
        .await
    }

    /// Prunes the transactions, effects, events and checkpoint summaries of
    /// every checkpoint in epochs eligible for pruning.
    pub async fn prune_for_eligible_epochs(
        perpetual_db: &Arc<AuthorityPerpetualTables>,
        checkpoint_store: &Arc<CheckpointStore>,
        num_epochs_to_retain: u64,
        starting_checkpoint_number: CheckpointSequenceNumber,
        max_eligible_checkpoint: CheckpointSequenceNumber,
        cutoff_timestamp_ms: CheckpointTimestamp,
        metrics: Arc<AuthorityStorePruningMetrics>,
        progress_tracker: Option<&Arc<CheckpointProgressTracker>>,
    ) -> anyhow::Result<()> {
        let _scope = monitored_scope("PruneForEligibleEpochs");

        let mut checkpoint_number = starting_checkpoint_number;
        let mut last_pruned_timestamp_ms = 0;
        let current_epoch = checkpoint_store
            .get_highest_executed_checkpoint()?
            .map(|c| c.epoch())
            .unwrap_or_default();

        let mut checkpoints_to_prune = vec![];
        let mut checkpoint_content_to_prune = vec![];
        let mut effects_to_prune = vec![];

        let mut pruning_start = Instant::now();

        while let Some(ckpt) = checkpoint_store
            .tables
            .certified_checkpoints
            .get(&(checkpoint_number + 1))?
        {
            let checkpoint = ckpt.into_inner();
            // Stop pruning at this checkpoint if any of the following holds:
            // - Its epoch is within the retention window. This is the hard correctness
            //   bound: parts of the system (e.g. the state accumulator) still require
            //   access to old object versions of recently retained epochs.
            // - It reaches the highest eligible checkpoint watermark (including the
            //   watermark itself).
            // - Its timestamp is newer than the retention cutoff. This paces pruning
            //   against the chain's own (consensus-agreed, monotonic) time rather than
            //   wall-clock, so the retained span is bounded to one retention window whether
            //   the node is catching up or at tip.
            if (current_epoch < checkpoint.epoch() + num_epochs_to_retain)
                || (checkpoint.sequence_number() >= max_eligible_checkpoint)
                || (checkpoint.timestamp_ms > cutoff_timestamp_ms)
            {
                break;
            }
            checkpoint_number = checkpoint.sequence_number();
            last_pruned_timestamp_ms = checkpoint.timestamp_ms;

            let content = checkpoint_store
                .get_checkpoint_contents(&checkpoint.contents_digest)?
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "checkpoint content data is missing: {}",
                        checkpoint.sequence_number
                    )
                })?;
            let effects = perpetual_db
                .effects
                .multi_get(content.iter().map(|tx| tx.effects))?;

            debug!("scheduling pruning for checkpoint {:?}", checkpoint_number);
            checkpoints_to_prune.push(*checkpoint.digest());
            checkpoint_content_to_prune.push(content);
            effects_to_prune.extend(effects.into_iter().flatten());

            if effects_to_prune.len() >= MAX_TRANSACTIONS_IN_BATCH
                || checkpoints_to_prune.len() >= MAX_CHECKPOINTS_IN_BATCH
            {
                Self::prune_checkpoints(
                    perpetual_db,
                    checkpoint_store,
                    checkpoint_number,
                    checkpoints_to_prune,
                    checkpoint_content_to_prune,
                    &effects_to_prune,
                    metrics.clone(),
                )?;

                // Published per batch so dashboards show progress during long
                // drains, not only at drain completion.
                metrics
                    .last_pruned_effects_checkpoint_timestamp_ms
                    .set(last_pruned_timestamp_ms as i64);

                // Report pruning time for this batch so the progress logger
                // shows time alongside the checkpoint deltas it reads from the
                // DB (which are already updated at this point).
                if let Some(tracker) = progress_tracker {
                    tracker.add_checkpoint_pruning_time(pruning_start.elapsed());
                    pruning_start = Instant::now();
                }

                checkpoints_to_prune = vec![];
                checkpoint_content_to_prune = vec![];
                effects_to_prune = vec![];
                // yield back to the tokio runtime. Prevent potential halt of other tasks
                tokio::task::yield_now().await;
            }
        }

        if !checkpoints_to_prune.is_empty() {
            Self::prune_checkpoints(
                perpetual_db,
                checkpoint_store,
                checkpoint_number,
                checkpoints_to_prune,
                checkpoint_content_to_prune,
                &effects_to_prune,
                metrics.clone(),
            )?;

            metrics
                .last_pruned_effects_checkpoint_timestamp_ms
                .set(last_pruned_timestamp_ms as i64);

            // Report pruning time for this batch so the progress logger
            // shows time alongside the checkpoint deltas it reads from the
            // DB (which are already updated at this point).
            if let Some(tracker) = progress_tracker {
                tracker.add_checkpoint_pruning_time(pruning_start.elapsed());
            }
        }

        Ok(())
    }

    /// Drops the RPC index history of expired epochs. The retention the
    /// store was opened with governs every history table, the transaction
    /// digests both API surfaces share included.
    fn prune_indexes(
        indexes: Option<&RpcIndexesStore>,
        metrics: &AuthorityStorePruningMetrics,
    ) -> anyhow::Result<()> {
        if let Some(indexes) = indexes {
            if let Some(earliest_retained_epoch) = indexes.prune()? {
                metrics
                    .earliest_retained_indexes_epoch
                    .set(earliest_retained_epoch as i64);
            }
        }
        Ok(())
    }

    /// Identifies and compacts the next eligible SST file in the
    /// `AuthorityStore` that meets the specified conditions for manual
    /// compaction. This function checks each SST file's metadata, including
    /// modification time and size, against a delay threshold to determine if it
    /// should be compacted. If a suitable file is found, it triggers a
    /// manual compaction and updates the last processed timestamp.
    fn compact_next_sst_file(
        perpetual_db: Arc<AuthorityPerpetualTables>,
        delay_days: usize,
        last_processed: Arc<Mutex<HashMap<String, SystemTime>>>,
    ) -> anyhow::Result<Option<LiveFile>> {
        let db_path = perpetual_db.objects.db.path_for_pruning();
        let mut state = last_processed
            .lock()
            .expect("failed to obtain a lock for last processed SST files");
        let mut sst_file_for_compaction: Option<LiveFile> = None;
        let time_threshold =
            SystemTime::now() - Duration::from_secs(delay_days as u64 * 24 * 60 * 60);
        for sst_file in perpetual_db.objects.db.live_files()? {
            let file_path = db_path.join(sst_file.name.clone().trim_matches('/'));
            let last_modified = std::fs::metadata(file_path)?.modified()?;
            if !PERIODIC_PRUNING_TABLES.contains(&sst_file.column_family_name)
                || sst_file.level < 1
                || sst_file.start_key.is_none()
                || sst_file.end_key.is_none()
                || last_modified > time_threshold
                || state.get(&sst_file.name).unwrap_or(&UNIX_EPOCH) > &time_threshold
            {
                continue;
            }
            if let Some(candidate) = &sst_file_for_compaction {
                if candidate.size > sst_file.size {
                    continue;
                }
            }
            sst_file_for_compaction = Some(sst_file);
        }
        let Some(sst_file) = sst_file_for_compaction else {
            return Ok(None);
        };
        info!(
            "Manual compaction of sst file {:?}. Size: {:?}, level: {:?}",
            sst_file.name, sst_file.size, sst_file.level
        );
        perpetual_db.objects.compact_range_raw(
            &sst_file.column_family_name,
            sst_file.start_key.clone().unwrap(),
            sst_file.end_key.clone().unwrap(),
        )?;
        state.insert(sst_file.name.clone(), SystemTime::now());
        Ok(Some(sst_file))
    }

    fn setup_pruning(
        config: AuthorityStorePruningConfig,
        epoch_duration_ms: u64,
        perpetual_db: Arc<AuthorityPerpetualTables>,
        checkpoint_store: Arc<CheckpointStore>,
        rpc_indexes_store: Option<Arc<RpcIndexesStore>>,
        metrics: Arc<AuthorityStorePruningMetrics>,
        progress_tracker: Option<Arc<CheckpointProgressTracker>>,
        mut executed_rx: watch::Receiver<CheckpointSequenceNumber>,
    ) -> Sender<()> {
        let (sender, mut recv) = tokio::sync::oneshot::channel();
        debug!(
            "Starting store pruner with num_epochs_to_retain_for_checkpoints={:?}",
            config.num_epochs_to_retain_for_checkpoints()
        );

        // Periodic background compaction of aged SST files, independent of the
        // execution-driven pruning loop below. The task holds the db weakly so
        // that it cannot keep a dropped node's database open, and exits once
        // the db is gone.
        let perpetual_db_for_compaction = Arc::downgrade(&perpetual_db);
        if let Some(delay_days) = config.periodic_compaction_threshold_days {
            spawn_monitored_task!(async move {
                let last_processed = Arc::new(Mutex::new(HashMap::new()));
                loop {
                    let Some(db) = perpetual_db_for_compaction.upgrade() else {
                        break;
                    };
                    let state = Arc::clone(&last_processed);
                    let result = tokio::task::spawn_blocking(move || {
                        Self::compact_next_sst_file(db, delay_days, state)
                    })
                    .await;
                    let mut sleep_interval_secs = 1;
                    match result {
                        Err(err) => error!("Failed to compact sst file: {:?}", err),
                        Ok(Err(err)) => error!("Failed to compact sst file: {:?}", err),
                        Ok(Ok(None)) => {
                            sleep_interval_secs = 3600;
                        }
                        _ => {}
                    }
                    tokio::time::sleep(Duration::from_secs(sleep_interval_secs)).await;
                }
            });
        }

        metrics
            .num_epochs_to_retain_for_objects
            .set(config.num_epochs_to_retain as i64);
        metrics.num_epochs_to_retain_for_checkpoints.set(
            config
                .num_epochs_to_retain_for_checkpoints
                .unwrap_or_default() as i64,
        );

        let prune_checkpoints = !matches!(
            config.num_epochs_to_retain_for_checkpoints(),
            None | Some(u64::MAX) | Some(0)
        );
        let prune_indexes = config.num_epochs_to_retain_for_indexes.is_some();

        // Execution-driven pruning: on every nudge from the checkpoint executor,
        // drain each enabled pruner fully to its chain-time cutoff. Draining
        // once before the first nudge handles any startup backlog. The `watch`
        // nudge coalesces many executed checkpoints into a single drain.
        tokio::task::spawn(async move {
            // The target of the last completed drain: the executed-checkpoint
            // timestamp observed when that drain started. Comparing it against
            // the current executed watermark measures how far pruning has
            // fallen behind execution in chain time — bounded and independent
            // of epoch-duration variance. Initialized to `u64::MAX` so no lag
            // is reported before the first drain completes.
            let mut last_drain_target_ms: CheckpointTimestamp = u64::MAX;
            let mut last_backlog_warn: Option<Instant> = None;
            loop {
                // The executed position this pass prunes up to.
                let highest_executed = checkpoint_store
                    .get_highest_executed_checkpoint()
                    .ok()
                    .flatten();
                let caught_up_to = highest_executed
                    .as_ref()
                    .map(|checkpoint| checkpoint.timestamp_ms)
                    .unwrap_or(u64::MAX);

                // Lag tracking only makes sense when something is actually
                // being pruned.
                if prune_checkpoints {
                    let lag_ms = caught_up_to.saturating_sub(last_drain_target_ms);
                    metrics.pruning_chain_time_lag_ms.set(lag_ms as i64);
                    if lag_ms > PRUNING_BACKLOG_WARN_THRESHOLD_MS
                        && last_backlog_warn
                            .is_none_or(|at| at.elapsed() >= PRUNING_BACKLOG_WARN_INTERVAL)
                    {
                        warn!(
                            lag_ms,
                            "pruning has fallen behind execution; the database grows until \
                             pruning catches up"
                        );
                        last_backlog_warn = Some(Instant::now());
                    }
                }

                // Only batch (debounce) while catching up: if execution lags the
                // highest synced checkpoint by more than the threshold there is a
                // backlog to coalesce; near the tip the lag is tiny and we prune
                // promptly.
                let executed_seq = highest_executed
                    .as_ref()
                    .map(|checkpoint| checkpoint.sequence_number())
                    .unwrap_or(0);
                let synced_seq = checkpoint_store
                    .get_highest_synced_checkpoint_seq_number()
                    .ok()
                    .flatten()
                    .unwrap_or(0);
                let catching_up =
                    synced_seq.saturating_sub(executed_seq) > PRUNING_DEBOUNCE_MIN_LAG;

                if prune_checkpoints {
                    if let Err(err) = Self::prune_checkpoints_for_eligible_epochs(
                        &perpetual_db,
                        &checkpoint_store,
                        config.clone(),
                        metrics.clone(),
                        epoch_duration_ms,
                        progress_tracker.as_ref(),
                    )
                    .await
                    {
                        error!("Failed to prune checkpoints: {:?}", err);
                    }
                }
                if prune_indexes {
                    // `RpcIndexesStore::prune` blocks queries on its lock
                    // while dropping column families; keep it off the async
                    // workers.
                    let rpc_indexes_store = rpc_indexes_store.clone();
                    let metrics = metrics.clone();
                    let result = tokio::task::spawn_blocking(move || {
                        Self::prune_indexes(rpc_indexes_store.as_deref(), &metrics)
                    })
                    .await;
                    if let Ok(Err(err)) | Err(err) = result.map_err(anyhow::Error::from) {
                        error!("Failed to prune indexes: {:?}", err);
                    }
                }

                if prune_checkpoints {
                    last_drain_target_ms = caught_up_to;
                    metrics.pruning_chain_time_lag_ms.set(0);
                }

                tokio::select! {
                    _ = &mut recv => break,
                    // `changed()` cannot error: the paired sender lives in the
                    // `AuthorityStorePruner` returned to the caller.
                    _ = executed_rx.changed() => {}
                }

                // Debounce only while catching up: let more executed checkpoints
                // accumulate before the next drain so their object deletions
                // coalesce into larger, fewer batches. Skipped near the tip so
                // pruning stays prompt.
                if catching_up {
                    tokio::time::sleep(PRUNING_NUDGE_DEBOUNCE).await;
                }
            }
        });
        sender
    }

    /// Initializes a new instance of `AuthorityStorePruner` with the provided
    /// configuration, database connections, and metrics registry.
    pub fn new(
        perpetual_db: Arc<AuthorityPerpetualTables>,
        checkpoint_store: Arc<CheckpointStore>,
        rpc_indexes_store: Option<Arc<RpcIndexesStore>>,
        pruning_config: AuthorityStorePruningConfig,
        epoch_duration_ms: u64,
        registry: &Registry,
        progress_tracker: Option<Arc<CheckpointProgressTracker>>,
    ) -> Self {
        // Coordination channel between the checkpoint executor and the pruner
        // task. The pruner task receives nudges (`executed_rx`); the sending
        // end is kept on the returned handle for `nudge`.
        let (executed, executed_rx) = watch::channel(0);
        AuthorityStorePruner {
            _pruner_cancel_handle: Self::setup_pruning(
                pruning_config,
                epoch_duration_ms,
                perpetual_db,
                checkpoint_store,
                rpc_indexes_store,
                AuthorityStorePruningMetrics::new(registry),
                progress_tracker,
                executed_rx,
            ),
            executed,
        }
    }

    /// Compacts the entire range of objects stored in the `AuthorityStore` by
    /// invoking a range compaction on the database.
    pub fn compact(perpetual_db: &Arc<AuthorityPerpetualTables>) -> Result<(), TypedStoreError> {
        perpetual_db.objects.compact_range(
            &ObjectKey(ObjectId::ZERO, Version::MIN_VALID_INCL),
            &ObjectKey(ObjectId::MAX, Version::MAX_VALID_EXCL),
        )
    }
}

#[derive(Clone)]
pub struct ObjectsCompactionFilter {
    db: Weak<AuthorityPrunerTables>,
    metrics: Arc<ObjectCompactionMetrics>,
}

impl ObjectsCompactionFilter {
    pub fn new(db: Arc<AuthorityPrunerTables>, registry: &Registry) -> Self {
        Self {
            db: Arc::downgrade(&db),
            metrics: ObjectCompactionMetrics::new(registry),
        }
    }
    pub fn filter(&mut self, key: &[u8], value: &[u8]) -> anyhow::Result<Decision> {
        let ObjectKey(object_id, version) = bincode::DefaultOptions::new()
            .with_big_endian()
            .with_fixint_encoding()
            .deserialize(key)?;
        let object: StoreObjectWrapper = bcs::from_bytes(value)?;
        // Compaction sees raw on-disk rows, which may be legacy V1; migrate
        // before `into_inner()`, which panics on an un-migrated V1.
        if matches!(object.migrate().into_inner(), StoreObject::Value(_)) {
            if let Some(db) = self.db.upgrade() {
                match db.object_tombstones.get(&object_id)? {
                    Some(gc_version) => {
                        if version <= gc_version {
                            self.metrics.key_removed.inc();
                            return Ok(Decision::Remove);
                        }
                        self.metrics.key_kept.inc();
                    }
                    None => self.metrics.key_not_found.inc(),
                }
            }
        }
        Ok(Decision::Keep)
    }
}

struct ObjectCompactionMetrics {
    key_removed: IntCounter,
    key_kept: IntCounter,
    key_not_found: IntCounter,
}

impl ObjectCompactionMetrics {
    pub fn new(registry: &Registry) -> Arc<Self> {
        Arc::new(Self {
            key_removed: register_int_counter_with_registry!(
                "objects_compaction_filter_key_removed",
                "Compaction key removed",
                registry
            )
            .unwrap(),
            key_kept: register_int_counter_with_registry!(
                "objects_compaction_filter_key_kept",
                "Compaction key kept",
                registry
            )
            .unwrap(),
            key_not_found: register_int_counter_with_registry!(
                "objects_compaction_filter_key_not_found",
                "Compaction key not found",
                registry
            )
            .unwrap(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{path::Path, sync::Arc};

    use iota_sdk_types::{ObjectId, TransactionDigest, Version};
    use iota_swarm_config::test_utils::{CommitteeFixture, empty_contents};
    use iota_types::{
        messages_checkpoint::{CheckpointSequenceNumber, CheckpointTimestamp},
        object::Object,
        storage::ObjectKey,
    };
    use more_asserts as ma;
    use prometheus_filtered::Registry;
    use tokio::sync::{oneshot, watch};
    use tracing::info;
    use typed_store::Map;

    use super::AuthorityStorePruner;
    use crate::{
        authority::{
            authority_store_pruner::AuthorityStorePruningMetrics,
            authority_store_tables::AuthorityPerpetualTables,
            authority_store_types::{StoreObjectWrapper, get_store_object},
        },
        checkpoints::CheckpointStore,
    };

    #[cfg(not(target_env = "msvc"))]
    #[tokio::test]
    async fn test_db_size_after_compaction() -> Result<(), anyhow::Error> {
        let tmp_dir = iota_common::tempdir();
        let perpetual_db = Arc::new(AuthorityPerpetualTables::open(tmp_dir.path(), None));
        let total_unique_object_ids = 10_000;
        let num_versions_per_object = 10;
        let mut id = ObjectId::ZERO;
        let mut to_delete = vec![];
        for _ in 0..total_unique_object_ids {
            for i in (0..num_versions_per_object).rev() {
                if i < num_versions_per_object - 2 {
                    to_delete.push((id, Version::from(i)));
                }
                let obj = get_store_object(Object::immutable_with_id_for_testing(id), None);
                perpetual_db
                    .objects
                    .insert(&ObjectKey(id, Version::from(i)), &obj)?;
            }
            id = id.next_lexicographical();
        }

        fn get_sst_size(path: &Path) -> u64 {
            let mut size = 0;
            for entry in std::fs::read_dir(path).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if let Some(ext) = path.extension() {
                    if ext != "sst" {
                        continue;
                    }
                    size += std::fs::metadata(path).unwrap().len();
                }
            }
            size
        }

        let db_path = tmp_dir.path().join("perpetual");
        let start = ObjectKey(ObjectId::ZERO, Version::MIN_VALID_INCL);
        let end = ObjectKey(ObjectId::MAX, Version::MAX_VALID_EXCL);

        perpetual_db.objects.compact_range(&start, &end)?;
        let before_compaction_size = get_sst_size(&db_path);

        let mut batch = perpetual_db.objects.batch();
        batch.delete_batch(
            &perpetual_db.objects,
            to_delete
                .into_iter()
                .map(|(id, version)| ObjectKey(id, version)),
        )?;
        batch.write()?;

        perpetual_db.objects.compact_range(&start, &end)?;
        let after_compaction_size = get_sst_size(&db_path);

        info!(
            "Before compaction disk size = {:?}, after compaction disk size = {:?}",
            before_compaction_size, after_compaction_size
        );
        ma::assert_le!(after_compaction_size, before_compaction_size);
        Ok(())
    }

    /// A legacy V1 row reaching the objects compaction filter (a pre-V2 object
    /// left on disk after an in-place upgrade or a V1 formal-snapshot restore)
    /// must be migrated before `into_inner()`, which panics on an un-migrated
    /// V1 wrapper.
    #[tokio::test]
    async fn compaction_filter_handles_legacy_v1_row() {
        use bincode::Options;
        use iota_sdk_types::Owner;
        use typed_store::rocksdb::compaction_filter::Decision;

        use super::ObjectsCompactionFilter;
        use crate::authority::{
            authority_store_tables::AuthorityPrunerTables,
            authority_store_types::{StoreData, StoreObjectV1, StoreObjectValue},
        };

        // A V1 `Value` row is what a pre-V2 binary wrote for a live object;
        // only `Value` rows reach the tombstone lookup.
        let object_key = ObjectKey(ObjectId::random(), Version::from_u64(1));
        let v1_value = StoreObjectValue {
            data: StoreData::Coin(42),
            owner: Owner::Immutable,
            previous_transaction: TransactionDigest::random(),
            storage_rebate: 7,
        };
        let key_bytes = bincode::DefaultOptions::new()
            .with_big_endian()
            .with_fixint_encoding()
            .serialize(&object_key)
            .unwrap();
        let value_bytes = bcs::to_bytes(&StoreObjectWrapper::V1(StoreObjectV1::Value(Box::new(
            v1_value,
        ))))
        .unwrap();

        // The filter holds only a `Weak`, so keep a strong ref alive for the
        // tombstone lookup to run.
        let tmp_dir = iota_common::tempdir();
        let pruner_db = Arc::new(AuthorityPrunerTables::open(tmp_dir.path()));
        let mut filter = ObjectsCompactionFilter::new(pruner_db.clone(), &Registry::default());

        // No tombstone: the row must survive.
        let decision = filter
            .filter(&key_bytes, &value_bytes)
            .expect("legacy V1 row must not panic");
        assert!(matches!(decision, Decision::Keep));

        // Tombstoned at this version: the row must be compacted away, which
        // proves the migrated row reached the tombstone-lookup branch.
        pruner_db
            .object_tombstones
            .insert(&object_key.0, &object_key.1)
            .unwrap();
        let decision = filter
            .filter(&key_bytes, &value_bytes)
            .expect("legacy V1 row must not panic");
        assert!(matches!(decision, Decision::Remove));
    }

    /// Builds a single-epoch chain of checkpoints with the given timestamps,
    /// runs checkpoint pruning with the provided ceiling / retention window /
    /// cutoff, and returns the resulting `HighestPruned` watermark.
    async fn run_checkpoint_pruning(
        timestamps_ms: &[CheckpointTimestamp],
        max_eligible_checkpoint: CheckpointSequenceNumber,
        cutoff_timestamp_ms: CheckpointTimestamp,
        num_epochs_to_retain: u64,
    ) -> Option<CheckpointSequenceNumber> {
        run_checkpoint_pruning_with_metrics(
            timestamps_ms,
            max_eligible_checkpoint,
            cutoff_timestamp_ms,
            num_epochs_to_retain,
            AuthorityStorePruningMetrics::new_for_test(),
        )
        .await
    }

    /// Like [`run_checkpoint_pruning`], but with the metrics under the
    /// caller's control. Note that all fixture checkpoints share one
    /// empty-contents digest, which the run deletes with its first batch, so
    /// only single-batch runs are meaningful here.
    async fn run_checkpoint_pruning_with_metrics(
        timestamps_ms: &[CheckpointTimestamp],
        max_eligible_checkpoint: CheckpointSequenceNumber,
        cutoff_timestamp_ms: CheckpointTimestamp,
        num_epochs_to_retain: u64,
        metrics: Arc<AuthorityStorePruningMetrics>,
    ) -> Option<CheckpointSequenceNumber> {
        let perpetual_dir = iota_common::tempdir();
        let perpetual_db = Arc::new(AuthorityPerpetualTables::open(perpetual_dir.path(), None));
        let checkpoint_store = CheckpointStore::new_for_tests();

        let committee = CommitteeFixture::generate(rand::rngs::OsRng, 0, 4);
        let checkpoints = committee.make_checkpoints_with_timestamps(timestamps_ms, None);

        // All empty checkpoints share the same content digest, so a single
        // insert covers every checkpoint's content lookup during pruning.
        checkpoint_store
            .insert_checkpoint_contents(empty_contents().into_inner().into_checkpoint_contents())
            .unwrap();
        for checkpoint in &checkpoints {
            checkpoint_store
                .insert_certified_checkpoint(checkpoint)
                .unwrap();
        }
        checkpoint_store
            .update_highest_executed_checkpoint(checkpoints.last().unwrap())
            .unwrap();

        AuthorityStorePruner::prune_for_eligible_epochs(
            &perpetual_db,
            &checkpoint_store,
            num_epochs_to_retain,
            0,
            max_eligible_checkpoint,
            cutoff_timestamp_ms,
            metrics,
            None,
        )
        .await
        .unwrap();

        checkpoint_store
            .get_highest_pruned_checkpoint_seq_number()
            .unwrap()
    }

    // Checkpoints 1..=9 with timestamps 1000..=9000. The cutoff at 5000 prunes
    // through checkpoint 5 and stops at 6 (timestamp 6000 > 5000).
    #[tokio::test]
    async fn test_checkpoint_pruning_stops_at_timestamp_cutoff() {
        let timestamps: Vec<_> = (1..=9).map(|i| i * 1000).collect();
        let pruned = run_checkpoint_pruning(&timestamps, u64::MAX, 5000, 0).await;
        assert_eq!(pruned, Some(5));
    }

    // A cutoff below every checkpoint's timestamp prunes nothing: the first
    // checkpoint (timestamp 1000 > 0) already exceeds the cutoff.
    #[tokio::test]
    async fn test_checkpoint_pruning_cutoff_before_all() {
        let timestamps: Vec<_> = (1..=9).map(|i| i * 1000).collect();
        let pruned = run_checkpoint_pruning(&timestamps, u64::MAX, 0, 0).await;
        assert_eq!(pruned, None);
    }

    // With a cutoff past every timestamp, pruning is instead bounded by the hard
    // ceiling: it stops at max_eligible_checkpoint (7), pruning through 6.
    #[tokio::test]
    async fn test_checkpoint_pruning_bounded_by_max_eligible() {
        let timestamps: Vec<_> = (1..=9).map(|i| i * 1000).collect();
        let pruned = run_checkpoint_pruning(&timestamps, 7, u64::MAX, 0).await;
        assert_eq!(pruned, Some(6));
    }

    // Duplicate timestamps at the boundary: every checkpoint with timestamp
    // <= cutoff prunes; the first one strictly greater stops the run.
    #[tokio::test]
    async fn test_checkpoint_pruning_duplicate_boundary_timestamps() {
        let timestamps = [1000, 2000, 2000, 2000, 3000, 4000];
        let pruned = run_checkpoint_pruning(&timestamps, u64::MAX, 2000, 0).await;
        assert_eq!(pruned, Some(4));
    }

    // The epoch-count guard is the hard floor: with a retention window of one
    // epoch and all checkpoints in the current epoch, nothing prunes regardless
    // of how permissive the timestamp cutoff and ceiling are.
    #[tokio::test]
    async fn test_checkpoint_pruning_epoch_guard_takes_precedence() {
        let timestamps: Vec<_> = (1..=9).map(|i| i * 1000).collect();
        let pruned = run_checkpoint_pruning(&timestamps, u64::MAX, u64::MAX, 1).await;
        assert_eq!(pruned, None);
    }

    // A pruning pass publishes the timestamp of the last checkpoint it pruned,
    // ending at the timestamp of the checkpoint the run stopped at.
    #[tokio::test]
    async fn test_pruning_publishes_last_pruned_timestamp() {
        let timestamps: Vec<_> = (1..=9).map(|i| i * 1000).collect();
        let metrics = AuthorityStorePruningMetrics::new_for_test();
        let pruned =
            run_checkpoint_pruning_with_metrics(&timestamps, u64::MAX, 5000, 0, metrics.clone())
                .await;
        assert_eq!(pruned, Some(5));
        assert_eq!(
            metrics.last_pruned_effects_checkpoint_timestamp_ms.get(),
            5000
        );
    }

    // A nudge wakes the pruner task's subscription.
    #[tokio::test]
    async fn test_nudge_wakes_subscriber() {
        let pruner = AuthorityStorePruner {
            _pruner_cancel_handle: oneshot::channel().0,
            executed: watch::channel(0).0,
        };
        let mut rx = pruner.executed.subscribe();
        pruner.nudge(42);
        rx.changed().await.expect("nudge should notify subscriber");
        assert_eq!(*rx.borrow(), 42);
    }
}
