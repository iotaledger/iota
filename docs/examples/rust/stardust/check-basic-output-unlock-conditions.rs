// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Example demonstrating queries for checking the unlock conditions of a basic
//! output. In order to work, it requires a network with test objects
//! generated from iota-genesis-builder/src/stardust/test_outputs.

use anyhow::anyhow;
use iota_sdk::{
    IotaClientBuilder,
    rpc_types::{IotaData, IotaObjectDataOptions},
    types::{base_types::ObjectID, stardust::output::BasicOutput},
};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Build an iota client for a local network
    let iota_client = IotaClientBuilder::default().build_localnet().await?;

    // This object id was fetched manually. It refers to a Basic Output object that
    // contains some Native Tokens.
    let basic_output_object_id = ObjectID::from_hex_literal(
        "0xde09139ed46b9f5f876671e4403f312fad867c5ae5d300a252e4b6a6f1fa1fbd",
    )?;
    // Get Basic Output object
    let basic_output_object = iota_client
        .read_api()
        .get_object_with_options(
            basic_output_object_id,
            IotaObjectDataOptions::new().with_bcs(),
        )
        .await?
        .data
        .ok_or(anyhow!("Basic output not found"))?;

    // Convert the basic output object into its Rust representation
    let basic_output = bcs::from_bytes::<BasicOutput>(
        &basic_output_object
            .bcs
            .expect("should contain bcs")
            .try_as_move()
            .expect("should convert it to a move object")
            .bcs_bytes,
    )?;

    println!("Basic Output infos: {basic_output:?}");

    if let Some(sdruc) = basic_output.storage_deposit_return {
        println!("Storage Deposit Return Unlock Condition infos: {sdruc:?}");
    }
    if let Some(tuc) = basic_output.timelock {
        println!("Timelocked until: {}", tuc.unix_time);
    }
    if let Some(euc) = basic_output.expiration {
        println!("Expiration Unlock Condition infos: {euc:?}");
    }

    Ok(())
}
