// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Types and associated logic to use while persisting
//! data to the database.

use std::{collections::BTreeMap, time::Duration};

use async_trait::async_trait;
use futures::{FutureExt, StreamExt};
use iota_types::{
    full_checkpoint_content::CheckpointData, messages_checkpoint::CheckpointSequenceNumber,
};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::{
    errors::IndexerError,
    types::{IndexedCheckpoint, IndexerResult},
};

pub(crate) const CHECKPOINT_COMMIT_BATCH_SIZE: usize = 100;
pub(crate) const UNPROCESSED_CHECKPOINT_SIZE_LIMIT: usize = 1000;

/// Defines the logic of writing operations to the database.
///
/// The writing can refer to one or multiple tables in the database.
#[async_trait]
pub trait Writer<T: Send + Sync + 'static>: Send + Sync {
    /// Returns the writer name.
    fn name(&self) -> String;

    /// Commits batch of transformed data to DB.
    async fn persist(&self, batch: Vec<T>) -> IndexerResult<()>;

    /// Reads high watermark of the table DB.
    async fn get_watermark_hi(&self) -> IndexerResult<Option<CheckpointSequenceNumber>>;

    /// Sets high watermark of the table DB, also update metrics.
    async fn set_watermark_hi(&self, watermark_hi: CommitterWatermark) -> IndexerResult<()>;

    /// Gets the current max checkpoint that can be committed by the writer.
    ///
    /// This is for writers that have a predefined lag compared to the latest
    /// checkpoint in the network.
    ///
    /// One use-case is the objects snapshot handler, which waits for the lag
    /// between snapshot and latest checkpoint to reach a certain threshold.
    ///
    /// # Note
    /// By default, returns `u64::MAX`, which means no extra waiting is needed
    /// before committing.
    async fn get_max_committable_checkpoint(&self) -> IndexerResult<u64> {
        Ok(u64::MAX)
    }

    /// Processes the received data and persists it into a storage.
    ///
    /// - The data are received form the ingestion worker in which stage is
    ///   transformed into something which can be directly committed into the
    ///   database.
    /// - The data received by this function are not guaranteed to be in order.
    ///   The purpose of this function is to order the data by checkpoint
    ///   sequence number and to ensure data committed are in order and
    ///   contiguous.
    ///
    /// In addition, the method updates the watermark of the table of the data
    /// is persisted to.
    async fn persist_sequentially(
        &self,
        cp_receiver: iota_metrics::metered_channel::Receiver<(CommitterWatermark, T)>,
        cancel: CancellationToken,
    ) -> IndexerResult<()> {
        let checkpoint_commit_batch_size = std::env::var("CHECKPOINT_COMMIT_BATCH_SIZE")
            .ok()
            .and_then(|val| val.parse().ok())
            .unwrap_or(CHECKPOINT_COMMIT_BATCH_SIZE);
        let mut stream = iota_metrics::metered_channel::ReceiverStream::new(cp_receiver)
            .ready_chunks(checkpoint_commit_batch_size);

        // Mapping of ordered checkpoint data to ensure that we process them in order.
        // The key is just the checkpoint sequence number, and the tuple is
        // (CommitterWatermark, T).
        let mut unprocessed: BTreeMap<u64, (CommitterWatermark, _)> = BTreeMap::new();
        let mut tuple_batch = vec![];
        let mut next_cp_to_process = self
            .get_watermark_hi()
            .await?
            .map(|watermark| watermark.saturating_add(1))
            .unwrap_or_default();

        loop {
            if cancel.is_cancelled() {
                info!("transform and load task terminating gracefully");
                return Ok(());
            }

            let next_cp_present = unprocessed.contains_key(&next_cp_to_process);

            // Try to fetch new data tuple from the stream
            if unprocessed.len() >= UNPROCESSED_CHECKPOINT_SIZE_LIMIT && next_cp_present {
                tracing::debug!(
                    "Unprocessed checkpoint size reached limit {UNPROCESSED_CHECKPOINT_SIZE_LIMIT} \
                     at checkpoint {next_cp_to_process}. Current queue size: {}. Skip reading from stream...",
                    unprocessed.len()
                );
                tokio::time::sleep(Duration::from_secs(1)).await;
            } else {
                // Try to fetch new data tuple from the stream
                match stream.next().now_or_never() {
                    Some(Some(tuple_chunk)) => {
                        if cancel.is_cancelled() {
                            info!("transform and load task terminating gracefully");
                            return Ok(());
                        }
                        for (watermark, data) in tuple_chunk {
                            unprocessed
                                .insert(watermark.checkpoint_hi_inclusive, (watermark, data));
                        }
                    }
                    Some(None) => break, // Stream has ended
                    None => {}           // No new data tuple available right now
                }
            }

            // Process unprocessed checkpoints, even no new checkpoints from stream
            let checkpoint_lag_limiter = self.get_max_committable_checkpoint().await?;
            while next_cp_to_process <= checkpoint_lag_limiter {
                if let Some(data_tuple) = unprocessed.remove(&next_cp_to_process) {
                    tuple_batch.push(data_tuple);
                    next_cp_to_process += 1;
                } else {
                    break;
                }
            }

            if !tuple_batch.is_empty() && checkpoint_lag_limiter != 0 {
                let tuple_batch = std::mem::take(&mut tuple_batch);
                let (committer_watermark, _data) = tuple_batch.last().unwrap();
                let committer_watermark = committer_watermark.to_owned();
                let batch = tuple_batch
                    .into_iter()
                    .map(|(_cp_seq, data)| data)
                    .collect();
                self.persist(batch).await.map_err(|e| {
                    IndexerError::PostgresWrite(format!(
                        "failed to load transformed data into DB for handler {}: {e}",
                        self.name()
                    ))
                })?;
                self.set_watermark_hi(committer_watermark).await?;
            }
        }
        Err(IndexerError::ChannelClosed(format!(
            "checkpoint channel is closed unexpectedly for handler {}",
            self.name()
        )))
    }
}

