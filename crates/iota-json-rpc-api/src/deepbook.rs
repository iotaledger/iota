// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;

use iota_open_rpc_macros::open_rpc;

#[open_rpc(namespace = "iotax", tag = "DeepBook Read API")]
#[rpc(server, client, namespace = "iotax")]
pub trait DeepBookApi {
    #[method(name = "ping")]
    async fn ping(&self) -> RpcResult<String>;
}
