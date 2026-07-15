// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how to create a move view function call using the
//! transaction builder.
//!
//! Only public functions can be called as view functions. For modules that
//! record on-chain view functions metadata, the called function must
//! additionally be declared with the `#[view]` attribute; modules without
//! such metadata (like the framework packages below) fall back to signature
//! checks.
//!
//! cargo run --example move_view_function_call

#[path = "../utils.rs"]
mod utils;

use iota_json::IotaJsonValue;
use iota_json_rpc_types::{DevInspectResults, IotaTypeTag};
use iota_sdk::IotaClient;
use iota_sdk_types::{Address, ObjectId};
use serde_json::json;
use utils::setup_for_read;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let (client, sender) = setup_for_read().await?;

    // Move view function call to a public function: get the current timestamp in
    // milliseconds.
    let public_call_results = move_view_function_dev_inspect(
        sender,
        &client,
        "0x2".parse()?,
        "clock",
        "timestamp_ms",
        vec![],
        vec![IotaJsonValue::new(json!(ObjectId::CLOCK))?],
    )
    .await?
    .results;
    println!("{public_call_results:?}");

    Ok(())
}

async fn move_view_function_dev_inspect(
    sender: Address,
    client: &IotaClient,
    package_id: ObjectId,
    module_name: &str,
    function_name: &str,
    type_args: Vec<IotaTypeTag>,
    args: Vec<IotaJsonValue>,
) -> Result<DevInspectResults, anyhow::Error> {
    let pt = client
        .transaction_builder()
        .move_view_call_tx_kind(package_id, module_name, function_name, type_args, args)
        .await?;

    Ok(client
        .read_api()
        .dev_inspect_transaction_block(sender, pt, None, None, None)
        .await?)
}
