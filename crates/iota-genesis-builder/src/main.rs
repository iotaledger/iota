// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Creating a stardust objects snapshot out of a Hornet snapshot.
//! TIP that defines the Hornet snapshot file format:
//! https://github.com/iotaledger/tips/blob/main/tips/TIP-0035/tip-0035.md
use std::fs::File;

use iota_genesis_builder::stardust::{migration::Migration, parse::FullSnapshotParser};
use itertools::Itertools;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

const OBJECT_SNAPSHOT_FILE_PATH: &str = "stardust_object_snapshot.bin";
const BROTLI_COMPRESSOR_BUFFER_SIZE: usize = 4096;
const BROTLI_COMPRESSOR_QUALITY: u32 = 11; // Compression levels go from 0 to 11, where 11 has the highest compression ratio but requires more time
const BROTLI_COMPRESSOR_LG_WINDOW_SIZE: u32 = 22; // set LZ77 window size (0, 10-24) where bigger windows size improves density

fn main() -> anyhow::Result<()> {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    // Prepare files
    let Some(path) = std::env::args().nth(1) else {
        anyhow::bail!("please provide path to the Hornet full-snapshot file");
    };
    let file = File::open(path)?;

    // Start the Hornet snapshot parser
    let parser = FullSnapshotParser::new(file)?;

    // Prepare the migration using the parser output stream
    let migration = Migration::new(parser.header.target_milestone_timestamp())?;
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
