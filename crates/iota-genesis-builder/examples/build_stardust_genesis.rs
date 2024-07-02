// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Creating a genesis blob out of a local or remote stardust objects snapshots.

use std::{
    fs::File,
    io::{BufReader, Read},
};

use anyhow::anyhow;
use clap::{ArgGroup, Parser};
use iota_genesis_builder::{
    Builder, TarGzSnapshotUrl, BROTLI_COMPRESSOR_BUFFER_SIZE, IOTA_OBJECT_SNAPSHOT_URL,
    SHIMMER_OBJECT_SNAPSHOT_URL,
};
use iota_swarm_config::genesis_config::ValidatorGenesisConfigBuilder;
use rand::rngs::OsRng;
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[clap(
about = "Example tool for generating a genesis file from Iota and Shimmer object snapshots",
group = ArgGroup::new("snapshots").required(false)
)]
struct Cli {
    #[clap(long, group = "snapshots", help = "Path to the IOTA object snapshot")]
    iota_snapshot_path: Option<String>,
    #[clap(
        long,
        group = "snapshots",
        help = "Path to the Shimmer object snapshot"
    )]
    shimmer_snapshot_path: Option<String>,
    #[clap(
        short,
        long,
        default_value_t = false,
        help = "Decompress the input object snapshots"
    )]
    decompress: bool,
}

impl Cli {
    fn validate(self) -> anyhow::Result<Cli> {
        let iota_provided = self.iota_snapshot_path.is_some();
        let shimmer_provided = self.shimmer_snapshot_path.is_some();

        if iota_provided != shimmer_provided {
            return Err(anyhow!(
                "Either both --iota-snapshot-path and --shimmer-snapshot-path must be provided, or neither."
            ));
        }

        Ok(self)
    }

    fn has_snapshot_args(&self) -> bool {
        self.iota_snapshot_path.is_some() && self.shimmer_snapshot_path.is_some()
    }
}

fn main() -> anyhow::Result<()> {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    // Parse and validate the CLI arguments
    let cli = Cli::parse().validate()?;

    // Prepare the reader for the objects snapshots
    let (iota_snapshot_reader, shimmer_snapshot_reader): (Box<dyn Read>, Box<dyn Read>) = if cli
        .has_snapshot_args()
    {
        info!("Using local snapshots..");
        let iota_file = File::open(
            cli.iota_snapshot_path
                .expect("iota snapshot path should exist"),
        )?;
        let shimmer_file = File::open(
            cli.shimmer_snapshot_path
                .expect("shimmer snapshot path should exist"),
        )?;

        if cli.decompress {
            info!("Decompressing snapshots..");
            let iota_reader = Box::new(brotli::Decompressor::new(
                iota_file,
                BROTLI_COMPRESSOR_BUFFER_SIZE,
            ));
            let shimmer_reader = Box::new(brotli::Decompressor::new(
                shimmer_file,
                BROTLI_COMPRESSOR_BUFFER_SIZE,
            ));

            (iota_reader, shimmer_reader)
        } else {
            let iota_reader = Box::new(BufReader::new(iota_file));
            let shimmer_reader = Box::new(BufReader::new(shimmer_file));

            (iota_reader, shimmer_reader)
        }
    } else {
        warn!("No local snapshots provided");
        info!(
            "Downloading the IOTA snapshot from {}",
            IOTA_OBJECT_SNAPSHOT_URL
        );
        let iota_reader = Builder::download_tar_gz_snapshot_and_extract(TarGzSnapshotUrl::Iota)?;
        info!("IOTA snapshot downloaded successfully");

        info!(
            "Downloading the Shimmer snapshot from {}",
            SHIMMER_OBJECT_SNAPSHOT_URL
        );
        let shimmer_reader =
            Builder::download_tar_gz_snapshot_and_extract(TarGzSnapshotUrl::Shimmer)?;
        info!("Shimmer snapshot downloaded successfully");

        (iota_reader, shimmer_reader)
    };

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

    let genesis = builder.build();
    println!("{:?}", genesis);

    info!("Genesis built successfully");

    Ok(())
}
