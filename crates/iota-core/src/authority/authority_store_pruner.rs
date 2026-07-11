// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    cmp::{max, min},
    collections::{BTreeSet, HashMap},
    sync::{Arc, Mutex, Weak},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::anyhow;
use bincode::Options;
use iota_archival::reader::ArchiveReaderBalancer;
use iota_config::node::AuthorityStorePruningConfig;
use iota_metrics::{monitored_scope, spawn_monitored_task};
use iota_sdk_types::ObjectId;
use iota_types::{
    base_types::{SequenceNumber, VersionNumber},
    committee::EpochId,
    digests::TransactionDigest,
    effects::{TransactionEffects, TransactionEffectsAPI, TransactionEffectsExt},
    messages_checkpoint::{
        CheckpointContents, CheckpointContentsExt, CheckpointDigest, CheckpointSequenceNumber,
        CheckpointTimestamp,
    },
    storage::ObjectKey,
};
use once_cell::sync::Lazy;
use prometheus_filtered::{
    IntCounter, IntGauge, Registry, register_int_counter_with_registry,
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
    rocks::DBBatch,
    rocksdb::{LiveFile, compaction_filter::Decision},
};

use super::authority_store_tables::{AuthorityPerpetualTables, AuthorityPrunerTables};
use crate::{
    authority::{
        authority_store_types::{StoreObject, StoreObjectWrapper},
        historic_store::HistoricStore,
    },
    checkpoint_progress_tracker::CheckpointProgressTracker,
    checkpoints::{CheckpointStore, CheckpointWatermark},
    grpc_indexes::GrpcIndexesStore,
    jsonrpc_index::IndexStore,
};

static PERIODIC_PRUNING_TABLES: Lazy<BTreeSet<String>> = Lazy::new(|| {
    [
        "objects",
        "effects",
        "transactions",
        "events",
        "executed_effects",
        "executed_transactions_to_checkpoint",
    ]
    .into_iter()
    .map(|cf| cf.to_string())
    .collect()
});
pub const EPOCH_DURATION_MS_FOR_TESTING: u64 = 24 * 60 * 60 * 1000;
pub const MIN_EPOCHS_TO_RETAIN_FOR_INDEXES: u64 = 7;

/// Maximum number of checkpoints whose data is written in a single pruning
/// `WriteBatch`. Bounds batch memory only; it does not cap total work per run,
/// so it cannot cause the pruner to fall behind.
const MAX_CHECKPOINTS_IN_BATCH: usize = 10;
/// Maximum number of transactions whose effects are written in a single pruning
/// `WriteBatch`. Bounds batch memory only (see [`MAX_CHECKPOINTS_IN_BATCH`]).
const MAX_TRANSACTIONS_IN_BATCH: usize = 1000;

/// Chain-time slack, in milliseconds, allowed on top of the retention window
/// before the checkpoint executor is throttled by the pruner's leash
/// (`AuthorityStorePruner::await_leash`). It
/// absorbs transient bursts of high-contention checkpoints so execution runs at
/// the average prune rate rather than the peak; under sustained overload the
/// retained span stabilizes at `window + PRUNING_LEASH_SLACK_MS`, which is
/// negligible next to a multi-epoch window.
const PRUNING_LEASH_SLACK_MS: u64 = 60 * 60 * 1000;

/// While catching up (see [`PRUNING_DEBOUNCE_MIN_LAG`]), after a nudge wakes
/// the pruner it waits this long before draining so that more executed
/// checkpoints accumulate and their object deletions coalesce into larger,
/// fewer batches — which measurably improves catch-up throughput. Negligible
/// against the leash slack, so it never risks throttling execution.
const PRUNING_NUDGE_DEBOUNCE: Duration = Duration::from_millis(1000);

/// The debounce above is only applied while the node is catching up, i.e. when
/// execution lags the highest synced checkpoint by more than this many
/// checkpoints. Near the tip the lag is tiny, so pruning stays prompt
/// (per-checkpoint) and does not incur the debounce delay.
const PRUNING_DEBOUNCE_MIN_LAG: u64 = 100;

/// The `AuthorityStorePruner` manages the pruning process for object stores
/// within the `AuthorityStore`. It includes a cancellation handle that can be
/// used to stop the pruning task for objects.
///
/// It also owns the coordination channels between the checkpoint executor
/// (producer of new state) and the pruner task (consumer of aged-out state):
/// pruning is driven by execution progress rather than a timer — the executor
/// nudges after each checkpoint is made available, and the pruner drains fully
/// to its chain-time retention cutoff on every nudge. To keep on-disk state
/// bounded without a per-run rate cap (which could silently let the database
/// grow under sustained load), the executor is *leashed*: it stops scheduling
/// checkpoints while the pruner has fallen more than `PRUNING_LEASH_SLACK_MS`
/// behind its retention target.
pub struct AuthorityStorePruner {
    _objects_pruner_cancel_handle: oneshot::Sender<()>,
    /// Executor -> pruner: latest executed checkpoint sequence number. Updating
    /// it both records progress and wakes the pruner task to drain.
    executed: watch::Sender<CheckpointSequenceNumber>,
    /// Pruner -> executor: the executed-checkpoint timestamp the pruner has
    /// caught up to (the `highest_executed` it observed on its last completed
    /// drain). The leash throttles execution while it runs more than
    /// `PRUNING_LEASH_SLACK_MS` of chain-time ahead of this, i.e. ahead of the
    /// pruner's last completed drain. Initialized to `u64::MAX` so the executor
    /// is never leashed before the pruner has published a real value.
    frontier_ms: watch::Sender<CheckpointTimestamp>,
}

impl AuthorityStorePruner {
    /// Called by the executor after a checkpoint has been executed and made
    /// available (watermark bumped, subscribers notified). Wakes the pruner.
    pub fn nudge(&self, executed_seq: CheckpointSequenceNumber) {
        self.executed.send_replace(executed_seq);
    }

    /// Called by the executor before scheduling a checkpoint, passing the
    /// timestamp of the current highest-executed checkpoint. Returns once the
    /// pruner has caught up to within `PRUNING_LEASH_SLACK_MS` of chain-time of
    /// that executed watermark, throttling execution otherwise.
    ///
    /// The argument is the *executed* watermark, never the candidate
    /// checkpoint's timestamp: the pruner's frontier only ever advances to
    /// timestamps that have already executed, so gating on a not-yet-executed
    /// candidate could deadlock across a large chain-time gap between
    /// checkpoints.
    pub async fn await_leash(&self, executed_timestamp_ms: CheckpointTimestamp) {
        let mut rx = self.frontier_ms.subscribe();
        while executed_timestamp_ms.saturating_sub(*rx.borrow_and_update()) > PRUNING_LEASH_SLACK_MS
        {
            // `changed()` cannot error: the sender lives in `self`, which is
            // borrowed for the duration of this call.
            let _ = rx.changed().await;
        }
    }
}

/// The `AuthorityStorePruningMetrics` tracks various metrics related to the
/// pruning process of the `AuthorityStore`.
pub struct AuthorityStorePruningMetrics {
    pub last_pruned_checkpoint: IntGauge,
    pub num_pruned_objects: IntCounter,
    pub num_pruned_tombstones: IntCounter,
    pub last_pruned_effects_checkpoint: IntGauge,
    pub last_pruned_indexes_transaction: IntGauge,
    pub num_epochs_to_retain_for_objects: IntGauge,
    pub num_epochs_to_retain_for_checkpoints: IntGauge,
}

