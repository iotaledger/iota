// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod metrics;
mod worker;

use std::{path::PathBuf, sync::Arc};

use anyhow::Result;
use iota_data_ingestion_core::{
    DataIngestionMetrics, FileProgressStore, IndexerExecutor, ReaderOptions, WorkerPool,
};
use iota_names::config::IotaNamesConfig;
use tokio_util::sync::CancellationToken;
use tracing::info;

use self::{
    metrics::{IotaNamesMetrics, METRICS, start_prometheus_server},
    worker::IotaNamesWorker,
};

#[tokio::main]
async fn main() -> Result<()> {
    let _guard = telemetry_subscribers::TelemetryConfig::new()
        .with_env()
        .init();
    info!("Starting IOTA Names Indexer...");

    let registry = start_prometheus_server();

    METRICS.get_or_init(|| Arc::new(IotaNamesMetrics::new(&registry)));
    let metrics = DataIngestionMetrics::new(&registry);

    let progress_store = FileProgressStore::new("./progress_store").await?;

    let cancel_token = CancellationToken::new();
    let mut executor = IndexerExecutor::new(progress_store, 1, metrics, cancel_token);

    let worker = IotaNamesWorker::new(IotaNamesConfig::from_env().unwrap_or_default());
    let worker_pool = WorkerPool::new(
        worker,
        "iota_names_reader".to_string(),
        1,
        Default::default(),
    );

    executor.register(worker_pool).await.unwrap();
    executor
        .run(
            PathBuf::from("./chk".to_string()), /* path to a local directory where checkpoints
                                                 * are stored. */
            Some("http://host.docker.internal:9000/api/v1".to_string()),
            vec![],                   // optional remote store access options.
            ReaderOptions::default(), // remote_read_batch_size.
        )
        .await
        .unwrap();

    Ok(())
}
