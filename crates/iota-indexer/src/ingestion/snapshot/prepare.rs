// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use async_trait::async_trait;
use iota_data_ingestion_core::Worker;
use iota_metrics::metered_channel::Sender;
use iota_types::full_checkpoint_content::CheckpointData;

use crate::{
    config::SnapshotLagConfig,
    errors::IndexerError,
    ingestion::primary::{persist::TransactionObjectChangesToCommit, prepare::PrimaryWorker},
    metrics::IndexerMetrics,
    store::PgIndexerStore,
};

#[derive(Clone)]
pub struct ObjectsSnapshotHandler {
    pub store: PgIndexerStore,
    pub sender: Sender<(u64, TransactionObjectChangesToCommit)>,
    pub(crate) snapshot_config: SnapshotLagConfig,
    pub(crate) metrics: IndexerMetrics,
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

#[async_trait]
impl Worker for ObjectsSnapshotHandler {
    type Message = ();
    type Error = IndexerError;

    async fn process_checkpoint(
        &self,
        checkpoint: Arc<CheckpointData>,
    ) -> Result<Self::Message, Self::Error> {
        let transformed_data = PrimaryWorker::index_objects(&checkpoint, &self.metrics).await?;
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
