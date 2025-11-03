// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
use std::collections::BTreeMap;

use futures::{StreamExt, stream::ReadyChunks};
use iota_metrics::metered_channel::ReceiverStream;
use tap::tap::TapFallible;
use tracing::{error, info, instrument};

use crate::{
    ingestion::common::{
        persist::{CHECKPOINT_COMMIT_BATCH_SIZE, CommitterTables, CommitterWatermark},
        prepare::CheckpointObjectChanges,
    },
    metrics::IndexerMetrics,
    models::{
        display::StoredDisplay,
        epoch::{EndOfEpochUpdate, StartOfEpochUpdate},
        obj_indices::StoredObjectVersion,
    },
    store::{IndexerStore, PgIndexerStore},
    types::{
        EventIndex, IndexedCheckpoint, IndexedDeletedObject, IndexedEvent, IndexedObject,
        IndexedPackage, IndexedTransaction, IndexerResult, TxIndex,
    },
};
#[derive(Debug)]
pub(crate) struct CheckpointDataToCommit {
    pub(crate) checkpoint: IndexedCheckpoint,
    pub(crate) transactions: Vec<IndexedTransaction>,
    pub(crate) events: Vec<IndexedEvent>,
    pub(crate) event_indices: Vec<EventIndex>,
    pub(crate) tx_indices: Vec<TxIndex>,
    pub(crate) display_updates: BTreeMap<String, StoredDisplay>,
    pub(crate) object_changes: CheckpointObjectChanges,
    pub(crate) object_history_changes: TransactionObjectChangesToCommit,
    pub(crate) object_versions: Vec<StoredObjectVersion>,
    pub(crate) packages: Vec<IndexedPackage>,
    pub(crate) epoch: Option<EpochToCommit>,
}

#[derive(Clone, Debug, Default)]
pub struct TransactionObjectChangesToCommit {
    pub changed_objects: Vec<IndexedObject>,
    pub deleted_objects: Vec<IndexedDeletedObject>,
}

#[derive(Clone, Debug)]
pub struct EpochToCommit {
    pub(crate) last_epoch: Option<EndOfEpochUpdate>,
    pub(crate) new_epoch: StartOfEpochUpdate,
}

pub(crate) struct PrimaryWriter {
    state: PgIndexerStore,
    metrics: IndexerMetrics,
    pub stream: ReadyChunks<ReceiverStream<CheckpointDataToCommit>>,
    pub checkpoint_commit_batch_size: usize,
}

impl PrimaryWriter {
    pub fn new(
        state: PgIndexerStore,
        metrics: IndexerMetrics,
        tx_indexing_receiver: iota_metrics::metered_channel::Receiver<CheckpointDataToCommit>,
    ) -> Self {
        let checkpoint_commit_batch_size = std::env::var("CHECKPOINT_COMMIT_BATCH_SIZE")
            .unwrap_or(CHECKPOINT_COMMIT_BATCH_SIZE.to_string())
            .parse::<usize>()
            .unwrap();
        info!("Using checkpoint commit batch size {checkpoint_commit_batch_size}");

        let stream =
            ReceiverStream::new(tx_indexing_receiver).ready_chunks(checkpoint_commit_batch_size);

        Self {
            state,
            metrics,
            stream,
            checkpoint_commit_batch_size,
        }
    }

