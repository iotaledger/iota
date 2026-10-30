// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{num::NonZeroUsize, sync::Arc};

use async_graphql::{Context, ErrorExtensions, OutputType, SimpleObject, Subscription, Union};
use futures::{Stream, StreamExt, TryStreamExt, future};
use iota_indexer::read::IndexerReader;
use iota_indexer_streaming::{
    error::IndexerStreamingError,
    memory::{Config, InMemory, RecoveryPoint},
    metrics::InMemoryStreamMetrics,
};
use iota_sdk_types::TransactionDigest;
use prometheus_filtered::Registry;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tracing::warn;

use crate::{
    config::Limits,
    error::Error,
    types::{
        digest::Digest,
        event::Event,
        int::try_into_int,
        subscription::filter::{Filter, SubscriptionEventFilter, SubscriptionTransactionFilter},
        transaction_block::{TransactionBlock, TransactionBlockInner},
    },
};

mod filter;

/// Represents the channel size of the [`InMemory`] broker.
const BROKER_CHANNEL_SIZE: NonZeroUsize =
    NonZeroUsize::new(10_000).expect("value should be greater than 0");

/// Notifies that the subscription consumer has fallen behind the live
/// subscription stream and missed one or more payloads.
#[derive(SimpleObject, Clone)]
pub(crate) struct Lagged {
    /// Number of missed payloads since the previous emitted one.
    count: i32,
}

/// Possible responses from a subscription.
///
/// It could be one of the following:
/// - A successful payload from the subscription stream.
/// - A notice that the subscription has been lagged behind the network with the number of lost
///   payloads.
#[derive(Union, Clone)]
#[graphql(concrete(name = "EventSubscriptionPayload", params(Event)))]
#[graphql(concrete(name = "TransactionBlockSubscriptionPayload", params(TransactionBlock)))]
pub(crate) enum SubscriptionItem<T: OutputType> {
    /// Successfully received payload from the subscription stream.
    Payload(T),
    /// A notice that the subscription has been lagged behind the network.
    Lagged(Lagged),
}

/// Subscribe to events and transactions from the IOTA network.
pub struct Subscription;

#[Subscription]
impl Subscription {
    /// Subscribe to incoming transactions from the IOTA network.
    ///
    /// If no filter is provided, all transactions will be returned.
    ///
    /// # Stream recovery
    ///
    /// When `start_after` is provided with the digest of the last transaction
    /// the client received, the stream resumes from the transaction
    /// immediately after it. The identified transaction itself is not
    /// emitted.
    async fn transactions(
        &self,
        ctx: &Context<'_>,
        start_after: Option<Digest>,
        filter: Option<SubscriptionTransactionFilter>,
    ) -> async_graphql::Result<impl Stream<Item = Result<SubscriptionItem<TransactionBlock>, Error>>>
    {
        let stream = ctx.data_unchecked::<GraphQLStream>();
        let permit = stream.acquire_permit().map_err(|e| e.extend())?;
        Ok(stream.subscribe_transactions(start_after, filter, permit))
    }

    /// Subscribe to incoming events from the IOTA network.
    ///
    /// If no filter is provided, all events will be returned.
    ///
    /// # Stream recovery
    ///
    /// Events are streamed in transaction order, all events from a single
    /// transaction arrive contiguously before any events from the next one.
    ///
    /// When `start_after` is provided with a transaction digest, the stream
    /// resumes from the transaction immediately after it.
    ///
    /// Because a single transaction can emit multiple events, a disconnection
    /// may leave the client with only a partial set of events from the
    /// transaction it was last receiving. A transaction is considered fully
    /// received only once the client has seen an event from the following
    /// transaction (a digest change signals the previous transaction is
    /// complete).
    ///
    /// On reconnect, clients should pass the digest of the last fully
    /// received transaction as `start_after`. The stream resumes from the
    /// next transaction, so no partial transaction is ever observed.
    async fn events(
        &self,
        ctx: &Context<'_>,
        start_after: Option<Digest>,
        filter: Option<SubscriptionEventFilter>,
    ) -> async_graphql::Result<impl Stream<Item = Result<SubscriptionItem<Event>, Error>>> {
        let stream = ctx.data_unchecked::<GraphQLStream>();
        let permit = stream.acquire_permit().map_err(|e| e.extend())?;
        Ok(stream.subscribe_events(start_after, filter, permit))
    }
}

