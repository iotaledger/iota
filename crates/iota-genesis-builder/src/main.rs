// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Creating a stardust objects snapshot out of a Hornet snapshot.
use std::fs::File;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use sui_genesis_builder::stardust::{migration::Migration, parse::FullSnapshotParser};

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
    let object_snapshot = File::create("stardust_object_snapshot.bin")?;

    // Start the Hornet snapshot parser
    let parser = FullSnapshotParser::new(file)?;

    // Run the migration using the parser output stream
    Migration::new(parser.header.target_milestone_timestamp())?.run(
        parser.outputs().collect::<Result<Vec<_>, _>>()?.into_iter(),
        object_snapshot,
    )?;
    info!("Snapshot file written.");
    Ok(())
}
