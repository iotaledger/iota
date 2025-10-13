// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Types and associated logic to use while persisting
//! data to the database.

use std::collections::{BTreeMap, HashMap};

use async_trait::async_trait;
use futures::{FutureExt, StreamExt};
use iota_types::messages_checkpoint::CheckpointSequenceNumber;
use tap::tap::TapFallible;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, instrument};

use crate::{
    errors::IndexerError,
    metrics::IndexerMetrics,
    models::{
        display::StoredDisplay,
        epoch::{EndOfEpochUpdate, StartOfEpochUpdate},
        obj_indices::StoredObjectVersion,
    },
    store::{IndexerStore, PgIndexerStore},
    transform::CheckpointObjectChanges,
    types::{
        EventIndex, IndexedCheckpoint, IndexedDeletedObject, IndexedEvent, IndexedObject,
        IndexedPackage, IndexedTransaction, IndexerResult, TxIndex,
    },
};

pub(crate) const CHECKPOINT_COMMIT_BATCH_SIZE: usize = 100;
pub(crate) const UNPROCESSED_CHECKPOINT_SIZE_LIMIT: usize = 1000;

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

pub(crate) async fn start_tx_checkpoint_commit_task(
    state: PgIndexerStore,
    metrics: IndexerMetrics,
    tx_indexing_receiver: iota_metrics::metered_channel::Receiver<CheckpointDataToCommit>,
    mut next_checkpoint_sequence_number: CheckpointSequenceNumber,
    cancel: CancellationToken,
) -> IndexerResult<()> {
    use futures::StreamExt;

    info!("Indexer checkpoint commit task started...");
    let checkpoint_commit_batch_size = std::env::var("CHECKPOINT_COMMIT_BATCH_SIZE")
        .unwrap_or(CHECKPOINT_COMMIT_BATCH_SIZE.to_string())
        .parse::<usize>()
        .unwrap();
    info!("Using checkpoint commit batch size {checkpoint_commit_batch_size}");

    let mut stream = iota_metrics::metered_channel::ReceiverStream::new(tx_indexing_receiver)
        .ready_chunks(checkpoint_commit_batch_size);

    let mut unprocessed = HashMap::new();
    let mut batch = vec![];

    while let Some(indexed_checkpoint_batch) = stream.next().await {
        if cancel.is_cancelled() {
            break;
        }

        // split the batch into smaller batches per epoch to handle partitioning
        for checkpoint in indexed_checkpoint_batch {
            unprocessed.insert(checkpoint.checkpoint.sequence_number, checkpoint);
        }
        while let Some(checkpoint) = unprocessed.remove(&next_checkpoint_sequence_number) {
            let epoch = checkpoint.epoch.clone();
            batch.push(checkpoint);
            next_checkpoint_sequence_number += 1;
            if batch.len() == checkpoint_commit_batch_size || epoch.is_some() {
                commit_checkpoints(&state, batch, epoch, &metrics).await;
                batch = vec![];
            }
        }
        if !batch.is_empty() {
            commit_checkpoints(&state, batch, None, &metrics).await;
            batch = vec![];
        }
    }
    Ok(())
}

