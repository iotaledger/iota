// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeSet, ops::RangeInclusive, sync::Arc, time::Instant};

use futures::{StreamExt, TryStreamExt};
use tokio::sync::Mutex;
use tracing::{error, info};

use crate::{
    backfill::{
        BackfillTaskKind, backfill_instances::get_backfill_task, backfill_task::BackfillTask,
    },
    config::BackfillConfig,
    db::ConnectionPool,
    errors::IndexerError,
};

pub struct BackfillRunner {}

impl BackfillRunner {
    pub async fn run(
        runner_kind: BackfillTaskKind,
        pool: ConnectionPool,
        backfill_config: BackfillConfig,
        total_range: RangeInclusive<usize>,
    ) -> Result<(), IndexerError> {
        let task = get_backfill_task(runner_kind);
        Self::run_impl(pool, backfill_config, total_range, task).await
    }

    /// Main function to run the parallel queries and batch processing.
    async fn run_impl(
        pool: ConnectionPool,
        config: BackfillConfig,
        total_range: RangeInclusive<usize>,
        task: Arc<dyn BackfillTask>,
    ) -> Result<(), IndexerError> {
        let timer = Instant::now();
        // Keeps track of the checkpoint ranges (using starting checkpoint number)
        // that are in progress.
        let in_progress = Arc::new(Mutex::new(BTreeSet::new()));

        // Generate chunks from the total range
        let chunks = create_chunks(total_range.clone(), config.chunk_size);

        tokio_stream::iter(chunks)
            .map(|range| process_chunk(task.as_ref(), pool.clone(), range, in_progress.clone()))
            .buffer_unordered(config.max_concurrency)
            .try_collect::<Vec<_>>()
            .await?;

        info!(elapsed = ?timer.elapsed(), "Finished backfill");

        Ok(())
    }
}

/// Creates chunks based on the total range and chunk size.
fn create_chunks(
    total_range: RangeInclusive<usize>,
    chunk_size: usize,
) -> Vec<RangeInclusive<usize>> {
    let end = *total_range.end();
    total_range
        .step_by(chunk_size)
        .map(|chunk_start| {
            let chunk_end = std::cmp::min(chunk_start + chunk_size - 1, end);
            chunk_start..=chunk_end
        })
        .collect()
}

async fn process_chunk(
    task: &dyn BackfillTask,
    pool: ConnectionPool,
    range: RangeInclusive<usize>,
    in_progress: Arc<Mutex<BTreeSet<usize>>>,
) -> Result<(), IndexerError> {
    let start = *range.start();
    let end = *range.end();

    {
        let mut guard = in_progress.lock().await;
        guard.insert(start);
    }

    if let Err(e) = task.backfill_range(pool, &range).await {
        let min_in_progress = {
            let guard = in_progress.lock().await;
            guard.iter().next().cloned().unwrap_or(start)
        };

        error!(
            "Backfill failed for chunk {start}-{end}. Current minimum range start number: {min_in_progress}. Error: {e}"
        );
        return Err(e);
    }

    {
        let mut guard = in_progress.lock().await;
        guard.remove(&start);

        if let Some(cp) = guard.iter().next().cloned() {
            info!("Minimum range start number still in progress: {}", cp);
        }
    }

    info!(start, end, "Persisted backfill chunk successfully");
    Ok(())
}
