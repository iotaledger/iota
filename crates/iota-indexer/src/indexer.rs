// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{env, time::Duration};

use anyhow::{Context, Result};
use iota_data_ingestion_core::ReaderOptions;
use iota_metrics::spawn_monitored_task;
use prometheus::Registry;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::{
    build_json_rpc_server,
    config::{HistoricFallbackOptions, IngestionConfig, JsonRpcConfig, RetentionConfig},
    db::ConnectionPool,
    errors::IndexerError,
    historical_fallback::reader::HistoricalFallbackReader,
    ingestion::{common::connection::resolve_remote_url, primary::orchestration::PrimaryPipeline},
    metrics::IndexerMetrics,
    processors::processor_orchestrator::ProcessorOrchestrator,
    pruning::{
        pruner::Pruner,
        watermark_task::{WatermarkCache, WatermarkTask},
    },
    read::IndexerReader,
    store::{IndexerAnalyticalStore, IndexerStore, PgIndexerStore},
    system_package_task::SystemPackageTask,
};

/// Maximum timeout for resolving the remote checkpoint source.
const MAX_URL_RESOLUTION_TIMEOUT: Duration = Duration::from_secs(30);

pub struct Indexer;

impl Indexer {
    pub async fn start_writer_with_config(
        config: &IngestionConfig,
        store: PgIndexerStore,
        metrics: IndexerMetrics,
        retention_config: Option<RetentionConfig>,
        pruning_delay_ms: u64,
        pruning_batch_size: u64,
        cancel: CancellationToken,
    ) -> Result<(), IndexerError> {
        info!(
            "IOTA Indexer Writer (version {:?}) started...",
            env!("CARGO_PKG_VERSION")
        );

        info!("IOTA Indexer Writer config: {config:?}",);
        let extra_reader_options = ReaderOptions {
            batch_size: config.checkpoint_download_queue_size,
            timeout_secs: config.checkpoint_download_timeout,
            data_limit: config.checkpoint_download_queue_size_bytes,
            ..Default::default()
        };

        let remote_store_url =
            resolve_remote_url(&config.sources, MAX_URL_RESOLUTION_TIMEOUT).await?;

        if let Some(retention_config) = retention_config {
            let pruner = Pruner::new(
                store.clone(),
                retention_config,
                pruning_delay_ms,
                pruning_batch_size,
                metrics.clone(),
            )?;
            let cancel_clone = cancel.clone();
            spawn_monitored_task!(pruner.start(cancel_clone));
        }

        // If we already have chain identifier indexed (i.e. the first checkpoint has
        // been indexed), then we persist protocol configs for protocol versions
        // not yet in the db. Otherwise, we would do the persisting in
        // `commit_checkpoint` while the first cp is being indexed.
        if let Some(chain_id) = IndexerStore::get_chain_identifier(&store).await? {
            store.persist_protocol_configs_and_feature_flags(chain_id)?;
        }

        let primary_pipeline = PrimaryPipeline::setup(
            store.clone(),
            metrics.clone(),
            config.checkpoint_download_queue_size,
            cancel.clone(),
        )
        .await?;

        info!("Starting data ingestion executor...");
        let primary_pipeline_handle = primary_pipeline
            .run(
                config.sources.data_ingestion_path.clone(),
                remote_store_url,
                extra_reader_options,
            )
            .await;

        let result = primary_pipeline_handle
            .await
            .context("failed to join primary pipeline")?
            .context("primary pipeline failed");
        info!("Primary pipeline finished");
        // Tell other tasks (e.g. the pruner) to stop.
        cancel.cancel();
        result?;

        Ok(())
    }

    pub async fn start_reader(
        config: &JsonRpcConfig,
        store: PgIndexerStore,
        registry: &Registry,
        connection_pool: ConnectionPool,
        metrics: IndexerMetrics,
        cancel: CancellationToken,
    ) -> Result<(), IndexerError> {
        info!(
            "IOTA Indexer Reader (version {:?}) started...",
            env!("CARGO_PKG_VERSION")
        );

        // Create the watermark cache that will track pruning state
        let watermark_cache = WatermarkCache::new();
        let mut read = IndexerReader::new(connection_pool.clone(), watermark_cache.clone());

        if let HistoricFallbackOptions {
            fallback_kv_url: Some(ref url),
            fallback_kv_multi_fetch_batch_size,
            fallback_kv_concurrent_fetches,
            fallback_kv_cache_size,
        } = config.historic_fallback_options
        {
            let historic_fallback_reader = HistoricalFallbackReader::new(
                url.as_str(),
                fallback_kv_cache_size,
                read.package_resolver().clone(),
                fallback_kv_multi_fetch_batch_size,
                fallback_kv_concurrent_fetches,
                registry,
            )?;
            info!("HistoricalFallbackReader initialized with URL: {url}");
            read.with_fallback_reader(historic_fallback_reader);
        } else {
            info!("No config for HistoricalFallbackReader provided, skipping...");
        }

        let handle = build_json_rpc_server(
            store.clone(),
            registry,
            read.clone(),
            config,
            metrics,
            cancel.clone(),
        )
        .await
        .expect("json rpc server should not run into errors upon start.");

        tracing::info!("Starting watermark background task to track pruning state");
        let watermark_task = WatermarkTask::new(store, watermark_cache);
        watermark_task.start(cancel.clone());

        tracing::info!("Starting system package task");
        let system_package_task =
            SystemPackageTask::new(read, cancel, std::time::Duration::from_secs(10));
        spawn_monitored_task!(async move { system_package_task.run().await });

        tokio::spawn(async move { handle.stopped().await })
            .await
            .expect("rpc server task failed");

        Ok(())
    }

    pub async fn start_analytical_worker<
        S: IndexerAnalyticalStore + Clone + Send + Sync + 'static,
    >(
        store: S,
        metrics: IndexerMetrics,
    ) -> Result<(), IndexerError> {
        info!(
            "IOTA Indexer Analytical Worker (version {:?}) started...",
            env!("CARGO_PKG_VERSION")
        );
        let mut processor_orchestrator = ProcessorOrchestrator::new(store, metrics);
        processor_orchestrator.run_forever().await;
        Ok(())
    }
}
