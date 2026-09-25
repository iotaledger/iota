// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use tap::tap::TapFallible;
use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::{
    ingestion::common::persist::CommitterTables,
    metrics::IndexerMetrics,
    processors::POLL_INTERVAL,
    store::{IndexerAnalyticalStore, diesel_macro::spawn_blocking_task},
    types::IndexerResult,
};

const ADDRESS_METRICS_TABLES: &[CommitterTables] = &[
    CommitterTables::Transactions,
    CommitterTables::TxSenders,
    CommitterTables::TxRecipients,
];

const ADDRESS_PROCESSOR_BATCH_SIZE: usize = 80000;
const PARALLELISM: usize = 10;

pub struct AddressMetricsProcessor<S> {
    pub store: S,
    metrics: IndexerMetrics,
    pub address_processor_batch_size: usize,
    pub address_processor_parallelism: usize,
}

impl<S> AddressMetricsProcessor<S>
where
    S: IndexerAnalyticalStore + Clone + Sync + Send + 'static,
{
    pub fn new(store: S, metrics: IndexerMetrics) -> AddressMetricsProcessor<S> {
        let address_processor_batch_size = std::env::var("ADDRESS_PROCESSOR_BATCH_SIZE")
            .map(|s| s.parse::<usize>().unwrap_or(ADDRESS_PROCESSOR_BATCH_SIZE))
            .unwrap_or(ADDRESS_PROCESSOR_BATCH_SIZE);
        let address_processor_parallelism = std::env::var("ADDRESS_PROCESSOR_PARALLELISM")
            .map(|s| s.parse::<usize>().unwrap_or(PARALLELISM))
            .unwrap_or(PARALLELISM);
        Self {
            store,
            metrics,
            address_processor_batch_size,
            address_processor_parallelism,
        }
    }

    pub async fn start(&self) -> IndexerResult<()> {
        info!("Indexer address metrics async processor started...");
        let latest_tx_seq = self
            .store
            .get_address_metrics_last_processed_tx_seq()
            .await?;
        let mut last_processed_tx_seq = latest_tx_seq.unwrap_or_default().seq;
        loop {
            // The database may not hold history back to the cursor, either because
            // it was restored from a snapshot or because the pruner moved past it.
            let lower_bounds = self
                .store
                .get_watermark_lower_bounds(ADDRESS_METRICS_TABLES)
                .await?;
            // The cursor is the last processed key and the batch starts right after it,
            // so resume one below the first available key to include that key.
            last_processed_tx_seq = last_processed_tx_seq.max(lower_bounds.min_available_tx - 1);
            info!(
                "starting address processor from lowest available transaction {last_processed_tx_seq}"
            );

            let mut latest_tx = self.store.get_latest_stored_transaction().await?;
            while if let Some(tx) = latest_tx {
                tx.tx_sequence_number
                    < last_processed_tx_seq + self.address_processor_batch_size as i64
            } else {
                true
            } {
                sleep(POLL_INTERVAL).await;
                latest_tx = self.store.get_latest_stored_transaction().await?;
            }

            let batch_size = self.address_processor_batch_size;
            let batch_end_tx_seq = last_processed_tx_seq + batch_size as i64;

            // Ensure the batch end exists in the database. This does not happen in
            // normal circumstances: only an aggressive `pruning_delay_ms` can delete
            // it, in which case the loop resumes from the new lower bound.
            let Some(batch_end_tx) = self.store.get_tx(batch_end_tx_seq).await? else {
                warn!(
                    "transaction {batch_end_tx_seq} is not in the database, resuming from the lower bound"
                );
                sleep(POLL_INTERVAL).await;
                continue;
            };
            let batch_end_cp_seq = batch_end_tx.checkpoint_sequence_number;

            let mut persist_tasks = vec![];
            let step_size = batch_size / self.address_processor_parallelism;
            for chunk_start_tx_seq in (last_processed_tx_seq + 1
                ..last_processed_tx_seq + batch_size as i64 + 1)
                .step_by(step_size)
            {
                let address_store = self.store.clone();
                persist_tasks.push(spawn_blocking_task(move || {
                    address_store.persist_addresses_in_tx_range(
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
                    error!("error joining address persist tasks: {e:?}");
                })?
                .into_iter()
                .collect::<Result<Vec<_>, _>>()
                .tap_err(|e| {
                    error!("error persisting addresses or active addresses: {e:?}");
                })?;
            last_processed_tx_seq += self.address_processor_batch_size as i64;
            info!(
                "Persisted addresses and active addresses for tx seq: {}",
                last_processed_tx_seq,
            );
            self.metrics
                .latest_address_metrics_tx_seq
                .set(last_processed_tx_seq);

            self.store
                .calculate_and_persist_address_metrics(batch_end_cp_seq)
                .await?;
            info!(
                "Persisted address metrics for checkpoint: {}",
                batch_end_cp_seq
            );
        }
    }
}
