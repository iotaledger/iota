// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::num::NonZeroUsize;

use async_graphql::{Context, OutputType, ResultExt, SimpleObject, Subscription, Union};
use futures::{Stream, StreamExt, TryStreamExt, future};
use iota_indexer::read::IndexerReader;
use iota_indexer_streaming::{
    error::IndexerStreamingError,
    memory::{Config, InMemory, RecoveryPoint},
    metrics::InMemoryStreamMetrics,
};
use iota_json_rpc_types::Filter;
use iota_types::{digests::TransactionDigest, supported_protocol_versions::Chain};
use prometheus::Registry;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tracing::warn;

use crate::{
    error::Error,
    types::{
        chain_identifier::ChainIdentifierCache,
        digest::Digest,
        event::Event,
        subscription::filter::{SubscriptionEventFilter, SubscriptionTransactionFilter},
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
    count: u64,
}

/// Possible responses from a subscription.
///
/// It could be one of the following:
/// - A successful payload from the subscription stream.
/// - A notice that the subscription has been lagged behind the network with the
///   number of lost payloads.
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
        let chain_id_cache: &ChainIdentifierCache = ctx.data_unchecked();

        let db = ctx.data_unchecked();
        let metrics = ctx.data_unchecked();
        let chain = chain_id_cache
            .read(db, metrics)
            .await
            .extend()?
            .into_inner()
            .chain();

        if !matches!(chain, Chain::Unknown) {
            return Err(Error::UnsupportedFeature(format!(
                "Subscriptions are not yet supported on {}",
                chain.as_str()
            )))
            .extend();
        }

        let streams = ctx.data_unchecked::<GraphQLStream>();
        Ok(streams.subscribe_transactions(start_after, filter))
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
        let chain_id_cache: &ChainIdentifierCache = ctx.data_unchecked();

        let db = ctx.data_unchecked();
        let metrics = ctx.data_unchecked();
        let chain = chain_id_cache
            .read(db, metrics)
            .await
            .extend()?
            .into_inner()
            .chain();

        if !matches!(chain, Chain::Unknown) {
            return Err(Error::UnsupportedFeature(format!(
                "Subscriptions are not yet supported on {}",
                chain.as_str()
            )))
            .extend();
        }

        let streams = ctx.data_unchecked::<GraphQLStream>();
        Ok(streams.subscribe_events(start_after, filter))
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
}

impl GraphQLStream {
    pub(crate) async fn new(
        indexer_reader: IndexerReader,
        registry: &Registry,
    ) -> Result<Self, Error> {
        let config = Config {
            channel_buffer_size: BROKER_CHANNEL_SIZE,
            ..Default::default()
        };
        let streamer = InMemory::new(config, indexer_reader, InMemoryStreamMetrics::new(registry))
            .await
            .map_err(|e| Error::Internal(format!("failed to connect to postgres: {e}")))?;
        Ok(Self { streamer })
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
    ) -> impl Stream<Item = Result<SubscriptionItem<TransactionBlock>, Error>> {
        self.streamer
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
                        Ok(SubscriptionItem::Lagged(Lagged { count }))
                    }
                    Err(err) => Err(err.into()),
                };
                future::ready(subscription_item)
            })
            // intercept the error, send it to the client and terminate the stream.
            .scan(false, |should_terminate_stream, subscription_item| {
                if *should_terminate_stream {
                    return future::ready(None);
                }
                future::ready(Some(
                    subscription_item.inspect_err(|_| *should_terminate_stream = true),
                ))
            })
    }

    /// Subscribe to events from IOTA Network.
    pub(crate) fn subscribe_events(
        &self,
        start_after: Option<Digest>,
        filter: Option<SubscriptionEventFilter>,
    ) -> impl Stream<Item = Result<SubscriptionItem<Event>, Error>> {
        self.streamer
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
                        Ok(SubscriptionItem::Lagged(Lagged { count }))
                    }
                    Err(err) => Err(err.into()),
                };
                future::ready(subscription_item)
            })
            // intercept the error, send it to the client and terminate the stream.
            .scan(false, |should_terminate_stream, subscription_item| {
                if *should_terminate_stream {
                    return future::ready(None);
                }
                future::ready(Some(
                    subscription_item.inspect_err(|_| *should_terminate_stream = true),
                ))
            })
    }
}