// Unwrap: Caller needs to make sure indexed_checkpoint_batch is not empty
#[instrument(skip_all, fields(
    first = indexed_checkpoint_batch.first().as_ref().unwrap().checkpoint.sequence_number,
    last = indexed_checkpoint_batch.last().as_ref().unwrap().checkpoint.sequence_number
))]
async fn commit_checkpoints(
    state: &PgIndexerStore,
    indexed_checkpoint_batch: Vec<CheckpointDataToCommit>,
    epoch: Option<EpochToCommit>,
    metrics: &IndexerMetrics,
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
    let last_checkpoint_seq = checkpoint_batch.last().as_ref().unwrap().sequence_number;

    let guard = metrics.checkpoint_db_commit_latency.start_timer();
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
        let _step_1_guard = metrics.checkpoint_db_commit_latency_step_1.start_timer();
        let mut persist_tasks = vec![
            state.persist_transactions(tx_batch),
            state.persist_tx_indices(tx_indices_batch),
            state.persist_tx_global_order(tx_global_order_batch.clone()),
            state.persist_events(events_batch),
            state.persist_event_indices(event_indices_batch),
            state.persist_displays(display_updates_batch),
            state.persist_packages(packages_batch),
            state.persist_checkpoint_objects(object_changes_batch),
            state.persist_object_history(object_history_changes_batch.clone()),
            state.persist_object_versions(object_versions_batch.clone()),
        ];
        if let Some(epoch_data) = epoch.clone() {
            persist_tasks.push(state.persist_epoch(epoch_data));
        }
        futures::future::join_all(persist_tasks)
            .await
            .into_iter()
            .map(|res| {
                if res.is_err() {
                    error!("Failed to persist data with error: {:?}", res);
                }
                res
            })
            .collect::<IndexerResult<Vec<_>>>()
            .expect("Persisting data into DB should not fail.");
    }

    state
        .update_status_for_checkpoint_transactions(tx_global_order_batch)
        .await
        .inspect_err(|e| {
            error!("failed to update tx global order as indexed with error: {e}");
        })
        .expect("updating tx global order as indexed should not fail.");

    let is_epoch_end = epoch.is_some();

    // handle partitioning on epoch boundary
    if let Some(epoch_data) = epoch {
        state
            .advance_epoch(epoch_data)
            .await
            .tap_err(|e| {
                error!("Failed to advance epoch with error: {}", e.to_string());
            })
            .expect("Advancing epochs in DB should not fail.");
        metrics.total_epoch_committed.inc();

        // Refresh participation metrics after advancing epoch
        state
            .refresh_participation_metrics()
            .await
            .tap_err(|e| {
                error!("Failed to update participation metrics: {e}");
            })
            .expect("Updating participation metrics should not fail.");
    }

    state
        .persist_checkpoints(checkpoint_batch)
        .await
        .tap_err(|e| {
            error!(
                "Failed to persist checkpoint data with error: {}",
                e.to_string()
            );
        })
        .expect("Persisting data into DB should not fail.");

    if is_epoch_end {
        // The epoch has advanced so we update the configs for the new protocol version,
        // if it has changed.
        let chain_id = <PgIndexerStore as IndexerStore>::get_chain_identifier(state)
            .await
            .expect("Failed to get chain identifier")
            .expect("Chain identifier should have been indexed at this point");
        let _ = state.persist_protocol_configs_and_feature_flags(chain_id);
    }

    let elapsed = guard.stop_and_record();

    info!(
        elapsed,
        "Checkpoint {}-{} committed with {} transactions.",
        first_checkpoint_seq,
        last_checkpoint_seq,
        tx_count,
    );
    metrics
        .latest_tx_checkpoint_sequence_number
        .set(last_checkpoint_seq as i64);
    metrics
        .total_tx_checkpoint_committed
        .inc_by(checkpoint_num as u64);
    metrics.total_transaction_committed.inc_by(tx_count as u64);
    metrics
        .transaction_per_checkpoint
        .observe(tx_count as f64 / (last_checkpoint_seq - first_checkpoint_seq + 1) as f64);
    // 1000.0 is not necessarily the batch size, it's to roughly map average tx
    // commit latency to [0.1, 1] seconds, which is well covered by
    // DB_COMMIT_LATENCY_SEC_BUCKETS.
    metrics
        .thousand_transaction_avg_db_commit_latency
        .observe(elapsed * 1000.0 / tx_count as f64);
}

