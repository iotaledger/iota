// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Creating a stardust objects snapshot out of a Hornet snapshot.
//! TIP that defines the Hornet snapshot file format:
//! https://github.com/iotaledger/tips/blob/main/tips/TIP-0035/tip-0035.md
use std::{fs::File, io::BufWriter};

use anyhow::Result;
use clap::{Parser, Subcommand};
use iota_genesis_builder::{
    OBJECT_SNAPSHOT_FILE_PATH,
    stardust::{
        migration::{Migration, MigrationTargetNetwork},
        parse::HornetSnapshotParser,
        process_outputs::scale_amount_for_iota,
        types::address_swap_map::AddressSwapMap,
    },
};
use iota_types::stardust::coin_type::CoinType;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[clap(about = "Tool for migrating Iota Hornet full-snapshot file")]
struct Cli {
    #[clap(subcommand)]
    snapshot: Snapshot,
    #[clap(long, help = "Disable global snapshot verification")]
    disable_global_snapshot_verification: bool,
}

#[derive(Subcommand, Debug)]
enum Snapshot {
    #[clap(about = "Migrate an Iota Hornet full-snapshot file")]
    Iota {
        #[clap(long, help = "Path to the Iota Hornet full-snapshot file")]
        snapshot_path: String,
        #[clap(
            long,
            help = "Path to the address swap map file. This must be a CSV file with two columns, where an entry contains in the first column an IotaAddress present in the Hornet full-snapshot and in the second column an IotaAddress that will be used for the swap."
        )]
        address_swap_map_path: Option<String>,
        #[clap(long, value_parser = clap::value_parser!(MigrationTargetNetwork), help = "Target network for migration")]
        target_network: MigrationTargetNetwork,
    },
}

fn main() -> Result<()> {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    // Parse the CLI arguments
    let cli = Cli::parse();
    let (snapshot_path, address_swap_map_path, target_network, coin_type) = match cli.snapshot {
        Snapshot::Iota {
            snapshot_path,
            address_swap_map_path,
            target_network,
        } => (
            snapshot_path,
            address_swap_map_path,
            target_network,
            CoinType::Iota,
        ),
    };

    // Start the Hornet snapshot parser
    let mut snapshot_parser = if cli.disable_global_snapshot_verification {
        HornetSnapshotParser::new::<false>(File::open(snapshot_path)?)?
    } else {
        HornetSnapshotParser::new::<true>(File::open(snapshot_path)?)?
    };
    let total_supply = match coin_type {
        CoinType::Iota => scale_amount_for_iota(snapshot_parser.total_supply()?)?,
    };

    let address_swap_map = if let Some(address_swap_map_path) = address_swap_map_path {
        AddressSwapMap::from_csv(&address_swap_map_path)?
    } else {
        AddressSwapMap::default()
    };
    // Prepare the migration using the parser output stream
    let migration = Migration::new(
        snapshot_parser.target_milestone_timestamp(),
        total_supply,
        target_network,
        coin_type,
        address_swap_map,
    )?;

    // Prepare the writer for the objects snapshot
    let output_file = File::create(OBJECT_SNAPSHOT_FILE_PATH)?;
    let object_snapshot_writer = BufWriter::new(output_file);

    match coin_type {
        CoinType::Iota => {
            migration.run_for_iota(
                snapshot_parser.target_milestone_timestamp(),
                snapshot_parser.outputs(),
                object_snapshot_writer,
            )?;
        }
    }

    Ok(())
}
