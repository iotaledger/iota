// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Contains the implementation of in memory streaming of
//! transactions and events to subscribers.
//!
//! It leverages PostgreSQL NOTIFY channel for receiving committed checkpoints
//! notifications on which it fetches transactions by sequence number ranges,
//! extracts events from them, and forwards all data to subscribers through
//! [`tokio::sync::broadcast`].

use std::{
    fmt::Debug,
    num::{NonZeroI64, NonZeroUsize},
    pin::Pin,
    str::FromStr,
    sync::Arc,
    task::{Context, Poll},
    time::{Duration, Instant},
};

use futures::{Stream, StreamExt, TryFutureExt, stream};
use iota_indexer::{
    models::{
        events::StoredEvent,
        transactions::{StoredTransaction, stored_events_to_events},
    },
    read::IndexerReader,
};
use iota_types::event::Event;
use prometheus::IntGauge;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio_postgres::{
    AsyncMessage, Config as PostgresConfig, Connection, NoTls, Socket, tls::NoTlsStream,
};
use tokio_stream::wrappers::{BroadcastStream, errors::BroadcastStreamRecvError};
use tracing::{debug, error};

use crate::{
    error::{IndexerStreamingError, IndexerStreamingResult},
    metrics::{InMemoryStreamMetrics, METRICS_EVENT_LABEL, METRICS_TRANSACTION_LABEL},
};

/// Postgres NOTIFY channel name.
const CHANNEL_NAME: &str = "checkpoint_committed";

/// Delay in seconds before retrying to connect to the Postgres database in case
/// of failure.
const RETRY_POSTGRES_CONNECTION_DELAY: Duration = Duration::from_secs(5);

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

