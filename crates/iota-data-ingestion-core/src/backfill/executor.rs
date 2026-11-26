// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{ops::RangeInclusive, sync::Arc};

use backoff::backoff::Backoff;
use futures::{TryFutureExt, TryStreamExt, stream::BoxStream};
use iota_metrics::spawn_monitored_task;
use iota_types::{
    full_checkpoint_content::CheckpointData, messages_checkpoint::CheckpointSequenceNumber,
};
use tokio::{
    sync::mpsc,
    time::{sleep, timeout},
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::{
    IngestionError, IngestionResult, MAX_CHECKPOINTS_IN_PROGRESS, Reducer, Worker, WorkerPool,
    backfill::{
        options::BackfillOptions,
        order::ExpectedCheckpoints,
        source::{BackfillSource, backfill_source},
    },
    reader::v2::RemoteUrl,
    util::reset_backoff,
    worker_pool::WorkerPoolStatus,
};

const TASK_NAME: &str = "backfill";

/// Ingests a bounded checkpoint range from a store through a [`WorkerPool`].
///
/// The checkpoints reach the workers in the requested order. With several workers they
/// are processed in parallel and can finish out of order. A reducer, when one is set,
/// commits their messages strictly in the requested order. The executor keeps no
/// progress, the caller passes the range.
///
/// # Example
/// ```rust,no_run
/// use std::sync::Arc;
///
/// use async_trait::async_trait;
/// use iota_data_ingestion_core::{
///     BackfillExecutor, BackfillOptions, IngestionError, IngestionOrder, Worker,
///     reader::v2::RemoteUrl,
/// };
/// use iota_types::full_checkpoint_content::CheckpointData;
/// use tokio_util::sync::CancellationToken;
///
/// struct CustomWorker;
///
/// #[async_trait]
/// impl Worker for CustomWorker {
///     type Message = ();
///     type Error = IngestionError;
///
///     async fn process_checkpoint(
///         &self,
///         checkpoint: Arc<CheckpointData>,
///     ) -> Result<Self::Message, Self::Error> {
///         println!(
///             "processing checkpoint {}",
///             checkpoint.checkpoint_summary.sequence_number
///         );
///         Ok(())
///     }
/// }
///
/// #[tokio::main]
/// async fn main() {
///     let remote_url = RemoteUrl::HybridHistoricalStore {
///         historical_url: "https://checkpoints.mainnet.iota.cafe/ingestion/historical".into(),
///         live_url: None,
///     };
///     // ingest from the highest checkpoint of the range down to the lowest.
///     let options = BackfillOptions::default().with_order(IngestionOrder::Descending);
///     let executor =
///         BackfillExecutor::new(remote_url, CustomWorker, options, CancellationToken::new())
///             .unwrap();
///     // run the backfill for the checkpoints 0 to 1000.
///     let last_processed = executor.run(0..=1000).await.unwrap();
///     println!("last processed checkpoint: {last_processed:?}");
/// }
/// ```
pub struct BackfillExecutor<W: Worker> {
    source: Box<dyn BackfillSource>,
    pool: WorkerPool<W>,
    options: BackfillOptions,
    token: CancellationToken,
}

impl<W: Worker + 'static> BackfillExecutor<W> {
    /// Creates a new `BackfillExecutor` without a reducer.
    ///
    /// # Errors
    ///
    /// Fails if the store at `remote_url` cannot be set up or is not supported for a backfill.
    pub fn new(
        remote_url: RemoteUrl,
        worker: W,
        options: BackfillOptions,
        token: CancellationToken,
    ) -> IngestionResult<Self> {
        let source = backfill_source(remote_url, options.download_concurrency)?;
        Ok(Self::with_source(source, worker, options, token))
    }

    /// Creates a new `BackfillExecutor` with a reducer. The reducer commits the messages in batches
    /// of the workers in the requested order.
    ///
    /// # Errors
    ///
    /// Fails if the store at `remote_url` cannot be set up or is not supported for a backfill.
    pub fn new_with_reducer<R>(
        remote_url: RemoteUrl,
        worker: W,
        options: BackfillOptions,
        token: CancellationToken,
        reducer: R,
    ) -> IngestionResult<Self>
    where
        R: Reducer<W> + 'static,
    {
        let source = backfill_source(remote_url, options.download_concurrency)?;
        Ok(Self::with_source_and_reducer(
            source, worker, options, token, reducer,
        ))
    }

    /// Creates a `BackfillExecutor` without a reducer that reads from `source`.
    pub(crate) fn with_source(
        source: Box<dyn BackfillSource>,
        worker: W,
        options: BackfillOptions,
        token: CancellationToken,
    ) -> Self {
        let pool = WorkerPool::new(
            worker,
            TASK_NAME.into(),
            options.worker_concurrency,
            options.backoff.clone(),
        );
        Self {
            source,
            pool,
            options,
            token,
        }
    }

    /// Creates a `BackfillExecutor` with a reducer that reads from `source`.
    pub(crate) fn with_source_and_reducer<R>(
        source: Box<dyn BackfillSource>,
        worker: W,
        options: BackfillOptions,
        token: CancellationToken,
        reducer: R,
    ) -> Self
    where
        R: Reducer<W> + 'static,
    {
        let pool = WorkerPool::new_with_reducer(
            worker,
            TASK_NAME.into(),
            options.worker_concurrency,
            options.backoff.clone(),
            reducer,
        );
        Self {
            source,
            pool,
            options,
            token,
        }
    }

    /// Ingests every checkpoint of `range` and returns the last processed one. Returns `None`
    /// for an empty range. A run that the caller cancels also returns the last checkpoint it
    /// processed, or `None` if it processed none.
    ///
    /// # Errors
    ///
    /// Fails if the store cannot serve the range, or if the run stops before the range is done
    /// because the retries were exhausted.
    pub async fn run(
        self,
        range: RangeInclusive<CheckpointSequenceNumber>,
    ) -> IngestionResult<Option<CheckpointSequenceNumber>> {
        if range.is_empty() {
            return Ok(None);
        }
        let Self {
            source,
            pool,
            options,
            token,
        } = self;

        let order = options.order;
        let Some(checkpoints) = token
            .run_until_cancelled(source.checkpoints(range.clone(), order))
            .await
        else {
            info!("backfill of {range:?} was cancelled before any checkpoint was processed");
            return Ok(None);
        };
        let mut checkpoints = checkpoints?;
        let (first, last) = order.first_and_last(&range);
        let total_checkpoints_to_ingest = first.abs_diff(last) + 1;

        // A failed retry cancels its token. A child token leaves the caller's token alone.
        let caller_token = token;
        let token = caller_token.child_token();
        let (checkpoint_sender, checkpoint_receiver) = mpsc::channel(MAX_CHECKPOINTS_IN_PROGRESS);
        let (status_sender, mut status_receiver) = mpsc::channel(MAX_CHECKPOINTS_IN_PROGRESS);
        // The pool reports how many checkpoints of the range are done, so it starts at 0.
        let pool = pool.with_backfill_starting_checkpoint(first).run(
            0,
            checkpoint_receiver,
            status_sender,
            token.clone(),
        );
        let pool_handle = spawn_monitored_task!(pool)
            .inspect_err(|err| error!("backfill worker pool panicked: {err}"));

        let mut expected = ExpectedCheckpoints::new(range.clone(), order);
        let mut sent = 0;
        let mut processed = 0;
        let mut relay_error = None;
        loop {
            tokio::select! {
                status = status_receiver.recv() => {
                    let Some(status) = status else { break };
                    if let WorkerPoolStatus::Running((_, reported)) = status {
                        processed = reported;
                        if processed >= total_checkpoints_to_ingest {
                            token.cancel();
                        }
                    }
                }
                checkpoint = Self::next_checkpoint_with_retry(&options, &mut checkpoints),
                    if !token.is_cancelled()
                        && !expected.is_finished()
                        && sent - processed < MAX_CHECKPOINTS_IN_PROGRESS as u64 =>
                {
                    let relayed = Self::relay_checkpoint_to_worker_pool(
                        checkpoint,
                        &mut expected,
                        &checkpoint_sender,
                    );
                    match relayed.await {
                        Ok(()) => sent += 1,
                        Err(err) => {
                            relay_error = Some(err);
                            // start graceful shutdown since the relay got an irrecoverable error.
                            token.cancel();
                        }
                    }
                }
            }
        }
        // wait the worker pool task to graceful shutdown.
        let _ = pool_handle.await;
        if let Some(err) = relay_error {
            return Err(err);
        }
        // The first checkpoint that is not processed. There is none when the range is done.
        let Some(next) = order.nth(&range, processed) else {
            return Ok(Some(last));
        };
        if caller_token.is_cancelled() {
            info!("backfill of {range:?} was cancelled before checkpoint {next} was processed");
            return Ok(processed
                .checked_sub(1)
                .and_then(|index| order.nth(&range, index)));
        }
        // No relay error and no cancel, so a worker or the reducer exhausted its retries.
        Err(IngestionError::Shutdown {
            component: "worker pool".into(),
            msg: format!("stopped before checkpoint {next} of {range:?} was processed"),
        })
    }

    /// Checks the next checkpoint of the store and sends it to the pool.
    async fn relay_checkpoint_to_worker_pool(
        checkpoint: IngestionResult<Option<CheckpointData>>,
        expected: &mut ExpectedCheckpoints,
        checkpoint_sender: &mpsc::Sender<Arc<CheckpointData>>,
    ) -> IngestionResult<()> {
        let Some(checkpoint) = checkpoint? else {
            return Err(IngestionError::Verification(format!(
                "the store did not return checkpoints {:?}",
                expected.remaining_checkpoints()
            )));
        };
        expected.verify_next(checkpoint.checkpoint_summary.sequence_number)?;
        let _ = checkpoint_sender.send(Arc::new(checkpoint)).await;
        Ok(())
    }

    /// Returns the next checkpoint of the store, or `None` when the stream ended.
    async fn next_checkpoint_with_retry(
        options: &BackfillOptions,
        checkpoints: &mut BoxStream<'_, IngestionResult<CheckpointData>>,
    ) -> IngestionResult<Option<CheckpointData>> {
        let mut backoff = reset_backoff(&options.backoff);
        loop {
            let Ok(next_checkpoint) = timeout(options.timeout, checkpoints.try_next()).await else {
                // The store was too slow, so wait and ask again while the backoff allows.
                let Some(wait) = backoff.next_backoff() else {
                    return Err(IngestionError::CheckpointNotAvailableYet);
                };
                sleep(wait).await;
                continue;
            };
            return next_checkpoint;
        }
    }
}
