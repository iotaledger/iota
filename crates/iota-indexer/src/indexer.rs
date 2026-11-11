// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::env;

use anyhow::{Context, Result};
use iota_data_ingestion_core::ReaderOptions;
use iota_metrics::spawn_monitored_task;
use prometheus_filtered::Registry;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::{
    build_json_rpc_server,
    config::{HistoricFallbackOptions, IngestionConfig, JsonRpcConfig, RetentionConfig},
    db::ConnectionPool,
    errors::IndexerError,
    historical_fallback::reader::HistoricalFallbackReader,
    ingestion::{
        common::connection::{MAX_URL_RESOLUTION_TIMEOUT, resolve_remote_url},
        primary::orchestration::PrimaryPipeline,
    },
    metrics::IndexerMetrics,
    processors::processor_orchestrator::ProcessorOrchestrator,
    pruning::{
        pruner::Pruner,
        watermark_task::{WatermarkCache, WatermarkTask},
    },
    read::IndexerReader,
    store::{
        IndexerAnalyticalStore, IndexerStore, PgIndexerStore,
        pg_partition_manager::EpochPartitionKey,
    },
    system_package_task::SystemPackageTask,
};

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

        store
            .execute_in_blocking_worker(|this| EpochPartitionInvariant { store: this }.enforce())
            .await?;

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
            store
                .execute_in_blocking_worker(move |this| {
                    this.persist_protocol_configs_and_feature_flags(chain_id)
                })
                .await?;
        }

        // Restore metric from the DB so it doesn't stay as 0 until next epoch change.
        let (_, latest_epoch) = IndexerStore::get_available_epoch_range(&store).await?;
        metrics.last_committed_epoch.set(latest_epoch as i64);

        let primary_pipeline = PrimaryPipeline::setup(
            store.clone(),
            metrics.clone(),
            config.checkpoint_download_queue_size,
            cancel.clone(),
            config.operational_level,
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
            fallback_kv_max_response_size_bytes,
        } = config.historic_fallback_options
        {
            let historic_fallback_reader = HistoricalFallbackReader::new(
                url.as_str(),
                fallback_kv_cache_size,
                read.package_resolver().clone(),
                fallback_kv_multi_fetch_batch_size,
                fallback_kv_concurrent_fetches,
                fallback_kv_max_response_size_bytes,
                registry,
            )?;
            info!("HistoricalFallbackReader initialized with URL: {url}");
            read.with_fallback_reader(historic_fallback_reader);
        } else {
            info!("No config for HistoricalFallbackReader provided, skipping...");
        }

        // Fill the cache before the server accepts requests, so that read requests
        // don't have to deal with not filled cache.
        let watermark_task = WatermarkTask::new(store.clone(), watermark_cache);
        watermark_task.update_watermarks().await?;

        let handle = build_json_rpc_server(
            store,
            registry,
            read.clone(),
            config,
            metrics,
            cancel.clone(),
        )
        .await
        .expect("json rpc server should not run into errors upon start.");

        tracing::info!("Starting watermark background task to track pruning state");
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
        cancel: CancellationToken,
    ) -> Result<(), IndexerError> {
        info!(
            "IOTA Indexer Analytical Worker (version {:?}) started...",
            env!("CARGO_PKG_VERSION")
        );
        let mut processor_orchestrator = ProcessorOrchestrator::new(store, metrics, cancel);
        processor_orchestrator.run_forever().await;
        Ok(())
    }
}

/// The invariant that each epoch-based partition holds only the keys of its epoch.
///
/// Wraps a [`PgIndexerStore`] to enforce the invariant at startup.
struct EpochPartitionInvariant {
    store: PgIndexerStore,
}

impl EpochPartitionInvariant {
    /// Aligns the range of key of epoch-based partitions.
    ///
    /// A range is considered aligned if the associated keys (e.g. transaction or checkpoint
    /// sequence numbers) belong to the epoch the partition corresponds to.
    ///
    /// For all epochs except for the current this implies partitions with keys in
    /// `[epoch_lowest_key, next_epoch_lowest_key)`.
    ///
    /// For the current epoch the aligned range becomes `[epoch_lowest_key, i64::MAX)`.
    ///
    /// # Motivation
    ///
    /// When we start the service the last partition `<table>_partition_<last_partition>` of
    /// epoch-partitioned tables may be in one of the following two states.
    ///
    /// * `last_partition >= current_epoch`: This is the case where no gap exists in the table
    ///   ingestion, and data from the current epoch go into the last_partition. Partition ranges in
    ///   such a case are aligned.
    /// * `last_partition < current_epoch`: This implies a gap in the table ingestion caused by a
    ///   change of operational level. In this case the partition range is not aligned and if left
    ///   untreated causes data from the current epoch stored in the partition of a previous epoch.
    ///   This in turn would break the pruning guarantee that dropping an epoch partition affects
    ///   data of the respective epochs only.
    ///
    /// Aligning partitions maintains pruning invariants, and moreover allows creating or using the
    /// correct partitions when backfilling data.
    ///
    /// # Errors
    ///
    /// Returns an error if a database query fails, if the `epochs` table has no row for a needed
    /// epoch, or if a partition number does not fit in `i64`.
    fn enforce(&self) -> Result<(), IndexerError> {
        let Some(checkpoint) = self.store.get_latest_checkpoint()? else {
            return Ok(());
        };
        let current_epoch_id = checkpoint.epoch as u64 + checkpoint.end_of_epoch as u64;
        let partition_manager = &self.store.partition_manager;
        for (table, (_, last_partition)) in partition_manager.get_table_partitions()? {
            let Some(key) = partition_manager.get_strategy(&table).epoch() else {
                continue;
            };
            if current_epoch_id <= last_partition {
                continue;
            }
            self.align_epoch_partition_range(&table, last_partition, key)?;
            self.create_new_epoch_partition(&table, current_epoch_id, key)?;
        }
        Ok(())
    }

    /// Changes the range of `<table>_partition_<epoch>` to the keys of `epoch`.
    ///
    /// The new range is from the first key of `epoch` to the first key of `epoch + 1`.
    ///
    /// # Errors
    ///
    /// Returns an error if a database query fails, if the `epochs` table has no row for `epoch` or
    /// `epoch + 1`, or if `epoch` does not fit in `i64`. Also returns an error if the partition has
    /// a row that is not in the new range.
    fn align_epoch_partition_range(
        &self,
        table: &str,
        epoch: u64,
        key: EpochPartitionKey,
    ) -> Result<(), IndexerError> {
        let bounds = self.store.map_epochs_to_low_bounds(&[epoch, epoch + 1])?;
        let range_start = key.start(bounds.get(epoch)?);
        let range_end = key.start(bounds.get(epoch + 1)?);
        self.store.partition_manager.alter_partition_range(
            table,
            epoch.try_into()?,
            range_start,
            range_end,
        )
    }

    /// Creates `<table>_partition_<epoch>` with the range from the first key of `epoch` to
    /// `MAXVALUE`.
    ///
    /// Does nothing if the partition exists.
    ///
    /// # Errors
    ///
    /// Returns an error if a database query fails, if the `epochs` table has no row for `epoch`,
    /// or if `epoch` does not fit in `i64`.
    fn create_new_epoch_partition(
        &self,
        table: &str,
        epoch: u64,
        key: EpochPartitionKey,
    ) -> Result<(), IndexerError> {
        let bounds = self.store.map_epochs_to_low_bounds(&[epoch])?;
        let range_start = key.start(bounds.get(epoch)?);
        self.store
            .partition_manager
            .create_partition(table, epoch.try_into()?, range_start)
    }
}