impl AuthorityStorePruningMetrics {
    /// Initializes a new instance of `AuthorityStorePruningMetrics` with the
    /// provided registry, registering various metrics that track the pruning
    /// operations in the `AuthorityStore`.
    pub fn new(registry: &Registry) -> Arc<Self> {
        let this = Self {
            last_pruned_checkpoint: register_int_gauge_with_registry!(
                "last_pruned_checkpoint",
                "Last pruned checkpoint",
                registry
            )
            .unwrap(),
            num_pruned_objects: register_int_counter_with_registry!(
                "num_pruned_objects",
                "Number of pruned objects",
                registry
            )
            .unwrap(),
            num_pruned_tombstones: register_int_counter_with_registry!(
                "num_pruned_tombstones",
                "Number of pruned tombstones",
                registry
            )
            .unwrap(),
            last_pruned_effects_checkpoint: register_int_gauge_with_registry!(
                "last_pruned_effects_checkpoint",
                "Last pruned effects checkpoint",
                registry
            )
            .unwrap(),
            last_pruned_indexes_transaction: register_int_gauge_with_registry!(
                "last_pruned_indexes_transaction",
                "Last pruned indexes transaction",
                registry
            )
            .unwrap(),
            num_epochs_to_retain_for_objects: register_int_gauge_with_registry!(
                "num_epochs_to_retain_for_objects",
                "Number of epochs to retain for objects",
                registry
            )
            .unwrap(),
            num_epochs_to_retain_for_checkpoints: register_int_gauge_with_registry!(
                "num_epochs_to_retain_for_checkpoints",
                "Number of epochs to retain for checkpoints",
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

/// Pruning modes for the `AuthorityStore`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PruningMode {
    Objects,
    Checkpoints,
}

/// Relocation target for one epoch-homogeneous pruning batch: instead of
/// deleting superseded object versions, move them into the historic store
/// bucket of the epoch whose checkpoints superseded them.
struct HistoricRelocation<'a> {
    store: &'a Arc<HistoricStore>,
    supersession_epoch: EpochId,
}

impl AuthorityStorePruner {
    /// prunes old versions of objects based on transaction effects
    async fn prune_objects(
        transaction_effects: Vec<TransactionEffects>,
        perpetual_db: &Arc<AuthorityPerpetualTables>,
        pruner_db: Option<&Arc<AuthorityPrunerTables>>,
        relocation: Option<HistoricRelocation<'_>>,
        checkpoint_number: CheckpointSequenceNumber,
        metrics: Arc<AuthorityStorePruningMetrics>,
    ) -> anyhow::Result<()> {
        let _scope = monitored_scope("ObjectsLivePruner");
        let mut wb = perpetual_db.objects.batch();
        let mut pruner_db_wb = pruner_db.map(|db| db.object_tombstones.batch());

        // Collect objects keys that need to be deleted from `transaction_effects`.
        let mut live_object_keys_to_prune = vec![];
        let mut object_tombstones_to_prune = vec![];
        for effects in &transaction_effects {
            for (object_id, seq_number) in effects.modified_at_versions() {
                live_object_keys_to_prune.push(ObjectKey(object_id, seq_number));
            }

            for deleted_object_key in effects.all_tombstones() {
                object_tombstones_to_prune
                    .push(ObjectKey(deleted_object_key.0, deleted_object_key.1));
            }
        }

        metrics
            .num_pruned_objects
            .inc_by(live_object_keys_to_prune.len() as u64);
        metrics
            .num_pruned_tombstones
            .inc_by(object_tombstones_to_prune.len() as u64);

        if let Some(relocation) = relocation {
            debug_assert!(
                pruner_db.is_none(),
                "the compaction filter and the historic store are mutually exclusive"
            );
            Self::relocate_objects(
                &mut wb,
                perpetual_db,
                &relocation,
                live_object_keys_to_prune,
                object_tombstones_to_prune,
            )?;
            perpetual_db.set_highest_pruned_checkpoint(&mut wb, checkpoint_number)?;
            metrics.last_pruned_checkpoint.set(checkpoint_number as i64);
            wb.write()?;
            return Ok(());
        }

        let mut updates: HashMap<ObjectId, (VersionNumber, VersionNumber)> = HashMap::new();
        for ObjectKey(object_id, seq_number) in live_object_keys_to_prune {
            updates
                .entry(object_id)
                .and_modify(|range| *range = (min(range.0, seq_number), max(range.1, seq_number)))
                .or_insert((seq_number, seq_number));
        }

        for (object_id, (min_version, max_version)) in updates {
            debug!(
                "Pruning object {:?} versions {:?} - {:?}",
                object_id, min_version, max_version
            );
            match pruner_db_wb {
                Some(ref mut batch) => {
                    batch.insert_batch(
                        &pruner_db.expect("invariant checked").object_tombstones,
                        std::iter::once((object_id, max_version)),
                    )?;
                }
                None => {
                    let start_range = ObjectKey(object_id, min_version);
                    let end_range = ObjectKey(object_id, max_version + 1);
                    wb.schedule_delete_range(&perpetual_db.objects, &start_range, &end_range)?;
                }
            }
        }

        // Instead of using range deletes, we
        // need to do a scan of all the keys for the deleted objects and then do
        // point deletes to delete all the existing keys. This is because using
        // range delete to delete tombstones may leak objects (imagine a tombstone
        // is compacted away, but earlier version is still not). Using point
        // deletes guarantees that all earlier versions are deleted in the
        // database.
        if !object_tombstones_to_prune.is_empty() {
            let mut object_keys_to_delete = vec![];
            for ObjectKey(object_id, seq_number) in object_tombstones_to_prune {
                for result in perpetual_db.objects.safe_iter_with_bounds(
                    Some(ObjectKey(object_id, VersionNumber::MIN_VALID_INCL)),
                    Some(ObjectKey(object_id, seq_number.next().unwrap())),
                ) {
                    let (object_key, _) = result?;
                    assert_eq!(object_key.0, object_id);
                    object_keys_to_delete.push(object_key);
                }
            }

            wb.delete_batch(&perpetual_db.objects, object_keys_to_delete)?;
        }

        perpetual_db.set_highest_pruned_checkpoint(&mut wb, checkpoint_number)?;
        metrics.last_pruned_checkpoint.set(checkpoint_number as i64);

        if let Some(batch) = pruner_db_wb {
            batch.write()?;
        }
        wb.write()?;
        Ok(())
    }

    /// Moves superseded object versions into the historic epoch bucket
    /// instead of deleting them: the copies and the point deletes of the
    /// relocated keys go into the same `wb`, so relocation commits
    /// atomically with the watermark advance (the bucket's column families
    /// belong to the same database).
    ///
    /// This is the backstop behind commit-time relocation: rows already
    /// moved by checkpoint commit (or a previous run) come back absent from
    /// the `multi_get` and are skipped, which keeps replay idempotent. It
    /// does the full work only for backlog predating the feature.
    ///
    /// Tombstone heads (`Deleted`/`Wrapped`) are *not* relocated and *not*
    /// deleted: they are the newest version of their lineage, and every
    /// latest-version read depends on them staying in the live table. They
    /// are recorded in the bucket's expiry list and point-deleted from the
    /// live table only when the whole bucket expires. The versions *below* a
    /// tombstone need no lineage scan here: each of them was relocated by the
    /// effects of the transaction that superseded it (deletion consumes its
    /// input version like any other mutation).
    fn relocate_objects(
        wb: &mut DBBatch,
        perpetual_db: &Arc<AuthorityPerpetualTables>,
        relocation: &HistoricRelocation<'_>,
        live_object_keys_to_prune: Vec<ObjectKey>,
        tombstone_heads: Vec<ObjectKey>,
    ) -> anyhow::Result<()> {
        let values = perpetual_db
            .objects
            .multi_get(live_object_keys_to_prune.iter())?;
        let rows: Vec<_> = live_object_keys_to_prune
            .into_iter()
            .zip(values)
            .filter_map(|(key, value)| value.map(|value| (key, value)))
            .collect();

        relocation
            .store
            .prepare_bucket(relocation.supersession_epoch)?;
        relocation.store.stage_objects(
            wb,
            relocation.supersession_epoch,
            &rows,
            &tombstone_heads,
        )?;
        wb.delete_batch(&perpetual_db.objects, rows.iter().map(|(key, _)| *key))?;
        Ok(())
    }

    /// Stages one epoch-homogeneous batch of checkpoint-keyed history into
    /// the historic bucket of the checkpoints' epoch, inside the same
    /// perpetual-store batch that deletes the perpetual-side source rows.
    /// The checkpoint store's deletes live in a separate database and are
    /// committed after the perpetual batch; a crash in between leaves
    /// harmless duplicates that the idempotent replay overwrites.
    ///
    /// Rows already deleted by a previous run are skipped, which keeps
    /// replay idempotent: their historic copies were already written.
    fn relocate_checkpoint_data(
        perpetual_batch: &mut DBBatch,
        perpetual_db: &Arc<AuthorityPerpetualTables>,
        checkpoint_db: &Arc<CheckpointStore>,
        relocation: &HistoricRelocation<'_>,
        transactions: &[TransactionDigest],
        checkpoints_to_prune: &[CheckpointDigest],
        checkpoint_content_to_prune: &[CheckpointContents],
        effects_to_prune: &[TransactionEffects],
    ) -> anyhow::Result<()> {
        fn present<K: Copy, V>(keys: &[K], values: Vec<Option<V>>) -> Vec<(K, V)> {
            keys.iter()
                .copied()
                .zip(values)
                .filter_map(|(key, value)| value.map(|value| (key, value)))
                .collect()
        }

        let checkpoint_summaries = present(
            checkpoints_to_prune,
            checkpoint_db
                .tables
                .checkpoint_by_digest
                .multi_get(checkpoints_to_prune.iter())?,
        );
        // The contents-digest-to-sequence-number rows are derived from the
        // summaries instead of read from
        // `checkpoint_sequence_by_contents_digest`, whose rows are already
        // deleted after state accumulation.
        let checkpoint_seq_by_contents: Vec<_> = checkpoint_summaries
            .iter()
            .map(|(_, summary)| {
                let summary = summary.inner();
                (summary.content_digest, summary.sequence_number)
            })
            .collect();
        let checkpoint_range = checkpoint_summaries
            .iter()
            .map(|(_, summary)| summary.inner().sequence_number)
            .fold(None, |range: Option<(u64, u64)>, seq| {
                Some(range.map_or((seq, seq), |(min, max)| (min.min(seq), max.max(seq))))
            });

        let data = crate::authority::historic_store::CheckpointHistoryBatch {
            transactions: present(
                transactions,
                perpetual_db.transactions.multi_get(transactions.iter())?,
            ),
            effects: effects_to_prune
                .iter()
                .map(|effects| (effects.digest(), effects.clone()))
                .collect(),
            executed_effects: present(
                transactions,
                perpetual_db
                    .executed_effects
                    .multi_get(transactions.iter())?,
            ),
            events: present(
                transactions,
                perpetual_db.events_2.multi_get(transactions.iter())?,
            ),
            checkpoint_contents: checkpoint_content_to_prune
                .iter()
                .map(|contents| (contents.digest(), contents.clone()))
                .collect(),
            checkpoint_seq_by_contents,
            checkpoints: checkpoint_summaries,
            checkpoint_range,
        };
        relocation
            .store
            .prepare_bucket(relocation.supersession_epoch)?;
        relocation.store.stage_checkpoint_data(
            perpetual_batch,
            relocation.supersession_epoch,
            data,
        )?;
        Ok(())
    }

    /// Prunes checkpoint-related data from the `AuthorityStore`, including
    /// transaction effects, executed transactions, and checkpoint contents,
    /// based on the specified checkpoint number and list of checkpoints to
    /// prune. This function removes outdated data, updates pruning metrics,
    /// and maintains database consistency by updating watermarks.
    ///
    /// With `relocation` set, the checkpoint-keyed history (transactions,
    /// effects, events, checkpoint contents and summaries) is durably copied
    /// into the historic bucket of the checkpoints' epoch before the deletes
    /// are committed; on a crash in between, replay finds the not-yet-deleted
    /// rows and rewrites identical historic rows. Two families are still
    /// deleted outright: the legacy `events` table (a duplicate of `events_2`
    /// that is being migrated away) and `executed_transactions_to_checkpoint`
    /// (only consumed by the JSON-RPC read path, which does not serve
    /// historic data).
    fn prune_checkpoints(
        perpetual_db: &Arc<AuthorityPerpetualTables>,
        checkpoint_db: &Arc<CheckpointStore>,
        grpc_indexes_store: Option<&GrpcIndexesStore>,
        relocation: Option<HistoricRelocation<'_>>,
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

        if let Some(relocation) = &relocation {
            Self::relocate_checkpoint_data(
                &mut perpetual_batch,
                perpetual_db,
                checkpoint_db,
                relocation,
                &transactions,
                &checkpoints_to_prune,
                &checkpoint_content_to_prune,
                effects_to_prune,
            )?;
        }

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

            if let Some(event_digest) = effects.events_digest() {
                perpetual_batch
                    .delete_batch(&perpetual_db.events_2, [effects.transaction_digest()])?;
                if let Some(next_digest) = event_digest.next_lexicographical_opt() {
                    perpetual_batch.schedule_delete_range(
                        &perpetual_db.events,
                        &(*event_digest, 0),
                        &(next_digest, 0),
                    )?;
                }
            }
        }
        perpetual_batch.delete_batch(&perpetual_db.effects, effect_digests)?;

        let mut checkpoints_batch = checkpoint_db.tables.certified_checkpoints.batch();

        let checkpoint_content_digests =
            checkpoint_content_to_prune.iter().map(|ckpt| ckpt.digest());
        checkpoints_batch.delete_batch(
            &checkpoint_db.tables.checkpoint_content,
            checkpoint_content_digests,
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

        if let Some(grpc_indexes_store) = grpc_indexes_store {
            grpc_indexes_store.prune(checkpoint_number, &checkpoint_content_to_prune)?;
        }
        perpetual_batch.write()?;
        checkpoints_batch.write()?;
        metrics
            .last_pruned_effects_checkpoint
            .set(checkpoint_number as i64);
        Ok(())
    }

    /// Prunes old data based on effects from all checkpoints from epochs
    /// eligible for pruning
    pub async fn prune_objects_for_eligible_epochs(
        perpetual_db: &Arc<AuthorityPerpetualTables>,
        checkpoint_store: &Arc<CheckpointStore>,
        grpc_indexes_store: Option<&GrpcIndexesStore>,
        pruner_db: Option<&Arc<AuthorityPrunerTables>>,
        historic_store: Option<&Arc<HistoricStore>>,
        config: AuthorityStorePruningConfig,
        metrics: Arc<AuthorityStorePruningMetrics>,
        epoch_duration_ms: u64,
        progress_tracker: Option<&Arc<CheckpointProgressTracker>>,
    ) -> anyhow::Result<()> {
        let _scope = monitored_scope("PruneObjectsForEligibleEpochs");
        let (max_eligible_checkpoint_number, cutoff_timestamp_ms) = checkpoint_store
            .get_highest_executed_checkpoint()?
            .map(|c| {
                let window_ms = config
                    .num_epochs_to_retain
                    .saturating_mul(epoch_duration_ms);
                (
                    c.sequence_number(),
                    c.timestamp_ms.saturating_sub(window_ms),
                )
            })
            .unwrap_or_default();
        let pruned_checkpoint_number = perpetual_db
            .get_highest_pruned_checkpoint()?
            .unwrap_or_default();
        // The pruning mode that lags decides when an epoch bucket is complete
        // and seals it: the checkpoint pruner's eligibility is capped at the
        // objects watermark, so it always trails the objects pruner. Only
        // when checkpoint pruning is disabled does the objects pruner seal.
        let seals_buckets = matches!(
            config.num_epochs_to_retain_for_checkpoints(),
            None | Some(u64::MAX) | Some(0)
        );
        Self::prune_for_eligible_epochs(
            perpetual_db,
            checkpoint_store,
            grpc_indexes_store,
            pruner_db,
            historic_store,
            seals_buckets,
            PruningMode::Objects,
            config.num_epochs_to_retain,
            pruned_checkpoint_number,
            max_eligible_checkpoint_number,
            cutoff_timestamp_ms,
            metrics.clone(),
            progress_tracker,
        )
        .await
    }

    /// Asynchronously prunes checkpoint data for eligible epochs based on the
    /// configuration and current state of the `AuthorityStore`. This
    /// function determines the range of checkpoints that can be pruned,
    /// taking into account retention policies, archival watermarks, and the
    /// chain-time retention cutoff. It then delegates the pruning to the
    /// `prune_for_eligible_epochs` method.
    /// The function also updates pruning metrics and ensures proper handling of
    /// indirect objects.
    pub async fn prune_checkpoints_for_eligible_epochs(
        perpetual_db: &Arc<AuthorityPerpetualTables>,
        checkpoint_store: &Arc<CheckpointStore>,
        grpc_indexes_store: Option<&GrpcIndexesStore>,
        pruner_db: Option<&Arc<AuthorityPrunerTables>>,
        historic_store: Option<&Arc<HistoricStore>>,
        config: AuthorityStorePruningConfig,
        metrics: Arc<AuthorityStorePruningMetrics>,
        archive_readers: ArchiveReaderBalancer,
        epoch_duration_ms: u64,
        progress_tracker: Option<&Arc<CheckpointProgressTracker>>,
    ) -> anyhow::Result<()> {
        let _scope = monitored_scope("PruneCheckpointsForEligibleEpochs");
        let pruned_checkpoint_number = checkpoint_store
            .get_highest_pruned_checkpoint_seq_number()?
            .unwrap_or(0);
        let (last_executed_checkpoint, last_executed_timestamp_ms) = checkpoint_store
            .get_highest_executed_checkpoint()?
            .map(|c| (c.sequence_number(), c.timestamp_ms))
            .unwrap_or_default();
        let latest_archived_checkpoint = archive_readers
            .get_archive_watermark()
            .await?
            .unwrap_or(u64::MAX);
        let mut max_eligible_checkpoint = min(latest_archived_checkpoint, last_executed_checkpoint);
        if config.num_epochs_to_retain != u64::MAX {
            max_eligible_checkpoint = min(
                max_eligible_checkpoint,
                perpetual_db
                    .get_highest_pruned_checkpoint()?
                    .unwrap_or_default(),
            );
        }
        let num_epochs_to_retain = config
            .num_epochs_to_retain_for_checkpoints()
            .ok_or_else(|| anyhow!("config value not set"))?;
        let cutoff_timestamp_ms = last_executed_timestamp_ms
            .saturating_sub(num_epochs_to_retain.saturating_mul(epoch_duration_ms));
        debug!("Max eligible checkpoint {}", max_eligible_checkpoint);
        Self::prune_for_eligible_epochs(
            perpetual_db,
            checkpoint_store,
            grpc_indexes_store,
            pruner_db,
            historic_store,
            // The checkpoint pruner always seals: its eligibility is capped
            // at the objects watermark, so it is the lagging pruning mode.
            true,
            PruningMode::Checkpoints,
            num_epochs_to_retain,
            pruned_checkpoint_number,
            max_eligible_checkpoint,
            cutoff_timestamp_ms,
            metrics.clone(),
            progress_tracker,
        )
        .await
    }

    /// Prunes old object versions based on effects from all checkpoints from
    /// epochs eligible for pruning
    pub async fn prune_for_eligible_epochs(
        perpetual_db: &Arc<AuthorityPerpetualTables>,
        checkpoint_store: &Arc<CheckpointStore>,
        grpc_indexes_store: Option<&GrpcIndexesStore>,
        pruner_db: Option<&Arc<AuthorityPrunerTables>>,
        historic_store: Option<&Arc<HistoricStore>>,
        seals_buckets: bool,
        mode: PruningMode,
        num_epochs_to_retain: u64,
        starting_checkpoint_number: CheckpointSequenceNumber,
        max_eligible_checkpoint: CheckpointSequenceNumber,
        cutoff_timestamp_ms: CheckpointTimestamp,
        metrics: Arc<AuthorityStorePruningMetrics>,
        progress_tracker: Option<&Arc<CheckpointProgressTracker>>,
    ) -> anyhow::Result<()> {
        let _scope = monitored_scope("PruneForEligibleEpochs");

        let mut checkpoint_number = starting_checkpoint_number;
        let current_epoch = checkpoint_store
            .get_highest_executed_checkpoint()?
            .map(|c| c.epoch())
            .unwrap_or_default();

        let mut checkpoints_to_prune = vec![];
        let mut checkpoint_content_to_prune = vec![];
        let mut effects_to_prune = vec![];
        let mut batch_epoch: Option<EpochId> = None;

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

            // With relocation enabled a batch must not span epochs: relocated
            // rows are bucketed by the epoch of their checkpoint. Flush the
            // pending batch before crossing the boundary and seal the
            // finished epoch's bucket.
            if let Some(store) = historic_store {
                match batch_epoch {
                    Some(epoch) if epoch != checkpoint.epoch() => {
                        if !checkpoints_to_prune.is_empty() {
                            Self::prune_batch(
                                perpetual_db,
                                checkpoint_store,
                                grpc_indexes_store,
                                pruner_db,
                                Some(HistoricRelocation {
                                    store,
                                    supersession_epoch: epoch,
                                }),
                                mode,
                                checkpoint_number,
                                std::mem::take(&mut checkpoints_to_prune),
                                std::mem::take(&mut checkpoint_content_to_prune),
                                std::mem::take(&mut effects_to_prune),
                                metrics.clone(),
                            )
                            .await?;
                            if let Some(tracker) = progress_tracker {
                                let elapsed = pruning_start.elapsed();
                                match mode {
                                    PruningMode::Objects => {
                                        tracker.add_object_pruning_time(elapsed)
                                    }
                                    PruningMode::Checkpoints => {
                                        tracker.add_checkpoint_pruning_time(elapsed)
                                    }
                                }
                                pruning_start = Instant::now();
                            }
                        }
                        if seals_buckets {
                            store.seal_epoch(epoch)?;
                        }
                    }
                    None if seals_buckets => {
                        // A previous run may have finished exactly at an
                        // epoch boundary without sealing; catch up on any
                        // unsealed earlier buckets.
                        for epoch in store.list_epochs() {
                            if epoch >= checkpoint.epoch() {
                                break;
                            }
                            if !store.is_sealed(epoch)? {
                                store.seal_epoch(epoch)?;
                            }
                        }
                    }
                    _ => {}
                }
            }
            batch_epoch = Some(checkpoint.epoch());
            checkpoint_number = checkpoint.sequence_number();

            let content = checkpoint_store
                .get_checkpoint_contents(&checkpoint.content_digest)?
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
                Self::prune_batch(
                    perpetual_db,
                    checkpoint_store,
                    grpc_indexes_store,
                    pruner_db,
                    historic_store.map(|store| HistoricRelocation {
                        store,
                        supersession_epoch: batch_epoch
                            .expect("batch epoch is set before batching"),
                    }),
                    mode,
                    checkpoint_number,
                    std::mem::take(&mut checkpoints_to_prune),
                    std::mem::take(&mut checkpoint_content_to_prune),
                    std::mem::take(&mut effects_to_prune),
                    metrics.clone(),
                )
                .await?;

                // Report pruning time for this batch so the progress logger
                // shows time alongside the checkpoint deltas it reads from the
                // DB (which are already updated at this point).
                if let Some(tracker) = progress_tracker {
                    let elapsed = pruning_start.elapsed();
                    match mode {
                        PruningMode::Objects => tracker.add_object_pruning_time(elapsed),
                        PruningMode::Checkpoints => tracker.add_checkpoint_pruning_time(elapsed),
                    }
                    pruning_start = Instant::now();
                }

                // yield back to the tokio runtime. Prevent potential halt of other tasks
                tokio::task::yield_now().await;
            }
        }

        if !checkpoints_to_prune.is_empty() {
            Self::prune_batch(
                perpetual_db,
                checkpoint_store,
                grpc_indexes_store,
                pruner_db,
                historic_store.map(|store| HistoricRelocation {
                    store,
                    supersession_epoch: batch_epoch.expect("batch epoch is set before batching"),
                }),
                mode,
                checkpoint_number,
                checkpoints_to_prune,
                checkpoint_content_to_prune,
                effects_to_prune,
                metrics.clone(),
            )
            .await?;

            // Report pruning time for this batch so the progress logger
            // shows time alongside the checkpoint deltas it reads from the
            // DB (which are already updated at this point).
            if let Some(tracker) = progress_tracker {
                let elapsed = pruning_start.elapsed();
                match mode {
                    PruningMode::Objects => tracker.add_object_pruning_time(elapsed),
                    PruningMode::Checkpoints => tracker.add_checkpoint_pruning_time(elapsed),
                }
            }
        }

        Ok(())
    }

