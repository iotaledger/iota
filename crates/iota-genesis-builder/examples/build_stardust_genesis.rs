// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Creating a genesis blob out of a local stardust objects snapshot.

use iota_config::genesis::TokenDistributionScheduleBuilder;
use iota_genesis_builder::{Builder, OBJECT_SNAPSHOT_FILE_PATH};
use iota_swarm_config::genesis_config::ValidatorGenesisConfigBuilder;
use rand::rngs::OsRng;

fn main() -> anyhow::Result<()> {
    // Create the builder
    let mut builder = Builder::new().load_stardust_migration_objects(OBJECT_SNAPSHOT_FILE_PATH)?;

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
