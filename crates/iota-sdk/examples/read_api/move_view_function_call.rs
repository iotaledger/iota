// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how to create a move view function call using the
//! transaction builder.
//!
//! cargo run --example move_view_function_call

#[path = "../utils.rs"]
mod utils;

use iota_json::IotaJsonValue;
use iota_json_rpc_types::{DevInspectResults, IotaTypeTag};
use iota_sdk::IotaClient;
use iota_types::base_types::{IotaAddress, ObjectID};
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
        vec![IotaJsonValue::new(json!(iota_types::IOTA_CLOCK_OBJECT_ID))?],
    )
    .await?
    .results;
    println!("{public_call_results:?}");

    // Move view function call to a private function: get the current timestamp in
    // milliseconds.
    let private_call_results = move_view_function_dev_inspect(
        sender,
        &client,
        "0x2".parse()?,
        "random",
        "load_inner",
        vec![],
        vec![IotaJsonValue::new(json!(
            iota_types::IOTA_RANDOMNESS_STATE_OBJECT_ID
        ))?],
    )
    .await?
    .results;
    println!("{private_call_results:?}");

    Ok(())
}

async fn move_view_function_dev_inspect(
    sender: IotaAddress,
    client: &IotaClient,
    package_id: ObjectID,
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
