// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;

use crate::{
    ingestion::common::persist::CommitterTables,
    models::{
        checkpoints::StoredCheckpoint,
        move_call_metrics::StoredMoveCallMetrics,
        network_metrics::StoredEpochPeakTps,
        transactions::{
            StoredTransaction, StoredTransactionCheckpoint, StoredTransactionSuccessCommandCount,
            StoredTransactionTimestamp, TxSeq,
        },
        tx_count_metrics::StoredTxCountMetrics,
    },
    types::IndexerResult,
};

/// The lowest epoch, checkpoint and transaction still available in a set of
/// tables, taken from their pruning watermarks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WatermarkLowerBounds {
    pub min_available_epoch: i64,
    pub min_available_cp: i64,
    pub min_available_tx: i64,
}

/// Provides methods to get and persist metrics. Utility methods for calculating
/// metrics are also provided.
#[async_trait]
pub trait IndexerAnalyticalStore {
    /// Returns the lower bounds shared by all of `tables`, that is the highest
    /// lower bound among them, so that a range starting there has rows in
    /// every table.
    async fn get_watermark_lower_bounds(
        &self,
        tables: &[CommitterTables],
    ) -> IndexerResult<WatermarkLowerBounds>;
    async fn get_latest_stored_transaction(&self) -> IndexerResult<Option<StoredTransaction>>;
    async fn get_latest_stored_checkpoint(&self) -> IndexerResult<Option<StoredCheckpoint>>;
    async fn get_checkpoints_in_range(
        &self,
        start_checkpoint: i64,
        end_checkpoint: i64,
    ) -> IndexerResult<Vec<StoredCheckpoint>>;
    async fn get_tx_timestamps_in_checkpoint_range(
        &self,
        start_checkpoint: i64,
        end_checkpoint: i64,
    ) -> IndexerResult<Vec<StoredTransactionTimestamp>>;
    async fn get_tx_checkpoints_in_checkpoint_range(
        &self,
        start_checkpoint: i64,
        end_checkpoint: i64,
    ) -> IndexerResult<Vec<StoredTransactionCheckpoint>>;
    async fn get_tx_success_cmd_counts_in_checkpoint_range(
        &self,
        start_checkpoint: i64,
        end_checkpoint: i64,
    ) -> IndexerResult<Vec<StoredTransactionSuccessCommandCount>>;
    async fn get_tx(&self, tx_sequence_number: i64) -> IndexerResult<Option<StoredTransaction>>;
    async fn get_cp(&self, sequence_number: i64) -> IndexerResult<Option<StoredCheckpoint>>;

    // for network metrics including TPS and counts of objects etc.
    async fn get_latest_tx_count_metrics(&self) -> IndexerResult<Option<StoredTxCountMetrics>>;
    async fn get_latest_epoch_peak_tps(&self) -> IndexerResult<Option<StoredEpochPeakTps>>;
    fn persist_tx_count_metrics(
        &self,
        start_checkpoint: i64,
        end_checkpoint: i64,
    ) -> IndexerResult<()>;
    async fn persist_epoch_peak_tps(&self, epoch: i64) -> IndexerResult<()>;

    // for address metrics
    async fn get_address_metrics_last_processed_tx_seq(&self) -> IndexerResult<Option<TxSeq>>;
    fn persist_addresses_in_tx_range(
        &self,
        start_tx_seq: i64,
        end_tx_seq: i64,
    ) -> IndexerResult<()>;
    async fn calculate_and_persist_address_metrics(&self, checkpoint: i64) -> IndexerResult<()>;

    // for move call metrics
    async fn get_latest_move_call_metrics(&self) -> IndexerResult<Option<StoredMoveCallMetrics>>;
    async fn get_latest_move_call_tx_seq(&self) -> IndexerResult<Option<TxSeq>>;
    fn persist_move_calls_in_tx_range(
        &self,
        start_tx_seq: i64,
        end_tx_seq: i64,
    ) -> IndexerResult<()>;
    async fn calculate_and_persist_move_call_metrics(&self, epoch: i64) -> IndexerResult<()>;
}
