// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::BTreeMap,
    str::FromStr,
    sync::{Arc, Mutex},
    time::Instant,
};

use anyhow::Context;
use futures::{Stream, StreamExt, stream};
use iota_json_rpc_types::{Filter, IotaTransactionKind};
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    event::Event,
    transaction::TransactionDataAPI,
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{self};
use tokio_postgres::{AsyncMessage, Client, Config, Connection, NoTls, Socket, tls::NoTlsStream};
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, error, warn};

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

/// Active subscribers held in memory.
type Subscribers<T, F> = Arc<Mutex<BTreeMap<String, Subscriber<T, F>>>>;

/// A single subscriber to which we can send data based on provided filters.
struct Subscriber<T, F> {
    sender: mpsc::Sender<StreamEvent<T>>,
    filter: F,
}

/// Filter returned [`StoredEvent`] form the stream.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamEventFilter {
    #[default]
    All,
    EmittingPackage(ObjectID),
    EmittingModule {
        package: ObjectID,
        module: String,
    },
}

impl Filter<StoredEvent> for StreamEventFilter {
    fn matches(&self, event: &StoredEvent) -> bool {
        match self {
            StreamEventFilter::All => true,
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
    name: String,
    function: Option<String>,
}

/// Filter returned [`StoredTransaction`] form the stream.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamTransactionFilter {
    #[default]
    All,
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
            StreamTransactionFilter::All => true,
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
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct CheckpointCommitNotification {
    /// Committed checkpoint sequence number.
    checkpoint_sequence_number: i64,
    /// the minimum transaction sequence number.
    min_tx_sequence_number: i64,
    /// the maximum transaction sequence number.
    max_tx_sequence_number: i64,
}

/// Represents events that can be sent to stream subscribers in a broadcasting
/// system.
///
/// This enum encapsulates the different types of messages that subscribers can
/// receive when listening to a stream. It handles both normal message delivery
/// and backpressure scenarios where slow subscribers need to be managed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StreamEvent<T> {
    /// A regular message delivered to an active subscriber.
    ///
    /// This variant indicates normal operation where the subscriber is keeping
    /// up with the message flow and can continue receiving subsequent
    /// messages. The wrapped data represents the actual payload being
    /// streamed.
    Message(T),
    /// The final message sent to a subscriber that is lagging behind.
    ///
    /// This variant is sent when the server detects that a subscriber cannot
    /// process messages fast enough to keep up with the stream. After sending
    /// this message, the subscriber will be automatically removed from the
    /// broadcasting system to prevent memory buildup and maintain overall
    /// system performance.
    ///
    /// The wrapped data represents the last message the subscriber will
    /// receive, allowing for graceful cleanup or final processing before
    /// disconnection.
    ClosingDueToLag(T),
}

/// A lightweight in-memory fan-out broadcaster that distributes data to
/// multiple filtered subscribers.
///
/// It provides a publish-subscribe pattern where a single data producer can
/// efficiently distribute items to multiple consumers, each with their own
/// filtering criteria.
///
/// # Backpressure Handling
/// The "slow receiver" problem is handled through capacity-based monitoring:
///
/// - **Normal Operation**: When a subscriber's channel has capacity > 1,
///   messages are sent as [`StreamEvent::Message`] allowing continued
///   subscription
/// - **Backpressure Detection**: When capacity drops to 1 (channel nearly
///   full), the subscriber is marked as lagging behind the message production
///   rate
/// - **Graceful Termination**: Lagging subscribers receive their final message
///   wrapped in [`StreamEvent::ClosingDueToLag`] and are automatically removed
///   from the subscriber list
/// - **System Protection**: This prevents unbounded memory growth and maintains
///   overall broadcaster performance by avoiding blocking on slow consumers
///
/// # Filtering
///
/// Each subscriber can specify a filter that implements the `Filter<T>` trait.
/// Only items that match the subscriber's filter are sent to their channel.
/// Filtering happens after data retrieval but before channel transmission.
struct Broadcaster<T, F: Filter<T>> {
    subscribers: Subscribers<T, F>,
}

impl<T, F> Broadcaster<T, F>
where
    T: Clone + Send + 'static,
    F: Filter<T> + Send + Sync + 'static,
{
    /// Creates a new broadcaster that consumes items from the provided
    /// receiver.
    ///
    /// This method spawns a background task that continuously reads from
    /// [`mpsc::Receiver`] and distributes items to all registered
    /// subscribers. The broadcaster begins processing immediately upon
    /// creation.
    fn new(mut rx: mpsc::Receiver<T>) -> Self {
        let streamer = Self {
            subscribers: Default::default(),
        };

        let subscribers = streamer.subscribers.clone();

        tokio::spawn(async move {
            while let Some(data) = rx.recv().await {
                Self::send_to_all_subscribers(subscribers.clone(), data);
            }
        });
        streamer
    }

    fn send_to_all_subscribers(subscribers: Subscribers<T, F>, data: T) {
        let to_remove = {
            let mut to_remove = vec![];
            let mut subscribers_snapshot = subscribers.lock().unwrap_or_else(|poisoned| {
                error!("Subscribers mutex poisoned, recovering...");
                subscribers.clear_poison();
                poisoned.into_inner()
            });

            for (id, subscriber) in subscribers_snapshot.iter_mut() {
                if subscriber.sender.is_closed() {
                    warn!(
                        subscription_id = id,
                        "Subscriber closed connection, removing"
                    );
                    to_remove.push(id.clone());
                    continue;
                }

                if !(subscriber.filter.matches(&data)) {
                    continue;
                }

                let data = if subscriber.sender.capacity() > 1 {
                    StreamEvent::Message(data.clone())
                } else {
                    warn!(
                        subscription_id = id,
                        "Lagging behind, sending last message and removing"
                    );
                    to_remove.push(id.clone());
                    StreamEvent::ClosingDueToLag(data.clone())
                };

                // since we checked the capacity we can ignore the result
                let _ = subscriber.sender.try_send(data);
            }
            to_remove
        };
        if !to_remove.is_empty() {
            let mut subscribers = subscribers.lock().unwrap_or_else(|poisoned| {
                error!("Subscribers mutex poisoned, recovering...");
                subscribers.clear_poison();
                poisoned.into_inner()
            });
            for sub in to_remove {
                subscribers.remove(&sub);
            }
        }
    }

    /// Creates a new subscription to the broadcast stream with the specified
    /// filter.
    ///
    /// Each call to `subscribe` creates an independent stream that:
    /// - Receives only items that match the provided filter
    /// - Has its own bounded channel buffer ([`BUFFER_SIZE`] capacity)
    /// - Operates independently of other subscribers
    /// - Is automatically removed if it's disconnected
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use futures::StreamExt;
    ///
    /// // Attempt to subscribe
    /// let stream = broadcaster.subscribe(MyFilter::All).unwrap();
    /// tokio::spawn(async move {
    ///     while let Some(item) = stream.next().await {
    ///         process_item(item).await;
    ///     }
    /// });
    /// ```
    fn subscribe(&self, filter: F) -> impl Stream<Item = StreamEvent<T>> {
        let mut subscribers = self.subscribers.lock().unwrap_or_else(|poisoned| {
            error!("Subscribers mutex poisoned, recovering...");
            self.subscribers.clear_poison();
            poisoned.into_inner()
        });
        let (tx, rx) = mpsc::channel::<StreamEvent<T>>(BUFFER_SIZE);
        subscribers.insert(
            ObjectID::random().to_string(),
            Subscriber { sender: tx, filter },
        );
        ReceiverStream::new(rx)
    }
}

/// Provides filtered, real-time streams of transactions
/// and events from the IOTA Indexer by listening to PostgreSQL NOTIFY
/// messages triggered when new checkpoints are committed to the indexer
/// database.
///
/// The streamer consists of:
/// - A PostgreSQL connection listening for `checkpoint_committed` notifications
/// - Internal broadcasters that fan-out data to multiple subscribers
/// - Configurable filters for events and transactions
/// - Automatic subscriber management with backpressure handling
///
/// # Usage
///
/// ```rust,ignore
/// use iota_indexer::stream::{IndexerStreamer, StreamEventFilter, StreamTransactionFilter};
///
/// // Create a new streamer
/// let streamer = IndexerStreamer::new(db_url, indexer_reader).await?;
///
/// // Subscribe to all events
/// let events = streamer.subscribe_events(StreamEventFilter::All).unwrap()
/// tokio::spawn(async move {
///     use futures::StreamExt;
///     while let Some(event) = events.next().await {
///         println!("New event: {:?}", event);
///     }
/// });
/// ```
///
/// # Backpressure Handling
/// The "slow receiver" problem is handled through capacity-based monitoring:
///
/// - **Normal Operation**: When a subscriber's channel has capacity > 1,
///   messages are sent as [`StreamEvent::Message`] allowing continued
///   subscription
/// - **Backpressure Detection**: When capacity drops to 1 (channel nearly
///   full), the subscriber is marked as lagging behind the message production
///   rate
/// - **Graceful Termination**: Lagging subscribers receive their final message
///   wrapped in [`StreamEvent::ClosingDueToLag`] and are automatically removed
///   from the subscriber list
/// - **System Protection**: This prevents unbounded memory growth and maintains
///   overall broadcaster performance by avoiding blocking on slow consumers
pub struct IndexerStreamer {
    events_broadcaster: Broadcaster<StoredEvent, StreamEventFilter>,
    transactions_broadcaster: Broadcaster<StoredTransaction, StreamTransactionFilter>,
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

