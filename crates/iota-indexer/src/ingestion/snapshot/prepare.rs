// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use async_trait::async_trait;
use iota_data_ingestion_core::Worker;
use iota_metrics::metered_channel::Sender;
use iota_types::full_checkpoint_content::CheckpointData;

use crate::{
    errors::IndexerError,
    ingestion::{
        common::persist::CommitterWatermark,
        primary::{persist::TransactionObjectChangesToCommit, prepare::PrimaryWorker},
    },
    metrics::IndexerMetrics,
};

#[derive(Clone)]
pub struct ObjectsSnapshotWorker {
    pub sender: Sender<(CommitterWatermark, TransactionObjectChangesToCommit)>,
    pub(crate) metrics: IndexerMetrics,
}

impl ObjectsSnapshotWorker {
    pub fn new(
        sender: Sender<(CommitterWatermark, TransactionObjectChangesToCommit)>,
        metrics: IndexerMetrics,
    ) -> ObjectsSnapshotWorker {
        Self { sender, metrics }
    }
}

#[async_trait]
impl Worker for ObjectsSnapshotWorker {
    type Message = ();
    type Error = IndexerError;

    async fn process_checkpoint(
        &self,
        checkpoint: Arc<CheckpointData>,
    ) -> Result<Self::Message, Self::Error> {
        let transformed_data = PrimaryWorker::index_objects(&checkpoint, &self.metrics).await?;
        self.sender
            .send((
                CommitterWatermark::from(checkpoint.as_ref()),
                transformed_data,
            ))
            .await
            .map_err(|_| {
                IndexerError::MpscChannel(
                    "failed to send checkpoint object changes, receiver half closed".into(),
                )
            })?;
        Ok(())
    }
}
