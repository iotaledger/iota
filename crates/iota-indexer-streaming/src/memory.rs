// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Contains the implementation of in memory streaming of
//! transactions and events to subscribers.
//!
//! It leverages PostgreSQL NOTIFY channel for receiving committed checkpoints
//! notifications on which it fetches transactions by sequence number ranges,
//! extracts events from them, and forwards all data to subscribers through
//! [`tokio::sync::broadcast`]. Supports backfill of historical data from
//! indexer database enabling stream recovery after a disconnection.

use std::{
    collections::VecDeque,
    fmt::Debug,
    future::Future,
    num::{NonZeroI64, NonZeroUsize},
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
    task::{Context, Poll},
    time::{Duration, Instant},
};

use backoff::ExponentialBackoff;
use diesel::{
    PgConnection, QueryResult, RunQueryDsl,
    pg::PgNotification,
    r2d2::{ConnectionManager, PooledConnection},
};
use futures::{Stream, StreamExt, TryFutureExt, future, stream};
use iota_indexer::{
    models::{
        events::StoredEvent,
        transactions::{StoredTransaction, stored_events_to_events},
    },
    read::IndexerReader,
};
use iota_sdk_types::Event;
use iota_types::digests::TransactionDigest;
use prometheus::{Histogram, IntGauge};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio_stream::wrappers::{BroadcastStream, errors::BroadcastStreamRecvError};
use tracing::{Instrument, debug, error};

use crate::{
    error::{IndexerStreamingError, IndexerStreamingResult},
    metrics::{InMemoryStreamMetrics, METRICS_EVENT_LABEL, METRICS_TRANSACTION_LABEL},
};

pub type PoolConnection = PooledConnection<ConnectionManager<PgConnection>>;

/// Postgres NOTIFY channel name.
const CHANNEL_NAME: &str = "checkpoint_committed";

/// Delay in seconds before retrying to connect to the Postgres database in case
/// of failure.
const RETRY_POSTGRES_CONNECTION_DELAY: Duration = Duration::from_secs(5);
/// Interval between polling for new notifications from the Postgres NOTIFY
/// channel when client processed them all.
const PG_NOTIFICATION_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Notification received from PostgreSQL NOTIFY channel when a checkpoint is
/// committed.
///
/// It implies that the [`iota_indexer`] has applied the migrations which
/// enables the Postgres database to send notification through the channel.
///
/// The [`CHANNEL_NAME`] should reflect the same name used in the migrations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
struct CheckpointCommitNotification {
    /// The sequence number of the committed checkpoint.
    checkpoint_sequence_number: i64,
    /// The minimum transaction sequence number in this checkpoint.
    min_tx_sequence_number: i64,
    /// The maximum transaction sequence number in this checkpoint.
    max_tx_sequence_number: i64,
}

/// Represents the possible configuration of the [`InMemory`] streaming of
/// transactions and events data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
pub struct Config {
    /// The buffer size of the [`tokio::sync::broadcast`] channel used for
    /// broadcasting transactions and events data to subscribers.
    ///
    /// - default: 1000
    pub channel_buffer_size: NonZeroUsize,
    /// The maximum number of checkpoint notifications to batch together for
    /// processing.
    ///
    /// This controls how many PostgreSQL NOTIFY messages are collected before
    /// resolving transaction bounds and fetching data from the database. Each
    /// notification represents a committed checkpoint containing one or more
    /// transactions.
    ///
    /// **Performance Trade-offs:**
    /// - **Higher values**: Reduce database query frequency but increase
    ///   latency and memory usage per batch
    /// - **Lower values**: Increase responsiveness but may cause more frequent
    ///   database queries for small checkpoints
    ///
    /// The value of 10 provides a good balance between throughput and latency
    /// for typical checkpoint sizes.
    pub notification_chunk_size: NonZeroUsize,
    /// The maximum number of transactions to send to subscribers in a single
    /// batch.
    ///
    /// This controls how many transactions are processed and broadcast together
    /// when streaming data to subscribers. Large checkpoints (e.g., genesis
    /// with thousands of transactions) are automatically split into
    /// multiple batches of this size to maintain consistent performance.
    ///
    /// **Performance Trade-offs:**
    /// - **Too small**: May fall behind the indexer commit rate, causing the
    ///   streaming service to lag behind real-time data ingestion
    /// - **Too large**: May overwhelm subscribers with large batches, causing
    ///   them to lag or drop messages due to slow processing
    ///
    /// The value of 50 provides good balance between indexer synchronization
    /// and subscriber responsiveness for typical workloads.
    pub transaction_batch_size: NonZeroI64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            channel_buffer_size: NonZeroUsize::new(1000).expect("value should be greater than 0"),
            transaction_batch_size: NonZeroI64::new(50).expect("value should be greater than 0"),
            notification_chunk_size: NonZeroUsize::new(10).expect("value should be greater than 0"),
        }
    }
}

