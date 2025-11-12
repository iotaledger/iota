// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;
use iota_types::messages_checkpoint::CheckpointSequenceNumber;

use crate::{
    config::SnapshotLagConfig,
    ingestion::{
        common::persist::{CommitterWatermark, ObjectsSnapshotHandlerTables, Writer},
        primary::persist::TransactionObjectChangesToCommit,
    },
    metrics::IndexerMetrics,
    store::{IndexerStore, PgIndexerStore},
    types::IndexerResult,
};

#[derive(Clone)]
pub(crate) struct ObjectSnapshotWriter {
    pub store: PgIndexerStore,
    pub(crate) snapshot_config: SnapshotLagConfig,
    pub(crate) metrics: IndexerMetrics,
}

impl ObjectSnapshotWriter {
    pub fn new(
        store: PgIndexerStore,
        metrics: IndexerMetrics,
        snapshot_config: SnapshotLagConfig,
    ) -> ObjectSnapshotWriter {
        Self {
            store,
            metrics,
            snapshot_config,
        }
    }
}

#[async_trait]
impl Writer<TransactionObjectChangesToCommit> for ObjectSnapshotWriter {
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

    async fn get_watermark_hi(&self) -> IndexerResult<Option<CheckpointSequenceNumber>> {
        self.store
            .get_latest_object_snapshot_checkpoint_sequence_number()
            .await
    }

    async fn set_watermark_hi(&self, watermark: CommitterWatermark) -> IndexerResult<()> {
        self.store
            .update_watermarks_upper_bound::<ObjectsSnapshotHandlerTables>(watermark)
            .await?;
        self.metrics
            .latest_object_snapshot_sequence_number
            .set(watermark.checkpoint_hi_inclusive as i64);
        Ok(())
    }

    async fn get_max_committable_checkpoint(&self) -> IndexerResult<u64> {
        let latest_checkpoint = self.store.get_latest_checkpoint_sequence_number().await?;
        Ok(latest_checkpoint
            .map(|seq| seq.saturating_sub(self.snapshot_config.snapshot_min_lag as u64))
            .unwrap_or_default()) // hold snapshot handler until at least one checkpoint is in DB
    }
}
