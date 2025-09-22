// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{str::FromStr, time::Instant};

use anyhow::Context;
use futures::{Stream, StreamExt, TryFutureExt, stream};
use iota_json_rpc_types::{Filter, IotaTransactionKind};
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    event::Event,
    transaction::TransactionDataAPI,
};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio_postgres::{AsyncMessage, Client, Config, Connection, NoTls, Socket, tls::NoTlsStream};
use tokio_stream::wrappers::{BroadcastStream, errors::BroadcastStreamRecvError};
use tracing::{debug, error};

use crate::{
    errors::IndexerError,
    indexer_reader::IndexerReader,
    models::{
        events::StoredEvent,
        transactions::{StoredTransaction, stored_events_to_events},
    },
};

/// Buffer size used in the mpsc bounded channels.
const BUFFER_SIZE: usize = 1000;
/// Postgres Notify channel name.
const CHANNEL_NAME: &str = "checkpoint_committed";

/// Filter returned [`StoredEvent`] form the stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamEventFilter {
    EmittingPackage(ObjectID),
    EmittingModule { package: ObjectID, module: String },
}

impl Filter<StoredEvent> for StreamEventFilter {
    fn matches(&self, event: &StoredEvent) -> bool {
        match self {
            StreamEventFilter::EmittingPackage(pkg_addr) => {
                event.package.as_slice() == pkg_addr.as_ref()
            }
            StreamEventFilter::EmittingModule { package, module } => {
                event.package.as_slice() == package.as_ref() && event.module == *module
            }
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Module {
    pub name: String,
    pub function: Option<String>,
}

/// Filter returned [`StoredTransaction`] form the stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamTransactionFilter {
    Kind(IotaTransactionKind),
    SigningAddress(IotaAddress),
    Function {
        package: ObjectID,
        module: Option<Module>,
    },
}

impl Filter<StoredTransaction> for StreamTransactionFilter {
    fn matches(&self, transaction: &StoredTransaction) -> bool {
        match self {
            StreamTransactionFilter::Kind(kind) => transaction.transaction_kind == *kind as i16,
            StreamTransactionFilter::SigningAddress(address) => transaction
                .try_into_sender_signed_data()
                .map(|data| data.transaction_data().sender() == *address)
                .unwrap_or_default(),
            StreamTransactionFilter::Function { package, module } => transaction
                .try_into_sender_signed_data()
                .map(|data| {
                    data.transaction_data()
                        .move_calls()
                        .iter()
                        .any(|(p, m, f)| match module {
                            Some(module) => {
                                let Some(ref function) = module.function else {
                                    return *p == package && *m == module.name;
                                };
                                *p == package && *m == module.name && *f == function
                            }
                            None => *p == package,
                        })
                })
                .unwrap_or_default(),
        }
    }
}

/// Represents a notification about a checkpoint commit in Indexer Database, it
/// acts as a trigger to request transaction related to that particular
/// checkpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct CheckpointCommitNotification {
    /// Committed checkpoint sequence number.
    checkpoint_sequence_number: i64,
    /// The minimum transaction sequence number.
    min_tx_sequence_number: i64,
    /// The maximum transaction sequence number.
    max_tx_sequence_number: i64,
}

/// Provides filtered, real-time streams of transactions
/// and events from the IOTA Indexer by listening to PostgreSQL NOTIFY
/// messages triggered when new checkpoints are committed to the indexer
/// database.
///
/// The streamer consists of:
/// - A PostgreSQL connection listening for `checkpoint_committed` notifications
/// - Internal broadcasters that fan-out data to multiple subscribers using a
///   [`tokio::sync::broadcast`] channels
///
/// # Usage
///
/// ```rust,ignore
/// use iota_indexer::stream::{IndexerStreamer, StreamTransactionFilter};
///
/// // Create a new streamer
/// let streamer = IndexerStreamer::new(db_url, indexer_reader).await?;
///
/// // Subscribe to all events
/// let events = streamer.subscribe_events().unwrap()
/// tokio::spawn(async move {
///     use futures::StreamExt;
///     while let Some(event) = events.next().await {
///         println!("New event: {:?}", event);
///     }
/// });
/// ```
pub struct IndexerStreamer {
    event_tx: broadcast::Sender<StoredEvent>,
    transaction_tx: broadcast::Sender<StoredTransaction>,
    // To receive notifications from the database we must keep the client alive.
    _client: Client,
}

impl IndexerStreamer {
    /// Creates a new `IndexerStreamer` instance.
    ///
    /// This method establishes a connection to PostgreSQL, sets up the
    /// notification listener, and spawns the background task that processes
    /// checkpoint notifications.
    pub async fn new(db_url: &str, indexer_reader: IndexerReader) -> Result<Self, IndexerError> {
        let (client, connection) = Config::from_str(db_url)
            .map_err(|e| IndexerError::Generic(format!("failed to parse Postgresdb url: {e}")))?
            .connect(NoTls)
            .await
            .context("Failed to connect to Postgres")?;

        let (event_tx, _) = broadcast::channel(BUFFER_SIZE);
        let (transaction_tx, _) = broadcast::channel(BUFFER_SIZE);

        // the database connection must be spawned into a separate task in order to
        // communicate with the database.
        tokio::spawn({
            Self::process_checkpoint_notifications(
                connection,
                indexer_reader,
                event_tx.clone(),
                transaction_tx.clone(),
            )
            .inspect_err(|e| error!("failed to process checkpoint notification: {e}"))
        });

        // listen for notifications on a specific channel.
        client
            .execute(&format!("LISTEN {CHANNEL_NAME};"), &[])
            .await
            .context("failed to listen to channel notifications")?;

        Ok(Self {
            event_tx,
            transaction_tx,
            _client: client,
        })
    }

