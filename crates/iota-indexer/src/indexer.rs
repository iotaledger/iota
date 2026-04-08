// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{env, time::Duration};

use anyhow::{Context, Result};
use iota_data_ingestion_core::ReaderOptions;
use iota_metrics::spawn_monitored_task;
use prometheus::Registry;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::{
    build_json_rpc_server,
    config::{
        HistoricFallbackOptions, IngestionConfig, JsonRpcConfig, RetentionConfig, SnapshotLagConfig,
    },
    db::ConnectionPool,
    errors::IndexerError,
    historical_fallback::reader::HistoricalFallbackReader,
    ingestion::{
        common::connection::resolve_remote_url, primary::orchestration::PrimaryPipeline,
        snapshot::orchestration::SnapshotPipelineBuilder,
    },
    metrics::IndexerMetrics,
    processors::processor_orchestrator::ProcessorOrchestrator,
    pruning::{optimistic_pruner::OptimisticPruner, pruner::Pruner},
    read::IndexerReader,
    store::{IndexerAnalyticalStore, IndexerStore, PgIndexerStore},
};

/// Maximum timeout for resolving the remote checkpoint source.
const MAX_URL_RESOLUTION_TIMEOUT: Duration = Duration::from_secs(30);

pub struct Indexer;

impl Indexer {
    pub async fn start_writer_with_config(
        config: &IngestionConfig,
        store: PgIndexerStore,
        metrics: IndexerMetrics,
        snapshot_config: SnapshotLagConfig,
        retention_config: Option<RetentionConfig>,
        optimistic_pruner_batch_size: Option<u64>,
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
            let pruner = Pruner::new(store.clone(), retention_config, metrics.clone())?;
            let cancel_clone = cancel.clone();
            spawn_monitored_task!(pruner.start(cancel_clone));
        }

        if let Some(optimistic_pruner_batch_size) = optimistic_pruner_batch_size {
            info!("Starting indexer optimistic tables pruner");
            let optimistic_pruner = OptimisticPruner::new(
                store.clone(),
                optimistic_pruner_batch_size,
                metrics.clone(),
            )?;
            let cancellation_token_for_optimistic_pruner = cancel.child_token();
            spawn_monitored_task!(
                optimistic_pruner.start(cancellation_token_for_optimistic_pruner)
            );
        }

        // If we already have chain identifier indexed (i.e. the first checkpoint has
        // been indexed), then we persist protocol configs for protocol versions
        // not yet in the db. Otherwise, we would do the persisting in
        // `commit_checkpoint` while the first cp is being indexed.
        if let Some(chain_id) = IndexerStore::get_chain_identifier(&store).await? {
            store.persist_protocol_configs_and_feature_flags(chain_id)?;
        }

        let mut primary_pipeline = PrimaryPipeline::setup(
            store.clone(),
            metrics.clone(),
            config.checkpoint_download_queue_size,
            cancel.clone(),
        )
        .await?;

        let snapshot_pipeline_builder = SnapshotPipelineBuilder::new(
            store.clone(),
            metrics.clone(),
            snapshot_config,
            config.checkpoint_download_queue_size,
            cancel.clone(),
        )
        .await?;

        // data_ingestion_path can only feed data to one executor,
        // but if we have remote_store_url we can use many executors
        let use_separate_executors = remote_store_url.is_some();
        let snapshot_pipeline = if use_separate_executors {
            snapshot_pipeline_builder
                .finalize_with_dedicated_executor()
                .await?
        } else {
            warn!(
                "Sharing the same executor between Primary and Snapshot pipelines due to not \
                 provided --remote-store-url argument. Limited possibilities for Snapshot lag \
                 config. This may be deprecated in the future."
            );
            snapshot_pipeline_builder
                .finalize_with_shared_executor(&mut primary_pipeline.executor)
                .await?
        };

        info!("Starting data ingestion executor...");
        let mut primary_pipeline_handle = primary_pipeline
            .run(
                config.sources.data_ingestion_path.clone(),
                remote_store_url.clone(),
                extra_reader_options.clone(),
            )
            .await;

        let mut snapshot_pipeline_handle = snapshot_pipeline
            .run(remote_store_url, extra_reader_options)
            .await;

        let mut primary_pipeline_done = false;
        let mut snapshot_pipeline_done = false;
        while !primary_pipeline_done || !snapshot_pipeline_done {
            tokio::select! {
                result = &mut primary_pipeline_handle, if !primary_pipeline_done => {
                    result.context("failed to join primary pipeline")?.context("primary pipeline failed")?;
                    info!("Primary pipeline finished successfully");
                    primary_pipeline_done = true;
                },
                result = &mut snapshot_pipeline_handle, if !snapshot_pipeline_done => {
                    result.context("failed to join snapshot pipeline")?.context("snapshot pipeline failed")?;
                    info!("Snapshot pipeline finished successfully");
                    snapshot_pipeline_done = true;
                },
            }
            cancel.cancel();
        }

        Ok(())
    }

    pub async fn start_reader(
        config: &JsonRpcConfig,
        store: PgIndexerStore,
        registry: &Registry,
        connection_pool: ConnectionPool,
        metrics: IndexerMetrics,
    ) -> Result<(), IndexerError> {
        info!(
            "IOTA Indexer Reader (version {:?}) started...",
            env!("CARGO_PKG_VERSION")
        );

        let mut read = IndexerReader::new(connection_pool.clone());

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

        let handle = build_json_rpc_server(store, registry, read, config, metrics)
            .await
            .expect("json rpc server should not run into errors upon start.");
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
