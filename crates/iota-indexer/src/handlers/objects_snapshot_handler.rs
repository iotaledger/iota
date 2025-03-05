// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;
use iota_data_ingestion_core::Worker;
use iota_metrics::{get_metrics, metered_channel::Sender, spawn_monitored_task};
use iota_rest_api::CheckpointData;
use tokio_util::sync::CancellationToken;
use tracing::info;

use super::{
    CommonHandler, Handler, TransactionObjectChangesToCommit, checkpoint_handler::CheckpointHandler,
};
use crate::{
    errors::IndexerError,
    metrics::IndexerMetrics,
    store::{IndexerStore, PgIndexerStore},
    types::IndexerResult,
};

#[derive(Clone)]
pub struct ObjectsSnapshotHandler {
    pub store: PgIndexerStore,
    pub sender: Sender<(u64, TransactionObjectChangesToCommit)>,
    snapshot_config: SnapshotLagConfig,
    metrics: IndexerMetrics,
}

pub struct CheckpointObjectChanges {
    pub checkpoint_sequence_number: u64,
    pub object_changes: TransactionObjectChangesToCommit,
}

#[derive(Debug, Clone)]
pub struct SnapshotLagConfig {
    pub snapshot_min_lag: usize,
    pub sleep_duration: u64,
}

impl SnapshotLagConfig {
    const DEFAULT_MIN_LAG: usize = 300;
    const DEFAULT_SLEEP_DURATION_SEC: u64 = 5;
}

impl SnapshotLagConfig {
    pub fn new(min_lag: Option<usize>, sleep_duration: Option<u64>) -> Self {
        let default_config = Self::default();
        Self {
            snapshot_min_lag: min_lag.unwrap_or(default_config.snapshot_min_lag),
            sleep_duration: sleep_duration.unwrap_or(default_config.sleep_duration),
        }
    }
}

impl Default for SnapshotLagConfig {
    fn default() -> Self {
        SnapshotLagConfig {
            snapshot_min_lag: Self::DEFAULT_MIN_LAG,
            sleep_duration: Self::DEFAULT_SLEEP_DURATION_SEC,
        }
    }
}

#[async_trait]
impl Worker for ObjectsSnapshotHandler {
    type Message = ();
    type Error = IndexerError;

    async fn process_checkpoint(
        &self,
        checkpoint: &CheckpointData,
    ) -> Result<Self::Message, Self::Error> {
        let transformed_data = CheckpointHandler::index_objects(checkpoint, &self.metrics).await?;
        self.sender
            .send((
                checkpoint.checkpoint_summary.sequence_number,
                transformed_data,
            ))
            .await
            .map_err(|_| {
                IndexerError::MpscChannel(
                    "Failed to send checkpoint object changes, receiver half closed".into(),
                )
            })?;
        Ok(())
    }
}

#[async_trait]
impl Handler<TransactionObjectChangesToCommit> for ObjectsSnapshotHandler {
    fn name(&self) -> String {
        "objects_snapshot_handler".to_string()
    }

    async fn load(
        &self,
        transformed_data: Vec<TransactionObjectChangesToCommit>,
    ) -> IndexerResult<()> {
        self.store
            .backfill_objects_snapshot(transformed_data)
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
) -> IndexerResult<(ObjectsSnapshotHandler, u64)> {
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
    let common_handler = CommonHandler::new(Box::new(objects_snapshot_handler.clone()));
    spawn_monitored_task!(common_handler.start_transform_and_load(receiver, cancel));
    Ok((objects_snapshot_handler, watermark_hi.unwrap_or_default()))
}

impl ObjectsSnapshotHandler {
    pub fn new(
        store: PgIndexerStore,
        sender: Sender<(u64, TransactionObjectChangesToCommit)>,
        metrics: IndexerMetrics,
        snapshot_config: SnapshotLagConfig,
    ) -> ObjectsSnapshotHandler {
        Self {
            store,
            sender,
            metrics,
            snapshot_config,
        }
    }
}
