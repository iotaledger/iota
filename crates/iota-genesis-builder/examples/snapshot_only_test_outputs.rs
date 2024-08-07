// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Example to create a new full snapshot with only test outputs and sample
//! outputs coming from a previous one.

use std::{fs::File, path::Path};

use clap::{Parser, Subcommand};
use iota_genesis_builder::{
    stardust::{
        parse::HornetSnapshotParser,
        test_outputs::{add_snapshot_test_outputs, to_micros, STARDUST_TOTAL_SUPPLY_SHIMMER_MICRO},
    },
    IF_STARDUST_ADDRESS,
};
use iota_sdk::types::block::address::Address;
use iota_types::{gas_coin::STARDUST_TOTAL_SUPPLY_IOTA, stardust::coin_type::CoinType};

pub const IF_SHIMMER_STARDUST_ADDRESS: &str =
    "smr1qqzhsp9x3m22l55wlclawlw25536e3rgghep77awcfrgh60uxhxuq6vlak7";

const WITH_SAMPLING: bool = false;

#[derive(Parser, Debug)]
#[clap(about = "Tool for generating Iota and Shimmer Hornet full-snapshot files with test data")]
struct Cli {
    #[clap(subcommand)]
    snapshot: Snapshot,
}

#[derive(Subcommand, Debug)]
enum Snapshot {
    #[clap(about = "Parse an Iota Hornet full-snapshot file")]
    Iota {
        #[clap(long, help = "Path to the Iota Hornet full-snapshot file")]
        snapshot_path: String,
    },
    #[clap(about = "Parse a Shimmer Hornet full-snapshot file")]
    Shimmer {
        #[clap(long, help = "Path to the Shimmer Hornet full-snapshot file")]
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
        CoinType::Iota => to_micros(STARDUST_TOTAL_SUPPLY_IOTA),
        CoinType::Shimmer => STARDUST_TOTAL_SUPPLY_SHIMMER_MICRO,
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
        Snapshot::Shimmer { snapshot_path } => (snapshot_path, CoinType::Shimmer),
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

    let mut randomness_seed = 0;
    let mut delegator_address = "";
    match coin_type {
        CoinType::Iota => {
            // IOTA coin type values
            randomness_seed = 0;
            delegator_address = IF_STARDUST_ADDRESS;
        }
        CoinType::Shimmer => {
            // Shimmer coin type values
            randomness_seed = 1;
            delegator_address = IF_SHIMMER_STARDUST_ADDRESS;
        }
    }

    add_snapshot_test_outputs::<false>(
        &current_path,
        &new_path,
        coin_type,
        randomness_seed,
        *Address::try_from_bech32(delegator_address)?.as_ed25519(),
        WITH_SAMPLING,
    )
    .await?;

    parse_snapshot::<false>(&new_path, coin_type)?;

    Ok(())
}
