// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Creating a genesis blob out of a local stardust objects snapshot.

use std::{
    fs::File,
    io::{BufReader, Read},
};

use clap::Parser;
use iota_config::genesis::TokenDistributionScheduleBuilder;
use iota_genesis_builder::{Builder, BROTLI_COMPRESSOR_BUFFER_SIZE, OBJECT_SNAPSHOT_FILE_PATH};
use iota_swarm_config::genesis_config::ValidatorGenesisConfigBuilder;
use rand::rngs::OsRng;

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
    // Create the builder
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

    // Create validators
    let mut validators = Vec::new();
    let mut key_pairs = Vec::new();
    let mut rng = OsRng;
    for i in 0..4 {
        let validator_config = ValidatorGenesisConfigBuilder::default().build(&mut rng);
        let validator_info = validator_config.to_validator_info(format!("validator-{i}"));
        let validator_addr = validator_info.info.iota_address();
        validators.push(validator_addr);
        key_pairs.push(validator_config.key_pair);
        builder = builder.add_validator(validator_info.info, validator_info.proof_of_possession);
    }

    // Custom TokenDistributionSchedule
    let mut schedule = TokenDistributionScheduleBuilder::new();
    schedule.default_allocation_for_validators(validators.clone());
    builder = builder.with_token_distribution_schedule(schedule.build());

    // Add keys (builds it for the first time)
    for key in &key_pairs {
        builder = builder.add_validator_signature(key);
    }

    // Save to file
    builder.build().save("genesis.blob")?;
    Ok(())
}
