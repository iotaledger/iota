// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use async_graphql::{Context, Subscription};
use futures::{Stream, StreamExt, future};
use iota_indexer::{
    indexer_reader::IndexerReader,
    stream::{IndexerStreamer, StreamEventFilter, StreamTransactionFilter},
};
use iota_json_rpc_types::Filter;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tracing::warn;

use crate::{
    error::Error,
    types::{
        event::Event,
        subscription::filter::{SubscriptionEventFilter, SubscriptionTransactionFilter},
        transaction_block::{TransactionBlock, TransactionBlockInner},
    },
};

mod filter;

/// Subscribe to events and transactions from the IOTA network.
pub struct Subscription;

#[Subscription]
impl Subscription {
    /// Subscribe to incoming transactions from the IOTA network.
    ///
    /// If no filter is provided, all transactions will be returned.
    async fn transactions(
        &self,
        ctx: &Context<'_>,
        filter: Option<SubscriptionTransactionFilter>,
    ) -> impl Stream<Item = Result<TransactionBlock, Error>> {
        let streams = ctx.data_unchecked::<GraphQLStream>().clone();
        streams.subscribe_transactions(filter.map(Into::<StreamTransactionFilter>::into))
    }

    /// Subscribe to incoming events from the IOTA network.
    ///
    /// If no filter is provided, all events will be returned.
    async fn events(
        &self,
        ctx: &Context<'_>,
        filter: Option<SubscriptionEventFilter>,
    ) -> impl Stream<Item = Result<Event, Error>> {
        let streams = ctx.data_unchecked::<GraphQLStream>().clone();
        streams.subscribe_events(filter.map(Into::<StreamEventFilter>::into))
    }
}

/// Provides real-time data streams for the GraphQL subscription feature.
///
/// It wraps the low-level [`IndexerStreamer`] and handles necessary
/// data processing, filtering, and subscription-specific error handling before
/// yielding items to GraphQL.
///
/// It ensures that when a critical data error occurs during item conversion,
/// the resulting stream is gracefully terminated by the server.
#[derive(Clone)]
pub(crate) struct GraphQLStream {
    streamer: Arc<IndexerStreamer>,
}

impl GraphQLStream {
    pub(crate) async fn new(db_url: &str, indexer_reader: IndexerReader) -> Result<Self, Error> {
        let streamer = IndexerStreamer::new(db_url, indexer_reader)
            .await
            .map_err(|e| Error::Internal(format!("failed to connect to postgres: {e}")))?;
        Ok(Self {
            streamer: Arc::new(streamer),
        })
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
        filter: Option<StreamTransactionFilter>,
    ) -> impl Stream<Item = Result<TransactionBlock, Error>> {
        self.streamer
            .subscribe_transactions()
            // - Some(Some(item)): Yield the item and continue the stream.
            // - Some(None): Do not yield an item, but continue the stream (used for filtering).
            // - None: Crucially, this signal stops the entire stream immediately, which is what we
            // need for server-side dropping of the stream.
            .scan(false, move |should_terminate_stream, stored| {
                if *should_terminate_stream {
                    return future::ready(None);
                }

                let Ok(stored) = stored.inspect_err(|BroadcastStreamRecvError::Lagged(count)| {
                    warn!("subscriber lagging by {count} messages")
                }) else {
                    return future::ready(Some(None));
                };

                if !Self::matches_filter(filter.as_ref(), &stored) {
                    return future::ready(Some(None));
                }

                let checkpoint_viewed_at = stored.checkpoint_sequence_number as u64;
                match TransactionBlockInner::try_from(stored) {
                    Ok(inner) => {
                        let tx = TransactionBlock {
                            inner,
                            checkpoint_viewed_at,
                        };
                        future::ready(Some(Some(Ok(tx))))
                    }
                    Err(e) => {
                        *should_terminate_stream = true;
                        future::ready(Some(Some(Err(e))))
                    }
                }
            })
            .filter_map(future::ready)
    }

    /// Subscribe to events from IOTA Network.
    pub(crate) fn subscribe_events(
        &self,
        filter: Option<StreamEventFilter>,
    ) -> impl Stream<Item = Result<Event, Error>> {
        self.streamer
            .subscribe_events()
            // - Some(Some(item)): Yield the item and continue the stream.
            // - Some(None): Do not yield an item, but continue the stream (used for filtering).
            // - None: Crucially, this signal stops the entire stream immediately, which is what we
            // need for server-side dropping of the stream.
            .scan(false, move |should_terminate_stream, stored| {
                if *should_terminate_stream {
                    return future::ready(None);
                }

                let Ok(stored) = stored.inspect_err(|BroadcastStreamRecvError::Lagged(count)| {
                    warn!("subscriber lagging by {count} messages")
                }) else {
                    return future::ready(Some(None));
                };

                if !Self::matches_filter(filter.as_ref(), &stored) {
                    return future::ready(Some(None));
                }

                match Event::try_from_stored_event(stored, 0) {
                    Ok(ev) => future::ready(Some(Some(Ok(ev)))),
                    Err(e) => {
                        *should_terminate_stream = true;
                        future::ready(Some(Some(Err(e))))
                    }
                }
            })
            .filter_map(future::ready)
    }
}