    /// Writes indexed checkpoint data to the database, and then update
    /// watermark upper bounds and metrics. Expects
    /// `indexed_checkpoint_batch` to be non-empty, and contain contiguous
    /// checkpoints. There can be at most one epoch boundary at the end. If
    /// an epoch boundary is detected, epoch-partitioned tables must be
    /// advanced.
    // Unwrap: Caller needs to make sure indexed_checkpoint_batch is not empty
    #[instrument(skip_all, fields(
        first = indexed_checkpoint_batch.first().as_ref().unwrap().checkpoint.sequence_number,
        last = indexed_checkpoint_batch.last().as_ref().unwrap().checkpoint.sequence_number
    ))]
    pub(crate) async fn commit_checkpoints(
        &self,
        indexed_checkpoint_batch: Vec<CheckpointDataToCommit>,
        epoch: Option<EpochToCommit>,
    ) {
        let batch_len = indexed_checkpoint_batch.len();
        let mut checkpoint_batch = Vec::with_capacity(batch_len);
        let mut tx_batch = Vec::with_capacity(batch_len);
        let mut events_batch = Vec::with_capacity(batch_len);
        let mut tx_indices_batch = Vec::with_capacity(batch_len);
        let mut event_indices_batch = Vec::with_capacity(batch_len);
        let mut display_updates_batch = BTreeMap::new();
        let mut object_changes_batch = Vec::with_capacity(batch_len);
        let mut object_history_changes_batch = Vec::with_capacity(batch_len);
        let mut object_versions_batch = Vec::with_capacity(batch_len);
        let mut packages_batch = Vec::with_capacity(batch_len);

        for indexed_checkpoint in indexed_checkpoint_batch {
            let CheckpointDataToCommit {
                checkpoint,
                transactions,
                events,
                event_indices,
                tx_indices,
                display_updates,
                object_changes,
                object_history_changes,
                object_versions,
                packages,
                ..
            } = indexed_checkpoint;
            checkpoint_batch.push(checkpoint);
            tx_batch.push(transactions);
            events_batch.push(events);
            tx_indices_batch.push(tx_indices);
            event_indices_batch.push(event_indices);
            display_updates_batch.extend(display_updates.into_iter());
            object_changes_batch.push(object_changes);
            object_history_changes_batch.push(object_history_changes);
            object_versions_batch.push(object_versions);
            packages_batch.push(packages);
        }

        let first_checkpoint_seq = checkpoint_batch.first().as_ref().unwrap().sequence_number;
        let committer_watermark = CommitterWatermark::from(checkpoint_batch.last().unwrap());

        let guard = self.metrics.checkpoint_db_commit_latency.start_timer();
        let tx_batch = tx_batch.into_iter().flatten().collect::<Vec<_>>();

        let tx_global_order_batch: Vec<_> = tx_batch.iter().map(Into::into).collect();
        let tx_indices_batch = tx_indices_batch.into_iter().flatten().collect::<Vec<_>>();
        let events_batch = events_batch.into_iter().flatten().collect::<Vec<_>>();
        let event_indices_batch = event_indices_batch
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let object_versions_batch = object_versions_batch
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let packages_batch = packages_batch.into_iter().flatten().collect::<Vec<_>>();
        let checkpoint_num = checkpoint_batch.len();
        let tx_count = tx_batch.len();

        {
            let _step_1_guard = self
                .metrics
                .checkpoint_db_commit_latency_step_1
                .start_timer();
            let mut persist_tasks = vec![
                self.state.persist_transactions(tx_batch),
                self.state.persist_tx_indices(tx_indices_batch),
                self.state
                    .persist_tx_global_order(tx_global_order_batch.clone()),
                self.state.persist_events(events_batch),
                self.state.persist_event_indices(event_indices_batch),
                self.state.persist_displays(display_updates_batch),
                self.state.persist_packages(packages_batch),
                self.state.persist_checkpoint_objects(object_changes_batch),
                self.state
                    .persist_object_history(object_history_changes_batch.clone()),
                self.state
                    .persist_object_versions(object_versions_batch.clone()),
            ];
            if let Some(epoch_data) = epoch.clone() {
                persist_tasks.push(self.state.persist_epoch(epoch_data));
            }
            futures::future::join_all(persist_tasks)
                .await
                .into_iter()
                .map(|res| {
                    if res.is_err() {
                        error!("failed to persist data with error: {:?}", res);
                    }
                    res
                })
                .collect::<IndexerResult<Vec<_>>>()
                .expect("persisting data into DB should not fail.");
        }

        self.state
            .update_status_for_checkpoint_transactions(tx_global_order_batch)
            .await
            .inspect_err(|e| {
                error!("failed to update tx global order as indexed with error: {e}");
            })
            .expect("updating tx global order as indexed should not fail.");

        let is_epoch_end = epoch.is_some();

        // On epoch boundary, we need to modify the existing partitions' upper bound,
        // and introduce a new partition for incoming data for the upcoming epoch.
        if let Some(epoch_data) = epoch {
            self.state
                .advance_epoch(epoch_data)
                .await
                .tap_err(|e| {
                    error!("failed to advance epoch with error: {}", e.to_string());
                })
                .expect("advancing epochs in DB should not fail.");
            self.metrics.total_epoch_committed.inc();

            // Refresh participation metrics after advancing epoch
            self.state
                .refresh_participation_metrics()
                .await
                .tap_err(|e| {
                    error!("failed to update participation metrics: {e}");
                })
                .expect("updating participation metrics should not fail.");
        }

        self.state
            .persist_checkpoints(checkpoint_batch)
            .await
            .tap_err(|e| {
                error!(
                    "failed to persist checkpoint data with error: {}",
                    e.to_string()
                );
            })
            .expect("persisting data into DB should not fail.");

        if is_epoch_end {
            // The epoch has advanced so we update the configs for the new protocol version,
            // if it has changed.
            let chain_id = <PgIndexerStore as IndexerStore>::get_chain_identifier(&self.state)
                .await
                .expect("failed to get chain identifier")
                .expect("chain identifier should have been indexed at this point");
            let _ = self
                .state
                .persist_protocol_configs_and_feature_flags(chain_id);
        }

        self.state
            .update_watermarks_upper_bound::<CommitterTables>(committer_watermark)
            .await
            .tap_err(|e| {
                error!(
                    "Failed to update watermark upper bound with error: {}",
                    e.to_string()
                );
            })
            .expect("Updating watermark upper bound in DB should not fail.");

        let elapsed = guard.stop_and_record();

        info!(
            elapsed,
            "Checkpoint {}-{} committed with {} transactions.",
            first_checkpoint_seq,
            committer_watermark.checkpoint_hi_inclusive,
            tx_count,
        );
        self.metrics
            .latest_tx_checkpoint_sequence_number
            .set(committer_watermark.checkpoint_hi_inclusive as i64);
        self.metrics
            .total_tx_checkpoint_committed
            .inc_by(checkpoint_num as u64);
        self.metrics
            .total_transaction_committed
            .inc_by(tx_count as u64);
        self.metrics.transaction_per_checkpoint.observe(
            tx_count as f64
                / (committer_watermark.checkpoint_hi_inclusive - first_checkpoint_seq + 1) as f64,
        );
        // 1000.0 is not necessarily the batch size, it's to roughly map average tx
        // commit latency to [0.1, 1] seconds, which is well covered by
        // DB_COMMIT_LATENCY_SEC_BUCKETS.
        self.metrics
            .thousand_transaction_avg_db_commit_latency
            .observe(elapsed * 1000.0 / tx_count as f64);
    }
}
