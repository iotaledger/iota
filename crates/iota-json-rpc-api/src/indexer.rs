// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_types::{
    DynamicFieldPage, EventFilter, EventPage, IotaEvent, IotaObjectDataOptions, IotaObjectResponse,
    IotaObjectResponseQuery, IotaTransactionBlockEffects, IotaTransactionBlockResponseQuery,
    ObjectsPage, TransactionBlocksPage, TransactionFilter,
};
use iota_open_rpc_macros::open_rpc;
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    digests::TransactionDigest,
    dynamic_field::DynamicFieldName,
    event::EventID,
};
use jsonrpsee::{
    core::{RpcResult, SubscriptionResult},
    proc_macros::rpc,
};

/// Provides methods to query transactions, events, or objects and allows to
/// subscribe to data streams.
#[open_rpc(namespace = "iotax", tag = "Extended API")]
#[rpc(server, client, namespace = "iotax")]
pub trait IndexerApi {
    /// Return the list of objects owned by an address.
    /// Note that if the address owns more than `QUERY_MAX_RESULT_LIMIT` objects,
    /// the pagination is not accurate, because previous page may have been updated when
    /// the next page is fetched.
    /// Please use iotax_queryObjects if this is a concern.
    #[rustfmt::skip]
    #[method(name = "getOwnedObjects")]
    async fn get_owned_objects(
        &self,
        /// the owner's IOTA address
        address: IotaAddress,
        /// the objects query criteria.
        query: Option<IotaObjectResponseQuery>,
        /// An optional paging cursor. If provided, the query will start from the next item after the specified cursor. Default to start from the first item if not specified.
        cursor: Option<ObjectID>,
        /// Max number of items returned per page, default to [QUERY_MAX_RESULT_LIMIT] if not specified.
        limit: Option<usize>,
    ) -> RpcResult<ObjectsPage>;

    /// Return list of transactions for a specified query criteria.
    #[rustfmt::skip]
    #[method(name = "queryTransactionBlocks")]
    async fn query_transaction_blocks(
        &self,
        /// the transaction query criteria.
        query: IotaTransactionBlockResponseQuery,
        /// An optional paging cursor. If provided, the query will start from the next item after the specified cursor. Default to start from the first item if not specified.
        cursor: Option<TransactionDigest>,
        /// Maximum item returned per page, default to QUERY_MAX_RESULT_LIMIT if not specified.
        limit: Option<usize>,
        /// query result ordering, default to false (ascending order), oldest record first.
        descending_order: Option<bool>,
    ) -> RpcResult<TransactionBlocksPage>;

    /// Return list of events for a specified query criteria.
    #[rustfmt::skip]
    #[method(name = "queryEvents")]
    async fn query_events(
        &self,
        /// The event query criteria. See [Event filter](https://docs.iota.org/developer/iota-101/using-events#applying-event-filters) documentation for examples.
        query: EventFilter,
        /// optional paging cursor
        cursor: Option<EventID>,
        /// maximum number of items per page, default to [QUERY_MAX_RESULT_LIMIT] if not specified.
        limit: Option<usize>,
        /// query result ordering, default to false (ascending order), oldest record first.
        descending_order: Option<bool>,
    ) -> RpcResult<EventPage>;

    /// Subscribe to a stream of IOTA event
    #[rustfmt::skip]
    #[subscription(name = "subscribeEvent", item = IotaEvent)]
    fn subscribe_event(
        &self,
        /// The filter criteria of the event stream. See [Event filter](https://docs.iota.org/developer/iota-101/using-events#applying-event-filters) documentation for examples.
        filter: EventFilter,
    ) -> SubscriptionResult;

    /// Subscribe to a stream of IOTA transaction effects
    #[subscription(name = "subscribeTransaction", item = IotaTransactionBlockEffects)]
    fn subscribe_transaction(&self, filter: TransactionFilter) -> SubscriptionResult;

    /// Return the list of dynamic field objects owned by an object.
    #[rustfmt::skip]
    #[method(name = "getDynamicFields")]
    async fn get_dynamic_fields(
        &self,
        /// The ID of the parent object
        parent_object_id: ObjectID,
        /// An optional paging cursor. If provided, the query will start from the next item after the specified cursor. Default to start from the first item if not specified.
        cursor: Option<ObjectID>,
        /// Maximum item returned per page, default to [QUERY_MAX_RESULT_LIMIT] if not specified.
        limit: Option<usize>,
    ) -> RpcResult<DynamicFieldPage>;

    /// Return the dynamic field object information for a specified object
    #[rustfmt::skip]
    #[method(name = "getDynamicFieldObject")]
    async fn get_dynamic_field_object(
        &self,
        /// The ID of the queried parent object
        parent_object_id: ObjectID,
        /// The Name of the dynamic field
        name: DynamicFieldName,
    ) -> RpcResult<IotaObjectResponse>;

    /// Return the dynamic field object information for a specified object with content options.
    #[rustfmt::skip]
    #[method(name = "getDynamicFieldObjectV2")]
    async fn get_dynamic_field_object_v2(
        &self,
        /// The ID of the queried parent object
        parent_object_id: ObjectID,
        /// The Name of the dynamic field
        name: DynamicFieldName,
        /// Options for specifying the content to be returned
        options: Option<IotaObjectDataOptions>,
    ) -> RpcResult<IotaObjectResponse>;
}
