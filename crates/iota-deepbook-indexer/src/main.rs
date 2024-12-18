// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    env,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::Arc,
};

use anyhow::Result;
use clap::*;
use iota_config::Config;
use iota_data_ingestion_core::DataIngestionMetrics;
use iota_deepbook_indexer::{
    config::IndexerConfig,
    iota_deepbook_indexer::{IotaDeepBookDataMapper, PgDeepbookPersistent},
    metrics::DeepBookIndexerMetrics,
    postgres_manager::get_connection_pool,
    server::run_server,
};
use iota_indexer_builder::{
    indexer_builder::IndexerBuilder,
    iota_datasource::IotaCheckpointDatasource,
    progress::{OutOfOrderSaveAfterDurationPolicy, ProgressSavingPolicy},
};
use iota_metrics::start_prometheus_server;
use iota_sdk::IotaClientBuilder;
use iota_types::base_types::ObjectID;
use tracing::info;

#[derive(Parser, Clone, Debug)]
struct Args {
    /// Path to a yaml config
    #[clap(long, short)]
    config_path: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _guard = telemetry_subscribers::TelemetryConfig::new()
        .with_env()
        .init();

    let args = Args::parse();

    // load config
    let config_path = if let Some(path) = args.config_path {
        path
    } else {
        env::current_dir()
            .expect("Couldn't get current directory")
            .join("config.yaml")
    };
    let config = IndexerConfig::load(&config_path)?;

    // Init metrics server
    let metrics_address =
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), config.metric_port);
    let registry_service = start_prometheus_server(metrics_address);
    let registry = registry_service.default_registry();
    iota_metrics::init_metrics(&registry);
    info!("Metrics server started at port {}", config.metric_port);

    let indexer_meterics = DeepBookIndexerMetrics::new(&registry);
    let ingestion_metrics = DataIngestionMetrics::new(&registry);

    let db_url = config.db_url.clone();
    let datastore = PgDeepbookPersistent::new(
        get_connection_pool(db_url.clone()).await,
        ProgressSavingPolicy::OutOfOrderSaveAfterDuration(OutOfOrderSaveAfterDurationPolicy::new(
            tokio::time::Duration::from_secs(30),
        )),
    );

    let iota_client = Arc::new(
        IotaClientBuilder::default()
            .build(config.iota_rpc_url.clone())
            .await?,
    );
    let iota_checkpoint_datasource = IotaCheckpointDatasource::new(
        config.remote_store_url,
        iota_client,
        config.concurrency as usize,
        config
            .checkpoints_path
            .map(|p| p.into())
            .unwrap_or(tempfile::tempdir()?.into_path()),
        config.deepbook_genesis_checkpoint,
        ingestion_metrics.clone(),
        Box::new(indexer_meterics.clone()),
    );

    let service_address =
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), config.service_port);
    run_server(service_address, datastore.clone());

    let indexer = IndexerBuilder::new(
        "IotaDeepBookIndexer",
        iota_checkpoint_datasource,
        IotaDeepBookDataMapper {
            metrics: indexer_meterics.clone(),
            package_id: ObjectID::from_hex_literal(&config.deepbook_package_id.clone())
                .unwrap_or_else(|err| panic!("Failed to parse deepbook package ID: {}", err)),
        },
        datastore,
    )
    .build();
    indexer.start().await?;

    Ok(())
}
