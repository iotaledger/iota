// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Creating a genesis blob out of a local stardust objects snapshot.

use std::path::PathBuf;

use clap::Parser;
use iota_config::genesis::TokenDistributionScheduleBuilder;
use iota_genesis_builder::{
    Builder, OBJECT_SNAPSHOT_FILE_PATH, SnapshotSource, genesis_build_effects::GenesisBuildEffects,
};
use iota_swarm_config::genesis_config::ValidatorGenesisConfigBuilder;
use rand::rngs::OsRng;

#[derive(Parser, Debug)]
#[command(
    about = "Example Tool for generating a genesis file from a Stardust Migration Objects snapshot"
)]
struct Cli {
    #[arg(long, default_value_t = OBJECT_SNAPSHOT_FILE_PATH.to_string(), help = "Path to the Stardust Migration Objects snapshot file")]
    snapshot_path: String,
}

fn main() -> anyhow::Result<()> {
    // Create the builder
    // Parse the CLI arguments
    let cli = Cli::parse();

    // Prepare the reader for the objects snapshot
    let path = PathBuf::from(cli.snapshot_path);
    let object_snapshot_source = SnapshotSource::Local(path);

    // Start building
    let mut builder = Builder::new().add_migration_source(object_snapshot_source);

    // Create validators
    let mut validators = Vec::new();
    let mut key_pairs = Vec::new();
    let mut rng = OsRng;
    for i in 0..4 {
        let validator_config = ValidatorGenesisConfigBuilder::default().build(&mut rng);
        let validator_info = validator_config.to_validator_info(format!("validator-{i}"));
        let validator_addr = validator_info.info.iota_address();
        validators.push(validator_addr);
        key_pairs.push(validator_config.authority_key_pair);
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

    let GenesisBuildEffects {
        genesis,
        migration_tx_data,
    } = builder.build();
    // Save to file
    genesis.save("genesis.blob")?;
    migration_tx_data.unwrap().save("migration.blob")?;
    Ok(())
}