/// The indexer writer operates on checkpoint data, which contains information
/// on the current epoch, checkpoint, and transaction.
///
/// These three numbers form the watermark upper bound for each committed table.
/// The reader and pruner are responsible for determining which of the three
/// units will be used for a particular table.
#[derive(Clone, Copy, Ord, PartialOrd, Eq, PartialEq)]
pub struct CommitterWatermark {
    /// Highest epoch written for given table. Doesn't mean that data for the
    /// whole epoch is persisted as it still may be in progress.
    pub epoch_hi_inclusive: u64,
    /// Highest checkpoint for which all data is already written for given
    /// table.
    pub checkpoint_hi_inclusive: u64,
    /// Exclusive upper transaction sequence number bound for this table's
    /// data.
    pub tx_hi: u64,
}
impl From<&IndexedCheckpoint> for CommitterWatermark {
    fn from(checkpoint: &IndexedCheckpoint) -> Self {
        Self {
            epoch_hi_inclusive: checkpoint.epoch,
            checkpoint_hi_inclusive: checkpoint.sequence_number,
            tx_hi: checkpoint.network_total_transactions,
        }
    }
}
impl From<&CheckpointData> for CommitterWatermark {
    fn from(checkpoint: &CheckpointData) -> Self {
        Self {
            epoch_hi_inclusive: checkpoint.checkpoint_summary.epoch,
            checkpoint_hi_inclusive: checkpoint.checkpoint_summary.sequence_number,
            tx_hi: checkpoint.checkpoint_summary.network_total_transactions,
        }
    }
}
/// Enum representing tables that a committer updates.
#[derive(
    Debug,
    Eq,
    PartialEq,
    strum_macros::Display,
    strum_macros::EnumString,
    strum_macros::EnumIter,
    strum_macros::AsRefStr,
    Hash,
    Serialize,
    Deserialize,
    Clone,
)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum CommitterTables {
    // Unpruned tables
    ChainIdentifier,
    Display,
    Epochs,
    FeatureFlags,
    Objects,
    ObjectsVersion,
    Packages,
    ProtocolConfigs,
    // Prunable tables
    ObjectsHistory,
    Transactions,
    Events,
    EventEmitPackage,
    EventEmitModule,
    EventSenders,
    EventStructInstantiation,
    EventStructModule,
    EventStructName,
    EventStructPackage,
    TxCallsPkg,
    TxCallsMod,
    TxCallsFun,
    TxChangedObjects,
    TxDigests,
    TxInputObjects,
    TxKinds,
    TxRecipients,
    TxSenders,
    Checkpoints,
    PrunerCpWatermark,
}
/// Enum representing tables that the objects snapshot processor updates.
#[derive(
    Debug,
    Eq,
    PartialEq,
    strum_macros::Display,
    strum_macros::EnumString,
    strum_macros::EnumIter,
    strum_macros::AsRefStr,
    Hash,
    Serialize,
    Deserialize,
    Clone,
)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ObjectsSnapshotHandlerTables {
    ObjectsSnapshot,
}
