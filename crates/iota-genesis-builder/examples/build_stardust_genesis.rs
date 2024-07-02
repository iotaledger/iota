// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Creating a genesis blob out of a local stardust objects snapshot.

use std::{
    fs::File,
    io::{BufReader, Read},
};

use clap::Parser;
use iota_genesis_builder::{Builder, BROTLI_COMPRESSOR_BUFFER_SIZE, OBJECT_SNAPSHOT_FILE_PATH};
use iota_swarm_config::genesis_config::ValidatorGenesisConfigBuilder;
use rand::rngs::OsRng;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[clap(
    about = "Example Tool for generating a genesis file from a Stardust Migration Objects snapshot"
)]
struct Cli {
    #[clap(
        short,
        long,
        default_value_t = false,
        help = "Decompress the input object snapshot"
    )]
    decompress: bool,
    #[clap(long, default_value_t = OBJECT_SNAPSHOT_FILE_PATH.to_string(), help = "Path to the Stardust Migration Objects snapshot file")]
    snapshot_path: String,
}

fn main() -> anyhow::Result<()> {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    // Parse the CLI arguments
    let cli = Cli::parse();

    // Prepare the reader for the objects snapshot
    let input_file = File::open(cli.snapshot_path)?;
    let object_snapshot_reader: Box<dyn Read> = if cli.decompress {
        Box::new(brotli::Decompressor::new(
            input_file,
            BROTLI_COMPRESSOR_BUFFER_SIZE,
        ))
    } else {
        Box::new(BufReader::new(input_file))
    };

    // Start building
    let mut builder = Builder::new().add_migration_objects(object_snapshot_reader)?;
    let mut key_pairs = Vec::new();
    let mut rng = OsRng;
    for i in 0..4 {
        let validator_config = ValidatorGenesisConfigBuilder::default().build(&mut rng);
        let validator_info = validator_config.to_validator_info(format!("validator-{i}"));
        builder = builder.add_validator(validator_info.info, validator_info.proof_of_possession);
        key_pairs.push(validator_config.key_pair);
    }
    for key in &key_pairs {
        builder = builder.add_validator_signature(key);
    }
    let _genesis = builder.build();
    println!("{:?}", _genesis);
    Ok(())
}