    /// Dispatches one pruning batch to the mode-specific pruner.
    async fn prune_batch(
        perpetual_db: &Arc<AuthorityPerpetualTables>,
        checkpoint_store: &Arc<CheckpointStore>,
        grpc_indexes_store: Option<&GrpcIndexesStore>,
        pruner_db: Option<&Arc<AuthorityPrunerTables>>,
        relocation: Option<HistoricRelocation<'_>>,
        mode: PruningMode,
        checkpoint_number: CheckpointSequenceNumber,
        checkpoints_to_prune: Vec<CheckpointDigest>,
        checkpoint_content_to_prune: Vec<CheckpointContents>,
        effects_to_prune: Vec<TransactionEffects>,
        metrics: Arc<AuthorityStorePruningMetrics>,
    ) -> anyhow::Result<()> {
        match mode {
            PruningMode::Objects => {
                Self::prune_objects(
                    effects_to_prune,
                    perpetual_db,
                    pruner_db,
                    relocation,
                    checkpoint_number,
                    metrics,
                )
                .await
            }
            PruningMode::Checkpoints => Self::prune_checkpoints(
                perpetual_db,
                checkpoint_store,
                grpc_indexes_store,
                relocation,
                checkpoint_number,
                checkpoints_to_prune,
                checkpoint_content_to_prune,
                &effects_to_prune,
                metrics,
            ),
        }
    }

