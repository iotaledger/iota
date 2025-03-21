// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{env, path::PathBuf};

use anyhow::Result;
use iota_data_ingestion::{
    ArchivalConfig, ArchivalReducer, BlobTaskConfig, BlobWorker, DynamoDBProgressStore,
    HistoricalReducer, HistoricalWriterConfig, KVStoreTaskConfig, KVStoreWorker, RelayWorker,
    common,
};
use iota_data_ingestion_core::{DataIngestionMetrics, IndexerExecutor, ReaderOptions, WorkerPool};
use iota_rest_api::Client;
use prometheus::Registry;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
enum Task {
    Archival(ArchivalConfig),
    Blob(BlobTaskConfig),
    Kv(KVStoreTaskConfig),
    Historical(HistoricalWriterConfig),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
struct TaskConfig {
    #[serde(flatten)]
    task: Task,
    name: String,
    concurrency: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
struct ProgressStoreConfig {
    pub aws_access_key_id: String,
    pub aws_secret_access_key: String,
    pub aws_region: String,
    pub table_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct IndexerConfig {
    path: PathBuf,
    tasks: Vec<TaskConfig>,
    progress_store: ProgressStoreConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    remote_store_url: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    remote_store_options: Vec<(String, String)>,
    #[serde(default = "default_remote_read_batch_size")]
    remote_read_batch_size: usize,
    #[serde(default = "default_metrics_host")]
    metrics_host: String,
    #[serde(default = "default_metrics_port")]
    metrics_port: u16,
}

fn default_metrics_host() -> String {
    "127.0.0.1".to_string()
}

fn default_metrics_port() -> u16 {
    8081
}

fn default_remote_read_batch_size() -> usize {
    100
}

fn setup_env(token: CancellationToken) {
    let default_hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |panic| {
        default_hook(panic);
        std::process::exit(12);
    }));

    tokio::spawn(async move {
        #[cfg(unix)]
        let terminate = async {
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("Cannot listen to SIGTERM signal")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = tokio::signal::ctrl_c() => tracing::info!("CTRL+C signal received, shutting down"),
            _ = terminate => tracing::info!("SIGTERM signal received, shutting down")
        };

        token.cancel();
    });
}

#[tokio::main]
async fn main() -> Result<()> {
    let token = CancellationToken::new();
    let child_token = token.child_token();
    setup_env(token);

    let args: Vec<String> = env::args().collect();
    assert_eq!(args.len(), 2, "configuration yaml file is required");
    let config: IndexerConfig = serde_yaml::from_str(&std::fs::read_to_string(&args[1])?)?;

    // setup metrics
    let _guard = telemetry_subscribers::TelemetryConfig::new()
        .with_env()
        .init();
    let registry_service = iota_metrics::start_prometheus_server(
        format!("{}:{}", config.metrics_host, config.metrics_port).parse()?,
    );
    let registry: Registry = registry_service.default_registry();
    iota_metrics::init_metrics(&registry);
    let metrics = DataIngestionMetrics::new(&registry);

    let progress_store = DynamoDBProgressStore::new(
        &config.progress_store.aws_access_key_id,
        &config.progress_store.aws_secret_access_key,
        config.progress_store.aws_region,
        config.progress_store.table_name,
    )
    .await;

    let mut executor =
        IndexerExecutor::new(progress_store, config.tasks.len(), metrics, child_token);
    for task_config in config.tasks {
        match task_config.task {
            Task::Archival(archival_config) => {
                let reducer = ArchivalReducer::new(archival_config).await?;
                executor
                    .update_watermark(task_config.name.clone(), reducer.get_watermark().await?)
                    .await?;
                let worker_pool = WorkerPool::new_with_reducer(
                    RelayWorker,
                    task_config.name,
                    task_config.concurrency,
                    reducer,
                );
                executor.register(worker_pool).await?;
            }
            Task::Blob(blob_config) => {
                let rest_client = Client::new(&blob_config.node_rest_api_url);
                let watermark = executor.read_watermark(task_config.name.clone()).await?;
                let current_epoch = common::current_epoch(&rest_client).await?;
                let current_epoch_first_checkpoint_seq_num =
                    common::epoch_first_checkpoint_sequence_number(&rest_client, current_epoch)
                        .await?;
                // if watermark is less than the first checkpoint of current epoch
                // is safe to assume that an epoch was changed.
                let worker = if watermark < current_epoch_first_checkpoint_seq_num {
                    // updating the watermark ensures that the worker will start synchronization
                    // from that point onward.
                    executor
                        .update_watermark(
                            task_config.name.clone(),
                            current_epoch_first_checkpoint_seq_num,
                        )
                        .await?;
                    // get the range from the first checkpoint of the watermark's epoch to the
                    // watermark
                    let reset_range = common::checkpoint_sequence_number_range_to_watermark(
                        &rest_client,
                        watermark,
                    )
                    .await?;
                    let worker = BlobWorker::new(blob_config, rest_client, current_epoch)?;
                    worker.reset_remote_store(reset_range).await?;
                    worker
                } else {
                    BlobWorker::new(blob_config, rest_client, current_epoch)?
                };

                let worker_pool =
                    WorkerPool::new(worker, task_config.name, task_config.concurrency);
                executor.register(worker_pool).await?;
            }
            Task::Kv(kv_config) => {
                let worker_pool = WorkerPool::new(
                    KVStoreWorker::new(kv_config).await,
                    task_config.name,
                    task_config.concurrency,
                );
                executor.register(worker_pool).await?;
            }
            Task::Historical(historical_config) => {
                let reducer = HistoricalReducer::new(historical_config).await?;
                executor
                    .update_watermark(task_config.name.clone(), reducer.get_watermark().await?)
                    .await?;
                let worker_pool = WorkerPool::new_with_reducer(
                    RelayWorker,
                    task_config.name,
                    task_config.concurrency,
                    reducer,
                );
                executor.register(worker_pool).await?;
            }
        };
    }

    let reader_options = ReaderOptions {
        batch_size: config.remote_read_batch_size,
        ..Default::default()
    };

    executor
        .run(
            config.path,
            config.remote_store_url,
            config.remote_store_options,
            reader_options,
        )
        .await?;
    Ok(())
}