/// Where to start a recovery stream relative to a known transaction digest.
///
/// - [`Inclusive`](Self::Inclusive): yield the identified transaction, then
///   everything after it.
/// - [`Exclusive`](Self::Exclusive): skip the identified transaction; start
///   from the next one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
pub enum RecoveryPoint {
    /// Include the transaction identified by the digest.
    Inclusive(TransactionDigest),
    /// Start after the transaction identified by the digest.
    Exclusive(TransactionDigest),
}

impl RecoveryPoint {
    /// Returns the contained transaction digest.
    fn digest(&self) -> TransactionDigest {
        match self {
            Self::Inclusive(d) | Self::Exclusive(d) => *d,
        }
    }

    /// Checks if the starting transaction should be included in the stream.
    fn is_inclusive(&self) -> bool {
        matches!(self, Self::Inclusive(_))
    }
}

/// Provides real-time streaming of transactions and events from the IOTA
/// Indexer by listening to PostgreSQL NOTIFY messages triggered when new
/// checkpoints are committed to the indexer database.
///
/// Also supports backfill of historical data from indexer database enabling
/// stream recovery after a disconnection.
///
/// The streamer consists of:
/// - A PostgreSQL connection listening for notifications after every committed
///   checkpoint.
/// - Internal broadcasters that fan-out data to multiple subscribers using a
///   [`tokio::sync::broadcast`] channels.
///
/// # Usage
///
/// ```rust,ignore
/// use iota_indexer_streaming::{memory::InMemory, metrics::InMemoryStreamMetrics};
///
/// // create a new streamer
/// let streamer = InMemory::new(
///     Default::default(),
///     indexer_reader,
///     InMemoryStreamMetrics::new(registry),
/// )
/// .await?;
///
/// // subscribe to all events
/// let events = streamer.subscribe_events(None);
/// tokio::spawn(async move {
///     use futures::StreamExt;
///     while let Some(event) = events.next().await {
///         println!("New event: {event:?}");
///     }
/// });
/// ```
pub struct InMemory {
    event_tx: broadcast::Sender<IndexerStreamingResult<StoredEvent>>,
    transaction_tx: broadcast::Sender<IndexerStreamingResult<StoredTransaction>>,
    reader: IndexerReader,
    metrics: Arc<InMemoryStreamMetrics>,
    config: Config,
}

impl InMemory {
    /// Creates a new `InMemory` instance.
    ///
    /// It performs the following steps:
    /// - establishes a connection to PostgreSQL.
    /// - sets up the notification listener.
    /// - spawns the background task that processes checkpoint notifications.
    /// - handles automatically reconnecting to PostgreSQL if the connection is
    ///   lost.
    pub async fn new(
        config: Config,
        indexer_reader: IndexerReader,
        metrics: impl Into<Arc<InMemoryStreamMetrics>>,
    ) -> IndexerStreamingResult<Self> {
        let metrics = metrics.into();

        let (event_tx, _) = broadcast::channel(config.channel_buffer_size.get());
        let (transaction_tx, _) = broadcast::channel(config.channel_buffer_size.get());

        // task responsible for establishing a permanent postgres connection for
        // listening to notifications and broadcasting events and transactions to
        // subscribers.
        tokio::spawn({
            let event_tx = event_tx.clone();
            let transaction_tx = transaction_tx.clone();
            let metrics = metrics.clone();
            let indexer_reader = indexer_reader.clone();
            let span = tracing::info_span!("live_broker");

            async move {
                loop {
                    let mut connection = match indexer_reader.get_pool().get() {
                        Ok(value) => value,
                        Err(e) => {
                            error!(
                                "failed to get connection from postgres connection pool with error: {e:?}"
                            );
                            Self::publish_error(
                                IndexerStreamingError::Postgres(e.to_string()),
                                &event_tx,
                                &transaction_tx,
                            );
                            tokio::time::sleep(RETRY_POSTGRES_CONNECTION_DELAY).await;
                            continue;
                        }
                    };

                    if let Err(e) =
                        diesel::sql_query(format!("LISTEN {CHANNEL_NAME}")).execute(&mut connection)
                    {
                        error!("failed listening to postgres notify channel: {e}");
                        Self::publish_error(e.into(), &event_tx, &transaction_tx);
                        continue;
                    }

                    if let Err(e) = Self::process_checkpoint_notifications(
                        &metrics,
                        &config,
                        &mut connection,
                        &indexer_reader,
                        &event_tx,
                        &transaction_tx,
                    )
                    .await
                    {
                        error!("processing checkpoint notifications failed: {e}");
                        Self::publish_error(e, &event_tx, &transaction_tx);
                    }
                }
            }.instrument(span)
        });

        Ok(Self {
            event_tx,
            transaction_tx,
            reader: indexer_reader,
            metrics,
            config,
        })
    }

