// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_types::{
    AddressMetrics, EpochInfo, EpochMetricsPage, EpochPage,
    MoveCallMetrics, NetworkMetrics,
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
        /// Optional paging cursor
        cursor: Option<BigInt<u64>>,
        /// Maximum number of items per page
        limit: Option<usize>,
        /// Flag to return results in descending order
        descending_order: Option<bool>,
    ) -> RpcResult<EpochPage>;

    /// Return a list of epoch metrics, which is a subset of epoch info
    #[method(name = "getEpochMetrics")]
    async fn get_epoch_metrics(
        &self,
        /// Optional paging cursor
        cursor: Option<BigInt<u64>>,
        /// Maximum number of items per page
        limit: Option<usize>,
        /// Flag to return results in descending order
        descending_order: Option<bool>,
    ) -> RpcResult<EpochMetricsPage>;

    /// Return current epoch info
    #[method(name = "getCurrentEpoch")]
    async fn get_current_epoch(&self) -> RpcResult<EpochInfo>;

    /// Return Network metrics
    #[method(name = "getNetworkMetrics")]
    async fn get_network_metrics(&self) -> RpcResult<NetworkMetrics>;

    /// Return move call metrics
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
