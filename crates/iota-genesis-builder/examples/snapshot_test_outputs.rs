// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Example to add test outputs to a full snapshot.

use std::{fs::File, path::Path};

use iota_genesis_builder::stardust::{
    parse::FullSnapshotParser, test_outputs::add_snapshot_test_outputs,
    types::output_header::TOTAL_SUPPLY_IOTA,
};

fn parse_snapshot<P: AsRef<Path>>(path: P) -> anyhow::Result<()> {
    let file = File::open(path)?;
    let parser = FullSnapshotParser::new(file)?;

    println!("Output count: {}", parser.header.output_count());

    let total_supply = parser.outputs().try_fold(0, |acc, output| {
        Ok::<_, anyhow::Error>(acc + output?.1.amount())
    })?;

    assert_eq!(total_supply, TOTAL_SUPPLY_IOTA);

    println!("Total supply: {total_supply}");

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let Some(current_path) = std::env::args().nth(1) else {
        anyhow::bail!("please provide path to the full-snapshot file");
    };
    let mut new_path = String::from("test-");
    new_path.push_str(&current_path);

    parse_snapshot(&current_path)?;

    add_snapshot_test_outputs(&current_path, &new_path).await?;

    parse_snapshot(&new_path)?;

    Ok(())
}
