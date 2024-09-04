// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example gets objects in various ways.
//!
//! cargo run --example read_objects

use iota_json_rpc_types::IotaObjectDataOptions;
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
    let object_id = object.data.as_ref().unwrap().object_id;

    let object_bcs = client.read_api().get_move_object_bcs(object_id).await?;
    println!("Objects: {object_bcs:?}");

    let loaded_child_objects = client
        .read_api()
        .get_loaded_child_objects(object.data.unwrap().previous_transaction.unwrap())
        .await?;
    println!("Loaded child objects: {loaded_child_objects:?}");

    let objects = client
        .read_api()
        .multi_get_object_with_options(vec![object_id], IotaObjectDataOptions::default().with_bcs())
        .await?;
    println!("Objects: {objects:?}");

    Ok(())
}