        let (event_tx, event_rx) = mpsc::channel(BUFFER_SIZE);
        let (transaction_tx, transaction_rx) = mpsc::channel(BUFFER_SIZE);

        tokio::spawn(Self::process_checkpoint_notifications(
            client,
            connection,
            indexer_reader,
            event_tx,
            transaction_tx,
        ));

        Ok(Self {
            events_broadcaster: Broadcaster::new(event_rx),
            transactions_broadcaster: Broadcaster::new(transaction_rx),
        })
    }

    /// Subscribe to a filtered stream of blockchain events.
    ///
    /// Creates a new subscription that will receive events matching the
    /// provided filter. Each subscriber gets its own independent stream and
    /// won't be affected by the processing speed of other subscribers.
    ///
    /// # Note
    /// If the subscriber cannot keep up with the event production rate (slow
    /// subscriber problem), it will automatically receive a final
    /// [`StreamEvent::ClosingDueToLag`] message and be disconnected to prevent
    /// system-wide performance degradation. Subscribers can resubscribe at any
    /// time to resume receiving events, but **any messages that occurred during
    /// the disconnection period will be lost** - the new subscription will only
    /// receive events from the point of resubscription forward.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let events = streamer.subscribe_events(StreamEventFilter::All).unwrap();
    /// tokio::spawn(async move {
    ///     use futures::StreamExt;
    ///     while let Some(stream_event) = events.next().await {
    ///         match stream_event {
    ///             StreamEvent::Message(ev) => {
    ///                 println!("Transaction: {ev:?}");
    ///             }
    ///             StreamEvent::ClosingDueToLag(last_ev) => {
    ///                 println!("Last Transaction: {last_ev:?}");
    ///             }
    ///         }
    ///     }
    /// });
    /// ```
    pub fn subscribe_events(
        &self,
        filter: StreamEventFilter,
    ) -> impl Stream<Item = StreamEvent<StoredEvent>> {
        self.events_broadcaster.subscribe(filter)
    }

