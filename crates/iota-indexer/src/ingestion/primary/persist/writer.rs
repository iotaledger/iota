// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
use futures::{StreamExt, stream::ReadyChunks};
use iota_metrics::metered_channel::ReceiverStream;
use tap::tap::TapFallible;
use tracing::{error, info, instrument};

use super::{CheckpointBatch, CheckpointDataToCommit, EpochToCommit, data};
use crate::{
    ingestion::common::persist::{
        CHECKPOINT_COMMIT_BATCH_SIZE, CommitterTables, CommitterWatermark,
    },
    metrics::IndexerMetrics,
    store::{IndexerStore, PgIndexerStore},
    types::IndexerResult,
};

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

    /// Writes indexed checkpoint data to the database, and then updates
    /// watermark upper bounds and metrics.
    ///
    /// There can be at most one epoch boundary at the end. If
    /// an epoch boundary is detected, epoch-partitioned tables must be
    /// advanced.
    ///
    /// # Panics
    ///
    /// Panics if the batch is empty.
    #[instrument(skip_all, fields(
        first = data_batch.basic.first().as_ref().unwrap().checkpoint.sequence_number,
        last = data_batch.basic.last().as_ref().unwrap().checkpoint.sequence_number
    ))]
    pub(crate) async fn commit_checkpoints(
        &self,
        data_batch: CheckpointBatch,
        epoch: Option<EpochToCommit>,
    ) {
        let batch_len = data_batch.len();

        let CheckpointBatch {
            basic,
            objects_history,
            filtered_queries,
            combined_event_filters,
        } = data_batch;

        let mut checkpoint_batch = Vec::with_capacity(batch_len);
        let mut tx_batch = Vec::with_capacity(batch_len);
        let mut displays_batch = Vec::with_capacity(batch_len);
        let mut object_changes_batch = Vec::with_capacity(batch_len);
        let mut object_versions_batch = Vec::with_capacity(batch_len);
        let mut packages_batch = Vec::with_capacity(batch_len);

        for basic_data in basic {
            let data::Basic {
                checkpoint,
                transactions,
                displays,
                object_changes,
                object_versions,
                packages,
                ..
            } = basic_data;
            checkpoint_batch.push(checkpoint);
            tx_batch.push(transactions);
            displays_batch.extend(displays.into_values());
            object_changes_batch.push(object_changes);
            object_versions_batch.push(object_versions);
            packages_batch.push(packages);
        }

        let first_checkpoint_seq = checkpoint_batch.first().as_ref().unwrap().sequence_number;

        let committer_watermark = CommitterWatermark::from(checkpoint_batch.last().unwrap());
        let mut committer_tables = Vec::from_iter(CommitterTables::basic());
        if objects_history.is_some() {
            committer_tables.extend(CommitterTables::objects_history());
        }
        if filtered_queries.is_some() {
            committer_tables.extend(CommitterTables::filtered_queries());
        }
        if combined_event_filters.is_some() {
            committer_tables.extend(CommitterTables::combined_event_filters());
        }

        let guard = self.metrics.checkpoint_db_commit_latency.start_timer();
        let tx_batch = tx_batch.into_iter().flatten().collect::<Vec<_>>();

        let tx_global_order_batch: Vec<_> = tx_batch.iter().map(Into::into).collect();
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
                self.state.persist_displays(displays_batch),
                self.state
                    .persist_packages(packages_batch.into_iter().map(Into::into).collect()),
                self.state
                    .persist_object_versions(object_versions_batch.clone()),
                Box::pin({
                    let object_changes_batch = object_changes_batch.clone();
                    async move {
                        // We need to persist global order before writing objects, so that
                        // optimistic indexing is blocked from overwriting
                        // objects table with old tx data reference: https://github.com/iotaledger/iota/issues/10250
                        self.state
                            .persist_tx_global_order(tx_global_order_batch.clone())
                            .await?;
                        self.state.persist_objects(object_changes_batch).await
                    }
                }),
            ];

            if let Some(history_batch) = objects_history {
                let history_objects = history_batch
                    .into_iter()
                    .flat_map(|data::ObjectsHistory { history_objects }| history_objects)
                    .collect();
                persist_tasks.push(
                    // Backward history must be persisted before checkpointed_objects
                    // to prevent read races during consistent view queries.
                    Box::pin({
                        let object_changes_batch = object_changes_batch.clone();
                        async move {
                            self.state
                                .persist_object_backward_history(history_objects)
                                .await?;
                            self.state
                                .persist_checkpointed_objects(object_changes_batch)
                                .await
                        }
                    }),
                )
            } else {
                persist_tasks.push(
                    self.state
                        .persist_checkpointed_objects(object_changes_batch),
                )
            }

            if let Some(filtered_queries_batch) = filtered_queries {
                let mut tx_indices_batch = Vec::with_capacity(filtered_queries_batch.len());
                let mut events_batch = Vec::with_capacity(filtered_queries_batch.len());
                for data::FilteredQueries { tx_indices, events } in filtered_queries_batch {
                    tx_indices_batch.extend(tx_indices);
                    events_batch.extend(events);
                }
                persist_tasks.extend([
                    self.state.persist_tx_indices(tx_indices_batch),
                    self.state.persist_events(events_batch),
                ]);
            }

            if let Some(combined_event_filters) = combined_event_filters {
                let event_indices = combined_event_filters
                    .into_iter()
                    .flat_map(|data::CombinedEventFilters { event_indices }| event_indices)
                    .collect();
                persist_tasks.push(self.state.persist_event_indices(event_indices));
            }

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

        let is_epoch_end = epoch.is_some();

        // On epoch boundary, we need to modify the existing partitions' upper bound,
        // and introduce a new partition for incoming data for the upcoming epoch.
        if let Some(epoch_data) = epoch {
            let new_epoch_id = epoch_data.new_epoch.epoch;
            self.state
                .advance_epoch(epoch_data)
                .await
                .tap_err(|e| {
                    error!("failed to advance epoch with error: {}", e.to_string());
                })
                .expect("advancing epochs in DB should not fail.");
            self.metrics.last_committed_epoch.set(new_epoch_id);

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
            if let Err(e) = self
                .state
                .execute_in_blocking_worker(move |this| {
                    this.persist_protocol_configs_and_feature_flags(chain_id)
                })
                .await
            {
                error!("failed to persist protocol configs and feature flags: {e}");
            }
        }

        self.state
            .update_watermarks_upper_bound(committer_tables, committer_watermark)
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
            committer_watermark.max_committed_cp,
            tx_count,
        );
        self.metrics
            .latest_tx_checkpoint_sequence_number
            .set(committer_watermark.max_committed_cp as i64);
        self.metrics
            .total_tx_checkpoint_committed
            .inc_by(checkpoint_num as u64);
        self.metrics
            .total_transaction_committed
            .inc_by(tx_count as u64);
        self.metrics.transaction_per_checkpoint.observe(
            tx_count as f64
                / (committer_watermark.max_committed_cp - first_checkpoint_seq + 1) as f64,
        );
        // 1000.0 is not necessarily the batch size, it's to roughly map average tx
        // commit latency to [0.1, 1] seconds, which is well covered by
        // DB_COMMIT_LATENCY_SEC_BUCKETS.
        self.metrics
            .thousand_transaction_avg_db_commit_latency
            .observe(elapsed * 1000.0 / tx_count as f64);
    }
}