    /// Subscribes to a stream of [`StoredEvent`].
    ///
    /// By default all events are received, the client shall handle the
    /// filtering.
    ///
    /// When `start_from` is `None`, subscribes to live events only. When
    /// `start_from` is `Some(recovery_point)`, the stream first backfills
    /// historical events from the transaction identified by the recovery
    /// point's digest up to the tip of the network, then seamlessly transitions
    /// to live events. This enables stream recovery after a disconnection.
    ///
    /// The recovery point variant controls whether events of the starting
    /// transaction are included:
    /// - [`RecoveryPoint::Inclusive`]: events of the starting transaction are
    ///   yielded.
    /// - [`RecoveryPoint::Exclusive`]: events of the starting transaction are
    ///   skipped; streaming begins from the next transaction onward. Useful for
    ///   reconnection, where the client already received that transaction's
    ///   events in the previous session.
    ///
    /// # Note
    /// Since under the hood a [`tokio::sync::broadcast`] channel is used for
    /// live events, the slow subscriber problem will be handled according to [documentation](https://docs.rs/tokio/latest/tokio/sync/broadcast/index.html#lagging)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use iota_indexer_streaming::{error::IndexerStreamingError, memory::RecoveryPoint};
    /// use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
    ///
    /// // live events only.
    /// let event_stream = streamer.subscribe_events(None);
    /// tokio::spawn(async move {
    ///     use futures::StreamExt;
    ///     while let Some(ev) = event_stream.next().await {
    ///         if let Ok(ev) = ev.inspect_err(
    ///             |IndexerStreamingError::Lagged(BroadcastStreamRecvError::Lagged(num))| {
    ///                 println!("Lagged by {num} events")
    ///             },
    ///         ) {
    ///             println!("Received event: {ev:?}");
    ///         }
    ///     }
    /// });
    /// ```
    /// # Example with a starting transaction digest
    ///
    /// ```rust,ignore
    /// use iota_indexer_streaming::{error::IndexerStreamingError, memory::RecoveryPoint};
    /// use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
    ///
    /// // recover events from a specific transaction digest onwards.
    /// let event_stream = streamer.subscribe_events(Some(RecoveryPoint::Inclusive(tx_digest)));
    /// tokio::spawn(async move {
    ///     use futures::StreamExt;
    ///     while let Some(ev) = event_stream.next().await {
    ///         if let Ok(ev) = ev.inspect_err(
    ///             |IndexerStreamingError::Lagged(BroadcastStreamRecvError::Lagged(num))| {
    ///                 println!("Lagged by {num} events")
    ///             },
    ///         ) {
    ///             println!("Received event: {ev:?}");
    ///         }
    ///     }
    /// });
    /// ```
    pub fn subscribe_events(
        &self,
        start_from: Option<RecoveryPoint>,
    ) -> Pin<Box<dyn Stream<Item = IndexerStreamingResult<StoredEvent>> + Send>> {
        let stream = BroadcastStream::new(self.event_tx.subscribe());
        let live_stream = SubscriberStream::new(
            stream,
            METRICS_EVENT_LABEL,
            self.metrics.clone(),
            start_from.is_some(),
        )
        .map(Self::flatten_error);
        let Some(recovery_point) = start_from else {
            return Box::pin(live_stream);
        };

        let historical = HistoricalFetch::new(
            METRICS_EVENT_LABEL,
            recovery_point.digest(),
            self.reader.clone(),
            self.config.transaction_batch_size.get(),
            self.metrics.clone(),
        );

        let cursor = historical.cursor_handle();

        let historical_events = historical
            .into_stream()
            .skip_while({
                let mut include_starting_tx = recovery_point.is_inclusive();
                move |result| {
                    future::ready(match result {
                        Ok(_) if !include_starting_tx => {
                            include_starting_tx = true;
                            true
                        }
                        _ => false,
                    })
                }
            })
            // we use Either represented by left/right stream methods to unify the two stream types
            // returned by the closure: stream::iter (multiple events) and stream::once (single
            // error). This avoids an extra heap allocation from Box::pin.
            .map(|result| match result {
                Ok(tx) => match Self::stored_events_from_transaction(&tx) {
                    Ok(events) => stream::iter(events.into_iter().map(Ok)).left_stream(),
                    Err(e) => stream::once(future::err(e)).right_stream(),
                },
                Err(e) => stream::once(future::err(e)).right_stream(),
            })
            .flatten();

        Box::pin(
            historical_events
                // when switching from historical to live, the broadcast buffer may
                // contain transactions already delivered by the historical backfill.
                // Filter them out using the cursor to prevent duplicates.
                .chain({
                    let mut initial_lagged_skipped = false;
                    live_stream.skip_while(move |result| {
                        future::ready(match result {
                            Ok(tx) => tx.tx_sequence_number < cursor.load(Ordering::Relaxed),
                            Err(IndexerStreamingError::Lagged(_)) if !initial_lagged_skipped => {
                                initial_lagged_skipped = true;
                                true // skip this one
                            }
                            // surface any other error or second Lagged
                            Err(_) => false,
                        })
                    })
                }),
        )
    }

