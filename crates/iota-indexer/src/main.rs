// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use clap::{CommandFactory, FromArgMatches};
use iota_indexer::{IndexerConfig, errors::IndexerError, metrics::start_prometheus_server};
use tracing::{info, warn};

// Define the `GIT_REVISION` and `VERSION` consts
bin_version::bin_version!();

#[tokio::main]
async fn main() -> Result<(), IndexerError> {
    // NOTE: this is to print out tracing like info, warn & error.
    let _guard = telemetry_subscribers::TelemetryConfig::new()
        .with_env()
        .init();
    warn!(
        "WARNING: IOTA indexer is still experimental and we expect occasional breaking changes that require backfills."
    );

    let mut indexer_config = IndexerConfig::from_arg_matches_mut(
        &mut IndexerConfig::command().version(VERSION).get_matches(),
    )
    .unwrap();
    // TODO: Explore other options as in upstream.
    // For the moment we only use the fullnode for fetching checkpoints
    indexer_config.remote_store_url = Some(format!("{}/api/v1", indexer_config.rpc_client_url));
    info!("Parsed indexer config: {:#?}", indexer_config);
    let (_registry_service, registry) = start_prometheus_server(
        // NOTE: this parses the input host addr and port number for socket addr,
        // so unwrap() is safe here.
        format!(
            "{}:{}",
            indexer_config.client_metric_host, indexer_config.client_metric_port
        )
        .parse()
        .unwrap(),
        indexer_config.rpc_client_url.as_str(),
    )?;
    iota_indexer::db::setup_postgres::setup(indexer_config.clone(), registry.clone()).await?;

    Ok(())
}
