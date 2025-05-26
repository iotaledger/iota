// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod metrics;
mod worker;

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::Arc,
};

use anyhow::Result;
use iota_data_ingestion_core::{
    DataIngestionMetrics, FileProgressStore, IndexerExecutor, ReaderOptions, WorkerPool,
};
use iota_names::config::IotaNamesConfig;
use tokio_util::sync::CancellationToken;

use self::{
    metrics::{IotaNamesMetrics, METRICS},
    worker::IotaNamesWorker,
};

// struct IotaNamesRegistryEvent {
//     key: String,
//     value: NameRecord,
// }

#[tokio::main]
async fn main() -> Result<()> {
    let cancel_token = CancellationToken::new();
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9184);
    let registry = iota_metrics::start_prometheus_server(addr).default_registry();
    METRICS.get_or_init(|| Arc::new(IotaNamesMetrics::new(&registry)));

    let backfill_progress_file_path = "./backfill_progress".to_string();
    let progress_store = FileProgressStore::new(PathBuf::from(backfill_progress_file_path)).await?;

    let metrics = DataIngestionMetrics::new(&registry);
    let mut executor = IndexerExecutor::new(progress_store, 1, metrics, cancel_token);

    let worker = IotaNamesWorker::new(IotaNamesConfig::from_env().unwrap_or_default());
    let worker_pool = WorkerPool::new(
        worker,
        "iota_names_reader".to_string(),
        1,
        Default::default(),
    );
    // register the worker pool to the executor.
    executor.register(worker_pool).await.unwrap();
    // run the ingestion pipeline.
    executor
        .run(
            PathBuf::from("./chk".to_string()), /* path to a local directory where checkpoints
                                                 * are stored. */
            Some("http://localhost:9000/api/v1".to_string()),
            vec![],                   // optional remote store access options.
            ReaderOptions::default(), // remote_read_batch_size.
        )
        .await
        .unwrap();

    // To get the metrics open: http://localhost:9184/metrics

    Ok(())
}