    /// Subscribe to a stream of [`StoredTransaction`].
    ///
    /// By default all transactions are received, the client shall handle the
    /// filtering.
    ///
    /// When `start_from` is `None`, subscribes to live transactions only. When
    /// `start_from` is `Some(recovery_point)`, the stream first backfills
    /// historical transactions from the one identified by the recovery point's
    /// digest up to the tip of the network, then seamlessly transitions to
    /// live transactions. This enables stream recovery after a disconnection.
    ///
    /// The recovery point variant controls whether the starting transaction
    /// itself is included:
    /// - [`RecoveryPoint::Inclusive`]: the starting transaction is yielded.
    /// - [`RecoveryPoint::Exclusive`]: streaming begins from the transaction
    ///   immediately after the one identified by the digest. Useful for
    ///   reconnection, where the client already received that transaction in
    ///   the previous session.
    ///
    /// # Note
    /// Since under the hood a [`tokio::sync::broadcast`] channel is used for
    /// live transactions, the slow subscriber problem will be handled according to [documentation](https://docs.rs/tokio/latest/tokio/sync/broadcast/index.html#lagging)
    ///
    /// # Example
    /// ```rust,ignore
    /// use iota_indexer_streaming::error::IndexerStreamingError;
    /// use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
    ///
    /// // live transactions only.
    /// let tx_stream = streamer.subscribe_transactions(None);
    /// tokio::spawn(async move {
    ///     use futures::StreamExt;
    ///     while let Some(tx) = tx_stream.next().await {
    ///         if let Ok(tx) = tx.inspect_err(
    ///             |IndexerStreamingError::Lagged(BroadcastStreamRecvError::Lagged(num))| {
    ///                 println!("Lagged by {num} transactions")
    ///             },
    ///         ) {
    ///             println!("Received transaction: {tx:?}");
    ///         }
    ///     }
    /// });
    /// ```
    /// # Example with a starting transaction digest
    /// ```rust,ignore
    /// use iota_indexer_streaming::{error::IndexerStreamingError, memory::RecoveryPoint};
    /// use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
    ///
    /// // recover transactions from a specific transaction digest onwards.
    /// let tx_stream = streamer.subscribe_transactions(Some(RecoveryPoint::Inclusive(tx_digest)));
    /// tokio::spawn(async move {
    ///     use futures::StreamExt;
    ///     while let Some(tx) = tx_stream.next().await {
    ///         if let Ok(tx) = tx.inspect_err(
    ///             |IndexerStreamingError::Lagged(BroadcastStreamRecvError::Lagged(num))| {
    ///                 println!("Lagged by {num} transactions")
    ///             },
    ///         ) {
    ///             println!("Received transaction: {tx:?}");
    ///         }
    ///     }
    /// });
    /// ```
    pub fn subscribe_transactions(
        &self,
        start_from: Option<RecoveryPoint>,
    ) -> Pin<Box<dyn Stream<Item = IndexerStreamingResult<StoredTransaction>> + Send>> {
        let stream = BroadcastStream::new(self.transaction_tx.subscribe());
        let live_stream = SubscriberStream::new(
            stream,
            METRICS_TRANSACTION_LABEL,
            self.metrics.clone(),
            start_from.is_some(),
        )
        .map(Self::flatten_error);

        let Some(recovery_point) = start_from else {
            return Box::pin(live_stream);
        };

        let historical = HistoricalFetch::new(
            METRICS_TRANSACTION_LABEL,
            recovery_point.digest(),
            self.reader.clone(),
            self.config.transaction_batch_size.get(),
            self.metrics.clone(),
        );

        let cursor = historical.cursor_handle();

        Box::pin(
            historical
                .into_stream()
                .skip_while({
                    let mut include_starting_tx = recovery_point.is_inclusive();
                    move |result| {
                        future::ready(match result {
                            Ok(_) if !include_starting_tx => {
                                include_starting_tx = true;
                                true
                            }
                            _ => false,
                        })
                    }
                })
                // when switching from historical to live, the broadcast buffer may
                // contain transactions already delivered by the historical backfill.
                // Filter them out using the cursor to prevent duplicates.
                .chain({
                    let mut initial_lagged_skipped = false;
                    live_stream.skip_while(move |result| {
                        future::ready(match result {
                            Ok(tx) => tx.tx_sequence_number < cursor.load(Ordering::Relaxed),
                            Err(IndexerStreamingError::Lagged(_)) if !initial_lagged_skipped => {
                                initial_lagged_skipped = true;
                                true // skip this one
                            }
                            // surface any other error or second Lagged
                            Err(_) => false,
                        })
                    })
                }),
        )
    }

    /// Flattens nested `Result` types from the broadcast stream.
    fn flatten_error<T>(
        result: Result<IndexerStreamingResult<T>, BroadcastStreamRecvError>,
    ) -> IndexerStreamingResult<T> {
        match result {
            Ok(Ok(ev)) => Ok(ev),
            Ok(Err(e)) => Err(e),
            Err(err) => Err(err.into()),
        }
    }