    /// Subscribe to a filtered stream of blockchain transactions.
    ///
    /// Creates a new subscription that will receive transactions matching the
    /// provided filter. Each subscriber gets its own independent stream and
    /// won't be affected by the processing speed of other subscribers.
    ///
    /// # Note
    /// If the subscriber cannot keep up with the event production rate (slow
    /// subscriber problem), it will automatically receive a final
    /// [`StreamEvent::ClosingDueToLag`] message and be disconnected to prevent
    /// system-wide performance degradation. Subscribers can resubscribe at any
    /// time to resume receiving events, but **any messages that occurred during
    /// the disconnection period will be lost** - the new subscription will only
    /// receive events from the point of resubscription forward.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let filter = StreamTransactionFilter::Kind(IotaTransactionKind::ProgrammableTransaction);
    /// let txs = streamer.subscribe_transactions(filter).unwrap();
    /// tokio::spawn(async move {
    ///     use futures::StreamExt;
    ///     while let Some(stream_event) = txs.next().await {
    ///         match stream_event {
    ///             StreamEvent::Message(tx) => {
    ///                 println!("Transaction: {tx:?}");
    ///             }
    ///             StreamEvent::ClosingDueToLag(last_tx) => {
    ///                 println!("Last Transaction: {last_tx:?}");
    ///             }
    ///         }
    ///     }
    /// });
    /// ```
    pub fn subscribe_transactions(
        &self,
        filter: StreamTransactionFilter,
    ) -> impl Stream<Item = StreamEvent<StoredTransaction>> {
        self.transactions_broadcaster.subscribe(filter)
    }

