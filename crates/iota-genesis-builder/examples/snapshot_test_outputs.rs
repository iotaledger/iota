// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Example to add test outputs to a full snapshot.

use std::{fs::File, path::Path};

use iota_genesis_builder::stardust::{
    parse::HornetSnapshotParser, test_outputs::add_snapshot_test_outputs,
};
use iota_types::gas_coin::TOTAL_SUPPLY_IOTA;

fn parse_snapshot<const VERIFY: bool>(path: impl AsRef<Path>) -> anyhow::Result<()> {
    let file = File::open(path)?;
    let mut parser = HornetSnapshotParser::new::<VERIFY>(file)?;

    println!("Output count: {}", parser.header.output_count());

    let total_supply = parser.outputs().try_fold(0, |acc, output| {
        Ok::<_, anyhow::Error>(acc + output?.1.amount())
    })?;

    // Total supply is in IOTA, snapshot supply is Nanos
    assert_eq!(total_supply, TOTAL_SUPPLY_IOTA * 1_000_000);

    println!("Total supply: {total_supply}");

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let Some(current_path) = std::env::args().nth(1) else {
        anyhow::bail!("please provide path to the full-snapshot file");
    };
    let mut new_path = String::from("test-");
    // prepend "test-" before the file name
    if let Some(pos) = current_path.rfind('/') {
        let mut current_path = current_path.clone();
        current_path.insert_str(pos + 1, &new_path);
        new_path = current_path;
    } else {
        new_path.push_str(&current_path);
    }

    parse_snapshot::<false>(&current_path)?;

    add_snapshot_test_outputs::<false>(&current_path, &new_path).await?;

    parse_snapshot::<false>(&new_path)?;

    Ok(())
}
