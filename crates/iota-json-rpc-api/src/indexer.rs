// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_types::{
    DynamicFieldPage, EventFilter, EventPage, IotaEvent, IotaObjectResponse,
    IotaObjectResponseQuery, IotaTransactionBlockEffects, IotaTransactionBlockResponseQuery,
    ObjectsPage, Page, TransactionBlocksPage, TransactionFilter,
};
use iota_open_rpc_macros::open_rpc;
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    digests::TransactionDigest,
    dynamic_field::DynamicFieldName,
    event::EventID,
};
use jsonrpsee::{core::RpcResult, proc_macros::rpc};

/// Provide methods to query transactions, events, or objects.
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
        /// the owner's Iota address
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
        /// The event query criteria. See [Event filter](https://docs.iota.io/build/event_api#event-filters) documentation for examples.
        query: EventFilter,
        /// optional paging cursor
        cursor: Option<EventID>,
        /// maximum number of items per page, default to [QUERY_MAX_RESULT_LIMIT] if not specified.
        limit: Option<usize>,
        /// query result ordering, default to false (ascending order), oldest record first.
        descending_order: Option<bool>,
    ) -> RpcResult<EventPage>;

    /// Subscribe to a stream of Iota event
    #[rustfmt::skip]
    #[subscription(name = "subscribeEvent", item = IotaEvent)]
    async fn subscribe_event(
        &self,
        /// The filter criteria of the event stream. See [Event filter](https://docs.iota.io/build/event_api#event-filters) documentation for examples.
        filter: EventFilter,
    );

    /// Subscribe to a stream of Iota transaction effects
    #[subscription(name = "subscribeTransaction", item = IotaTransactionBlockEffects)]
    async fn subscribe_transaction(&self, filter: TransactionFilter);

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

    /// Return the resolved address given resolver and name
    #[rustfmt::skip]
    #[method(name = "resolveNameServiceAddress")]
    async fn resolve_name_service_address(
        &self,
        /// The name to resolve
        name: String,
    ) -> RpcResult<Option<IotaAddress>>;

    /// Return the resolved names given address,
    /// if multiple names are resolved, the first one is the primary name.
    #[rustfmt::skip]
    #[method(name = "resolveNameServiceNames")]
    async fn resolve_name_service_names(
        &self,
        /// The address to resolve
        address: IotaAddress,
        cursor: Option<ObjectID>,
        limit: Option<usize>,
    ) -> RpcResult<Page<String, ObjectID>>;
}