/// Provides real-time streaming of transactions and events from the IOTA
/// Indexer by listening to PostgreSQL NOTIFY messages triggered when new
/// checkpoints are committed to the indexer database.
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
/// use iota_indexer_streaming::memory::{InMemory, StreamTransactionFilter};
///
/// // create a new streamer
/// let streamer = InMemory::new(db_url, Default::default(), indexer_reader).await?;
///
/// // subscribe to all events
/// let events = streamer.subscribe_events().unwrap()
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
    metrics: Arc<InMemoryStreamMetrics>,
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
        db_url: &str,
        config: Config,
        indexer_reader: IndexerReader,
        metrics: impl Into<Arc<InMemoryStreamMetrics>>,
    ) -> IndexerStreamingResult<Self> {
        let metrics = metrics.into();

        let pg_config = PostgresConfig::from_str(db_url).map_err(|e| {
            IndexerStreamingError::Postgres(format!("failed to parse Postgresdb url: {e}"))
        })?;

        let (event_tx, _) = broadcast::channel(config.channel_buffer_size.get());
        let (transaction_tx, _) = broadcast::channel(config.channel_buffer_size.get());

        // task responsible for establishing a permanent postgres connection for
        // listening to notifications and broadcasting events and transactions to
        // subscribers.
        tokio::spawn({
            let event_tx = event_tx.clone();
            let transaction_tx = transaction_tx.clone();
            let metrics = metrics.clone();

            async move {
                loop {
                    let (client, connection) = match pg_config.connect(NoTls).await {
                        Ok(value) => value,
                        Err(e) => {
                            error!("unable to connect to postgres: {e}");
                            Self::publish_error(e.into(), &event_tx, &transaction_tx);
                            tokio::time::sleep(RETRY_POSTGRES_CONNECTION_DELAY).await;
                            continue;
                        }
                    };

                    // the client's queries require the connection to be actively polled to execute.
                    // process_checkpoint_notifications is a long-running future that polls the
                    // connection for notifications and should never resolve (unless a fatal error
                    // occurs).
                    let query_fut = async {
                        client
                            .query(&format!("LISTEN {CHANNEL_NAME};"), &[])
                            .await
                            .map_err(Into::into)
                    };

                    let process_notification_fut = Self::process_checkpoint_notifications(
                        &metrics,
                        &config,
                        connection,
                        &indexer_reader,
                        &event_tx,
                        &transaction_tx,
                    );

                    // by using try_join!, we poll both futures concurrently, which allows the
                    // client query to execute while the connection is being actively polled
                    // for incoming notifications.
                    if let Err(e) = futures::try_join!(query_fut, process_notification_fut) {
                        error!("processing checkpoint notifications failed: {e}");
                        Self::publish_error(e, &event_tx, &transaction_tx);
                    }
                }
            }
        });

        Ok(Self {
            event_tx,
            transaction_tx,
            metrics,
        })
    }

    /// Subscribes to a stream of [`StoredEvent`].
    ///
    /// By default all events are received, the client shall handle the
    /// filtering.
    ///
    /// # Note
    /// Since under the hood a [`tokio::sync::broadcast`] channel is used the
    /// slow subscriber problem will be handled according to [documentation](https://docs.rs/tokio/latest/tokio/sync/broadcast/index.html#lagging)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let event_stream = streamer.subscribe_events().unwrap();
    /// tokio::spawn(async move {
    ///    use futures::StreamExt;
    ///    while let Some(ev) = event_stream.next().await {
    ///        if let Ok(ev) = ev.inspect_err(|BroadcastStreamRecvError::Lagged(num)| {
    ///            println!("Lagged by {num} events")
    ///        }) {
    ///            println!("Received event: {ev:?}");
    ///        }
    ///    }
    /// });
    /// ```
    pub fn subscribe_events(&self) -> impl Stream<Item = IndexerStreamingResult<StoredEvent>> {
        let stream = BroadcastStream::new(self.event_tx.subscribe());
        SubscriberStream::new(stream, METRICS_EVENT_LABEL, self.metrics.clone())
            .map(Self::flatten_error)
    }

    /// Subscribe to a stream of [`StoredTransaction`].
    ///
    /// By default all transactions are received, the client shall handle the
    /// filtering.
    ///
    /// # Note
    /// Since under the hood a [`tokio::sync::broadcast`] channel is used the
    /// slow subscriber problem will be handled according to [documentation](https://docs.rs/tokio/latest/tokio/sync/broadcast/index.html#lagging)
    ///
    /// # Example
    /// ```rust,ignore
    /// let tx_stream = streamer.subscribe_transactions().unwrap();
    /// tokio::spawn(async move {
    ///    use futures::StreamExt;
    ///    while let Some(tx) = tx_stream.next().await {
    ///        if let Ok(tx) = tx.inspect_err(|BroadcastStreamRecvError::Lagged(num)| {
    ///            println!("Lagged by {num} transactions")
    ///        }) {
    ///            println!("Received transaction: {tx:?}");
    ///        }
    ///    }
    /// });
    /// ```
    pub fn subscribe_transactions(
        &self,
    ) -> impl Stream<Item = IndexerStreamingResult<StoredTransaction>> {
        let stream = BroadcastStream::new(self.transaction_tx.subscribe());
        SubscriberStream::new(stream, METRICS_TRANSACTION_LABEL, self.metrics.clone())
            .map(Self::flatten_error)
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
        mut connection: Connection<Socket, NoTlsStream>,
        indexer_reader: &IndexerReader,
        event_tx: &broadcast::Sender<IndexerStreamingResult<StoredEvent>>,
        transaction_tx: &broadcast::Sender<IndexerStreamingResult<StoredTransaction>>,
    ) -> IndexerStreamingResult<()> {
        let mut backoff = backoff::ExponentialBackoff::default();
        backoff.max_elapsed_time = Some(Duration::from_secs(5));
        backoff.initial_interval = Duration::from_millis(100);
        backoff.current_interval = backoff.initial_interval;
        backoff.multiplier = 1.0;

        // create a stream from the connection that forwards messages to the channel.
        let mut stream = stream::poll_fn(move |cx| connection.poll_message(cx))
            .ready_chunks(config.notification_chunk_size.get());

        while let Some(messages) = stream.next().await {
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
        Err(IndexerStreamingError::Postgres(
            "postgres notification stream closed, retrying establishing connection...".into(),
        ))
    }

    /// Resolves the transaction sequence number bounds from the given messages
    /// batch.
    fn resolve_tx_bounds(
        metrics: &InMemoryStreamMetrics,
        messages: &[Result<AsyncMessage, tokio_postgres::Error>],
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
        messages: &'a [Result<AsyncMessage, tokio_postgres::Error>],
    ) -> impl Iterator<Item = IndexerStreamingResult<CheckpointCommitNotification>> + 'a {
        messages.iter().filter_map(|msg_result| {
            match msg_result {
                Ok(AsyncMessage::Notification(n)) => {
                    match serde_json::from_str::<CheckpointCommitNotification>(n.payload()) {
                        Ok(notification) => {
                            metrics
                                .notified_checkpoint_sequence_number
                                .set(notification.checkpoint_sequence_number);

                            Some(Ok(notification))
                        }
                        Err(_) => None,
                    }
                }
                // not a notification message, skip
                Ok(AsyncMessage::Notice(msg)) => {
                    tracing::warn!("received a postgres notice: {msg}");
                    None
                }
                Ok(_) => None,
                Err(e) => Some(Err(IndexerStreamingError::Postgres(format!(
                    "database connection error: {e}"
                )))),
            }
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
                senders: vec![Some(native.sender.to_vec())],
                package: native.package_id.to_vec(),
                module: native.transaction_module.to_string(),
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
    ) -> Self {
        let active_subscriber_number = metrics.active_subscriber_number.with_label_values(&[label]);
        let lagging_subscribers = metrics.lagging_subscribers.with_label_values(&[label]);

        active_subscriber_number.inc();

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