    /// Expires historic epoch buckets that have fallen out of retention:
    /// point-deletes the bucket's tombstone heads from the live `objects`
    /// table, then drops the whole bucket.
    ///
    /// The heads must be deleted first: a crash in between leaves the bucket
    /// (and its expiry list) in place, so the next run re-issues the
    /// idempotent deletes before dropping. The reversed order would lose the
    /// expiry list and leak the heads in the live table forever.
    fn drop_expired_historic_epochs(
        perpetual_db: &Arc<AuthorityPerpetualTables>,
        historic_store: &Arc<HistoricStore>,
        current_epoch: EpochId,
        num_epochs_to_retain: u64,
    ) -> anyhow::Result<()> {
        let _scope = monitored_scope("DropExpiredHistoricEpochs");
        for epoch in historic_store.list_epochs() {
            if current_epoch.saturating_sub(epoch) <= num_epochs_to_retain {
                break;
            }
            let tombstone_heads = historic_store.tombstone_heads(epoch)?;
            let num_heads = tombstone_heads.len();
            for chunk in tombstone_heads.chunks(10_000) {
                let mut wb = perpetual_db.objects.batch();
                wb.delete_batch(&perpetual_db.objects, chunk.iter().copied())?;
                wb.write()?;
            }
            historic_store.drop_epoch(epoch)?;
            info!(
                epoch,
                num_tombstone_heads = num_heads,
                "dropped expired historic epoch bucket"
            );
        }
        Ok(())
    }