    /// Listens for database notifications and processes them.
    ///
    /// - resolves from received notifications the transaction sequence number
    ///   bounds.
    /// - splits the transaction sequence number bounds into batches if
    ///   exceeded.
    /// - fetches the transactions within the batch bounds and sends them to
    ///   subscribers alongside extracted events.
    /// - leverages exponential backoff retries for transient errors when
    ///   processing notifications.
    async fn process_checkpoint_notifications(
        metrics: &InMemoryStreamMetrics,
        config: &Config,
        connection: &mut PoolConnection,
        indexer_reader: &IndexerReader,
        event_tx: &broadcast::Sender<IndexerStreamingResult<StoredEvent>>,
        transaction_tx: &broadcast::Sender<IndexerStreamingResult<StoredTransaction>>,
    ) -> IndexerStreamingResult<()> {
        let mut backoff = ExponentialBackoff::default();
        backoff.max_elapsed_time = Some(Duration::from_secs(5));
        backoff.initial_interval = Duration::from_millis(100);
        backoff.current_interval = backoff.initial_interval;
        backoff.multiplier = 1.0;

        loop {
            // Poll the PostgreSQL NOTIFY channel for new checkpoint commit
            // notifications. The iterator is non-blocking, it drains whatever
            // is currently buffered and returns None when empty. We re-poll
            // after a short sleep since new notifications can arrive at any time.
            let messages = connection
                .notifications_iter()
                .take(config.notification_chunk_size.get())
                .collect::<Vec<QueryResult<PgNotification>>>();

            if messages.is_empty() {
                tokio::time::sleep(PG_NOTIFICATION_POLL_INTERVAL).await;
                continue;
            }

            // auto-records duration on drop (after each iteration).
            let _record_processed_checkpoint_notifications =
                metrics.process_notification_batch_latency.start_timer();

            if let Some((min_tx_sequence_number, max_tx_sequence_number)) =
                Self::resolve_tx_bounds(metrics, &messages)?
            {
                metrics
                    .notified_tx_seq_num_start_range
                    .set(min_tx_sequence_number);
                metrics
                    .notified_tx_seq_num_end_range
                    .set(max_tx_sequence_number);

                let mut start = min_tx_sequence_number;

                while start <= max_tx_sequence_number {
                    let end = (start + config.transaction_batch_size.get().saturating_sub(1))
                        .min(max_tx_sequence_number);

                    if let Err(e) = backoff::future::retry(backoff.clone(), || {
                        Self::process_transaction_batch(
                            metrics,
                            start,
                            end,
                            indexer_reader,
                            event_tx,
                            transaction_tx,
                        )
                        .map_err(backoff::Error::transient)
                        .inspect_err(|e| {
                            error!("transient error processing transaction batch: {e}")
                        })
                    })
                    .await
                    {
                        // once we exhaust all backoff retries, we publish the error and move on to
                        // the next batch. The client can decide how to handle the error
                        // accordingly.
                        error!(
                            batch_start = start,
                            batch_end = end,
                            error = ?e,
                            "batch processing failed after retries, publishing error to clients"
                        );
                        Self::publish_error(e, event_tx, transaction_tx);
                    }

                    start = end + 1;
                }
            }
        }
    }

    /// Resolves the transaction sequence number bounds from the given messages
    /// batch.
    fn resolve_tx_bounds(
        metrics: &InMemoryStreamMetrics,
        messages: &[QueryResult<PgNotification>],
    ) -> IndexerStreamingResult<Option<(i64, i64)>> {
        let mut filtered_messages = Self::filter_checkpoint_notifications(metrics, messages);

        let first = filtered_messages.next().transpose()?;
        let last = filtered_messages.last().transpose()?;

        Ok(first.map(|f| {
            (
                f.min_tx_sequence_number,
                last.unwrap_or(f).max_tx_sequence_number,
            )
        }))
    }

    /// Fetches transactions from the database within the given range and
    /// publish them to subscribers alongside extracted events from every
    /// transaction.
    async fn process_transaction_batch(
        metrics: &InMemoryStreamMetrics,
        start: i64,
        end: i64,
        indexer_reader: &IndexerReader,
        event_tx: &broadcast::Sender<IndexerStreamingResult<StoredEvent>>,
        transaction_tx: &broadcast::Sender<IndexerStreamingResult<StoredTransaction>>,
    ) -> IndexerStreamingResult<()> {
        // auto-records duration on drop (function return).
        let _record_function_execution_latency =
            metrics.process_transaction_batch_latency.start_timer();
        let db_query_timer = metrics.query_tx_from_indexer_db_latency.start_timer();

        let transactions: Vec<StoredTransaction> = indexer_reader
            .spawn_blocking(move |this| {
                this.multi_get_transactions_by_sequence_numbers_range(start, end)
            })
            .await?;

        let elapsed = db_query_timer.stop_and_record();
        debug!(
            "transactions query took: {:?}, tx: {}",
            Duration::from_secs_f64(elapsed),
            transactions.len()
        );

        let publish_data_to_subscribers_timer = metrics
            .broadcast_tx_and_ev_to_subscribers_latency
            .start_timer();

        Self::publish_tx_and_events(metrics, transactions, event_tx, transaction_tx).await?;

        let elapsed = publish_data_to_subscribers_timer.stop_and_record();
        debug!(
            "broadcast data took: {:?}",
            Duration::from_secs_f64(elapsed)
        );

        metrics
            .broadcasted_to_subscribers_tx_seq_num_start_range
            .set(start);
        metrics
            .broadcasted_to_subscribers_tx_seq_num_end_range
            .set(end);
        Ok(())
    }

