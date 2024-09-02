// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example uses the read api to showcase the available
//! functions that aren't covered in the read_api.rs example.
//!
//! cargo run --example read_api_tx

use iota_json_rpc_types::{IotaObjectDataOptions, IotaTransactionBlockResponseOptions};
use iota_sdk::IotaClientBuilder;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let client = IotaClientBuilder::default().build_testnet().await?;

    let object = client
        .read_api()
        .get_object_with_options(
            "0x5".parse()?,
            IotaObjectDataOptions::new().with_previous_transaction(),
        )
        .await?;
    println!("{object:?}");
    let tx_id = object.data.unwrap().previous_transaction.unwrap();

    let move_package = client.read_api().get_loaded_child_objects(tx_id).await?;
    println!("Move package: {move_package:?}");

    let tx_response = client
        .read_api()
        .get_transaction_with_options(tx_id, IotaTransactionBlockResponseOptions::new())
        .await?;
    println!("Tx: {tx_response:?}");

    Ok(())
}
