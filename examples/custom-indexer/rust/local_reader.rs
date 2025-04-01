// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{env, path::PathBuf, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use iota_data_ingestion_core::{
    DataIngestionMetrics, FileProgressStore, IndexerExecutor, ReaderOptions, Worker, WorkerPool,
};
use iota_types::full_checkpoint_content::CheckpointData;
use prometheus::Registry;
use tokio_util::sync::CancellationToken;

struct CustomWorker;

#[async_trait]
impl Worker for CustomWorker {
    type Message = ();
    type Error = anyhow::Error;

    async fn process_checkpoint(&self, checkpoint: Arc<CheckpointData>) -> Result<Self::Message> {
        // custom processing logic
        println!(
            "Processing Local checkpoint: {}",
            checkpoint.checkpoint_summary.to_string()
        );
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let concurrency = 5;
    let metrics = DataIngestionMetrics::new(&Registry::new());
    let backfill_progress_file_path =
        env::var("BACKFILL_PROGRESS_FILE_PATH").unwrap_or("/tmp/local_reader_progress".to_string());
    let progress_store = FileProgressStore::new(backfill_progress_file_path).await?;
    let mut executor = IndexerExecutor::new(
        progress_store,
        1, // number of workflow types
        metrics,
        CancellationToken::new(),
    );
    let worker_pool = WorkerPool::new(
        CustomWorker,
        "local_reader".to_string(),
        concurrency,
        Default::default(),
    );

    executor.register(worker_pool).await?;
    executor
        .run(
            PathBuf::from("./chk".to_string()), // path to a local directory
            None,
            vec![],                   // optional remote store access options
            ReaderOptions::default(), // remote_read_batch_size
        )
        .await?;
    Ok(())
}
