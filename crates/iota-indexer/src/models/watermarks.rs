// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
//
use std::str::FromStr;

use diesel::prelude::*;

use crate::{
    ingestion::common::persist::CommitterWatermark,
    pruning::pruner::PrunableTable,
    schema::watermarks::{self},
};

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
    pub min_available_tx: i64,
    /// Minimum available checkpoint sequence number for this entity's data.
    /// Pruner updates this field based on min_available_epoch.
    pub min_available_cp: i64,
}

impl StoredWatermark {
    pub fn from_upper_bound_update(entity: &str, watermark: CommitterWatermark) -> Self {
        StoredWatermark {
            entity: entity.to_string(),
            current_epoch: watermark.current_epoch as i64,
            max_committed_cp: watermark.max_committed_cp as i64,
            max_committed_tx: watermark.max_committed_tx as i64,
            ..StoredWatermark::default()
        }
    }

    pub fn from_lower_bound_update(
        entity: &str,
        min_available_epoch: u64,
        min_available_cp: u64,
        min_available_tx: u64,
    ) -> Self {
        StoredWatermark {
            entity: entity.to_string(),
            min_available_epoch: min_available_epoch as i64,
            min_available_cp: min_available_cp as i64,
            min_available_tx: min_available_tx as i64,
            ..StoredWatermark::default()
        }
    }

    pub fn entity(&self) -> Option<PrunableTable> {
        PrunableTable::from_str(&self.entity).ok()
    }

    /// Determine whether to set a new minimum available epoch based on the
    /// retention policy.
    pub fn new_min_available_epoch(&self, retention: u64) -> Option<u64> {
        if self.min_available_epoch as u64 + retention <= self.current_epoch as u64 {
            Some((self.current_epoch as u64).saturating_sub(retention - 1))
        } else {
            None
        }
    }
}
