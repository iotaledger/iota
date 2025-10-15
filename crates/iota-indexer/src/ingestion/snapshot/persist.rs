// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;
use iota_metrics::{get_metrics, spawn_monitored_task};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::{
    config::SnapshotLagConfig,
    ingestion::{
        common::persist::Writer, primary::persist::TransactionObjectChangesToCommit,
        snapshot::prepare::ObjectsSnapshotHandler,
    },
    metrics::IndexerMetrics,
    store::{IndexerStore, PgIndexerStore},
    types::IndexerResult,
};

#[async_trait]
impl Writer<TransactionObjectChangesToCommit> for ObjectsSnapshotHandler {
    fn name(&self) -> String {
        "objects_snapshot_handler".to_string()
    }

    async fn persist(
        &self,
        transformed_data: Vec<TransactionObjectChangesToCommit>,
    ) -> IndexerResult<()> {
        self.store
            .persist_objects_snapshot(transformed_data)
            .await?;
        Ok(())
    }

    // TODO: read watermark table when it's ready.
    async fn get_watermark_hi(&self) -> IndexerResult<Option<u64>> {
        self.store
            .get_latest_object_snapshot_checkpoint_sequence_number()
            .await
    }

    // TODO: update watermark table when it's ready.
    async fn set_watermark_hi(&self, watermark_hi: u64) -> IndexerResult<()> {
        self.metrics
            .latest_object_snapshot_sequence_number
            .set(watermark_hi as i64);
        Ok(())
    }

    async fn get_max_committable_checkpoint(&self) -> IndexerResult<u64> {
        let latest_checkpoint = self.store.get_latest_checkpoint_sequence_number().await?;
        Ok(latest_checkpoint
            .map(|seq| seq.saturating_sub(self.snapshot_config.snapshot_min_lag as u64))
            .unwrap_or_default()) // hold snapshot handler until at least one checkpoint is in DB
    }
}

pub async fn start_objects_snapshot_handler(
    store: PgIndexerStore,
    metrics: IndexerMetrics,
    snapshot_config: SnapshotLagConfig,
    cancel: CancellationToken,
) -> IndexerResult<(ObjectsSnapshotHandler, u64, JoinHandle<IndexerResult<()>>)> {
    info!("Starting object snapshot handler...");

    let global_metrics = get_metrics().unwrap();
    let (sender, receiver) = iota_metrics::metered_channel::channel(
        600,
        &global_metrics
            .channel_inflight
            .with_label_values(&["objects_snapshot_handler_checkpoint_data"]),
    );

    let objects_snapshot_handler =
        ObjectsSnapshotHandler::new(store.clone(), sender, metrics.clone(), snapshot_config);

    let watermark_hi = objects_snapshot_handler.get_watermark_hi().await?;
    let writer = objects_snapshot_handler.clone();
    let task_handle = spawn_monitored_task!(writer.persist_sequentially(receiver, cancel));
    Ok((
        objects_snapshot_handler,
        watermark_hi.unwrap_or_default(),
        task_handle,
    ))
}
