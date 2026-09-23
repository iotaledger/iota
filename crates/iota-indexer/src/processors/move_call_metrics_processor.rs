// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use tap::tap::TapFallible;
use tracing::{error, info};

use crate::{
    errors::IndexerError,
    ingestion::common::persist::CommitterTables,
    metrics::IndexerMetrics,
    processors::resume_cursor,
    store::{IndexerAnalyticalStore, diesel_macro::spawn_blocking_task},
    types::IndexerResult,
};

/// The tables the move call metrics are computed from.
const MOVE_CALL_METRICS_TABLES: &[CommitterTables] = &[
    CommitterTables::Transactions,
    CommitterTables::TxCallsFun,
    CommitterTables::Checkpoints,
];

const MOVE_CALL_PROCESSOR_BATCH_SIZE: usize = 80000;
const PARALLELISM: usize = 10;

pub struct MoveCallMetricsProcessor<S> {
    pub store: S,
    metrics: IndexerMetrics,
    pub move_call_processor_batch_size: usize,
    pub move_call_processor_parallelism: usize,
}

impl<S> MoveCallMetricsProcessor<S>
where
    S: IndexerAnalyticalStore + Clone + Sync + Send + 'static,
{
    pub fn new(store: S, metrics: IndexerMetrics) -> MoveCallMetricsProcessor<S> {
        let move_call_processor_batch_size = std::env::var("MOVE_CALL_PROCESSOR_BATCH_SIZE")
            .map(|s| s.parse::<usize>().unwrap_or(MOVE_CALL_PROCESSOR_BATCH_SIZE))
            .unwrap_or(MOVE_CALL_PROCESSOR_BATCH_SIZE);
        let move_call_processor_parallelism = std::env::var("MOVE_CALL_PROCESSOR_PARALLELISM")
            .map(|s| s.parse::<usize>().unwrap_or(PARALLELISM))
            .unwrap_or(PARALLELISM);
        Self {
            store,
            metrics,
            move_call_processor_batch_size,
            move_call_processor_parallelism,
        }
    }

    pub async fn start(&self) -> IndexerResult<()> {
        info!("Indexer move call metrics async processor started...");
        let latest_move_call_tx_seq = self.store.get_latest_move_call_tx_seq().await?;
        let mut last_processed_tx_seq = latest_move_call_tx_seq.unwrap_or_default().seq;
        let latest_move_call_epoch = self.store.get_latest_move_call_metrics().await?;
        let mut last_processed_epoch = latest_move_call_epoch.unwrap_or_default().epoch;
        loop {
            // The database may not hold history back to the cursor, either because
            // it was restored from a snapshot or because the pruner moved past it.
            let lower_bounds = self
                .store
                .get_watermark_lower_bounds(MOVE_CALL_METRICS_TABLES)
                .await?;
            last_processed_tx_seq =
                resume_cursor(last_processed_tx_seq, lower_bounds.min_available_tx);
            last_processed_epoch =
                resume_cursor(last_processed_epoch, lower_bounds.min_available_epoch);

            let mut latest_tx = self.store.get_latest_stored_transaction().await?;
            while if let Some(tx) = latest_tx {
                tx.tx_sequence_number
                    < last_processed_tx_seq + self.move_call_processor_batch_size as i64
            } else {
                true
            } {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                latest_tx = self.store.get_latest_stored_transaction().await?;
            }

            let batch_size = self.move_call_processor_batch_size;
            let batch_end_tx_seq = last_processed_tx_seq + batch_size as i64;

            // Confirm the end of the batch is in the database before doing the work,
            // so a pruned range is caught before anything is persisted.
            let batch_end_tx = self.store.get_tx(batch_end_tx_seq).await?.ok_or_else(|| {
                IndexerError::DataPruned(format!(
                    "transaction {batch_end_tx_seq} is not in the database"
                ))
            })?;
            let batch_end_cp_seq = batch_end_tx.checkpoint_sequence_number;
            let end_epoch = self
                .store
                .get_cp(batch_end_cp_seq)
                .await?
                .ok_or_else(|| {
                    IndexerError::DataPruned(format!(
                        "checkpoint {batch_end_cp_seq} is not in the database"
                    ))
                })?
                .epoch;

            let step_size = batch_size / self.move_call_processor_parallelism;
            let mut persist_tasks = vec![];
            for chunk_start_tx_seq in (last_processed_tx_seq + 1
                ..last_processed_tx_seq + batch_size as i64 + 1)
                .step_by(step_size)
            {
                let move_call_store = self.store.clone();
                persist_tasks.push(spawn_blocking_task(move || {
                    move_call_store.persist_move_calls_in_tx_range(
                        chunk_start_tx_seq,
                        chunk_start_tx_seq + step_size as i64,
                    )
                }));
            }
            futures::future::join_all(persist_tasks)
                .await
                .into_iter()
                .collect::<Result<Vec<_>, _>>()
                .tap_err(|e| {
                    error!("error joining move call persist tasks: {e:?}");
                })?
                .into_iter()
                .collect::<Result<Vec<_>, _>>()
                .tap_err(|e| {
                    error!("error persisting move calls: {e:?}");
                })?;
            last_processed_tx_seq += batch_size as i64;
            info!("Persisted move_calls at tx seq: {}", last_processed_tx_seq);
            self.metrics
                .latest_move_call_metrics_tx_seq
                .set(last_processed_tx_seq);

            for epoch in last_processed_epoch + 1..end_epoch {
                self.store
                    .calculate_and_persist_move_call_metrics(epoch)
                    .await?;
                info!("Persisted move_call_metrics for epoch: {}", epoch);
            }
            last_processed_epoch = end_epoch - 1;
        }
    }
}
