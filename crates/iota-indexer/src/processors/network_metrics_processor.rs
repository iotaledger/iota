// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use tap::tap::TapFallible;
use tracing::{error, info};

use crate::{
    errors::IndexerError, metrics::IndexerMetrics, store::IndexerAnalyticalStore,
    types::IndexerResult,
};

const MIN_NETWORK_METRICS_PROCESSOR_BATCH_SIZE: usize = 10;
const MAX_NETWORK_METRICS_PROCESSOR_BATCH_SIZE: usize = 80000;
const NETWORK_METRICS_PROCESSOR_PARALLELISM: usize = 1;

pub struct NetworkMetricsProcessor<S> {
    pub store: S,
    metrics: IndexerMetrics,
    pub min_network_metrics_processor_batch_size: usize,
    pub max_network_metrics_processor_batch_size: usize,
    pub network_metrics_processor_parallelism: usize,
}

impl<S> NetworkMetricsProcessor<S>
where
    S: IndexerAnalyticalStore + Clone + Sync + Send + 'static,
{
    pub fn new(store: S, metrics: IndexerMetrics) -> NetworkMetricsProcessor<S> {
        let min_network_metrics_processor_batch_size =
            std::env::var("MIN_NETWORK_METRICS_PROCESSOR_BATCH_SIZE")
                .map(|s| {
                    s.parse::<usize>()
                        .unwrap_or(MIN_NETWORK_METRICS_PROCESSOR_BATCH_SIZE)
                })
                .unwrap_or(MIN_NETWORK_METRICS_PROCESSOR_BATCH_SIZE);
        let max_network_metrics_processor_batch_size =
            std::env::var("MAX_NETWORK_METRICS_PROCESSOR_BATCH_SIZE")
                .map(|s| {
                    s.parse::<usize>()
                        .unwrap_or(MAX_NETWORK_METRICS_PROCESSOR_BATCH_SIZE)
                })
                .unwrap_or(MAX_NETWORK_METRICS_PROCESSOR_BATCH_SIZE);
        let network_metrics_processor_parallelism =
            std::env::var("NETWORK_METRICS_PROCESSOR_PARALLELISM")
                .map(|s| {
                    s.parse::<usize>()
                        .unwrap_or(NETWORK_METRICS_PROCESSOR_PARALLELISM)
                })
                .unwrap_or(NETWORK_METRICS_PROCESSOR_PARALLELISM);
        Self {
            store,
            metrics,
            min_network_metrics_processor_batch_size,
            max_network_metrics_processor_batch_size,
            network_metrics_processor_parallelism,
        }
    }

    pub async fn start(&self) -> IndexerResult<()> {
        info!("Indexer network metrics async processor started...");
        let latest_tx_count_metrics = self
            .store
            .get_latest_tx_count_metrics()
            .await
            .unwrap_or_default();
        let latest_epoch_peak_tps = self
            .store
            .get_latest_epoch_peak_tps()
            .await
            .unwrap_or_default();
        let mut last_processed_cp_seq = latest_tx_count_metrics
            .unwrap_or_default()
            .checkpoint_sequence_number;
        let mut last_processed_peak_tps_epoch = latest_epoch_peak_tps.unwrap_or_default().epoch;

        loop {
            let latest_stored_checkpoint = loop {
                if let Some(latest_stored_checkpoint) =
                    self.store.get_latest_stored_checkpoint().await?
                {
                    if latest_stored_checkpoint.sequence_number
                        >= last_processed_cp_seq
                            + self.min_network_metrics_processor_batch_size as i64
                    {
                        break latest_stored_checkpoint;
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            };

            let available_checkpoints =
                latest_stored_checkpoint.sequence_number - last_processed_cp_seq;
            let batch_size =
                available_checkpoints.min(self.max_network_metrics_processor_batch_size as i64);

            info!(
                "Preparing tx count metrics for checkpoints [{}-{}]",
                last_processed_cp_seq + 1,
                last_processed_cp_seq + batch_size
            );

            let step_size =
                (batch_size as usize / self.network_metrics_processor_parallelism).max(1);
            let mut persist_tasks = vec![];

            for chunk_start_cp in
                (last_processed_cp_seq + 1..=last_processed_cp_seq + batch_size).step_by(step_size)
            {
                let chunk_end_cp =
                    (chunk_start_cp + step_size as i64).min(last_processed_cp_seq + batch_size + 1);

                let store = self.store.clone();
                persist_tasks.push(tokio::task::spawn_blocking(move || {
                    store.persist_tx_count_metrics(chunk_start_cp, chunk_end_cp)
                }));
            }

            futures::future::join_all(persist_tasks)
                .await
                .into_iter()
                .collect::<Result<Vec<_>, _>>()
                .tap_err(|e| error!("Error joining network persist tasks: {:?}", e))?
                .into_iter()
                .collect::<Result<Vec<_>, _>>()
                .tap_err(|e| error!("Error persisting tx count metrics: {:?}", e))?;

            last_processed_cp_seq += batch_size;

            self.metrics
                .latest_network_metrics_cp_seq
                .set(last_processed_cp_seq);

            let end_cp = self
                .store
                .get_checkpoints_in_range(last_processed_cp_seq, last_processed_cp_seq + 1)
                .await?
                .first()
                .ok_or(IndexerError::PostgresRead(
                    "Cannot read checkpoint from PG for epoch peak TPS".to_string(),
                ))?
                .clone();
            for epoch in last_processed_peak_tps_epoch + 1..end_cp.epoch {
                self.store.persist_epoch_peak_tps(epoch).await?;
                last_processed_peak_tps_epoch = epoch;
                info!("Persisted epoch peak TPS for epoch {}", epoch);
            }
        }
    }
}
