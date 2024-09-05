// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use futures::{stream, StreamExt};
use futures_core::Stream;
use iota_json_rpc_api::{IndexerApiClient, ReadApiClient};
use iota_json_rpc_types::{EventFilter, EventPage, IotaEvent};
use iota_types::{base_types::TransactionDigest, event::EventID};
use jsonrpsee::core::client::Subscription;

use crate::{
    error::{Error, IotaRpcResult},
    RpcClient,
};

/// Event API provides the functionality to fetch, query, or subscribe to events
/// on the Iota network.
#[derive(Clone)]
pub struct EventApi {
    api: Arc<RpcClient>,
}

impl EventApi {
    pub(crate) fn new(api: Arc<RpcClient>) -> Self {
        Self { api }
    }

    /// Return a stream of events, or an error upon failure.
    ///
    /// Subscription is only possible via WebSockets.
    /// For a list of possible event filters, see [EventFilter].
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// use std::str::FromStr;
    ///
    /// use futures::StreamExt;
    /// use iota_json_rpc_types::EventFilter;
    /// use iota_sdk::IotaClientBuilder;
    /// use iota_types::base_types::IotaAddress;
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default()
    ///         .ws_url("wss://rpc.mainnet.iota.io:443")
    ///         .build("https://fullnode.mainnet.iota.io:443")
    ///         .await?;
    ///     let mut subscribe_all = iota
    ///         .event_api()
    ///         .subscribe_event(EventFilter::All(vec![]))
    ///         .await?;
    ///     loop {
    ///         println!("{:?}", subscribe_all.next().await);
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub async fn subscribe_event(
        &self,
        filter: EventFilter,
    ) -> IotaRpcResult<impl Stream<Item = IotaRpcResult<IotaEvent>>> {
        match &self.api.ws {
            Some(c) => {
                let subscription: Subscription<IotaEvent> = c.subscribe_event(filter).await?;
                Ok(subscription.map(|item| Ok(item?)))
            }
            _ => Err(Error::Subscription(
                "Subscription only supported by WebSocket client.".to_string(),
            )),
        }
    }

    /// Return a list of events for the given transaction digest, or an error
    /// upon failure.
    pub async fn get_events(&self, digest: TransactionDigest) -> IotaRpcResult<Vec<IotaEvent>> {
        Ok(self.api.http.get_events(digest).await?)
    }

    /// Return a paginated response with events for the given event filter, or
    /// an error upon failure.
    ///
    /// The ordering of the events can be set with the `descending_order`
    /// argument. For a list of possible event filters, see [EventFilter].
    pub async fn query_events(
        &self,
        query: EventFilter,
        cursor: impl Into<Option<EventID>>,
        limit: impl Into<Option<usize>>,
        descending_order: bool,
    ) -> IotaRpcResult<EventPage> {
        Ok(self
            .api
            .http
            .query_events(query, cursor.into(), limit.into(), Some(descending_order))
            .await?)
    }

    /// Return a stream of events for the given event filter.
    ///
    /// The ordering of the events can be set with the `descending_order`
    /// argument. For a list of possible event filters, see [EventFilter].
    pub fn get_events_stream(
        &self,
        query: EventFilter,
        cursor: impl Into<Option<EventID>>,
        descending_order: bool,
    ) -> impl Stream<Item = IotaEvent> + '_ {
        let cursor = cursor.into();

        stream::unfold(
            (vec![], cursor, true, query),
            move |(mut data, cursor, first, query)| async move {
                if let Some(item) = data.pop() {
                    Some((item, (data, cursor, false, query)))
                } else if (cursor.is_none() && first) || cursor.is_some() {
                    let page = self
                        .query_events(query.clone(), cursor, Some(100), descending_order)
                        .await
                        .ok()?;
                    let mut data = page.data;
                    data.reverse();
                    data.pop()
                        .map(|item| (item, (data, page.next_cursor, false, query)))
                } else {
                    None
                }
            },
        )
    }
}
