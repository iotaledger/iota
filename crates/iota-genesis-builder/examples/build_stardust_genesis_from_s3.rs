// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Creating a genesis blob out of a remote stardust objects snapshots.

use clap::Parser;
use iota_genesis_builder::{Builder, SnapshotSource, SnapshotUrl};
use iota_swarm_config::genesis_config::ValidatorGenesisConfigBuilder;
use rand::rngs::OsRng;
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[command(about = "Example that builds genesis in a temp folder with migration snapshots")]
struct Args {
    /// Remotely stored migration snapshots.
    #[arg(
        long,
        name = "iota|<full-url>",
        help = "Remote migration snapshots.",
        default_values_t = vec![SnapshotUrl::Iota],
    )]
    #[arg(num_args(0..))]
    migration_snapshots: Vec<SnapshotUrl>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let args = Args::parse();
    let migration_sources = args.migration_snapshots.into_iter().map(SnapshotSource::S3);

    // Start building
    info!("Building the genesis..");
    let mut builder = Builder::new();

    for migration_source in migration_sources {
        builder = builder.add_migration_source(migration_source);
    }

    let mut key_pairs = Vec::new();
    let mut rng = OsRng;
    for i in 0..4 {
        let validator_config = ValidatorGenesisConfigBuilder::default().build(&mut rng);
        let validator_info = validator_config.to_validator_info(format!("validator-{i}"));
        builder = builder.add_validator(validator_info.info, validator_info.proof_of_possession);
        key_pairs.push(validator_config.authority_key_pair);
    }

    builder = tokio::task::spawn_blocking(move || {
        for key in &key_pairs {
            builder = builder.add_validator_signature(key);
        }

        builder
    })
    .await?;
    info!("Genesis built successfully");

    let dir = tempfile::TempDir::new()?;
    builder.save(dir.path())?;
    Builder::load(dir.path()).await?;
    info!("Builder saved and re-loaded successfully");

    Ok(())
}
