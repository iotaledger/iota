// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Creating a genesis blob out of a remote stardust objects snapshots.

use iota_genesis_builder::{
    Builder, SnapshotUrl, IOTA_OBJECT_SNAPSHOT_URL, SHIMMER_OBJECT_SNAPSHOT_URL,
};
use iota_swarm_config::genesis_config::ValidatorGenesisConfigBuilder;
use rand::rngs::OsRng;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

fn main() -> anyhow::Result<()> {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    info!("Reading IOTA snapshot from {}", IOTA_OBJECT_SNAPSHOT_URL);
    let iota_snapshot_reader = SnapshotUrl::Iota.to_reader()?;

    info!(
        "Reading Shimmer snapshot from {}",
        SHIMMER_OBJECT_SNAPSHOT_URL
    );
    let shimmer_snapshot_reader = SnapshotUrl::Shimmer.to_reader()?;

    // Start building
    info!("Building the genesis..");
    let mut builder = Builder::new()
        .add_migration_objects(iota_snapshot_reader)?
        .add_migration_objects(shimmer_snapshot_reader)?;

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

    info!("Genesis built successfully");

    Ok(())
}
