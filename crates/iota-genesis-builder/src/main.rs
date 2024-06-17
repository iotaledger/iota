// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Creating a stardust objects snapshot out of a Hornet snapshot.
//! TIP that defines the Hornet snapshot file format:
//! https://github.com/iotaledger/tips/blob/main/tips/TIP-0035/tip-0035.md
use std::{fs::File, str::FromStr};

use iota_genesis_builder::{
    stardust::{
        migration::{Migration, MigrationTargetNetwork},
        parse::FullSnapshotParser,
    },
    BROTLI_COMPRESSOR_BUFFER_SIZE, BROTLI_COMPRESSOR_LG_WINDOW_SIZE, BROTLI_COMPRESSOR_QUALITY,
    OBJECT_SNAPSHOT_FILE_PATH,
};
use iota_types::{gas_coin::GAS, smr::SMR};
use itertools::Itertools;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

fn main() -> anyhow::Result<()> {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    // Prepare files
    let Some(iota_snapshot_path) = std::env::args().nth(1) else {
        anyhow::bail!("please provide path to the Hornet full-snapshot file");
    };
    let Some(shimmer_snapshot_path) = std::env::args().nth(2) else {
        anyhow::bail!("please provide path to the Hornet full-snapshot file");
    };
    let target_network = std::env::args()
        .nth(3)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "please provide the target network for which the snapshot is being generated ('{}', '{}' or '{}')",
                MigrationTargetNetwork::Mainnet,
                MigrationTargetNetwork::Testnet("(optional-string)".to_owned()),
                MigrationTargetNetwork::Alphanet("(optional-string)".to_owned()),
            )
        })
        .and_then(|target_network_str| MigrationTargetNetwork::from_str(&target_network_str))?;

    // Start the Hornet snapshot parser
    let iota_snapshot_parser = FullSnapshotParser::new(File::open(iota_snapshot_path)?)?;
    let shimmer_snapshot_parser = FullSnapshotParser::new(File::open(shimmer_snapshot_path)?)?;

    // Use the target milestone timestamp and total supply from the iota snapshot.
    let target_milestone_timestamp = iota_snapshot_parser.target_milestone_timestamp();
    let total_supply = iota_snapshot_parser.total_supply()?;

    let chained_parser = iota_snapshot_parser
        .outputs()
        .map(|result| result.map(|(header, output)| (header, output, GAS::type_tag())))
        .chain(
            shimmer_snapshot_parser
                .outputs()
                .map(|result| result.map(|(header, output)| (header, output, SMR::type_tag()))),
        );

    // Prepare the migration using the parser output stream
    let migration = Migration::new(
        target_milestone_timestamp,
        total_supply,
        target_network.clone(),
    )?;

    // Prepare the compressor writer for the objects snapshot
    let object_snapshot_writer = brotli::CompressorWriter::new(
        File::create(OBJECT_SNAPSHOT_FILE_PATH)?,
        BROTLI_COMPRESSOR_BUFFER_SIZE,
        BROTLI_COMPRESSOR_QUALITY,
        BROTLI_COMPRESSOR_LG_WINDOW_SIZE,
    );

    // Run the migration and write the objects snapshot
    chained_parser.process_results(|outputs| migration.run(outputs, object_snapshot_writer))??;

    Ok(())
}
