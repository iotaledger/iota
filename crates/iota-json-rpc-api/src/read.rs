// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_types::{
    Checkpoint, CheckpointId, CheckpointPage, IotaEvent, IotaGetPastObjectRequest,
    IotaLoadedChildObjectsResponse, IotaObjectDataOptions, IotaObjectResponse,
    IotaPastObjectResponse, IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions,
    ProtocolConfigResponse,
};
use iota_open_rpc_macros::open_rpc;
use iota_types::{
    base_types::{ObjectID, SequenceNumber, TransactionDigest},
    iota_serde::BigInt,
};
use jsonrpsee::{core::RpcResult, proc_macros::rpc};

/// The `ReadApi` trait provides a set of asynchronous methods for reading data
/// on the IOTA network. This trait is designed to be used in an RPC context,
/// allowing clients to fetch information about transactions, objects,
/// checkpoints, events, and protocol configurations.
///
/// The following methods are available in this trait:
///
/// - `get_transaction_block`: Returns the transaction for a given digest.
/// - `multi_get_transaction_blocks`: Returns an ordered list of transactions
///   for given digests.
/// - `get_object`: Returns the object information for a specified object ID.
/// - `multi_get_objects`: Retrieves object information for a list of specified
///   object IDs.
/// - `try_get_past_object`: Returns the object at a specified version, if
///   available.
/// - `try_multi_get_past_objects`: Retrieves objects at specified versions, if
///   available.
/// - `get_loaded_child_objects`: Fetches the loaded child objects for a given
///   transaction digest.
/// - `get_checkpoint`: Returns a checkpoint for a given checkpoint ID.
/// - `get_checkpoints`: Fetches a list of checkpoints.
/// - `get_checkpoints_deprecated_limit`: Fetches a list of checkpoints
///   (deprecated).
/// - `get_events`: Returns transaction events for a given transaction digest.
/// - `get_total_transaction_blocks`: Returns the total number of transaction
///   blocks known to the server.
/// - `get_latest_checkpoint_sequence_number`: Returns the sequence number of
///   the latest executed checkpoint.
/// - `get_protocol_config`: Fetches the protocol configuration for a given
///   version.
/// - `get_chain_identifier`: Returns the first four bytes of the chain's
///   genesis checkpoint digest.
#[open_rpc(namespace = "iota", tag = "Read API")]
#[rpc(server, client, namespace = "iota")]
pub trait ReadApi {
    /// Return the transaction response object.
    #[rustfmt::skip]
    #[method(name = "getTransactionBlock")]
    async fn get_transaction_block(
        &self,
        /// the digest of the queried transaction
        digest: TransactionDigest,
        /// options for specifying the content to be returned
        options: Option<IotaTransactionBlockResponseOptions>,
    ) -> RpcResult<IotaTransactionBlockResponse>;

    /// Returns an ordered list of transaction responses
    /// The method will throw an error if the input contains any duplicate or
    /// the input size exceeds QUERY_MAX_RESULT_LIMIT
    #[rustfmt::skip]
    #[method(name = "multiGetTransactionBlocks")]
    async fn multi_get_transaction_blocks(
        &self,
        /// A list of transaction digests.
        digests: Vec<TransactionDigest>,
        /// config options to control which fields to fetch
        options: Option<IotaTransactionBlockResponseOptions>,
    ) -> RpcResult<Vec<IotaTransactionBlockResponse>>;

    /// Return the object information for a specified object
    #[rustfmt::skip]
    #[method(name = "getObject")]
    async fn get_object(
        &self,
        /// the ID of the queried object
        object_id: ObjectID,
        /// options for specifying the content to be returned
        options: Option<IotaObjectDataOptions>,
    ) -> RpcResult<IotaObjectResponse>;

    /// Return the object data for a list of objects
    #[rustfmt::skip]
    #[method(name = "multiGetObjects")]
    async fn multi_get_objects(
        &self,
        /// the IDs of the queried objects
        object_ids: Vec<ObjectID>,
        /// options for specifying the content to be returned
        options: Option<IotaObjectDataOptions>,
    ) -> RpcResult<Vec<IotaObjectResponse>>;

