// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    cmp,
    collections::BTreeSet,
    ops::RangeInclusive,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Instant,
};

use futures::{StreamExt, TryStreamExt, stream::unfold};
use tokio::sync::Mutex;
use tokio_stream::Stream;
use tracing::{error, info};

use crate::{
    backfill::{BackfillTaskKind, get_backfill_task, task::BackfillTask},
    config::BackfillConfig,
    db::ConnectionPool,
    errors::IndexerError,
};

/// Entry point for orchestrating backfills.
///
/// `BackfillRunner` selects the appropriate backfill implementation, splits the
/// requested checkpoint range into manageable chunks, and dispatches them in
/// parallel.
pub struct BackfillRunner;

impl BackfillRunner {
    /// Execute a backfill over `total_range` using the specified task kind.
    pub async fn run(
        runner_kind: BackfillTaskKind,
        pool: ConnectionPool,
        backfill_config: BackfillConfig,
        total_range: RangeInclusive<usize>,
    ) -> Result<(), IndexerError> {
        let task = get_backfill_task(runner_kind, *total_range.start()).await;
        Self::run_impl(pool, backfill_config, total_range, task).await
    }

    async fn run_impl(
        pool: ConnectionPool,
        config: BackfillConfig,
        total_range: RangeInclusive<usize>,
        task: Arc<dyn BackfillTask>,
    ) -> Result<(), IndexerError> {
        let timer = Instant::now();
        let processed_counter = Arc::new(AtomicUsize::new(0));
        // Keeps track of the ranges (using starting range number)
        // that are in progress.
        let in_progress = Arc::new(Mutex::new(BTreeSet::new()));

        // Generate chunks
        let chunk_stream = generate_chunks(total_range, config.chunk_size);

        // Process chunks in parallel, fail-fast on error
        chunk_stream
            .map(|range| {
                let pool = pool.clone();
                let task = task.clone();
                let in_progress = in_progress.clone();
                let counter = processed_counter.clone();

                async move {
                    let start_cp = *range.start();
                    let end_cp = *range.end();

                    // Mark this chunk as in-progress
                    in_progress.lock().await.insert(start_cp);

                    // Execute backfill for the range
                    if let Err(e) = task.backfill_range(pool, &range).await {
                        let min_range_restart_num = {
                            let mut guard = in_progress.lock().await;
                            guard.remove(&start_cp);
                            guard.iter().next().cloned().unwrap_or(start_cp)
                        };
                        error!("Chunk {start_cp}-{end_cp} failed. Minimum range restart number: {min_range_restart_num}. Error: {e}", );
                        return Err(e);
                    }

                    // Get the minimum range start
                    let min_range_start = {
                        let mut guard = in_progress.lock().await;
                        // Remove processed chunk from in-progress
                        guard.remove(&start_cp);
                        guard.iter().next().cloned()
                    };

                    // Update metrics
                    let count = end_cp - start_cp + 1;
                    let total = counter.fetch_add(count, Ordering::Relaxed) + count;
                    let elapsed = timer.elapsed().as_secs_f64();
                    let avg_rate = total as f64 / elapsed;
                    info!(
                        processed = total,
                        secs = elapsed,
                        rate = avg_rate,
                        min_range_start,
                        "Avg backfill speed"
                    );

                    Ok(())
                }
            })
            .buffer_unordered(config.max_concurrency)
            .try_for_each(|_| async { Ok(()) })
            .await?;

        let total = processed_counter.load(Ordering::Relaxed);
        let elapsed = timer.elapsed().as_secs_f64();
        let final_rate = total as f64 / elapsed;
        info!(
            total,
            secs = elapsed,
            rate = final_rate,
            "Completed backfill"
        );

        Ok(())
    }
}

/// Returns an asynchronous stream that yields consecutive, non-overlapping subranges ("chunks")
/// from the given inclusive range, each with a maximum length of `chunk_size`.
///
/// This is useful for processing a large range in smaller, manageable pieces, such as
/// batching database queries or parallelizing work.
///
/// # Example
///
/// ```rust
/// use futures::StreamExt;
///
/// let range = 0..=10;
/// let chunk_size = 3;
/// let mut stream = chunk_range_stream(range, chunk_size);
///
/// // This will yield: 0..=2, 3..=5, 6..=8, 9..=10
/// while let Some(chunk) = stream.next().await {
///     println!("{:?}", chunk);
/// }
/// ```
fn generate_chunks(
    total: RangeInclusive<usize>,
    chunk_size: usize,
) -> impl Stream<Item = RangeInclusive<usize>> {
    let end = *total.end();
    let start = *total.start();
    unfold(start, move |state| {
        let end = end;
        let size = chunk_size;
        async move {
            if state > end {
                None
            } else {
                let chunk_end = cmp::min(state + size - 1, end);
                let next = state + size;
                Some((state..=chunk_end, next))
            }
        }
    })
}
