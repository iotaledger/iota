// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashMap},
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

use iota_types::messages_checkpoint::CheckpointSequenceNumber;
use tap::tap::TapFallible;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, instrument, warn};

use super::{CheckpointDataToCommit, EpochToCommit};
use crate::{metrics::IndexerMetrics, store::IndexerStore, types::IndexerResult};

pub(crate) const CHECKPOINT_COMMIT_BATCH_SIZE: usize = 100;

// Checkpoint tracking for robust progress logging
struct CheckpointTracker {
    last_checkpoint_checkpoint: AtomicU64,
    last_checkpoint_time: std::sync::Mutex<Instant>,
    start_time: Instant,
    checkpoint_interval: u64,
}

impl CheckpointTracker {
    fn new(checkpoint_interval: u64) -> Self {
        let now = Instant::now();
        Self {
            last_checkpoint_checkpoint: AtomicU64::new(0),
            last_checkpoint_time: std::sync::Mutex::new(now),
            start_time: now,
            checkpoint_interval,
        }
    }

    fn should_log_checkpoint(&self, checkpoint_seq: u64) -> bool {
        checkpoint_seq > 0 && checkpoint_seq % self.checkpoint_interval == 0
    }

    fn log_checkpoint(&self, checkpoint_seq: u64, tx_count: usize, elapsed_commit_ms: f64) {
        let now = Instant::now();
        let last_checkpoint_seq = self.last_checkpoint_checkpoint.load(Ordering::Relaxed);

        // Calculate rates and timing
        let total_duration = now.duration_since(self.start_time);
        let avg_checkpoints_per_sec = if total_duration.as_secs_f64() > 0.0 {
            checkpoint_seq as f64 / total_duration.as_secs_f64()
        } else {
            0.0
        };

        let interval_info = if last_checkpoint_seq > 0 {
            let mut last_time = self.last_checkpoint_time.lock().unwrap();
            let interval_duration = now.duration_since(*last_time);
            let interval_checkpoints = checkpoint_seq - last_checkpoint_seq;
            let interval_rate = if interval_duration.as_secs_f64() > 0.0 {
                interval_checkpoints as f64 / interval_duration.as_secs_f64()
            } else {
                0.0
            };
            *last_time = now;

            format!(
                "interval_checkpoints={}, interval_duration_s={:.2}, interval_rate_cp_s={:.2}",
                interval_checkpoints,
                interval_duration.as_secs_f64(),
                interval_rate
            )
        } else {
            "interval_info=first_checkpoint".to_string()
        };

        warn!(
            checkpoint_seq = checkpoint_seq,
            total_txs = tx_count,
            elapsed_commit_ms = elapsed_commit_ms,
            total_duration_s = total_duration.as_secs_f64(),
            avg_checkpoints_per_sec = avg_checkpoints_per_sec,
            checkpoint_interval = self.checkpoint_interval,
            "{} {}",
            format_args!(
                "Synced checkpoint {} | {} | avg_rate={:.2} cp/s | total_time={:.2}s | commit_time={:.2}ms",
                checkpoint_seq,
                interval_info,
                avg_checkpoints_per_sec,
                total_duration.as_secs_f64(),
                elapsed_commit_ms
            ),
            ""
        );

        self.last_checkpoint_checkpoint
            .store(checkpoint_seq, Ordering::Relaxed);
    }
}

// Global checkpoint tracker - using lazy_static or once_cell would be better
// but keeping simple
static CHECKPOINT_TRACKER: std::sync::OnceLock<CheckpointTracker> = std::sync::OnceLock::new();

pub async fn start_tx_checkpoint_commit_task<S>(
    state: S,
    metrics: IndexerMetrics,
    tx_indexing_receiver: iota_metrics::metered_channel::Receiver<CheckpointDataToCommit>,
    mut next_checkpoint_sequence_number: CheckpointSequenceNumber,
    cancel: CancellationToken,
) -> IndexerResult<()>
where
    S: IndexerStore + Clone + Sync + Send + 'static,
{
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
        if !batch.is_empty() && unprocessed.is_empty() {
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
async fn commit_checkpoints<S>(
    state: &S,
    indexed_checkpoint_batch: Vec<CheckpointDataToCommit>,
    epoch: Option<EpochToCommit>,
    metrics: &IndexerMetrics,
) where
    S: IndexerStore + Clone + Sync + Send + 'static,
{
    let mut checkpoint_batch = vec![];
    let mut tx_batch = vec![];
    let mut events_batch = vec![];
    let mut tx_indices_batch = vec![];
    let mut event_indices_batch = vec![];
    let mut display_updates_batch = BTreeMap::new();
    let mut object_changes_batch = vec![];
    let mut object_history_changes_batch = vec![];
    let mut object_versions_batch = vec![];
    let mut packages_batch = vec![];

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
            epoch: _,
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
            state.persist_events(events_batch),
            state.persist_event_indices(event_indices_batch),
            state.persist_displays(display_updates_batch),
            state.persist_packages(packages_batch),
            state.persist_objects(object_changes_batch.clone()),
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
        let chain_id = state
            .get_chain_identifier()
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

    // Log timestamp for major checkpoints (5000, 10000, 15000, etc.)
    // Check if any checkpoint was crossed in this batch
    let checkpoint_interval = 5000;
    let first_checkpoint = (first_checkpoint_seq / checkpoint_interval + 1) * checkpoint_interval;
    let last_checkpoint = (last_checkpoint_seq / checkpoint_interval) * checkpoint_interval;

    if last_checkpoint >= first_checkpoint && last_checkpoint > 0 {
        let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
        warn!(
            checkpoint_seq = last_checkpoint,
            batch_range = format!("{}-{}", first_checkpoint_seq, last_checkpoint_seq),
            timestamp = %timestamp,
            total_txs = tx_count,
            elapsed_ms = elapsed * 1000.0,
            "[indexer][profiling]: Checkpoint {} crossed in batch {} at {}",
            last_checkpoint,
            format!("{}-{}", first_checkpoint_seq, last_checkpoint_seq),
            timestamp
        );
    }

    // Robust checkpoint logging with comprehensive metrics
    if let Some(tracker) = CHECKPOINT_TRACKER.get() {
        if tracker.should_log_checkpoint(last_checkpoint_seq) {
            tracker.log_checkpoint(last_checkpoint_seq, tx_count, elapsed * 1000.0);
        }
    }

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