/// Defines the logic of writing operations to the database.
///
/// The writing can refer to one or multiple tables in the database.
#[async_trait]
pub trait Writer<T: Send + Sync + 'static>: Send + Sync {
    /// Returns the writer name.
    fn name(&self) -> String;

    /// Commits batch of transformed data to DB.
    async fn persist(&self, batch: Vec<T>) -> IndexerResult<()>;

    /// Reads high watermark of the table DB.
    async fn get_watermark_hi(&self) -> IndexerResult<Option<u64>>;

    /// Sets high watermark of the table DB, also update metrics.
    async fn set_watermark_hi(&self, watermark_hi: u64) -> IndexerResult<()>;

    /// Gets the current max checkpoint that can be committed by the writer.
    ///
    /// This is for writers that have a predefined lag compared to the latest
    /// checkpoint in the network.
    ///
    /// One use-case is the objects snapshot handler, which waits for the lag
    /// between snapshot and latest checkpoint to reach a certain threshold.
    ///
    /// # Note
    /// By default, returns `u64::MAX`, which means no extra waiting is needed
    /// before committing.
    async fn get_max_committable_checkpoint(&self) -> IndexerResult<u64> {
        Ok(u64::MAX)
    }

    /// Processes the received data and persists it into a storage.
    ///
    /// - The data are received form the ingestion worker in which stage is
    ///   transformed into something which can be directly commited into the
    ///   database.
    /// - The data received by this function are not guaranteed to be in order.
    ///   The purpose of this function is to order the data by checkpoint
    ///   sequence number and to ensure data committed are in order and
    ///   contiguous.
    ///
    /// In addition, the method updates the watermark of the table of the data
    /// is persisted to.
    async fn persist_sequentially(
        &self,
        cp_receiver: iota_metrics::metered_channel::Receiver<(u64, T)>,
        cancel: CancellationToken,
    ) -> IndexerResult<()> {
        let checkpoint_commit_batch_size = std::env::var("CHECKPOINT_COMMIT_BATCH_SIZE")
            .ok()
            .and_then(|val| val.parse().ok())
            .unwrap_or(CHECKPOINT_COMMIT_BATCH_SIZE);
        let mut stream = iota_metrics::metered_channel::ReceiverStream::new(cp_receiver)
            .ready_chunks(checkpoint_commit_batch_size);

        let mut unprocessed = BTreeMap::new();
        let mut tuple_batch = vec![];
        let mut next_cp_to_process = self
            .get_watermark_hi()
            .await?
            .map(|n| n.saturating_add(1))
            .unwrap_or_default();

        loop {
            if cancel.is_cancelled() {
                info!("transform and load task terminating gracefully");
                return Ok(());
            }

            // Try to fetch new data tuple from the stream
            if unprocessed.len() >= UNPROCESSED_CHECKPOINT_SIZE_LIMIT {
                tracing::debug!(
                    "Unprocessed checkpoint size reached limit {UNPROCESSED_CHECKPOINT_SIZE_LIMIT}, skip reading from stream..."
                );
            } else {
                // Try to fetch new data tuple from the stream
                match stream.next().now_or_never() {
                    Some(Some(tuple_chunk)) => {
                        if cancel.is_cancelled() {
                            info!("transform and load task terminating gracefully");
                            return Ok(());
                        }
                        for (cp_seq, data) in tuple_chunk {
                            unprocessed.insert(cp_seq, (cp_seq, data));
                        }
                    }
                    Some(None) => break, // Stream has ended
                    None => {}           // No new data tuple available right now
                }
            }

            // Process unprocessed checkpoints, even no new checkpoints from stream
            let checkpoint_lag_limiter = self.get_max_committable_checkpoint().await?;
            while next_cp_to_process <= checkpoint_lag_limiter {
                if let Some(data_tuple) = unprocessed.remove(&next_cp_to_process) {
                    tuple_batch.push(data_tuple);
                    next_cp_to_process += 1;
                } else {
                    break;
                }
            }

            if !tuple_batch.is_empty() && checkpoint_lag_limiter != 0 {
                let tuple_batch = std::mem::take(&mut tuple_batch);
                let (last_checkpoint_seq, _data) = tuple_batch.last().unwrap();
                let last_checkpoint_seq = last_checkpoint_seq.to_owned();
                let batch = tuple_batch
                    .into_iter()
                    .map(|(_cp_seq, data)| data)
                    .collect();
                self.persist(batch).await.map_err(|e| {
                    IndexerError::PostgresWrite(format!(
                        "Failed to load transformed data into DB for handler {}: {e}",
                        self.name()
                    ))
                })?;
                self.set_watermark_hi(last_checkpoint_seq).await?;
            }
        }
        Err(IndexerError::ChannelClosed(format!(
            "Checkpoint channel is closed unexpectedly for handler {}",
            self.name()
        )))
    }
}
