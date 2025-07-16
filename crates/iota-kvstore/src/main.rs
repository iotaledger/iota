// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    io::{self, Write},
    str::FromStr,
};

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use iota_data_ingestion_core::{
    DataIngestionMetrics, FileProgressStore, IndexerExecutor, WorkerPool,
    reader::v2::{CheckpointReaderConfig, RemoteUrl},
};
use iota_kvstore::{BigTableClient, KeyValueStoreReader, KvWorker};
use iota_types::{base_types::ObjectID, digests::TransactionDigest, storage::ObjectKey};
use prometheus::Registry;
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
    instance_id: String,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Ingest Checkpoints from a provide network into Key Value pairs
    Ingestion {
        /// The network to ingest checkpoints from
        #[arg(default_value_t)]
        network: Network,
    },
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
    Watermark,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _guard = TelemetryConfig::new().with_env().init();
    let app = App::parse();
    match app.command {
        Command::Ingestion { network } => run_ingestion(app.instance_id, network).await?,
        Command::Fetch { entry } => run_fetch(app.instance_id, entry).await?,
    }
    Ok(())
}

async fn run_ingestion(instance_id: String, network: Network) -> Result<()> {
    let client = BigTableClient::new_remote(
        instance_id,
        false,
        None,
        "ingestion".to_string(),
        "iota",
        None,
    )
    .await?;

    let progress_store = FileProgressStore::new("./kvstore_progress.json").await?;

    let mut executor = IndexerExecutor::new(
        progress_store,
        1,
        DataIngestionMetrics::new(&Registry::new()),
        Default::default(),
    );

    let worker_pool = WorkerPool::new(
        KvWorker { client },
        "bigtable".into(),
        50,
        Default::default(),
    );
    executor.register(worker_pool).await?;
    let config = CheckpointReaderConfig {
        remote_store_url: Some(RemoteUrl::HybridHistoricalStore {
            historical_url: format!("https://checkpoints.{network}.iota.cafe/ingestion/historical"),
            live_url: Some(format!(
                "https://checkpoints.{network}.iota.cafe/ingestion/live"
            )),
        }),
        ..Default::default()
    };
    executor.run_with_config(config).await?;
    Ok(())
}

async fn run_fetch(instance_id: String, entry: Entry) -> Result<()> {
    let mut client =
        BigTableClient::new_remote(instance_id, true, None, "cli".to_string(), "iota", None)
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
        Entry::Watermark => {
            let watermark = client.get_latest_checkpoint().await?;
            println!("watermark is {watermark}");
            return Ok(());
        }
    };

    match result {
        Some(bytes) => io::stdout().write_all(&bytes?)?,
        None => println!("not found"),
    }
    Ok(())
}
