// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Creating a genesis vanilla blob.

use iota_config::genesis::TokenDistributionScheduleBuilder;
use iota_genesis_builder::Builder;
use iota_swarm_config::genesis_config::ValidatorGenesisConfigBuilder;
use rand::rngs::OsRng;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

fn main() -> anyhow::Result<()> {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let mut builder = Builder::new();
    let mut key_pairs = Vec::new();
    let mut rng = OsRng;
    let mut validators = vec![];
    for i in 0..4 {
        let validator_config = ValidatorGenesisConfigBuilder::default().build(&mut rng);
        let validator_info = validator_config.to_validator_info(format!("validator-{i}"));
        let validator_addr = validator_info.info.iota_address();
        builder = builder.add_validator(validator_info.info, validator_info.proof_of_possession);
        key_pairs.push(validator_config.key_pair);
        validators.push(validator_addr);
    }
    for key in &key_pairs {
        builder = builder.add_validator_signature(key);
    }
    let mut schedule = TokenDistributionScheduleBuilder::new();
    schedule.default_allocation_for_validators(validators);
    let builder = builder.with_token_distribution_schedule(schedule.build());
    let genesis = builder.build();
    genesis.save("genesis-vanilla.blob")?;
    Ok(())
}
