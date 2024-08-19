// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use super::{fetcher::CheckpointDownloadData, interface::Handler};
use crate::metrics::IndexerMetrics;
use anyhow::Result;
use iota_rest_api::CheckpointData;
use tap::TapFallible;
use tracing::error;

// Limit indexing parallelism on big checkpoints to avoid OOM,
// by limiting the total size of batch checkpoints to ~50MB.
// On testnet, most checkpoints are < 200KB, some can go up to 50MB.
const CHECKPOINT_PROCESSING_BATCH_DATA_LIMIT: usize = 50000000;
const CHECKPOINT_PROCESSING_BATCH_SIZE: usize = 100;

pub async fn run<S>(
    stream: S,
    mut handlers: Vec<Box<dyn Handler>>,
    metrics: IndexerMetrics,
) -> Result<()>
where
    S: futures::Stream<Item = CheckpointDownloadData> + std::marker::Unpin,
{
    use futures::StreamExt;

    let batch_size = std::env::var("CHECKPOINT_PROCESSING_BATCH_SIZE")
        .unwrap_or(CHECKPOINT_PROCESSING_BATCH_SIZE.to_string())
        .parse::<usize>()
        .unwrap();
    tracing::info!("Indexer runner is starting with {batch_size}");
    let mut chunks: futures::stream::ReadyChunks<S> = stream.ready_chunks(batch_size);
    while let Some(checkpoints) = chunks.next().await {
        // TODO create tracing spans for processing
        let mut cp_batch = vec![];
        let mut cp_batch_total_size = 0;
        for checkpoint in checkpoints.iter() {
            cp_batch_total_size += checkpoint.size;
            cp_batch.push(checkpoint.data.clone());
            if cp_batch_total_size >= CHECKPOINT_PROCESSING_BATCH_DATA_LIMIT {
                call_handlers_on_checkpoints_batch(&mut handlers, &cp_batch).await?;
                metrics.indexing_batch_size.set(cp_batch_total_size as i64);
                cp_batch = vec![];
                cp_batch_total_size = 0;
            }
        }
        if !cp_batch.is_empty() {
            call_handlers_on_checkpoints_batch(&mut handlers, &cp_batch).await?;
            metrics.indexing_batch_size.set(cp_batch_total_size as i64);
        }
    }

    Ok(())
}

async fn call_handlers_on_checkpoints_batch(
    handlers: &mut Vec<Box<dyn Handler>>,
    cp_batch: &Vec<CheckpointData>,
) -> Result<()> {
    use futures::StreamExt;

    let mut handlers_results: futures::stream::FuturesUnordered<_> = handlers
        .iter_mut()
        .map(|handler| async { handler.process_checkpoints(&cp_batch).await })
        .collect();

    while let Some(handler_result) = handlers_results.next().await {
        handler_result.tap_err(|e| {
            error!(
                "One of checkpoint processing handlers failed: {}",
                e.to_string(),
            );
        })?;
    }

    Ok(())
}
