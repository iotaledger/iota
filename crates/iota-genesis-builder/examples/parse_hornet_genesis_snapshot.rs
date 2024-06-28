// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Example demonstrating parsing Stardust UTXOs from a snapshot file
//! and verifying the total supply.
use std::fs::File;

use iota_genesis_builder::stardust::parse::HornetSnapshotParser;
use iota_types::gas_coin::TOTAL_SUPPLY_IOTA;

fn main() -> anyhow::Result<()> {
    let Some(path) = std::env::args().nth(1) else {
        anyhow::bail!("please provide path to the Hornet genesis snapshot file");
    };
    let file = File::open(path)?;

    let parser = HornetSnapshotParser::new(file)?;
    println!("Output count: {}", parser.header.output_count());

    let total_supply = parser.outputs().try_fold(0, |acc, output| {
        Ok::<_, anyhow::Error>(acc + output?.1.amount())
    })?;
    // Total supply is in IOTA, snapshot supply is Micros
    assert_eq!(total_supply, TOTAL_SUPPLY_IOTA * 1_000_000);
    println!("Total supply: {total_supply}");
    Ok(())
}
