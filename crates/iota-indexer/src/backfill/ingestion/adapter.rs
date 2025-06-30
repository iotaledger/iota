// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use dashmap::DashMap;
use iota_data_ingestion_core::Worker;
use iota_types::{
    full_checkpoint_content::CheckpointData, messages_checkpoint::CheckpointSequenceNumber,
};
use tokio::sync::Notify;

use crate::backfill::ingestion::IngestionBackfill;

#[derive(Clone)]
pub(crate) struct Adapter<T: IngestionBackfill> {
    pub(crate) ready_checkpoints: Arc<DashMap<CheckpointSequenceNumber, Vec<T::ProcessedType>>>,
    pub(crate) notify: Arc<Notify>,
}

#[async_trait::async_trait]
impl<T: IngestionBackfill> Worker for Adapter<T> {
    type Error = anyhow::Error;
    type Message = ();

    async fn process_checkpoint(&self, checkpoint: Arc<CheckpointData>) -> anyhow::Result<()> {
        let processed = T::process_checkpoint(checkpoint.clone());
        self.ready_checkpoints
            .insert(checkpoint.checkpoint_summary.sequence_number, processed);
        self.notify.notify_waiters();
        Ok(())
    }
}
