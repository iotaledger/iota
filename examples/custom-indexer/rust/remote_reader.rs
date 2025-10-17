// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{env, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use iota_data_ingestion_core::{
    DataIngestionMetrics, FileProgressStore, IndexerExecutor, ReaderOptions, Worker, WorkerPool,
    reader::v2::{CheckpointReaderConfig, RemoteUrl},
};
use iota_types::full_checkpoint_content::CheckpointData;
use prometheus::Registry;

struct CustomWorker;

#[async_trait]
impl Worker for CustomWorker {
    type Message = ();
    type Error = anyhow::Error;

    async fn process_checkpoint(&self, checkpoint: Arc<CheckpointData>) -> Result<Self::Message> {
        // custom processing logic
        println!(
            "Processing checkpoint: {}",
            checkpoint.checkpoint_summary.to_string()
        );
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Number of Workers to process checkpoints in parallel.
    let concurrency = 5;
    let metrics = DataIngestionMetrics::new(&Registry::new());
    let progress_file_path =
        env::var("PROGRESS_FILE_PATH").unwrap_or("/tmp/remote_reader_progress".to_string());
    // Save last processed checkpoint to a file.
    let progress_store = FileProgressStore::new(progress_file_path).await?;

    let mut executor = IndexerExecutor::new(
        progress_store,
        1, // should match the total number of registered workers.
        metrics,
        Default::default(),
    );
    let worker_pool = WorkerPool::new(
        CustomWorker,
        "remote_reader".to_string(),
        concurrency,
        Default::default(),
    );

    executor.register(worker_pool).await?;

    let config = CheckpointReaderConfig {
        // It's also possible to start a fullnode locally and use the gRPC connection to sync
        // checkpoints data.
        //
        // remote_store_url: Some(RemoteUrl::Fullnode("http://127.0.0.1:50051".to_string())),
        remote_store_url: Some(RemoteUrl::HybridHistoricalStore {
            historical_url: "https://checkpoints.mainnet.iota.cafe/ingestion/historical".into(),
            live_url: Some("https://checkpoints.mainnet.iota.cafe/ingestion/live".into()),
        }),
        reader_options: ReaderOptions::default(),
        ..Default::default()
    };
    executor.run_with_config(config).await?;
    Ok(())
}