    async fn process_checkpoint_notifications(
        client: Client,
        mut connection: Connection<Socket, NoTlsStream>,
        indexer_reader: IndexerReader,
        event_tx: mpsc::Sender<StoredEvent>,
        transaction_tx: mpsc::Sender<StoredTransaction>,
    ) -> Result<(), IndexerError> {
        // Create a channel for receiving messages
        let (tx, rx) = mpsc::channel(BUFFER_SIZE);

        // Create a stream from the connection that forwards messages to the channel
        let stream = stream::poll_fn(move |cx| connection.poll_message(cx));
        let connection_task = async move {
            let mut stream = stream;
            while let Some(message) = stream.next().await {
                match message {
                    Ok(msg) => {
                        if tx.send(msg).await.is_err() {
                            error!("notification receiver dropped");
                            break;
                        }
                    }
                    Err(e) => {
                        error!("connection error: {e}");
                        break;
                    }
                }
            }
        };

        // Spawn the connection handler
        tokio::spawn(connection_task);

        // Listen for notifications on a specific channel
        client
            .execute(&format!("LISTEN {CHANNEL_NAME};"), &[])
            .await
            .context("failed to listen to channel notifications")?;

        let mut notification_stream = ReceiverStream::new(rx).ready_chunks(BUFFER_SIZE);

        while let Some(messages) = notification_stream.next().await {
            if let Some((first, last)) = Self::first_and_last_checkpoint_notifications(&messages) {
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
                Self::publish_tx_and_events(transactions, &event_tx, &transaction_tx)?;
                let duration = instant.elapsed();
                debug!("broadcast data took: {duration:?}");
            }
        }

        Ok(())
    }

    fn publish_tx_and_events(
        transactions: Vec<StoredTransaction>,
        event_tx: &mpsc::Sender<StoredEvent>,
        transaction_tx: &mpsc::Sender<StoredTransaction>,
    ) -> Result<(), IndexerError> {
        for tx in transactions {
            for event in Self::stored_events_from_transaction(&tx)? {
                if let Err(e) = event_tx.try_send(event) {
                    error!("failed to queue event: {e}");
                }
            }
            if let Err(e) = transaction_tx.try_send(tx) {
                error!("failed to queue transaction: {e}");
            }
        }
        Ok(())
    }

    fn first_and_last_checkpoint_notifications(
        messages: &[AsyncMessage],
    ) -> Option<(CheckpointCommitNotification, CheckpointCommitNotification)> {
        let mut first: Option<CheckpointCommitNotification> = None;
        let mut last: Option<CheckpointCommitNotification> = None;

        for msg in messages {
            match msg {
                AsyncMessage::Notification(n) => {
                    match serde_json::from_str::<CheckpointCommitNotification>(n.payload()) {
                        Ok(parsed) => {
                            first.get_or_insert(parsed.clone());
                            last = Some(parsed);
                        }
                        Err(e) => {
                            error!("failed parsing checkpoint notification: {e}");
                        }
                    }
                }
                AsyncMessage::Notice(notice) => {
                    warn!("received PostgreSQL notice: {}", notice.message());
                }
                _ => {}
            }
        }

        first.and_then(|f| last.map(|l| (f, l)))
    }

    fn stored_events_from_transaction(
        tx: &StoredTransaction,
    ) -> Result<Vec<StoredEvent>, IndexerError> {
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
                event_type: native.type_.to_canonical_string(/* with_prefix */ true),
                timestamp_ms: tx.timestamp_ms,
                bcs: native.contents.clone(),
            })
            .collect();
        Ok(stored)
    }
}
