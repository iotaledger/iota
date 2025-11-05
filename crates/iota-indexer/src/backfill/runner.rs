// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    cmp,
    ops::RangeInclusive,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Instant,
};

use futures::{StreamExt, TryStreamExt, stream::unfold};
use tokio_stream::Stream;
use tracing::{error, info};

use crate::{
    backfill::{Backfill, BackfillKind, get_backfill},
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
        runner_kind: BackfillKind,
        pool: ConnectionPool,
        backfill_config: BackfillConfig,
        total_range: RangeInclusive<usize>,
    ) -> Result<(), IndexerError> {
        let backfill = get_backfill(runner_kind, *total_range.start()).await?;
        Self::run_impl(pool, backfill_config, total_range, backfill).await
    }

    async fn run_impl(
        pool: ConnectionPool,
        config: BackfillConfig,
        total_range: RangeInclusive<usize>,
        backfill: Arc<dyn Backfill>,
    ) -> Result<(), IndexerError> {
        let timer = Instant::now();
        let processed_counter = Arc::new(AtomicUsize::new(0));

        // Generate chunks
        let chunk_stream = chunk_range_stream(total_range, config.chunk_size);

        // Process chunks in parallel, fail-fast on error
        chunk_stream
            .map(|range| {
                let pool = pool.clone();
                let backfill = backfill.clone();
                let counter = processed_counter.clone();

                async move {
                    let start = *range.start();
                    let end = *range.end();

                    // Execute backfill for the range
                    if let Err(e) = backfill.backfill_range(pool, &range).await {
                        error!("chunk {start}-{end} failed. Error: {e}");
                        return Err(e);
                    }

                    // Update metrics
                    let count = end - start + 1;
                    let total = counter.fetch_add(count, Ordering::Relaxed) + count;
                    let elapsed = timer.elapsed().as_secs_f64();
                    let avg_rate = total as f64 / elapsed;
                    info!(
                        processed = total,
                        secs = elapsed,
                        rate = avg_rate,
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

/// Returns an asynchronous stream that yields consecutive, non-overlapping
/// subranges ("chunks") from the given inclusive range, each with a maximum
/// length of `chunk_size`.
///
/// This is useful for processing a large range in smaller, manageable pieces,
/// such as batching database queries or parallelizing work.
fn chunk_range_stream(
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

#[cfg(test)]
mod tests {
    use futures::{StreamExt, pin_mut};

    use super::*;

    #[tokio::test]
    async fn test_chunk_range_stream() {
        let range = 0..=10;
        let chunk_size = 3;
        let stream = chunk_range_stream(range, chunk_size);
        pin_mut!(stream);

        let mut results = vec![];
        while let Some(chunk) = stream.next().await {
            results.push(chunk);
        }

        assert_eq!(results, vec![0..=2, 3..=5, 6..=8, 9..=10]);
    }
}