    /// Publishes transactions and extracted events from them to subscribers.
    async fn publish_tx_and_events(
        metrics: &InMemoryStreamMetrics,
        transactions: Vec<StoredTransaction>,
        event_tx: &broadcast::Sender<IndexerStreamingResult<StoredEvent>>,
        transaction_tx: &broadcast::Sender<IndexerStreamingResult<StoredTransaction>>,
    ) -> IndexerStreamingResult<()> {
        // we ignore errors here because we may receive an error if no subscribers are
        // registered which may happen.
        for tx in transactions {
            for event in Self::stored_events_from_transaction(&tx)? {
                _ = event_tx.send(Ok(event));
            }
            _ = transaction_tx.send(Ok(tx));
        }

        // we sacrifice per-event/transaction granularity to avoid degrading
        // performance from frequent metric updates in a hot path.
        metrics
            .channel_pending_messages
            .with_label_values(&[METRICS_EVENT_LABEL])
            .set(event_tx.len() as i64);

        metrics
            .channel_pending_messages
            .with_label_values(&[METRICS_TRANSACTION_LABEL])
            .set(transaction_tx.len() as i64);
        Ok(())
    }

    /// Relay an irrecoverable error to all subscribers.
    ///
    /// Providing transparency on what happen on the broker side, the client can
    /// decide how to handle the error accordingly.
    fn publish_error(
        error: IndexerStreamingError,
        event_tx: &broadcast::Sender<IndexerStreamingResult<StoredEvent>>,
        transaction_tx: &broadcast::Sender<IndexerStreamingResult<StoredTransaction>>,
    ) {
        _ = event_tx.send(Err(error.clone()));
        _ = transaction_tx.send(Err(error));
    }

    /// Filters and parses database notifications into
    /// [`CheckpointCommitNotification`] from PostgreSQL messages.
    fn filter_checkpoint_notifications<'a>(
        metrics: &'a InMemoryStreamMetrics,
        messages: &'a [QueryResult<PgNotification>],
    ) -> impl Iterator<Item = IndexerStreamingResult<CheckpointCommitNotification>> + 'a {
        messages.iter().filter_map(|msg_result| match msg_result {
            Ok(PgNotification { payload, .. }) => {
                match serde_json::from_str::<CheckpointCommitNotification>(payload) {
                    Ok(notification) => {
                        metrics
                            .notified_checkpoint_sequence_number
                            .set(notification.checkpoint_sequence_number);

                        Some(Ok(notification))
                    }
                    Err(_) => None,
                }
            }
            Err(e) => Some(Err(IndexerStreamingError::Postgres(format!(
                "database connection error: {e}"
            )))),
        })
    }

    /// Extract [`StoredEvent`]'s from [`StoredTransaction`].
    fn stored_events_from_transaction(
        tx: &StoredTransaction,
    ) -> IndexerStreamingResult<Vec<StoredEvent>> {
        let with_prefix = true;
        let native_events: Vec<Event> = stored_events_to_events(tx.events.clone())?;
        let stored = native_events
            .into_iter()
            .enumerate()
            .map(|(idx, native)| StoredEvent {
                tx_sequence_number: tx.tx_sequence_number,
                event_sequence_number: idx as i64,
                transaction_digest: tx.transaction_digest.clone(),
                senders: vec![Some(native.sender.as_bytes().to_vec())],
                package: native.package_id.as_bytes().to_vec(),
                module: native.module.to_string(),
                event_type: native.type_.to_canonical_string(with_prefix),
                timestamp_ms: tx.timestamp_ms,
                bcs: native.contents,
            })
            .collect();
        Ok(stored)
    }
}

/// A [`Stream`] wrapper that provides metrics capabilities for the
/// [`BroadcastStream`].
///
/// It counts internally the total numbers of subscribers by incrementing the
/// value every time the [`new`](Self::new) constructor is invoked and
/// decrementing it when the stream is dropped.
///
/// Also the provides a way to track the lagging status of
/// the subscriber.
struct SubscriberStream<T> {
    /// The inner stream implementation we want to wrap.
    inner: BroadcastStream<T>,
    /// Tracks if the subscriber is active.
    active_subscriber_number: IntGauge,
    /// Tracks if the subscriber is lagging.
    lagging_subscribers: IntGauge,
    /// Tracks if this subscriber is currently lagged.
    is_lagging: bool,
    /// Timer to track lag duration.
    lag_start: Option<Instant>,
}