/// Provides real-time data streams for the GraphQL subscription feature.
///
/// It wraps the low-level [`InMemory`] streamer and handles the necessary
/// data processing, filtering, and subscription-specific error handling before
/// yielding items to GraphQL.
///
/// It ensures that when a critical data error occurs during item conversion,
/// the resulting stream is gracefully terminated by the server.
pub(crate) struct GraphQLStream {
    streamer: InMemory,
    /// Caps the number of subscriptions active at the same time across all
    /// connections.
    active_subscriptions: Arc<Semaphore>,
}

impl GraphQLStream {
    pub(crate) async fn new(
        indexer_reader: IndexerReader,
        registry: &Registry,
        limits: &Limits,
    ) -> Result<Self, Error> {
        let config = Config {
            channel_buffer_size: BROKER_CHANNEL_SIZE,
            max_recovery_distance: Some(limits.max_subscription_recovery_distance.into()),
            ..Default::default()
        };
        let streamer = InMemory::new(config, indexer_reader, InMemoryStreamMetrics::new(registry))
            .await
            .map_err(|e| Error::Internal(format!("failed to connect to postgres: {e}")))?;
        Ok(Self {
            streamer,
            active_subscriptions: Arc::new(Semaphore::new(
                limits.max_active_subscriptions as usize,
            )),
        })
    }

