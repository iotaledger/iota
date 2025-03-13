// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example connects to the IOTA testnet and collects information about the
//! stakes in the network, the committee information, lists all the validators'
//! name, description, and iota address, and prints the reference gas price.
//!
//! cargo run --example governance_api

mod utils;

use iota_types::iota_system_state::iota_system_state_summary::IotaSystemStateSummary;
use utils::setup_for_read;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let (client, active_address) = setup_for_read().await?;

    // Stakes
    let stakes = client.governance_api().get_stakes(active_address).await?;

    println!(" *** Stakes ***");
    println!("{:?}", stakes);
    println!(" *** Stakes ***\n");

    // Committee Info
    let committee = client.governance_api().get_committee_info(None).await?; // None defaults to the latest epoch

    println!(" *** Committee Info ***");
    println!("{:?}", committee);
    println!(" *** Committee Info ***\n");

    // Latest IOTA System State
    let iota_system_state = client
        .governance_api()
        .get_latest_iota_system_state()
        .await?;

    println!(" *** IOTA System State ***");
    println!("{:?}", iota_system_state);
    println!(" *** IOTA System State ***\n");

    // List all active validators

    let active_validators = match iota_system_state {
        IotaSystemStateSummary::V1(v1) => v1.active_validators,
        IotaSystemStateSummary::V2(v2) => v2.active_validators,
        _ => panic!("unsupported IotaSystemStateSummary"),
    };

    println!(" *** List active validators *** ");
    active_validators.into_iter().for_each(|validator| {
        println!(
            "Name: {}, Description: {}, IotaAddress: {:?}",
            validator.name, validator.description, validator.iota_address
        )
    });

    println!(" *** List active validators ***\n");
    // Reference Gas Price
    let reference_gas_price = client.governance_api().get_reference_gas_price().await?;

    println!(" *** Reference Gas Price ***");
    println!("{:?}", reference_gas_price);
    println!(" *** Reference Gas Price ***\n");

    Ok(())
}