impl<T> SubscriberStream<T> {
    /// Represents the duration of the lag state of the subscriber, mostly to
    /// help Prometheus scrape the metric.
    const LAG_STATE: Duration = Duration::from_secs(1);

    pub fn new(
        inner: BroadcastStream<T>,
        label: &'static str,
        metrics: Arc<InMemoryStreamMetrics>,
        is_historical_used_before: bool,
    ) -> Self {
        let active_subscriber_number = metrics.active_subscriber_number.with_label_values(&[label]);
        let lagging_subscribers = metrics.lagging_subscribers.with_label_values(&[label]);

        if !is_historical_used_before {
            active_subscriber_number.inc();
        }

        Self {
            inner,
            active_subscriber_number,
            lagging_subscribers,
            is_lagging: false,
            lag_start: None,
        }
    }

    /// Marks the subscriber as lagging.
    fn mark_as_lagging(&mut self) {
        if !self.is_lagging {
            self.is_lagging = true;
            self.lag_start = Some(Instant::now());
            self.lagging_subscribers.inc();
        }
    }

    /// Clears the lag flag if the subscriber has been lagging.
    ///
    /// It holds the lagging state for at least [`LAG_STATE`](Self::LAG_STATE)
    /// second in order for Prometheus to scrape the metric.
    fn clear_lagging(&mut self) {
        if self.is_lagging
            && self
                .lag_start
                .as_ref()
                .is_some_and(|instant| instant.elapsed() >= Self::LAG_STATE)
        {
            self.lagging_subscribers.dec();
            self.is_lagging = false;
            self.lag_start = None;
        }
    }
}

impl<T> Drop for SubscriberStream<T> {
    fn drop(&mut self) {
        self.active_subscriber_number.dec();
        if self.is_lagging {
            self.lagging_subscribers.dec();
        }
    }
}
impl<T: Clone + Send + 'static> Stream for SubscriberStream<T> {
    type Item = Result<T, BroadcastStreamRecvError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // check if we should clear the lag flag (independent of message arrival)
        self.clear_lagging();

        let poll = Pin::new(&mut self.inner).poll_next(cx);

        if let Poll::Ready(Some(Err(BroadcastStreamRecvError::Lagged(_)))) = poll {
            self.mark_as_lagging();
        }

        poll
    }
}

/// Fetches historical transactions from the indexer database starting from a
/// given transaction digest up to the latest available sequence number.
///
/// This stream is used for stream recovery, the caller chains this stream with
/// a live broadcast stream to provide gap-free delivery from a recovery point.
struct HistoricalFetch {
    /// Transaction digest to start fetching from.
    start_from_digest: TransactionDigest,
    /// Next transaction sequence number to fetch.
    cursor: Arc<AtomicI64>,
    /// Whether [`Self::cursor`] has been resolved from
    /// [`Self::start_from_digest`].
    ///
    /// Needed because `0` is a valid cursor value (the genesis transaction),
    /// so it cannot be used as a starting point for the unresolved state.
    cursor_resolved: bool,
    /// Latest committed transaction sequence number in the database.
    latest_tx_in_db: i64,
    /// Database reader for fetching transactions.
    reader: IndexerReader,
    /// Buffered transactions from the last database batch fetch.
    buffer: VecDeque<StoredTransaction>,
    /// Number of transactions to fetch per database query.
    batch_size: i64,
    /// Upon an unrecoverable error, the stream should be closed.
    should_close_stream: bool,
    /// Tracks the latency of querying transaction batch from the indexer
    /// database.
    query_tx_from_indexer_db_latency: Histogram,
}

impl HistoricalFetch {
    fn new(
        label: &'static str,
        start_from_digest: TransactionDigest,
        reader: IndexerReader,
        batch_size: i64,
        metrics: Arc<InMemoryStreamMetrics>,
    ) -> Self {
        let active_subscriber_number = metrics.active_subscriber_number.with_label_values(&[label]);
        active_subscriber_number.inc();

        Self {
            start_from_digest,
            cursor: Arc::new(AtomicI64::new(0)),
            cursor_resolved: false,
            latest_tx_in_db: 0,
            reader,
            buffer: VecDeque::new(),
            batch_size,
            should_close_stream: false,
            query_tx_from_indexer_db_latency: metrics.query_tx_from_indexer_db_latency.clone(),
        }
    }

    /// Returns the current value of the cursor.
    fn cursor_value(&self) -> i64 {
        self.cursor.load(Ordering::Relaxed)
    }

    /// Updates the cursor to the given value.
    fn update_cursor(&self, value: i64) {
        self.cursor.store(value, Ordering::Relaxed);
    }

