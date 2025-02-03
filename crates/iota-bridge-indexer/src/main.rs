// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{env, path::PathBuf, sync::Arc};

use anyhow::Result;
use clap::*;
use ethers::providers::{Http, Middleware, Provider};
use iota_bridge::metrics::BridgeMetrics;
use iota_bridge_indexer::{
    config::IndexerConfig,
    eth_bridge_indexer::{EthDataMapper, EthSubscriptionDatasource, EthSyncDatasource},
    iota_bridge_indexer::{IotaBridgeDataMapper, PgBridgePersistent},
    iota_transaction_handler::handle_iota_transactions_loop,
    iota_transaction_queries::start_iota_tx_polling_task,
    metrics::BridgeIndexerMetrics,
    postgres_manager::{get_connection_pool, read_iota_progress_store},
};
use iota_config::Config;
use iota_data_ingestion_core::DataIngestionMetrics;
use iota_indexer_builder::{
    indexer_builder::{BackfillStrategy, IndexerBuilder},
    iota_datasource::IotaCheckpointDatasource,
};
use iota_metrics::{
    metered_channel::channel, spawn_logged_monitored_task, start_prometheus_server,
};
use iota_sdk::IotaClientBuilder;
use tokio::task::JoinHandle;
use tracing::info;

#[derive(Parser, Clone, Debug)]
struct Args {
    /// Path to a yaml config
    #[arg(long, short)]
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
    let registry_service = start_prometheus_server(
        format!("{}:{}", config.metric_url, config.metric_port,)
            .parse()
            .unwrap_or_else(|err| panic!("Failed to parse metric address: {}", err)),
    );
    let registry = registry_service.default_registry();

    iota_metrics::init_metrics(&registry);

    info!(
        "Metrics server started at {}::{}",
        config.metric_url, config.metric_port
    );
    let indexer_meterics = BridgeIndexerMetrics::new(&registry);
    let ingestion_metrics = DataIngestionMetrics::new(&registry);
    let bridge_metrics = Arc::new(BridgeMetrics::new(&registry));

    let db_url = config.db_url.clone();
    let datastore = PgBridgePersistent::new(get_connection_pool(db_url.clone()));

    let provider = Arc::new(
        Provider::<Http>::try_from(config.eth_rpc_url.clone())?
            .interval(std::time::Duration::from_millis(2000)),
    );

    let current_block = provider.get_block_number().await?.as_u64();
    let subscription_end_block = u64::MAX;

    // Start the eth subscription indexer

    let eth_subscription_datasource = EthSubscriptionDatasource::new(
        config.eth_iota_bridge_contract_address.clone(),
        config.eth_ws_url.clone(),
        indexer_meterics.clone(),
    )?;
    let eth_subscription_indexer = IndexerBuilder::new(
        "EthBridgeSubscriptionIndexer",
        eth_subscription_datasource,
        EthDataMapper {
            metrics: indexer_meterics.clone(),
        },
    )
    .with_backfill_strategy(BackfillStrategy::Disabled)
    .build(current_block, subscription_end_block, datastore.clone());
    let subscription_indexer_fut = spawn_logged_monitored_task!(eth_subscription_indexer.start());

    // Start the eth sync indexer
    let eth_sync_datasource = EthSyncDatasource::new(
        config.eth_iota_bridge_contract_address.clone(),
        config.eth_rpc_url.clone(),
        indexer_meterics.clone(),
        bridge_metrics.clone(),
    )?;
    let eth_sync_indexer = IndexerBuilder::new(
        "EthBridgeSyncIndexer",
        eth_sync_datasource,
        EthDataMapper {
            metrics: indexer_meterics.clone(),
        },
    )
    .with_backfill_strategy(BackfillStrategy::Partitioned { task_size: 1000 })
    .disable_live_task()
    .build(current_block, config.start_block, datastore.clone());
    let sync_indexer_fut = spawn_logged_monitored_task!(eth_sync_indexer.start());

    if let Some(iota_rpc_url) = config.iota_rpc_url.clone() {
        // Todo: impl datasource for iota RPC datasource
        start_processing_iota_checkpoints_by_querying_txns(
            iota_rpc_url,
            db_url.clone(),
            indexer_meterics.clone(),
            bridge_metrics,
        )
        .await?;
    } else {
        let iota_checkpoint_datasource = IotaCheckpointDatasource::new(
            config.remote_store_url,
            config.concurrency as usize,
            config.checkpoints_path.clone().into(),
            ingestion_metrics.clone(),
        );
        let indexer = IndexerBuilder::new(
            "IotaBridgeIndexer",
            iota_checkpoint_datasource,
            IotaBridgeDataMapper {
                metrics: indexer_meterics.clone(),
            },
        )
        .build(
            config
                .resume_from_checkpoint
                .unwrap_or(config.bridge_genesis_checkpoint),
            config.bridge_genesis_checkpoint,
            datastore,
        );
        indexer.start().await?;
    }
    // We are not waiting for the iota tasks to finish here, which is ok.
    futures::future::join_all(vec![subscription_indexer_fut, sync_indexer_fut]).await;

    Ok(())
}

async fn start_processing_iota_checkpoints_by_querying_txns(
    iota_rpc_url: String,
    db_url: String,
    indexer_metrics: BridgeIndexerMetrics,
    bridge_metrics: Arc<BridgeMetrics>,
) -> Result<Vec<JoinHandle<()>>> {
    let pg_pool = get_connection_pool(db_url.clone());
    let (tx, rx) = channel(
        100,
        &iota_metrics::get_metrics()
            .unwrap()
            .channel_inflight
            .with_label_values(&["iota_transaction_processing_queue"]),
    );
    let mut handles = vec![];
    let cursor =
        read_iota_progress_store(&pg_pool).expect("Failed to read cursor from iota progress store");
    let iota_client = IotaClientBuilder::default().build(iota_rpc_url).await?;
    handles.push(spawn_logged_monitored_task!(
        start_iota_tx_polling_task(iota_client, cursor, tx, bridge_metrics),
        "start_iota_tx_polling_task"
    ));
    handles.push(spawn_logged_monitored_task!(
        handle_iota_transactions_loop(pg_pool.clone(), rx, indexer_metrics.clone()),
        "handle_iota_transcations_loop"
    ));
    Ok(handles)
}