    /// Note there is no software-level guarantee/SLA that objects with past versions
    /// can be retrieved by this API, even if the object and version exists/existed.
    /// The result may vary across nodes depending on their pruning policies.
    /// Return the object information for a specified version
    #[rustfmt::skip]
    #[method(name = "tryGetPastObject")]
    async fn try_get_past_object(
        &self,
        /// the ID of the queried object
        object_id: ObjectID,
        /// the version of the queried object. If None, default to the latest known version
        version: SequenceNumber,
        /// options for specifying the content to be returned
        options: Option<IotaObjectDataOptions>,
    ) -> RpcResult<IotaPastObjectResponse>;

    /// Note there is no software-level guarantee/SLA that objects with past versions
    /// can be retrieved by this API, even if the object and version exists/existed.
    /// The result may vary across nodes depending on their pruning policies.
    /// Return the object information for a specified version
    #[rustfmt::skip]
    #[method(name = "tryMultiGetPastObjects")]
    async fn try_multi_get_past_objects(
        &self,
        /// a vector of object and versions to be queried
        past_objects: Vec<IotaGetPastObjectRequest>,
        /// options for specifying the content to be returned
        options: Option<IotaObjectDataOptions>,
    ) -> RpcResult<Vec<IotaPastObjectResponse>>;

    #[method(name = "getLoadedChildObjects")]
    async fn get_loaded_child_objects(
        &self,
        digest: TransactionDigest,
    ) -> RpcResult<IotaLoadedChildObjectsResponse>;

    /// Return a checkpoint
    #[rustfmt::skip]
    #[method(name = "getCheckpoint")]
    async fn get_checkpoint(
        &self,
        /// Checkpoint identifier, can use either checkpoint digest, or checkpoint sequence number as input.
        id: CheckpointId,
    ) -> RpcResult<Checkpoint>;

    /// Return paginated list of checkpoints
    #[rustfmt::skip]
    #[method(name = "getCheckpoints")]
    async fn get_checkpoints(
        &self,
        /// An optional paging cursor. If provided, the query will start from the next item after the specified cursor. Default to start from the first item if not specified.
        cursor: Option<BigInt<u64>>,
        /// Maximum item returned per page, default to [QUERY_MAX_RESULT_LIMIT_CHECKPOINTS] if not specified.
        limit: Option<usize>,
        /// query result ordering, default to false (ascending order), oldest record first.
        descending_order: bool,
    ) -> RpcResult<CheckpointPage>;

    #[rustfmt::skip]
    #[method(name = "getCheckpoints", version <= "0.31")]
    async fn get_checkpoints_deprecated_limit(
        &self,
        /// An optional paging cursor. If provided, the query will start from the next item after the specified cursor. Default to start from the first item if not specified.
        cursor: Option<BigInt<u64>>,
        /// Maximum item returned per page, default to [QUERY_MAX_RESULT_LIMIT_CHECKPOINTS] if not specified.
        limit: Option<BigInt<u64>>,
        /// query result ordering, default to false (ascending order), oldest record first.
        descending_order: bool,
    ) -> RpcResult<CheckpointPage>;

    /// Return transaction events.
    #[method(name = "getEvents")]
    async fn get_events(
        &self,
        /// the event query criteria.
        transaction_digest: TransactionDigest,
    ) -> RpcResult<Vec<IotaEvent>>;

    /// Return the total number of transaction blocks known to the server.
    #[method(name = "getTotalTransactionBlocks")]
    async fn get_total_transaction_blocks(&self) -> RpcResult<BigInt<u64>>;

    /// Return the sequence number of the latest checkpoint that has been
    /// executed
    #[method(name = "getLatestCheckpointSequenceNumber")]
    async fn get_latest_checkpoint_sequence_number(&self) -> RpcResult<BigInt<u64>>;

    /// Return the protocol config table for the given version number.
    /// If the version number is not specified, If none is specified, the node uses the version of the latest epoch it has processed.
    #[rustfmt::skip]
    #[method(name = "getProtocolConfig")]
    async fn get_protocol_config(
        &self,
        /// An optional protocol version specifier. If omitted, the latest protocol config table for the node will be returned.
        version: Option<BigInt<u64>>,
    ) -> RpcResult<ProtocolConfigResponse>;

    /// Return the first four bytes of the chain's genesis checkpoint digest.
    #[method(name = "getChainIdentifier")]
    async fn get_chain_identifier(&self) -> RpcResult<String>;
}
