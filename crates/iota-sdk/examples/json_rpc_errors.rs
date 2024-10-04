// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how one can convert RPC errors for further manual
//! handling.
//!
//! cargo run --example json_rpc_errors

mod utils;

use anyhow::bail;
use iota_sdk::error::{Error, JsonRpcError};
use utils::setup_for_read;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let (client, active_address) = setup_for_read().await?;
    let coin_type = Some("0x42".to_string());
    let coins = client
        .coin_read_api()
        .get_coins(active_address, coin_type.clone(), None, Some(5))
        .await;
    let error = coins.unwrap_err();
    if let Error::Rpc(rpc_error) = error {
        let converted: JsonRpcError = rpc_error.into();
        println!(" *** RpcError ***");
        println!("{converted}");
        println!("{}", converted.is_client_error());
    } else {
        bail!("Expected Error::Rpc, got {:?}", error);
    }
    Ok(())
}
