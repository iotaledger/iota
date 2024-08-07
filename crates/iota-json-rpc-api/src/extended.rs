// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_types::{
    CheckpointedObjectID, EpochInfo, EpochPage, IotaObjectResponseQuery, QueryObjectsPage,
};
use iota_open_rpc_macros::open_rpc;
use iota_types::iota_serde::BigInt;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};

/// The `ExtendedApi` trait provides a set of asynchronous methods for accessing
/// extended information on the IOTA blockchain. This trait is designed to be
/// used in an RPC context, allowing clients to fetch detailed data related to
/// epochs and transactions.
///
/// The following methods are available in this trait:
///
/// - `get_epochs`: Fetches a list of epoch information with optional pagination
///   and ordering.
/// - `get_current_epoch`: Retrieves the current epoch information.
/// - `query_objects`: Retrieves a paginated list of queried objects based on
///   query criteria.
/// - `get_total_transactions`: Returns the total number of transactions on the
///   IOTA blockchain.
#[open_rpc(namespace = "iotax", tag = "Extended API")]
#[rpc(server, client, namespace = "iotax")]
pub trait ExtendedApi {
    /// Return a list of epoch info
    #[rustfmt::skip]
    #[method(name = "getEpochs")]
    async fn get_epochs(
        &self,
        /// optional paging cursor
        cursor: Option<BigInt<u64>>,
        /// maximum number of items per page
        limit: Option<usize>,
        /// flag to return results in descending order
        descending_order: Option<bool>,
    ) -> RpcResult<EpochPage>;

    /// Return current epoch info
    #[method(name = "getCurrentEpoch")]
    async fn get_current_epoch(&self) -> RpcResult<EpochInfo>;

    /// Return the list of queried objects. Note that this is an enhanced full node only api.
    #[rustfmt::skip]
    #[method(name = "queryObjects")]
    async fn query_objects(
        &self,
        /// the objects query criteria.
        query: IotaObjectResponseQuery,
        /// An optional paging cursor. If provided, the query will start from the next item after the specified cursor. Default to start from the first item if not specified.
        cursor: Option<CheckpointedObjectID>,
        /// Max number of items returned per page, default to [QUERY_MAX_RESULT_LIMIT] if not specified.
        limit: Option<usize>,
    ) -> RpcResult<QueryObjectsPage>;

    #[method(name = "getTotalTransactions")]
    async fn get_total_transactions(&self) -> RpcResult<BigInt<u64>>;
}
