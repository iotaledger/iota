// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use anyhow::{Context, Result, bail};
use clap::Parser;
use iota_indexer_alt::{
    args::{Args, Command},
    config::{IndexerConfig, Merge},
    models::MIGRATIONS,
    start_indexer,
};
use iota_indexer_alt_framework::db::reset_database;
use tokio::fs;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Enable tracing, configured by environment variables.
    let _guard = telemetry_subscribers::TelemetryConfig::new()
        .with_env()
        .init();

    match args.command {
        Command::Indexer {
            client_args,
            indexer_args,
            config,
        } => {
            let indexer_config = read_config(&config).await?;

            start_indexer(
                args.db_args,
                indexer_args,
                client_args,
                indexer_config,
                true,
            )
            .await?;
        }

        Command::GenerateConfig => {
            let config = IndexerConfig::example();
            let config_toml = toml::to_string_pretty(&config)
                .context("Failed to serialize default configuration to TOML.")?;

            println!("{config_toml}");
        }

        Command::MergeConfigs { config } => {
            let mut files = config.into_iter();

            let Some(file) = files.next() else {
                bail!("At least one configuration file must be provided.");
            };

            let mut indexer_config = read_config(&file).await?;
            for file in files {
                indexer_config =
                    indexer_config.merge(read_config(&file).await.with_context(|| {
                        format!("Failed to read configuration file: {}", file.display())
                    })?);
            }

            let config_toml = toml::to_string_pretty(&indexer_config)
                .context("Failed to serialize merged configuration to TOML.")?;

            println!("{config_toml}");
        }

        Command::ResetDatabase { skip_migrations } => {
            reset_database(args.db_args, (!skip_migrations).then_some(&MIGRATIONS)).await?;
        }

        #[cfg(feature = "benchmark")]
        Command::Benchmark {
            benchmark_args,
            config,
        } => {
            let config_contents = fs::read_to_string(config)
                .await
                .context("failed to read configuration TOML file")?;

            let indexer_config: IndexerConfig = toml::from_str(&config_contents)
                .context("Failed to parse configuration TOML file.")?;

            iota_indexer_alt::benchmark::run_benchmark(
                args.db_args,
                benchmark_args,
                indexer_config,
            )
            .await?;
        }
    }

    Ok(())
}

async fn read_config(path: &Path) -> Result<IndexerConfig> {
    let config_contents = fs::read_to_string(path)
        .await
        .context("Failed to read configuration TOML file")?;

    toml::from_str(&config_contents).context("Failed to parse configuration TOML file.")
}
