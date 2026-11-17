// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
//
use std::str::FromStr;

use diesel::prelude::*;
use iota_types::full_checkpoint_content::CheckpointData;

use crate::{
    pruning::pruner::PrunableTable,
    schema::watermarks::{self},
    types::IndexedCheckpoint,
};

/// The indexer writer operates on checkpoint data, which contains information
/// on the current epoch, checkpoint, and transaction.
///
/// These three numbers form the watermark upper bound for each committed table.
/// The reader and pruner are responsible for determining which of the three
/// units will be used for a particular table.
#[derive(Clone, Copy, Ord, PartialOrd, Eq, PartialEq)]
pub struct WatermarkUpperBounds {
    /// Current epoch for given table. Doesn't mean that data for the
    /// whole epoch is persisted as it still may be in progress.
    pub current_epoch: i64,
    /// Maximum committed checkpoint for which all data is already written for
    /// given table.
    pub max_committed_cp: i64,
    /// Maximum committed transaction sequence number for this table's data.
    /// This is inclusive.
    pub max_committed_tx: i64,
}

impl From<&IndexedCheckpoint> for WatermarkUpperBounds {
    fn from(checkpoint: &IndexedCheckpoint) -> Self {
        Self {
            current_epoch: checkpoint.epoch as i64,
            max_committed_cp: checkpoint.sequence_number as i64,
            max_committed_tx: checkpoint.network_total_transactions.saturating_sub(1) as i64,
        }
    }
}

impl From<&CheckpointData> for WatermarkUpperBounds {
    fn from(checkpoint: &CheckpointData) -> Self {
        Self {
            current_epoch: checkpoint.checkpoint_summary.epoch as i64,
            max_committed_cp: checkpoint.checkpoint_summary.sequence_number as i64,
            max_committed_tx: checkpoint
                .checkpoint_summary
                .network_total_transactions
                .saturating_sub(1) as i64,
        }
    }
}

/// The lower bound of a watermark: the first available epoch, checkpoint, and
/// transaction of a table.
///
/// The pruner raises it when data goes out of the retention window. The indexer writer sets it to
/// the first checkpoint of a batch for a table that is new, or that has a gap before
/// the batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct WatermarkLowerBounds {
    /// Minimum available epoch.
    pub min_available_epoch: i64,
    /// Minimum available checkpoint sequence number.
    pub min_available_cp: i64,
    /// Minimum available transaction sequence number.
    pub min_available_tx: i64,
}

impl From<&IndexedCheckpoint> for WatermarkLowerBounds {
    fn from(checkpoint: &IndexedCheckpoint) -> Self {
        Self {
            min_available_epoch: checkpoint.epoch as i64,
            min_available_cp: checkpoint.sequence_number as i64,
            min_available_tx: checkpoint.min_tx_sequence_number as i64,
        }
    }
}

/// Represents a row in the `watermarks` table.
#[derive(Queryable, Insertable, Default, QueryableByName, Clone)]
#[diesel(table_name = watermarks, primary_key(entity))]
pub struct StoredWatermark {
    /// The table governed by this watermark, i.e `epochs`, `checkpoints`,
    /// `transactions`.
    pub entity: String,
    /// Current epoch for this entity's data. Committer updates this field.
    /// Pruner uses this to determine if pruning is necessary based on the
    /// retention policy.
    pub current_epoch: i64,
    /// Maximum committed checkpoint for this entity's data. Committer
    /// updates this field. All data of this entity in the checkpoint must
    /// be persisted before advancing this watermark. The committer refers
    /// to this on disaster recovery to resume writing.
    pub max_committed_cp: i64,
    /// Maximum committed transaction sequence number for this entity's data.
    /// Committer updates this field. This is inclusive.
    pub max_committed_tx: i64,
    /// Minimum available epoch for this entity's data. Pruner updates this
    /// field when the epoch range exceeds the retention policy.
    ///
    /// The writer also updates this on ingested tables with gaps.
    pub min_available_epoch: i64,
    /// Timestamp in milliseconds of the last update to the min_available_*
    /// columns. The pruner uses this to determine whether to prune or wait
    /// long enough that all in-flight reads complete or timeout before it
    /// acts on an updated watermark.
    pub min_bounds_updated_at_timestamp_ms: i64,
    /// Lowest key (epoch, checkpoint, or tx) that has not been pruned yet.
    /// The pruner uses this to track its progress.
    pub lowest_unpruned_key: i64,
    /// Minimum available transaction sequence number for this entity's data.
    /// Pruner updates this field based on min_available_epoch.
    ///
    /// The writer also updates this on ingested tables with gaps.
    pub min_available_tx: i64,
    /// Minimum available checkpoint sequence number for this entity's data.
    /// Pruner updates this field based on min_available_epoch.
    ///
    /// The writer also updates this on ingested tables with gaps.
    pub min_available_cp: i64,
}

impl StoredWatermark {
    /// Creates a row that holds only the upper bound of a watermark.
    pub fn from_upper_bound_update(entity: &str, upper_bound: WatermarkUpperBounds) -> Self {
        StoredWatermark {
            entity: entity.to_string(),
            current_epoch: upper_bound.current_epoch,
            max_committed_cp: upper_bound.max_committed_cp,
            max_committed_tx: upper_bound.max_committed_tx,
            ..StoredWatermark::default()
        }
    }

    /// Creates a row that holds only the lower bound of a watermark.
    pub fn from_lower_bound_update(entity: &str, lower_bound: WatermarkLowerBounds) -> Self {
        StoredWatermark {
            entity: entity.to_string(),
            min_available_epoch: lower_bound.min_available_epoch,
            min_available_cp: lower_bound.min_available_cp,
            min_available_tx: lower_bound.min_available_tx,
            ..StoredWatermark::default()
        }
    }

    /// Creates a value from both the lower and upper bounds.
    pub fn from_bounds(
        entity: &str,
        upper_bound: WatermarkUpperBounds,
        lower_bound: WatermarkLowerBounds,
    ) -> Self {
        StoredWatermark {
            current_epoch: upper_bound.current_epoch,
            max_committed_cp: upper_bound.max_committed_cp,
            max_committed_tx: upper_bound.max_committed_tx,
            ..Self::from_lower_bound_update(entity, lower_bound)
        }
    }

    pub fn entity(&self) -> Option<PrunableTable> {
        PrunableTable::from_str(&self.entity).ok()
    }
}