    /// Subscribe to a stream of [`StoredEvent`].
    ///
    /// By default all events are received, it's possible to filter on client
    /// side by using [`StreamEventFilter`] type.
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
    ///     use futures::StreamExt;
    ///     while let Some(ev) = event_stream.next().await {
    ///         match ev {
    ///             Ok(ev) => {
    ///                 if !StreamEventFilter::matches(&ev) {
    ///                     continue;
    ///                 };
    ///                 println!("Received event: {ev:?}");
    ///             }
    ///             Err(BroadcastStreamRecvError::Lagged(num)) => {
    ///                 println!("Lagged by {num} events");
    ///             }
    ///         }
    ///     }
    /// });
    /// ```
    pub fn subscribe_events(
        &self,
    ) -> impl Stream<Item = Result<StoredEvent, BroadcastStreamRecvError>> {
        BroadcastStream::new(self.event_tx.subscribe())
    }

    /// Subscribe to a stream of [`StoredTransaction`].
    ///
    /// By default all events are received, it's possible to filter on client
    /// side by using [`StreamTransactionFilter`] type.
    ///
    /// # Note
    /// Since under the hood a [`tokio::sync::broadcast`] channel is used the
    /// slow subscriber problem will be handled according to [documentation](https://docs.rs/tokio/latest/tokio/sync/broadcast/index.html#lagging)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let filter = StreamTransactionFilter::Kind(IotaTransactionKind::ProgrammableTransaction);
    /// let tx_stream = streamer.subscribe_transactions().unwrap();
    /// tokio::spawn(async move {
    ///     use futures::StreamExt;
    ///     while let Some(tx) = tx_stream.next().await {
    ///         match tx {
    ///             Ok(tx) => {
    ///                 if !filter.matches(&tx) {
    ///                     continue;
    ///                 };
    ///                 println!("Transaction: {tx:?}");
    ///             }
    ///             Err(BroadcastStreamRecvError::Lagged(num)) => {
    ///                 println!("Lagged by {num} transactions");
    ///             }
    ///         }
    ///     }
    /// });
    pub fn subscribe_transactions(
        &self,
    ) -> impl Stream<Item = Result<StoredTransaction, BroadcastStreamRecvError>> {
        BroadcastStream::new(self.transaction_tx.subscribe())
    }

