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
    let Some(stardust_snapshot_path) = std::env::args().nth(1) else {
        anyhow::bail!("please provide path to the Hornet full-snapshot file");
    };
    let target_network = std::env::args()
        .nth(2)
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
    let stardust_snapshot_file = File::open(stardust_snapshot_path)?;
    let parser = FullSnapshotParser::new(stardust_snapshot_file)?;

    // Prepare the migration using the parser output stream
    let migration = Migration::new(
        parser.target_milestone_timestamp(),
        parser.total_supply()?,
        target_network,
    )?;

    // Prepare the compressor writer for the objects snapshot
    let object_snapshot_writer = brotli::CompressorWriter::new(
        File::create(OBJECT_SNAPSHOT_FILE_PATH)?,
        BROTLI_COMPRESSOR_BUFFER_SIZE,
        BROTLI_COMPRESSOR_QUALITY,
        BROTLI_COMPRESSOR_LG_WINDOW_SIZE,
    );

    // Run the migration and write the objects snapshot
    parser
        .outputs()
        .process_results(|outputs| migration.run(outputs, object_snapshot_writer))??;
    Ok(())
}
