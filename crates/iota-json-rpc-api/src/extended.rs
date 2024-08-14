// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_types::{
    AddressMetrics, CheckpointedObjectID, EpochInfo, EpochMetricsPage, EpochPage,
    IotaObjectResponseQuery, MoveCallMetrics, NetworkMetrics, QueryObjectsPage,
};
use iota_open_rpc_macros::open_rpc;
use iota_types::iota_serde::BigInt;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};

/// Methods served exclusively by the indexer, supporting queries using refined
/// filters and providing access to system info and metrics.
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

    /// Return a list of epoch metrics, which is a subset of epoch info
    #[method(name = "getEpochMetrics")]
    async fn get_epoch_metrics(
        &self,
        /// optional paging cursor
        cursor: Option<BigInt<u64>>,
        /// maximum number of items per page
        limit: Option<usize>,
        /// flag to return results in descending order
        descending_order: Option<bool>,
    ) -> RpcResult<EpochMetricsPage>;

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

    /// Return Network metrics
    #[method(name = "getNetworkMetrics")]
    async fn get_network_metrics(&self) -> RpcResult<NetworkMetrics>;

    /// Return Network metrics
    #[method(name = "getMoveCallMetrics")]
    async fn get_move_call_metrics(&self) -> RpcResult<MoveCallMetrics>;

    /// Address related metrics
    #[method(name = "getLatestAddressMetrics")]
    async fn get_latest_address_metrics(&self) -> RpcResult<AddressMetrics>;
    #[method(name = "getCheckpointAddressMetrics")]
    async fn get_checkpoint_address_metrics(&self, checkpoint: u64) -> RpcResult<AddressMetrics>;
    #[method(name = "getAllEpochAddressMetrics")]
    async fn get_all_epoch_address_metrics(
        &self,
        descending_order: Option<bool>,
    ) -> RpcResult<Vec<AddressMetrics>>;

    #[method(name = "getTotalTransactions")]
    async fn get_total_transactions(&self) -> RpcResult<BigInt<u64>>;
}
