// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use iota_data_ingestion_core::{Worker, setup_single_workflow};
use iota_types::full_checkpoint_content::CheckpointData;

struct CustomWorker;

#[async_trait]
impl Worker for CustomWorker {
    type Message = ();
    type Error = anyhow::Error;

    async fn process_checkpoint(&self, checkpoint: Arc<CheckpointData>) -> Result<Self::Message> {
        // custom processing logic
        // print out the checkpoint number
        println!(
            "Processing checkpoint: {}",
            checkpoint.checkpoint_summary.to_string()
        );
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let (executor, _) = setup_single_workflow(
        CustomWorker,
        "http://127.0.0.1:9000/api/v1".to_string(), // fullnode REST API
        0,                                          // initial checkpoint number
        5,                                          // concurrency
        None,                                       // extra reader options
    )
    .await?;
    executor.await?;
    Ok(())
}
