// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    io::{self, Write},
    str::FromStr,
};

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use iota_kvstore::{BigTableClient, KeyValueStoreReader};
use iota_types::{base_types::ObjectID, digests::TransactionDigest, storage::ObjectKey};
use telemetry_subscribers::TelemetryConfig;

#[derive(Debug, Clone, Copy, Default, ValueEnum, strum::Display)]
#[strum(serialize_all = "snake_case")]
enum Network {
    #[default]
    Mainnet,
    Testnet,
    Devnet,
}

#[derive(Parser)]
#[command(name = "iota kvstore")]
#[command(about = "Ingest Checkpoints from a provided network into Key Value pairs", long_about = None)]
struct App {
    /// The instance ID of the BigTableDB
    #[arg(short, long)]
    instance_id: String,
    /// The column family to use for the Key Value pairs
    #[arg(short, long, default_value_t = String::from("iota"))]
    column_family: String,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Fetch a Key Value pair from the database
    Fetch {
        /// Fetch a specific entry from the database
        #[command(subcommand)]
        entry: Entry,
    },
}

#[derive(Subcommand)]
enum Entry {
    Object { id: String, version: u64 },
    Checkpoint { id: u64 },
    Transaction { id: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    let _guard = TelemetryConfig::new().with_env().init();
    let app = App::parse();
    match app.command {
        Command::Fetch { entry } => run_fetch(app.instance_id, app.column_family, entry).await?,
    }
    Ok(())
}

async fn run_fetch(instance_id: String, column_family: String, entry: Entry) -> Result<()> {
    let mut client = BigTableClient::new_remote(
        instance_id,
        true,
        None,
        "cli".to_string(),
        column_family,
        None,
    )
    .await?;

    let result = match entry {
        Entry::Object { id, version } => {
            let objects = client
                .get_objects(&[ObjectKey(ObjectID::from_str(&id)?, version.into())])
                .await?;
            objects.first().map(bcs::to_bytes)
        }
        Entry::Checkpoint { id } => {
            let checkpoints = client.get_checkpoints(&[id]).await?;
            checkpoints.first().map(bcs::to_bytes)
        }
        Entry::Transaction { id } => {
            let transactions = client
                .get_transactions(&[TransactionDigest::from_str(&id)?])
                .await?;
            transactions.first().map(bcs::to_bytes)
        }
    };

    match result {
        Some(bytes) => io::stdout().write_all(&bytes?)?,
        None => println!("not found"),
    }
    Ok(())
}