    async fn process_checkpoint_notifications(
        mut connection: Connection<Socket, NoTlsStream>,
        indexer_reader: IndexerReader,
        event_tx: broadcast::Sender<StoredEvent>,
        transaction_tx: broadcast::Sender<StoredTransaction>,
    ) -> Result<(), IndexerError> {
        // Batch checkpoints in smaller chunks to prevent subscriber overload.
        // Large batches (1000 checkpoints = ~4000-5000 transactions) would cause
        // immediate subscriber lag. Smaller batches (~10 checkpoints = ~40-50 tx) keep
        // pace.
        let buffer_size = BUFFER_SIZE / 100;
        // create a stream from the connection that forwards messages to the channel.
        let mut stream =
            stream::poll_fn(move |cx| connection.poll_message(cx)).ready_chunks(buffer_size);

        while let Some(messages) = stream.next().await {
            let mut filtered_messages = Self::filter_checkpoint_notifications(messages);

            let first = filtered_messages.next().transpose()?;
            let last = filtered_messages.last().transpose()?;

            if let Some((first, last)) = first.map(|f| (f, last.unwrap_or(f))) {
                debug!(notification = ?last);

                let instant = Instant::now();
                let transactions: Vec<StoredTransaction> = indexer_reader
                    .spawn_blocking(move |this| {
                        this.multi_get_transactions_by_sequence_numbers_range(
                            first.min_tx_sequence_number,
                            last.max_tx_sequence_number,
                        )
                    })
                    .await?;

                let duration = instant.elapsed();
                debug!(
                    "transactions query took: {duration:?}, tx: {}",
                    transactions.len()
                );

                let instant = Instant::now();
                Self::publish_tx_and_events(transactions, &event_tx, &transaction_tx).await?;
                let duration = instant.elapsed();
                debug!("broadcast data took: {duration:?}");
            }
        }
        Ok(())
    }

    async fn publish_tx_and_events(
        transactions: Vec<StoredTransaction>,
        event_tx: &broadcast::Sender<StoredEvent>,
        transaction_tx: &broadcast::Sender<StoredTransaction>,
    ) -> Result<(), IndexerError> {
        // we ignore errors here because we may receive an error if no subscribers are
        // registered which may happen.
        for tx in transactions {
            for event in Self::stored_events_from_transaction(&tx)? {
                _ = event_tx.send(event);
            }
            _ = transaction_tx.send(tx);
        }
        Ok(())
    }

    /// Filters and parses database notifications into
    /// [`CheckpointCommitNotification`] from PostgreSQL messages.
    fn filter_checkpoint_notifications(
        messages: Vec<Result<AsyncMessage, tokio_postgres::Error>>,
    ) -> impl Iterator<Item = Result<CheckpointCommitNotification, IndexerError>> {
        messages.into_iter().filter_map(|msg_result| {
            match msg_result {
                Ok(AsyncMessage::Notification(n)) => {
                    match serde_json::from_str::<CheckpointCommitNotification>(n.payload()) {
                        Ok(notification) => Some(Ok(notification)),
                        Err(_) => None,
                    }
                }
                Ok(_) => None, // Not a notification message, skip
                Err(e) => Some(Err(IndexerError::Generic(format!(
                    "database connection error: {e}"
                )))),
            }
        })
    }

    /// Extract [`StoredEvent`]'s from [`StoredTransaction`].
    fn stored_events_from_transaction(
        tx: &StoredTransaction,
    ) -> Result<Vec<StoredEvent>, IndexerError> {
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
                bcs: native.contents.clone(),
            })
            .collect();
        Ok(stored)
    }
}
