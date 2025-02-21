// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_open_rpc_macros::open_rpc;
use iota_types::bridge::BridgeSummary;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};

#[open_rpc(namespace = "iotax", tag = "Bridge Read API")]
#[rpc(server, client, namespace = "iotax")]
pub trait BridgeReadApi {
    /// Returns the latest BridgeSummary
    #[method(name = "getLatestBridge")]
    async fn get_latest_bridge(&self) -> RpcResult<BridgeSummary>;

    /// Returns the initial shared version of the bridge object, usually
    /// for the purpose of constructing an ObjectArg in a transaction.
    #[method(name = "getBridgeObjectInitialSharedVersion")]
    async fn get_bridge_object_initial_shared_version(&self) -> RpcResult<u64>;
}
