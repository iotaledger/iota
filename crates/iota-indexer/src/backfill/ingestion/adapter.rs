// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use dashmap::DashMap;
use iota_data_ingestion_core::Worker;
use iota_types::{
    full_checkpoint_content::CheckpointData, messages_checkpoint::CheckpointSequenceNumber,
};
use tokio::sync::Notify;

use crate::{backfill::ingestion::IngestionBackfill, errors::IndexerError};

/// Bridge between the ingestion engine and the backfill task.
#[derive(Clone)]
pub(crate) struct Adapter<T: IngestionBackfill> {
    pub(crate) ready_checkpoints: Arc<DashMap<CheckpointSequenceNumber, Vec<T::ProcessedType>>>,
    pub(crate) notify: Arc<Notify>,
}

/// The `Adapter` receives `CheckpointData` from the ingestion pipeline,
/// uses `T::process_checkpoint` to transform it and stores the processed data
/// in `ready_checkpoints`. It then signals any waiting backfill jobs via
/// `notify`.
#[async_trait::async_trait]
impl<T: IngestionBackfill> Worker for Adapter<T> {
    type Message = ();
    type Error = IndexerError;

    async fn process_checkpoint(
        &self,
        checkpoint: Arc<CheckpointData>,
    ) -> Result<(), IndexerError> {
        let processed = T::process_checkpoint(checkpoint.clone()).await?;
        self.ready_checkpoints
            .insert(checkpoint.checkpoint_summary.sequence_number, processed);
        self.notify.notify_waiters();
        Ok(())
    }
}
