// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Example to add test outputs to a full snapshot.

use std::{fs::File, path::Path};

use iota_genesis_builder::stardust::{
    parse::FullSnapshotParser, test_outputs::add_snapshot_test_outputs,
    types::output_header::TOTAL_SUPPLY_IOTA,
};

fn parse_snapshot(path: impl AsRef<Path>) -> anyhow::Result<()> {
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

fn main() -> anyhow::Result<()> {
    let Some(current_path) = std::env::args().nth(1) else {
        anyhow::bail!("please provide path to the full-snapshot file");
    };
    parse_snapshot(&current_path)?;

    let new_path = format!("test-{current_path}");
    println!("{new_path}");

    add_snapshot_test_outputs(&current_path, &new_path)?;

    parse_snapshot(&new_path)?;

    Ok(())
}
