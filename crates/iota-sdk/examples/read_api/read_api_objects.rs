// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example gets objects in various ways.
//!
//! cargo run --example read_api_objects

use iota_sdk::{IotaClientBuilder, rpc_types::IotaObjectDataOptions};

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
    println!("Object with previous transaction: {object:?}");
    let object_id = object.data.as_ref().unwrap().object_id;

    let object_bcs = client.read_api().get_move_object_bcs(object_id).await?;
    println!("Object bcs: {object_bcs:?}");

    let objects = client
        .read_api()
        .multi_get_object_with_options(vec![object_id], IotaObjectDataOptions::default().with_bcs())
        .await?;
    println!("Objects: {objects:?}");

    Ok(())
}
