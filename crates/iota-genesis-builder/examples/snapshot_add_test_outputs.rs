// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Example to add test outputs to a full snapshot.

use std::{fs::File, path::Path};

use clap::{Parser, Subcommand};
use iota_genesis_builder::stardust::{
    parse::HornetSnapshotParser,
    test_outputs::{add_snapshot_test_outputs, to_nanos},
};
use iota_types::{gas_coin::STARDUST_TOTAL_SUPPLY_IOTA, stardust::coin_type::CoinType};

#[derive(Parser, Debug)]
#[clap(about = "Tool for adding test data to IOTA Hornet full-snapshot")]
struct Cli {
    #[clap(subcommand)]
    snapshot: Snapshot,
}

#[derive(Subcommand, Debug)]
enum Snapshot {
    #[clap(about = "Parse an IOTA Hornet full-snapshot file")]
    Iota {
        #[clap(long, help = "Path to the IOTA Hornet full-snapshot file")]
        snapshot_path: String,
    },
}

fn parse_snapshot<const VERIFY: bool>(
    path: impl AsRef<Path>,
    coin_type: CoinType,
) -> anyhow::Result<()> {
    let file = File::open(path)?;
    let mut parser = HornetSnapshotParser::new::<VERIFY>(file)?;

    println!("Output count: {}", parser.header.output_count());

    let total_supply = parser.outputs().try_fold(0, |acc, output| {
        Ok::<_, anyhow::Error>(acc + output?.1.amount())
    })?;

    let expected_total_supply = match coin_type {
        CoinType::Iota => to_nanos(STARDUST_TOTAL_SUPPLY_IOTA),
    };

    assert_eq!(total_supply, expected_total_supply);

    println!("Total supply: {total_supply}");

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let (current_path, coin_type) = match cli.snapshot {
        Snapshot::Iota { snapshot_path } => (snapshot_path, CoinType::Iota),
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

    parse_snapshot::<false>(&current_path, coin_type)?;

    let randomness_seed = match coin_type {
        CoinType::Iota => 0,
    };
    add_snapshot_test_outputs::<false>(
        &current_path,
        &new_path,
        coin_type,
        randomness_seed,
        None,
        false,
    )
    .await?;

    parse_snapshot::<false>(&current_path, coin_type)?;

    Ok(())
}
