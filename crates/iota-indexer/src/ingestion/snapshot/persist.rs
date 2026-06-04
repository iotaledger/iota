// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;
use iota_types::messages_checkpoint::CheckpointSequenceNumber;

use crate::{
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
    pub(crate) metrics: IndexerMetrics,
}

impl ObjectSnapshotWriter {
    pub fn new(
        store: PgIndexerStore,
        metrics: IndexerMetrics,
    ) -> ObjectSnapshotWriter {
        Self {
            store,
            metrics,
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
            .set(watermark.max_committed_cp as i64);
        Ok(())
    }

    async fn get_max_committable_checkpoint(&self) -> IndexerResult<u64> {
        let latest_checkpoint = self.store.get_latest_checkpoint_sequence_number().await?;
        Ok(latest_checkpoint.unwrap_or_default())
    }
}