    fn prune_indexes(
        indexes: Option<&IndexStore>,
        config: &AuthorityStorePruningConfig,
        epoch_duration_ms: u64,
        metrics: &AuthorityStorePruningMetrics,
    ) -> anyhow::Result<()> {
        if let (Some(mut epochs_to_retain), Some(indexes)) =
            (config.num_epochs_to_retain_for_indexes, indexes)
        {
            if epochs_to_retain < MIN_EPOCHS_TO_RETAIN_FOR_INDEXES {
                warn!("num_epochs_to_retain_for_indexes is too low. Resetting it to 7");
                epochs_to_retain = MIN_EPOCHS_TO_RETAIN_FOR_INDEXES;
            }
            let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
            if let Some(cut_time_ms) =
                u64::try_from(now)?.checked_sub(epochs_to_retain * epoch_duration_ms)
            {
                let transaction_id = indexes.prune(cut_time_ms)?;
                metrics
                    .last_pruned_indexes_transaction
                    .set(transaction_id as i64);
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
        grpc_indexes_store: Option<Arc<GrpcIndexesStore>>,
        jsonrpc_index: Option<Arc<IndexStore>>,
        pruner_db: Option<Arc<AuthorityPrunerTables>>,
        historic_store: Option<Arc<HistoricStore>>,
        metrics: Arc<AuthorityStorePruningMetrics>,
        archive_readers: ArchiveReaderBalancer,
        progress_tracker: Option<Arc<CheckpointProgressTracker>>,
        mut executed_rx: watch::Receiver<CheckpointSequenceNumber>,
        frontier_tx: watch::Sender<CheckpointTimestamp>,
    ) -> Sender<()> {
        let (sender, mut recv) = tokio::sync::oneshot::channel();
        debug!(
            "Starting store pruner with num_epochs_to_retain={}",
            config.num_epochs_to_retain
        );

        let historic_epochs_to_retain = config
            .historic_store
            .as_ref()
            .map(|c| c.num_epochs_to_retain)
            .unwrap_or(u64::MAX);

        // Periodic background compaction of aged SST files, independent of the
        // execution-driven pruning loop below.
        let perpetual_db_for_compaction = perpetual_db.clone();
        if let Some(delay_days) = config.periodic_compaction_threshold_days {
            spawn_monitored_task!(async move {
                let last_processed = Arc::new(Mutex::new(HashMap::new()));
                loop {
                    let db = perpetual_db_for_compaction.clone();
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

        let prune_objects = config.num_epochs_to_retain != u64::MAX;
        let prune_checkpoints = !matches!(
            config.num_epochs_to_retain_for_checkpoints(),
            None | Some(u64::MAX) | Some(0)
        );
        let prune_indexes = config.num_epochs_to_retain_for_indexes.is_some();
        // The leash only makes sense when something is actually being pruned; if
        // no pruner is enabled the frontier stays at u64::MAX and execution is
        // never throttled.
        let leash_enabled = prune_objects || prune_checkpoints;

        // Execution-driven pruning: on every nudge from the checkpoint executor,
        // drain each enabled pruner fully to its chain-time cutoff, then publish
        // the pruning frontier for the executor's leash. Draining once before the
        // first nudge handles any startup backlog. The `watch` nudge coalesces
        // many executed checkpoints into a single drain.
        tokio::task::spawn(async move {
            loop {
                // The executed position this pass prunes up to. Published as the
                // frontier once draining completes, so the leash measures how far
                // execution has run ahead of the pruner's last completed drain —
                // bounded and independent of epoch-duration variance, and free of
                // the deadlock a `pruned + window` frontier could hit when the
                // epoch guard or a mismatched `epoch_duration_ms` keeps that value
                // permanently below `executed - slack`.
                let highest_executed = checkpoint_store
                    .get_highest_executed_checkpoint()
                    .ok()
                    .flatten();
                let caught_up_to = highest_executed
                    .as_ref()
                    .map(|checkpoint| checkpoint.timestamp_ms)
                    .unwrap_or(u64::MAX);

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

                if prune_objects {
                    if let Err(err) = Self::prune_objects_for_eligible_epochs(
                        &perpetual_db,
                        &checkpoint_store,
                        grpc_indexes_store.as_deref(),
                        pruner_db.as_ref(),
                        historic_store.as_ref(),
                        config.clone(),
                        metrics.clone(),
                        epoch_duration_ms,
                        progress_tracker.as_ref(),
                    )
                    .await
                    {
                        error!("Failed to prune objects: {:?}", err);
                    }
                }
                // Expire historic epoch buckets in the same drain: a cheap
                // no-op while nothing has aged out of retention, and
                // execution-driven like the pruning steps above. Not part of
                // the leash — dropping old buckets never blocks execution.
                if let Some(store) = &historic_store {
                    let current_epoch = highest_executed
                        .as_ref()
                        .map(|checkpoint| checkpoint.epoch())
                        .unwrap_or_default();
                    if let Err(err) = Self::drop_expired_historic_epochs(
                        &perpetual_db,
                        store,
                        current_epoch,
                        historic_epochs_to_retain,
                    ) {
                        error!("Failed to drop expired historic epochs: {:?}", err);
                    }
                }
                if prune_checkpoints {
                    if let Err(err) = Self::prune_checkpoints_for_eligible_epochs(
                        &perpetual_db,
                        &checkpoint_store,
                        grpc_indexes_store.as_deref(),
                        pruner_db.as_ref(),
                        historic_store.as_ref(),
                        config.clone(),
                        metrics.clone(),
                        archive_readers.clone(),
                        epoch_duration_ms,
                        progress_tracker.as_ref(),
                    )
                    .await
                    {
                        error!("Failed to prune checkpoints: {:?}", err);
                    }
                }
                if prune_indexes {
                    if let Err(err) = Self::prune_indexes(
                        jsonrpc_index.as_deref(),
                        &config,
                        epoch_duration_ms,
                        &metrics,
                    ) {
                        error!("Failed to prune indexes: {:?}", err);
                    }
                }

                if leash_enabled {
                    frontier_tx.send_replace(caught_up_to);
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
        grpc_indexes_store: Option<Arc<GrpcIndexesStore>>,
        jsonrpc_index: Option<Arc<IndexStore>>,
        mut pruning_config: AuthorityStorePruningConfig,
        is_validator: bool,
        epoch_duration_ms: u64,
        registry: &Registry,
        archive_readers: ArchiveReaderBalancer,
        pruner_db: Option<Arc<AuthorityPrunerTables>>,
        mut historic_store: Option<Arc<HistoricStore>>,
        progress_tracker: Option<Arc<CheckpointProgressTracker>>,
    ) -> Self {
        if pruning_config.num_epochs_to_retain > 0 && pruning_config.num_epochs_to_retain < u64::MAX
        {
            warn!(
                "Using objects pruner with num_epochs_to_retain = {} can lead to performance issues",
                pruning_config.num_epochs_to_retain
            );
            if is_validator {
                warn!("Resetting to aggressive pruner.");
                pruning_config.num_epochs_to_retain = 0;
            } else {
                warn!("Consider using an aggressive pruner (num_epochs_to_retain = 0)");
            }
        }

        assert!(
            historic_store.is_none() || pruner_db.is_none(),
            "the compaction filter pruner and the historic object store are mutually exclusive"
        );
        if is_validator && historic_store.is_some() {
            warn!("The historic object store is fullnode-only; disabling it on this validator.");
            historic_store = None;
        }
        if historic_store.is_some() && pruning_config.num_epochs_to_retain == u64::MAX {
            warn!(
                "The historic object store is enabled but the objects pruner is disabled \
                 (num_epochs_to_retain = u64::MAX); no object versions will ever be relocated."
            );
        }
        if historic_store.is_some() {
            if let Err(err) = Self::fast_forward_objects_watermark(&perpetual_db, &checkpoint_store)
            {
                error!("Failed to fast-forward the objects pruning watermark: {err:?}");
            }
        }

        // Coordination channels between the checkpoint executor and the pruner
        // task. The pruner task receives nudges (`executed_rx`) and publishes the
        // frontier (`frontier_tx`); the executor-facing ends are kept on the
        // returned handle for `nudge` / `await_leash`.
        let (executed, executed_rx) = watch::channel(0);
        let (frontier_ms, _) = watch::channel(u64::MAX);

        AuthorityStorePruner {
            _objects_pruner_cancel_handle: Self::setup_pruning(
                pruning_config,
                epoch_duration_ms,
                perpetual_db,
                checkpoint_store,
                grpc_indexes_store,
                jsonrpc_index,
                pruner_db,
                historic_store,
                AuthorityStorePruningMetrics::new(registry),
                archive_readers,
                progress_tracker,
                executed_rx,
                frontier_ms.clone(),
            ),
            executed,
            frontier_ms,
        }
    }

    /// Fast-forwards the objects pruning watermark past already-pruned
    /// checkpoint data.
    ///
    /// A node that previously ran with objects pruning disabled
    /// (`num_epochs_to_retain = u64::MAX`) but checkpoint pruning enabled has
    /// checkpoint contents missing below the checkpoint watermark. Relocation
    /// replays checkpoints from the objects watermark and would stall on the
    /// missing data, so the watermark starts at the checkpoint watermark
    /// instead. Superseded object versions from before that point stay in the
    /// live table; they are non-heads that every read path already skips.
    fn fast_forward_objects_watermark(
        perpetual_db: &Arc<AuthorityPerpetualTables>,
        checkpoint_store: &Arc<CheckpointStore>,
    ) -> anyhow::Result<()> {
        let objects_watermark = perpetual_db
            .get_highest_pruned_checkpoint()?
            .unwrap_or_default();
        let checkpoints_watermark = checkpoint_store
            .get_highest_pruned_checkpoint_seq_number()?
            .unwrap_or_default();
        if objects_watermark < checkpoints_watermark {
            warn!(
                objects_watermark,
                checkpoints_watermark,
                "Fast-forwarding the objects pruning watermark: checkpoint data below the \
                 checkpoint pruning watermark is already gone. Object versions superseded before \
                 that point remain in the live objects table and will never be relocated."
            );
            let mut wb = perpetual_db.pruned_checkpoint.batch();
            perpetual_db.set_highest_pruned_checkpoint(&mut wb, checkpoints_watermark)?;
            wb.write()?;
        }
        Ok(())
    }

    /// Compacts the entire range of objects stored in the `AuthorityStore` by
    /// invoking a range compaction on the database.
    pub fn compact(perpetual_db: &Arc<AuthorityPerpetualTables>) -> Result<(), TypedStoreError> {
        perpetual_db.objects.compact_range(
            &ObjectKey(ObjectId::ZERO, SequenceNumber::MIN_VALID_INCL),
            &ObjectKey(ObjectId::MAX, SequenceNumber::MAX_VALID_EXCL),
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
    use std::{collections::HashSet, path::Path, sync::Arc, time::Duration};

    use iota_sdk_types::{ObjectId, ObjectReference};
    use iota_swarm_config::test_utils::{CommitteeFixture, empty_contents};
    use iota_types::{
        base_types::{ObjectDigest, SequenceNumber},
        digests::TransactionDigest,
        effects::{
            TransactionEffects, TransactionEffectsAPIForTesting, TransactionEffectsExt,
            TransactionEffectsExtForTesting,
        },
        messages_checkpoint::{CheckpointSequenceNumber, CheckpointTimestamp},
        object::Object,
        storage::ObjectKey,
    };
    use more_asserts as ma;
    use prometheus_filtered::Registry;
    use tokio::sync::{oneshot, watch};
    use tracing::info;
    use typed_store::{
        Map,
        rocks::{DBMap, MetricConf, ReadWriteOptions, default_db_options},
    };

    use super::{AuthorityStorePruner, HistoricRelocation, PRUNING_LEASH_SLACK_MS, PruningMode};
    use crate::{
        authority::{
            authority_store_pruner::AuthorityStorePruningMetrics,
            authority_store_tables::AuthorityPerpetualTables,
            authority_store_types::{StoreObject, StoreObjectWrapper, get_store_object},
            historic_store::{HistoricStore, HistoricStoreMetrics},
        },
        checkpoints::CheckpointStore,
    };

    fn get_keys_after_pruning(path: &Path) -> anyhow::Result<HashSet<ObjectKey>> {
        let perpetual_db_path = path.join(Path::new("perpetual"));
        let cf_names = AuthorityPerpetualTables::describe_tables();
        let cfs: Vec<_> = cf_names
            .keys()
            .map(|x| (x.as_str(), default_db_options().options))
            .collect();
        let perpetual_db = typed_store::rocks::open_cf_opts(
            perpetual_db_path,
            None,
            MetricConf::new("perpetual_pruning"),
            &cfs,
        );

        let mut after_pruning = HashSet::new();
        let objects = DBMap::<ObjectKey, StoreObjectWrapper>::reopen(
            &perpetual_db?,
            Some("objects"),
            // open the db to bypass default db options which ignores range tombstones
            // so we can read the accurate number of retained versions
            &ReadWriteOptions::default(),
            false,
        )?;
        let iter = objects.safe_iter();
        for item in iter {
            after_pruning.insert(item?.0);
        }
        Ok(after_pruning)
    }

    type GenerateTestDataResult = (Vec<ObjectKey>, Vec<ObjectKey>, Vec<ObjectKey>);

    fn generate_test_data(
        db: Arc<AuthorityPerpetualTables>,
        num_versions_per_object: u64,
        num_object_versions_to_retain: u64,
        total_unique_object_ids: u32,
    ) -> Result<GenerateTestDataResult, anyhow::Error> {
        assert!(num_versions_per_object >= num_object_versions_to_retain);

        let (mut to_keep, mut to_delete, mut tombstones) = (vec![], vec![], vec![]);
        let mut batch = db.objects.batch();

        let mut id = ObjectId::ZERO;
        for _ in 0..total_unique_object_ids {
            for (counter, seq) in (0..num_versions_per_object).rev().enumerate() {
                let object_key = ObjectKey(id, SequenceNumber::from_u64(seq));
                if counter < num_object_versions_to_retain.try_into().unwrap() {
                    // latest `num_object_versions_to_retain` should not have been pruned
                    to_keep.push(object_key);
                } else {
                    to_delete.push(object_key);
                }
                let obj = get_store_object(Object::immutable_with_id_for_testing(id), None);
                batch.insert_batch(
                    &db.objects,
                    [(ObjectKey(id, SequenceNumber::from(seq)), obj.clone())],
                )?;
            }

            // Adding a tombstone for deleted object.
            if num_object_versions_to_retain == 0 {
                let tombstone_key = ObjectKey(id, SequenceNumber::from(num_versions_per_object));
                println!("Adding tombstone object {tombstone_key:?}");
                batch.insert_batch(
                    &db.objects,
                    [(tombstone_key, StoreObjectWrapper::V2(StoreObject::Deleted))],
                )?;
                tombstones.push(tombstone_key);
            }
            id = id.next_lexicographical();
        }
        batch.write().unwrap();
        assert_eq!(
            to_keep.len() as u64,
            std::cmp::min(num_object_versions_to_retain, num_versions_per_object)
                * total_unique_object_ids as u64
        );
        assert_eq!(
            tombstones.len() as u64,
            if num_object_versions_to_retain == 0 {
                total_unique_object_ids as u64
            } else {
                0
            }
        );
        Ok((to_keep, to_delete, tombstones))
    }

    async fn run_pruner(
        path: &Path,
        num_versions_per_object: u64,
        num_object_versions_to_retain: u64,
        total_unique_object_ids: u32,
    ) -> Vec<ObjectKey> {
        let registry = Registry::default();
        let metrics = AuthorityStorePruningMetrics::new(&registry);
        let to_keep = {
            let db = Arc::new(AuthorityPerpetualTables::open(path, None));
            let (to_keep, to_delete, tombstones) = generate_test_data(
                db.clone(),
                num_versions_per_object,
                num_object_versions_to_retain,
                total_unique_object_ids,
            )
            .unwrap();
            let mut effects =
                TransactionEffects::new_empty_v1_for_testing(TransactionDigest::default());
            for object in to_delete {
                effects.unsafe_add_deleted_live_object_for_testing(ObjectReference::new(
                    object.0,
                    object.1,
                    ObjectDigest::MIN,
                ));
            }
            for object in tombstones {
                effects.unsafe_add_object_tombstone_for_testing(ObjectReference::new(
                    object.0,
                    object.1,
                    ObjectDigest::MIN,
                ));
            }
            AuthorityStorePruner::prune_objects(vec![effects], &db, None, None, 0, metrics)
                .await
                .unwrap();
            to_keep
        };
        tokio::time::sleep(Duration::from_secs(3)).await;
        to_keep
    }

    fn open_historic(db: &Arc<AuthorityPerpetualTables>) -> Arc<HistoricStore> {
        Arc::new(
            HistoricStore::new_shared(db.database(), HistoricStoreMetrics::new_for_test()).unwrap(),
        )
    }

    /// Builds effects with production shapes: superseded input versions land
    /// in `modified_at_versions()`, while `all_tombstones()` reports tombstone
    /// refs at the effects' lamport version — strictly above every input
    /// version, like the real tombstone rows written at the deleting
    /// transaction's lamport version.
    fn effects_superseding(
        to_delete: &[ObjectKey],
        tombstones: &[ObjectKey],
    ) -> TransactionEffects {
        use std::collections::BTreeMap;

        use iota_sdk_types::{ExecutionStatus, GasCostSummary};

        let lamport_version = tombstones.iter().map(|key| key.1).max().unwrap_or_default();
        let mut effects = TransactionEffects::new_from_execution_v1(
            ExecutionStatus::Success,
            0,
            GasCostSummary::default(),
            vec![],
            std::collections::BTreeSet::new(),
            TransactionDigest::default(),
            lamport_version,
            BTreeMap::new(),
            None,
            None,
            vec![],
        );
        for object in to_delete {
            effects.unsafe_add_deleted_live_object_for_testing(ObjectReference::new(
                object.0,
                object.1,
                ObjectDigest::MIN,
            ));
        }
        for object in tombstones {
            // The tombstone's input state is the last superseded version.
            let input_version = SequenceNumber::from_u64(object.1.as_u64() - 1);
            effects.unsafe_add_object_tombstone_for_testing(ObjectReference::new(
                object.0,
                input_version,
                ObjectDigest::MIN,
            ));
        }
        effects
    }

    async fn relocate(
        db: &Arc<AuthorityPerpetualTables>,
        historic: &Arc<HistoricStore>,
        supersession_epoch: u64,
        effects: TransactionEffects,
        checkpoint_number: u64,
    ) {
        AuthorityStorePruner::prune_objects(
            vec![effects],
            db,
            None,
            Some(HistoricRelocation {
                store: historic,
                supersession_epoch,
            }),
            checkpoint_number,
            AuthorityStorePruningMetrics::new_for_test(),
        )
        .await
        .unwrap();
    }

    fn live_keys(db: &Arc<AuthorityPerpetualTables>) -> HashSet<ObjectKey> {
        db.objects.safe_iter().map(|item| item.unwrap().0).collect()
    }

    /// After relocation the live table contains exactly the heads, and the
    /// historic store contains exactly the superseded versions, bucketed by
    /// the supersession epoch.
    #[tokio::test]
    async fn commit_time_relocation_moves_pre_images_in_the_commit_batch() {
        use iota_sdk_types::{Address, Owner};
        use iota_types::{
            effects::TransactionEffectsExtForTesting, transaction::VerifiedTransaction,
        };

        use crate::{
            authority::authority_store::AuthorityStore, transaction_outputs::TransactionOutputs,
        };

        let tmp_dir = iota_common::tempdir();
        let perpetual_db = Arc::new(AuthorityPerpetualTables::open(tmp_dir.path(), None));
        let historic = open_historic(&perpetual_db);
        let store = AuthorityStore::open_no_genesis(
            perpetual_db.clone(),
            false,
            &Registry::default(),
            Some(historic.clone()),
        )
        .unwrap();

        // A live version v1 that the committed transaction supersedes with v2,
        // its pre-image carried in the outputs.
        let owner = Owner::Address(Address::ZERO);
        let object_id = ObjectId::random();
        let object_v1 = Object::with_id_owner_version_for_testing(
            object_id,
            SequenceNumber::from_u64(1),
            owner,
        );
        let object_v2 = Object::with_id_owner_version_for_testing(
            object_id,
            SequenceNumber::from_u64(2),
            owner,
        );
        let key_v1 = ObjectKey(object_id, object_v1.version());
        let key_v2 = ObjectKey(object_id, object_v2.version());
        perpetual_db
            .objects
            .insert(&key_v1, &get_store_object(object_v1.clone(), None))
            .unwrap();

        let transaction = VerifiedTransaction::new_genesis_transaction(vec![], vec![]);
        let effects = TransactionEffects::new_empty_v1_for_testing(*transaction.digest());
        let outputs = TransactionOutputs {
            transaction: Arc::new(transaction),
            effects,
            events: Default::default(),
            markers: Default::default(),
            wrapped: Default::default(),
            deleted: Default::default(),
            live_object_markers_to_delete: Default::default(),
            new_live_object_markers_to_init: Default::default(),
            written: [(object_id, object_v2)].into_iter().collect(),
            superseded: vec![(key_v1, object_v1)],
        };

        // One atomic batch: v2 written, v1 relocated and deleted from live.
        let batch = store.build_db_batch(3, 7, &[Arc::new(outputs)]).unwrap();
        batch.write().unwrap();

        assert!(perpetual_db.objects.get(&key_v2).unwrap().is_some());
        assert!(perpetual_db.objects.get(&key_v1).unwrap().is_none());
        assert!(historic.get_store_object(&key_v1).unwrap().is_some());
        assert_eq!(historic.list_epochs(), vec![3]);
        assert!(
            historic
                .get_object(&key_v1)
                .unwrap()
                .is_some_and(|object| object.version() == key_v1.1)
        );
    }

    #[tokio::test]
    async fn relocation_moves_superseded_versions_and_keeps_heads() {
        let tmp_dir = iota_common::tempdir();
        let db = Arc::new(AuthorityPerpetualTables::open(tmp_dir.path(), None));
        let historic = open_historic(&db);
        let (to_keep, to_delete, _) = generate_test_data(db.clone(), 3, 1, 100).unwrap();

        relocate(&db, &historic, 5, effects_superseding(&to_delete, &[]), 1).await;

        assert_eq!(live_keys(&db), HashSet::from_iter(to_keep));
        for key in &to_delete {
            assert!(
                historic.get_store_object(key).unwrap().is_some(),
                "{key:?} was not relocated"
            );
        }
        // Superseded in epoch 5, so the version created in epoch 1 lands in
        // bucket 5; dropping older buckets must not affect it.
        assert_eq!(historic.list_epochs(), vec![5]);
        historic.drop_epoch(1).unwrap();
        assert!(historic.get_store_object(&to_delete[0]).unwrap().is_some());
        assert_eq!(db.get_highest_pruned_checkpoint().unwrap(), Some(1));
    }

    /// Deleted lineages keep their tombstone as the live head; latest-version
    /// reads must still resolve to the tombstone after relocation.
    #[tokio::test]
    async fn relocation_keeps_tombstones_as_live_heads() {
        let tmp_dir = iota_common::tempdir();
        let db = Arc::new(AuthorityPerpetualTables::open(tmp_dir.path(), None));
        let historic = open_historic(&db);
        let (_, to_delete, tombstones) = generate_test_data(db.clone(), 3, 0, 10).unwrap();

        relocate(
            &db,
            &historic,
            2,
            effects_superseding(&to_delete, &tombstones),
            1,
        )
        .await;

        assert_eq!(live_keys(&db), HashSet::from_iter(tombstones.clone()));
        for tombstone in &tombstones {
            let latest = db
                .get_latest_object_ref_or_tombstone(tombstone.0)
                .unwrap()
                .expect("tombstone head must stay readable");
            assert_eq!(
                latest,
                ObjectReference::new(tombstone.0, tombstone.1, ObjectDigest::OBJECT_DELETED)
            );
        }
        assert_eq!(
            HashSet::<ObjectKey>::from_iter(historic.tombstone_heads(2).unwrap()),
            HashSet::from_iter(tombstones)
        );
        for key in &to_delete {
            assert!(historic.get_store_object(key).unwrap().is_some());
        }
    }

    /// Replaying relocation after a crash between the historic write and the
    /// live delete converges to the same state; replaying after completion is
    /// a no-op.
    #[tokio::test]
    async fn relocation_replay_is_idempotent() {
        let tmp_dir = iota_common::tempdir();
        let db = Arc::new(AuthorityPerpetualTables::open(tmp_dir.path(), None));
        let historic = open_historic(&db);
        let (to_keep, to_delete, _) = generate_test_data(db.clone(), 3, 1, 50).unwrap();

        // Simulate the crash window: the historic write landed, the live
        // delete and watermark did not.
        let rows: Vec<_> = to_delete
            .iter()
            .map(|key| (*key, db.objects.get(key).unwrap().unwrap()))
            .collect();
        historic.put_objects(4, &rows, &[]).unwrap();

        relocate(&db, &historic, 4, effects_superseding(&to_delete, &[]), 1).await;
        let live_after_first = live_keys(&db);
        assert_eq!(live_after_first, HashSet::from_iter(to_keep));

        // Replay with the same effects: every key is already gone from the
        // live table, so nothing changes.
        relocate(&db, &historic, 4, effects_superseding(&to_delete, &[]), 1).await;
        assert_eq!(live_keys(&db), live_after_first);
        for key in &to_delete {
            assert!(historic.get_store_object(key).unwrap().is_some());
        }
    }

    /// Expiring a bucket deletes its tombstone heads from the live table; a
    /// lineage resurrected at a higher version survives the exact-key delete
    /// of its stale tombstone.
    #[tokio::test]
    async fn historic_expiry_deletes_tombstone_heads_and_spares_resurrections() {
        let tmp_dir = iota_common::tempdir();
        let db = Arc::new(AuthorityPerpetualTables::open(tmp_dir.path(), None));
        let historic = open_historic(&db);
        let (_, to_delete, tombstones) = generate_test_data(db.clone(), 3, 0, 10).unwrap();

        relocate(
            &db,
            &historic,
            2,
            effects_superseding(&to_delete, &tombstones),
            1,
        )
        .await;

        // One lineage gets resurrected (e.g. unwrapped) at a higher version
        // after its tombstone.
        let resurrected_id = tombstones[0].0;
        let resurrected_key = ObjectKey(resurrected_id, SequenceNumber::from_u64(7));
        db.objects
            .insert(
                &resurrected_key,
                &get_store_object(Object::immutable_with_id_for_testing(resurrected_id), None),
            )
            .unwrap();

        // Bucket 2 is out of retention at epoch 100.
        AuthorityStorePruner::drop_expired_historic_epochs(&db, &historic, 100, 10).unwrap();

        assert_eq!(historic.list_epochs(), Vec::<u64>::new());
        assert_eq!(live_keys(&db), HashSet::from_iter([resurrected_key]));
        // Buckets within retention stay.
        let recent_key = ObjectKey(ObjectId::random(), SequenceNumber::from_u64(1));
        historic
            .put_objects(
                95,
                &[(
                    recent_key,
                    get_store_object(Object::immutable_with_id_for_testing(recent_key.0), None),
                )],
                &[],
            )
            .unwrap();
        AuthorityStorePruner::drop_expired_historic_epochs(&db, &historic, 100, 10).unwrap();
        assert_eq!(historic.list_epochs(), vec![95]);
    }

    /// Legacy V1 rows relocate as raw bytes and migrate at read time.
    #[tokio::test]
    async fn relocation_handles_legacy_v1_rows() {
        let tmp_dir = iota_common::tempdir();
        let db = Arc::new(AuthorityPerpetualTables::open(tmp_dir.path(), None));
        let historic = open_historic(&db);

        let object = Object::immutable_with_id_for_testing(ObjectId::random());
        let key = ObjectKey(object.id(), object.version());
        db.insert_store_object_v1_test_only(object.clone()).unwrap();

        relocate(&db, &historic, 3, effects_superseding(&[key], &[]), 1).await;

        assert!(db.objects.get(&key).unwrap().is_none());
        let relocated = historic
            .get_object(&key)
            .unwrap()
            .expect("V1 row must be readable from the historic store");
        assert_eq!(relocated.id(), object.id());
        assert_eq!(relocated.version(), object.version());
    }

    /// The live object set observed by state hashing and snapshots is
    /// unchanged by relocation and by bucket expiry.
    #[tokio::test]
    async fn live_object_set_is_invariant_under_relocation_and_expiry() {
        let tmp_dir = iota_common::tempdir();
        let db = Arc::new(AuthorityPerpetualTables::open(tmp_dir.path(), None));
        let historic = open_historic(&db);
        let (_, to_delete, _) = generate_test_data(db.clone(), 4, 1, 100).unwrap();

        let live_set_before: Vec<_> = db
            .iter_live_object_set()
            .map(|live| live.object_reference())
            .collect();

        relocate(&db, &historic, 3, effects_superseding(&to_delete, &[]), 1).await;
        let live_set_after: Vec<_> = db
            .iter_live_object_set()
            .map(|live| live.object_reference())
            .collect();
        assert_eq!(live_set_before, live_set_after);

        AuthorityStorePruner::drop_expired_historic_epochs(&db, &historic, 100, 10).unwrap();
        let live_set_after_expiry: Vec<_> = db
            .iter_live_object_set()
            .map(|live| live.object_reference())
            .collect();
        assert_eq!(live_set_before, live_set_after_expiry);
    }

    /// Checkpoint pruning with relocation moves the checkpoint-keyed history
    /// (transactions, effects, events, checkpoint contents and summaries)
    /// into the epoch bucket before deleting it, records the availability
    /// watermark, and replays idempotently.
    #[tokio::test]
    async fn checkpoint_relocation_moves_history_and_replays_idempotently() {
        use fastcrypto::traits::KeyPair;
        use iota_protocol_config::ProtocolConfig;
        use iota_sdk_types::GasCostSummary;
        use iota_types::{
            base_types::ExecutionDigests,
            committee::Committee,
            effects::TransactionEvents,
            messages_checkpoint::{
                CertifiedCheckpointSummary, CheckpointContents, CheckpointContentsExt,
                CheckpointSummary, CheckpointSummaryExt, SignedCheckpointSummary,
                VerifiedCheckpoint,
            },
            transaction::VerifiedTransaction,
        };

        use crate::checkpoints::CheckpointStore;

        let tmp_dir = iota_common::tempdir();
        let perpetual_db = Arc::new(AuthorityPerpetualTables::open(tmp_dir.path(), None));
        let checkpoint_db = CheckpointStore::new(&tmp_dir.path().join("checkpoints"));
        let historic = open_historic(&perpetual_db);

        // One transaction with effects and events, wired into one checkpoint.
        let transaction = VerifiedTransaction::new_genesis_transaction(vec![], vec![]);
        let tx_digest = *transaction.digest();
        let effects = TransactionEffects::new_empty_v1_for_testing(tx_digest);
        let fx_digest = effects.digest();
        let contents =
            CheckpointContents::new_with_digests_only_for_tests([ExecutionDigests::new(
                tx_digest, fx_digest,
            )]);

        let (committee, keys) = Committee::new_simple_test_committee();
        let summary = CheckpointSummary::new_with_protocol_config(
            &ProtocolConfig::get_for_max_version_UNSAFE(),
            committee.epoch,
            9,
            1,
            &contents,
            None,
            GasCostSummary::default(),
            None,
            100,
            Vec::new(),
        );
        let signatures = keys
            .iter()
            .map(|key| {
                SignedCheckpointSummary::new(
                    committee.epoch,
                    summary.clone(),
                    key,
                    key.public().into(),
                )
                .auth_sig()
                .clone()
            })
            .collect();
        let cert = CertifiedCheckpointSummary::new(summary, signatures, &committee).unwrap();
        let verified_checkpoint = VerifiedCheckpoint::new_unchecked(cert);
        let ckpt_digest = *verified_checkpoint.digest();
        let trusted_checkpoint = verified_checkpoint.serializable();

        perpetual_db
            .transactions
            .insert(&tx_digest, transaction.serializable_ref())
            .unwrap();
        perpetual_db.effects.insert(&fx_digest, &effects).unwrap();
        perpetual_db
            .executed_effects
            .insert(&tx_digest, &fx_digest)
            .unwrap();
        perpetual_db
            .events_2
            .insert(&tx_digest, &TransactionEvents(vec![]))
            .unwrap();
        checkpoint_db
            .tables
            .checkpoint_content
            .insert(&contents.digest(), &contents)
            .unwrap();
        checkpoint_db
            .tables
            .checkpoint_by_digest
            .insert(&ckpt_digest, &trusted_checkpoint)
            .unwrap();

        let run = || {
            AuthorityStorePruner::prune_checkpoints(
                &perpetual_db,
                &checkpoint_db,
                None,
                Some(HistoricRelocation {
                    store: &historic,
                    supersession_epoch: committee.epoch,
                }),
                9,
                vec![ckpt_digest],
                vec![contents.clone()],
                &vec![effects.clone()],
                AuthorityStorePruningMetrics::new_for_test(),
            )
            .unwrap()
        };
        run();

        // Source rows are gone.
        assert!(perpetual_db.transactions.get(&tx_digest).unwrap().is_none());
        assert!(perpetual_db.effects.get(&fx_digest).unwrap().is_none());
        assert!(
            perpetual_db
                .executed_effects
                .get(&tx_digest)
                .unwrap()
                .is_none()
        );
        // `events_2` rows are only deleted for effects that declare an
        // events digest; the empty test effects do not, so the seeded row
        // stays live (relocation still harvested it below).
        assert!(perpetual_db.events_2.get(&tx_digest).unwrap().is_some());
        assert!(
            checkpoint_db
                .tables
                .checkpoint_content
                .get(&contents.digest())
                .unwrap()
                .is_none()
        );
        assert!(
            checkpoint_db
                .tables
                .checkpoint_by_digest
                .get(&ckpt_digest)
                .unwrap()
                .is_none()
        );

        // Everything is served from the historic store.
        assert!(historic.get_transaction(&tx_digest).unwrap().is_some());
        assert!(historic.get_effects(&fx_digest).unwrap().is_some());
        assert_eq!(
            historic.get_executed_effects(&tx_digest).unwrap(),
            Some(fx_digest)
        );
        assert!(historic.get_events(&tx_digest).unwrap().is_some());
        assert!(
            historic
                .get_checkpoint_contents(&contents.digest())
                .unwrap()
                .is_some()
        );
        assert!(
            historic
                .get_checkpoint_by_digest(&ckpt_digest)
                .unwrap()
                .is_some()
        );
        assert_eq!(
            historic
                .get_checkpoint_seq_by_contents_digest(&contents.digest())
                .unwrap(),
            Some(9)
        );
        assert_eq!(historic.lowest_available_checkpoint().unwrap(), Some(9));

        // Replaying after a crash between the historic write and the deletes
        // (or after completion) converges: sources stay gone, history intact.
        run();
        assert!(historic.get_transaction(&tx_digest).unwrap().is_some());
        assert_eq!(historic.lowest_available_checkpoint().unwrap(), Some(9));
    }

    /// The historic buckets live in the perpetual database, so one snapshot
    /// covers live and historic column families consistently: every row is
    /// in exactly one of them, no matter when relocation ran relative to the
    /// snapshot.
    #[tokio::test]
    async fn db_checkpoint_covers_live_and_historic_consistently() {
        let tmp_dir = iota_common::tempdir();
        let db = Arc::new(AuthorityPerpetualTables::open(tmp_dir.path(), None));
        let historic = open_historic(&db);
        let (to_keep, to_delete, _) = generate_test_data(db.clone(), 3, 1, 60).unwrap();
        let (first_half, second_half) = to_delete.split_at(to_delete.len() / 2);

        relocate(&db, &historic, 1, effects_superseding(first_half, &[]), 1).await;

        let restore_dir = iota_common::tempdir();
        db.objects
            .checkpoint_db(&restore_dir.path().join("perpetual"))
            .unwrap();
        // Relocation after the snapshot must not affect the restored copy.
        relocate(&db, &historic, 1, effects_superseding(second_half, &[]), 2).await;

        let restored_db = Arc::new(AuthorityPerpetualTables::open(restore_dir.path(), None));
        let restored_historic = open_historic(&restored_db);
        for key in to_keep.iter().chain(&to_delete) {
            let in_live = restored_db.objects.get(key).unwrap().is_some();
            let in_history = restored_historic.get_store_object(key).unwrap().is_some();
            assert!(
                in_live ^ in_history,
                "{key:?} must be in exactly one restored table"
            );
        }
        // Relocated data reads back as full objects from the restored copy,
        // and the captured watermark allows idempotent replay of the torn
        // window.
        assert!(
            restored_historic
                .get_object(&first_half[0])
                .unwrap()
                .is_some()
        );
        assert_eq!(
            restored_db.get_highest_pruned_checkpoint().unwrap(),
            Some(1)
        );
    }

    // Tests pruning old version of live objects.
    #[tokio::test]
    async fn test_pruning_objects() {
        let tmp_dir = iota_common::tempdir();
        let to_keep = run_pruner(tmp_dir.path(), 3, 2, 1000).await;
        assert_eq!(
            HashSet::from_iter(to_keep),
            get_keys_after_pruning(tmp_dir.path()).unwrap()
        );
    }

    // Tests pruning deleted objects (object tombstones).
    #[tokio::test]
    async fn test_pruning_tombstones() {
        let tmp_dir = iota_common::tempdir();
        let to_keep = run_pruner(tmp_dir.path(), 0, 0, 1000).await;
        assert_eq!(to_keep.len(), 0);
        assert_eq!(get_keys_after_pruning(tmp_dir.path()).unwrap().len(), 0);

        let tmp_dir2 = iota_common::tempdir();
        let to_keep = run_pruner(tmp_dir2.path(), 3, 0, 1000).await;
        assert_eq!(to_keep.len(), 0);
        assert_eq!(get_keys_after_pruning(tmp_dir2.path()).unwrap().len(), 0);
    }

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
                    to_delete.push((id, SequenceNumber::from(i)));
                }
                let obj = get_store_object(Object::immutable_with_id_for_testing(id), None);
                perpetual_db
                    .objects
                    .insert(&ObjectKey(id, SequenceNumber::from(i)), &obj)?;
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
        let start = ObjectKey(ObjectId::ZERO, SequenceNumber::MIN_VALID_INCL);
        let end = ObjectKey(ObjectId::MAX, SequenceNumber::MAX_VALID_EXCL);

        perpetual_db.objects.compact_range(&start, &end)?;
        let before_compaction_size = get_sst_size(&db_path);

        let mut effects =
            TransactionEffects::new_empty_v1_for_testing(TransactionDigest::default());
        for object in to_delete {
            effects.unsafe_add_deleted_live_object_for_testing(ObjectReference::new(
                object.0,
                object.1,
                ObjectDigest::MIN,
            ));
        }
        let registry = Registry::default();
        let metrics = AuthorityStorePruningMetrics::new(&registry);
        let total_pruned = AuthorityStorePruner::prune_objects(
            vec![effects],
            &perpetual_db,
            None,
            None,
            0,
            metrics,
        )
        .await;
        info!("Total pruned keys = {:?}", total_pruned);

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
        let object_key = ObjectKey(ObjectId::random(), SequenceNumber::from_u64(1));
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

        let registry = Registry::default();
        let metrics = AuthorityStorePruningMetrics::new(&registry);
        AuthorityStorePruner::prune_for_eligible_epochs(
            &perpetual_db,
            &checkpoint_store,
            None,
            None,
            None,
            true,
            PruningMode::Checkpoints,
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

    // Builds a pruner handle with just the coordination channels (no pruning
    // task), for exercising `nudge` / `await_leash` in isolation.
    fn coordination_pruner() -> AuthorityStorePruner {
        AuthorityStorePruner {
            _objects_pruner_cancel_handle: oneshot::channel().0,
            executed: watch::channel(0).0,
            frontier_ms: watch::channel(u64::MAX).0,
        }
    }

    // The leash passes without blocking while the executed timestamp is within
    // the slack of the pruning frontier (and always before the pruner has run,
    // when the frontier is u64::MAX).
    #[tokio::test]
    async fn test_leash_passes_within_slack() {
        let pruner = coordination_pruner();
        // Frontier starts at u64::MAX: never leashed before the pruner runs.
        pruner.await_leash(1_000_000).await;

        pruner.frontier_ms.send_replace(500);
        // Gap exactly equals the slack -> still passes.
        pruner.await_leash(500 + PRUNING_LEASH_SLACK_MS).await;
    }

    // The leash blocks while the pruner is more than the slack behind, and
    // releases once the frontier advances.
    #[tokio::test]
    async fn test_leash_blocks_until_frontier_advances() {
        let pruner = Arc::new(coordination_pruner());
        pruner.frontier_ms.send_replace(0);
        let executed_ts = PRUNING_LEASH_SLACK_MS + 10_000;

        let waiter = pruner.clone();
        let handle = tokio::spawn(async move { waiter.await_leash(executed_ts).await });

        // Let the spawned task run until it parks on the frontier watch.
        tokio::task::yield_now().await;
        assert!(
            !handle.is_finished(),
            "leash must block while the pruner is more than the slack behind"
        );

        // Once the pruner catches up, the leash releases.
        pruner.frontier_ms.send_replace(executed_ts);
        handle
            .await
            .expect("leash should release after frontier advances");
    }

    // A nudge wakes the pruner task's subscription.
    #[tokio::test]
    async fn test_nudge_wakes_subscriber() {
        let pruner = coordination_pruner();
        let mut rx = pruner.executed.subscribe();
        pruner.nudge(42);
        rx.changed().await.expect("nudge should notify subscriber");
        assert_eq!(*rx.borrow(), 42);
    }
}