    /// Returns a shared reference to the cursor.
    ///
    /// The cursor tracks the next expected transaction sequence number and
    /// is updated as the historical stream progresses. Used by the live
    /// stream filter to skip already-delivered transactions.
    fn cursor_handle(&self) -> Arc<AtomicI64> {
        self.cursor.clone()
    }

    /// Retries the provided closure using an [`ExponentialBackoff`]
    async fn with_retry<F, Fut, T, E>(f: F) -> Result<T, E>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T, backoff::Error<E>>>,
    {
        let backoff = ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_secs(5)),
            initial_interval: Duration::from_millis(100),
            multiplier: 1.0,
            ..Default::default()
        };

        backoff::future::retry(backoff, f).await
    }

    /// Converts into a stream that yields historical transactions from the
    /// database in batches. The stream ends once all transactions up to
    /// the latest committed transaction to database have been yielded, at which
    /// point the caller can chain a live stream.
    fn into_stream(self) -> impl Stream<Item = IndexerStreamingResult<StoredTransaction>> {
        stream::unfold(self, |mut state| {
            let start_from = state.start_from_digest;
            let span = tracing::info_span!("historical_backfill", %start_from);
            async move {
                loop {
                    if state.should_close_stream {
                        return None;
                    }

                    // drain the buffer before fetching new transactions from the database.
                    // This is the main point of sending transactions to the caller.
                    if let Some(tx) = state.buffer.pop_front() {
                        return Some((Ok(tx), state));
                    }

                    // resolve the cursor from the digest if it hasn't been set yet.
                    if !state.cursor_resolved {
                        match Self::with_retry(|| async {
                            state
                                .reader
                                .db()
                                .resolve_cursor_tx_digest_to_seq_num(state.start_from_digest)
                                .await
                                .map_err(backoff::Error::transient)
                        })
                        .await
                        {
                            Ok(cursor) => {
                                state.cursor_resolved = true;
                                state.update_cursor(cursor);
                            },
                            Err(e) => {
                                state.should_close_stream = true;
                                let e = IndexerStreamingError::NotFound(format!(
                                    "unable to resolve transaction, may not exist or has been pruned: {e}"
                                ));
                                error!("{e}");
                                return Some((Err(e), state));
                            }
                        }
                    }

                    // check if cursor is ahead of latest available transaction on database, if so,
                    // refresh latest, so the stream can continue.
                    if state.cursor_value() > state.latest_tx_in_db {
                        state.latest_tx_in_db = match Self::with_retry(|| async {
                            state
                                .reader
                                .db()
                                .latest_tx_sequence_number()
                                .await
                                .map_err(backoff::Error::transient)
                        })
                        .await
                        {
                            Ok(Some(latest)) => latest,
                            // this case is very unlikely, but we should handle it gracefully. This
                            // is mostly because when we resolve the provided tx digest to a
                            // sequence number (in the previous step), this implies that we already
                            // have data in transactions table.
                            Ok(None) => {
                                state.should_close_stream = true;
                                let e = IndexerStreamingError::Postgres(
                                    "unable to fetch latest tx sequence number".into(),
                                );
                                error!("{e}");
                                return Some((Err(e), state));
                            }
                            Err(e) => {
                                state.should_close_stream = true;
                                let e = IndexerStreamingError::Postgres(format!(
                                    "unable to fetch latest tx sequence number: {e}"
                                ));
                                error!("{e}");
                                return Some((Err(e), state));
                            }
                        };

                        debug!(
                            cursor = state.cursor_value(),
                            latest = state.latest_tx_in_db,
                            "current state"
                        );

                        // if latest did not advance, we're at the tip and can close the historical
                        // backfill stream and move to the live one.
                        if state.cursor_value() > state.latest_tx_in_db {
                            debug!("reached the tip and are in sync with live data");
                            return None;
                        }
                    }

                    // fetch transaction batch from database and update the cursor.
                    let start = state.cursor_value();
                    let end = (start + state.batch_size.saturating_sub(1)).min(state.latest_tx_in_db);

                    let db_query_timer = state.query_tx_from_indexer_db_latency.start_timer();
                    match Self::with_retry(|| async {
                        state
                            .reader
                            .spawn_blocking(move |this| {
                                this.multi_get_transactions_by_sequence_numbers_range(start, end)
                            })
                            .await
                            .map_err(backoff::Error::transient)
                    })
                    .await
                    {
                        Ok(batch) => {
                            let elapsed = db_query_timer.stop_and_record();
                            debug!(
                                "transactions query took: {:?}, tx: {}",
                                Duration::from_secs_f64(elapsed),
                                batch.len()
                            );
                            state.buffer.extend(batch);
                            state.update_cursor(end + 1);
                        }
                        Err(e) => {
                            state.should_close_stream = true;
                            let e = IndexerStreamingError::Postgres(e.to_string());
                            error!(
                                batch_start = start,
                                batch_end = end,
                                error = ?e,
                                "batch processing failed after retries, publishing error to clients"
                            );
                            return Some((Err(e), state));
                        }
                    }
                }
            }
            .instrument(span)
        })
    }
}
