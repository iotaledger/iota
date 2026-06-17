// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_types::{
    DynamicFieldNameSchema, DynamicFieldPage, EventFilter, EventPage, IotaDynamicFieldInfo,
    IotaEvent, IotaEventID, IotaNameRecord, IotaObjectDataOptions, IotaObjectResponse,
    IotaObjectResponseQuery, IotaTransactionBlockEffects, IotaTransactionBlockResponse,
    IotaTransactionBlockResponseQuery, IotaTransactionBlockResponseQueryV2, ObjectsPage, Page,
    TransactionBlocksPage, TransactionFilter,
    iota_primitives::{
        Base58 as Base58Schema, IotaAddress as IotaAddressSchema, ObjectId as ObjectIdSchema,
    },
};
use iota_open_rpc_macros::open_rpc;
use iota_sdk_types::ObjectId;
use iota_types::{
    base_types::IotaAddress, digests::TransactionDigest, dynamic_field::DynamicFieldName,
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
    /// the pagination is not accurate, because previous page may have been updated
    /// when the next page is fetched.
    /// Please use iotax_queryObjects if this is a concern.
    #[rustfmt::skip]
    #[method(name = "getOwnedObjects")]
    #[schemars(with = "Page<IotaObjectResponse, ObjectIdSchema>")]
    async fn get_owned_objects(
        &self,
        /// the owner's IOTA address
        #[schemars(with = "IotaAddressSchema")] 
        address: IotaAddress,
        /// the objects query criteria.
        query: Option<IotaObjectResponseQuery>,
        /// An optional paging cursor. If provided, the query will start from the next item after the specified cursor. Default to start from the first item if not specified.
        #[schemars(with = "Option<ObjectIdSchema>")] 
        cursor: Option<ObjectId>,
        /// Max number of items returned per page, default to [QUERY_MAX_RESULT_LIMIT] if not specified.
        limit: Option<usize>,
    ) -> RpcResult<ObjectsPage>;

    /// Return list of transactions for a specified query criteria.
    #[rustfmt::skip]
    #[method(name = "queryTransactionBlocks", version <= "1.2.10")]
    #[schemars(with = "Page<IotaTransactionBlockResponse, Base58>")]
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

    /// Return list of transactions for a specified query criteria.
    #[rustfmt::skip]
    #[method(name = "queryTransactionBlocks")]
    #[schemars(with = "Page<IotaTransactionBlockResponse, Base58Schema>")]
    async fn query_transaction_blocks_v2(
        &self,
        /// the transaction query criteria.
        query: IotaTransactionBlockResponseQueryV2,
        /// An optional paging cursor. If provided, the query will start from the next item after the specified cursor. Default to start from the first item if not specified.
        #[schemars(with = "Option<Base58Schema>")]
        cursor: Option<TransactionDigest>,
        /// Maximum item returned per page, default to QUERY_MAX_RESULT_LIMIT if not specified.
        limit: Option<usize>,
        /// query result ordering, default to false (ascending order), oldest record first.
        descending_order: Option<bool>,
    ) -> RpcResult<TransactionBlocksPage>;

    /// Return list of events for a specified query criteria.
    #[rustfmt::skip]
    #[method(name = "queryEvents")]
    #[schemars(with = "Page<IotaEvent, IotaEventID>")]
    async fn query_events(
        &self,
        /// The event query criteria. See [Event filter](https://docs.iota.org/developer/iota-101/using-events#applying-event-filters) documentation for examples.
        query: EventFilter,
        /// optional paging cursor
        #[schemars(with = "Option<IotaEventID>")]
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
    #[schemars(with = "Page<IotaDynamicFieldInfo, ObjectIdSchema>")]
    async fn get_dynamic_fields(
        &self,
        /// The ID of the parent object
        #[schemars(with = "ObjectIdSchema")]
        parent_object_id: ObjectId,
        /// An optional paging cursor. If provided, the query will start from the next item after the specified cursor. Default to start from the first item if not specified.
        #[schemars(with = "Option<ObjectIdSchema>")]
        cursor: Option<ObjectId>,
        /// Maximum item returned per page, default to [QUERY_MAX_RESULT_LIMIT] if not specified.
        limit: Option<usize>,
    ) -> RpcResult<DynamicFieldPage>;

    /// Return the dynamic field object information for a specified object
    #[rustfmt::skip]
    #[method(name = "getDynamicFieldObject")]
    async fn get_dynamic_field_object(
        &self,
        /// The ID of the queried parent object
        #[schemars(with = "ObjectIdSchema")]
        parent_object_id: ObjectId,
        /// The Name of the dynamic field
        #[schemars(with = "DynamicFieldNameSchema")]
        name: DynamicFieldName,
    ) -> RpcResult<IotaObjectResponse>;

    /// Return the dynamic field object information for a specified object with
    /// content options.
    #[rustfmt::skip]
    #[method(name = "getDynamicFieldObjectV2")]
    async fn get_dynamic_field_object_v2(
        &self,
        /// The ID of the queried parent object
        #[schemars(with = "ObjectIdSchema")]
        parent_object_id: ObjectId,
        /// The Name of the dynamic field
        #[schemars(with = "DynamicFieldNameSchema")]
        name: DynamicFieldName,
        /// Options for specifying the content to be returned
        options: Option<IotaObjectDataOptions>,
    ) -> RpcResult<IotaObjectResponse>;

    /// Return the resolved record for the given name.
    #[method(name = "iotaNamesLookup")]
    async fn iota_names_lookup(
        &self,
        /// The name to resolve
        name: &str,
    ) -> RpcResult<Option<IotaNameRecord>>;

    /// Return the resolved name for the given address.
    #[method(name = "iotaNamesReverseLookup")]
    async fn iota_names_reverse_lookup(
        &self,
        /// The address to resolve.
        #[schemars(with = "IotaAddressSchema")]
        address: IotaAddress,
    ) -> RpcResult<Option<String>>;

    /// Find all registration NFTs for the given address.
    #[method(name = "iotaNamesFindAllRegistrationNFTs")]
    #[schemars(with = "Page<IotaObjectResponse, ObjectIdSchema>")]
    async fn iota_names_find_all_registration_nfts(
        &self,
        #[schemars(with = "IotaAddressSchema")] address: IotaAddress,
        #[schemars(with = "Option<ObjectIdSchema>")] cursor: Option<ObjectId>,
        limit: Option<usize>,
        options: Option<IotaObjectDataOptions>,
    ) -> RpcResult<ObjectsPage>;
}