    /// Reserves a slot for a new subscription, failing with a client error
    /// when the server-wide cap is reached.
    fn acquire_permit(&self) -> Result<OwnedSemaphorePermit, Error> {
        self.active_subscriptions
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::Client("too many active subscriptions, retry later".to_string()))
    }

    /// Sends the first error to the client and terminates the stream after it.
    /// The permit is held as long as the stream is alive.
    fn terminate_on_error<T>(
        stream: impl Stream<Item = Result<T, Error>>,
        permit: OwnedSemaphorePermit,
    ) -> impl Stream<Item = Result<T, Error>> {
        stream.scan(
            (false, permit),
            |(should_terminate_stream, _permit), subscription_item| {
                if *should_terminate_stream {
                    return future::ready(None);
                }
                future::ready(Some(
                    subscription_item.inspect_err(|_| *should_terminate_stream = true),
                ))
            },
        )
    }

    /// Checks if the provided filter matches the item.
    ///
    /// If no filter is provided, the item is **always** considered a match, and
    /// the function returns `true`.
    fn matches_filter<T, F>(filter: Option<&F>, item: &T) -> bool
    where
        F: Filter<T>,
    {
        filter.as_ref().map(|f| f.matches(item)).unwrap_or(true)
    }

    /// Subscribe to transactions from IOTA Network.
    pub(crate) fn subscribe_transactions(
        &self,
        start_after: Option<Digest>,
        filter: Option<SubscriptionTransactionFilter>,
        permit: OwnedSemaphorePermit,
    ) -> impl Stream<Item = Result<SubscriptionItem<TransactionBlock>, Error>> {
        let stream = self
            .streamer
            .subscribe_transactions(
                start_after.map(|digest| RecoveryPoint::Exclusive(TransactionDigest::from(digest))),
            )
            .try_filter(move |stored| future::ready(Self::matches_filter(filter.as_ref(), stored)))
            .then(|stored| {
                let subscription_item = match stored {
                    Ok(stored) => {
                        let checkpoint_viewed_at = stored.checkpoint_sequence_number as u64;
                        TransactionBlockInner::try_from(stored).map(|inner| {
                            SubscriptionItem::Payload(TransactionBlock {
                                inner,
                                checkpoint_viewed_at,
                            })
                        })
                    }
                    Err(IndexerStreamingError::Lagged(BroadcastStreamRecvError::Lagged(count))) => {
                        warn!("subscriber lagging by {count} messages");
                        try_into_int(count).map(|count| SubscriptionItem::Lagged(Lagged { count }))
                    }
                    Err(err) => Err(err.into()),
                };
                future::ready(subscription_item)
            });
        Self::terminate_on_error(stream, permit)
    }

    /// Subscribe to events from IOTA Network.
    pub(crate) fn subscribe_events(
        &self,
        start_after: Option<Digest>,
        filter: Option<SubscriptionEventFilter>,
        permit: OwnedSemaphorePermit,
    ) -> impl Stream<Item = Result<SubscriptionItem<Event>, Error>> {
        let stream = self
            .streamer
            .subscribe_events(
                start_after.map(|digest| RecoveryPoint::Exclusive(TransactionDigest::from(digest))),
            )
            .try_filter(move |stored| future::ready(Self::matches_filter(filter.as_ref(), stored)))
            .then(|stored| {
                let subscription_item = match stored {
                    Ok(stored) => {
                        Event::try_from_stored_event(stored, 0).map(SubscriptionItem::Payload)
                    }
                    Err(IndexerStreamingError::Lagged(BroadcastStreamRecvError::Lagged(count))) => {
                        warn!("subscriber lagging by {count} messages");
                        try_into_int(count).map(|count| SubscriptionItem::Lagged(Lagged { count }))
                    }
                    Err(err) => Err(err.into()),
                };
                future::ready(subscription_item)
            });
        Self::terminate_on_error(stream, permit)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use diesel::r2d2::{ConnectionManager, Pool};

    use super::*;

    #[tokio::test]
    async fn rejects_subscriptions_beyond_the_cap_until_one_is_dropped() {
        // The cap never touches the database, so a pool that cannot connect
        // is enough.
        let pool = Pool::builder()
            .connection_timeout(Duration::from_millis(1))
            .build_unchecked(ConnectionManager::<diesel::PgConnection>::new(
                "postgres://unused",
            ));
        let limits = Limits {
            max_active_subscriptions: 1,
            ..Default::default()
        };
        let stream = GraphQLStream::new(
            IndexerReader::new_without_watermark_cache(pool),
            &Registry::new(),
            &limits,
        )
        .await
        .unwrap();

        let first = stream.acquire_permit().unwrap();
        assert!(matches!(
            stream.acquire_permit(),
            Err(Error::Client(msg)) if msg == "too many active subscriptions, retry later"
        ));

        drop(first);
        let _again = stream.acquire_permit().unwrap();
    }

    #[tokio::test]
    async fn live_stream_holds_its_permit_until_dropped() {
        let active = Arc::new(Semaphore::new(1));
        let permit = active.clone().try_acquire_owned().unwrap();

        let items = futures::stream::pending::<Result<(), Error>>();
        let stream = GraphQLStream::terminate_on_error(items, permit);
        assert_eq!(active.available_permits(), 0);

        drop(stream);
        assert_eq!(active.available_permits(), 1);
    }

    #[tokio::test]
    async fn stream_ends_after_the_first_error_and_releases_its_permit() {
        let active = Arc::new(Semaphore::new(1));
        let permit = active.clone().try_acquire_owned().unwrap();

        let items = futures::stream::iter([Ok(1), Err(Error::Internal("boom".into())), Ok(2)]);
        let mut stream = GraphQLStream::terminate_on_error(items, permit);

        assert!(matches!(stream.next().await, Some(Ok(1))));
        assert_eq!(active.available_permits(), 0);
        assert!(matches!(stream.next().await, Some(Err(Error::Internal(_)))));
        assert!(stream.next().await.is_none());
        assert_eq!(active.available_permits(), 1);
    }
}
