// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Types and associated logic to use while persisting
//! data to the database.

use iota_types::full_checkpoint_content::CheckpointData;
use serde::{Deserialize, Serialize};

use crate::types::IndexedCheckpoint;

pub(crate) const CHECKPOINT_COMMIT_BATCH_SIZE: usize = 100;

/// The indexer writer operates on checkpoint data, which contains information
/// on the current epoch, checkpoint, and transaction.
///
/// These three numbers form the watermark upper bound for each committed table.
/// The reader and pruner are responsible for determining which of the three
/// units will be used for a particular table.
#[derive(Clone, Copy, Ord, PartialOrd, Eq, PartialEq)]
pub struct CommitterWatermark {
    /// Current epoch for given table. Doesn't mean that data for the
    /// whole epoch is persisted as it still may be in progress.
    pub current_epoch: u64,
    /// Maximum committed checkpoint for which all data is already written for
    /// given table.
    pub max_committed_cp: u64,
    /// Maximum committed transaction sequence number for this table's data.
    /// This is inclusive.
    pub max_committed_tx: u64,
}
impl From<&IndexedCheckpoint> for CommitterWatermark {
    fn from(checkpoint: &IndexedCheckpoint) -> Self {
        Self {
            current_epoch: checkpoint.epoch,
            max_committed_cp: checkpoint.sequence_number,
            max_committed_tx: checkpoint.network_total_transactions.saturating_sub(1),
        }
    }
}
impl From<&CheckpointData> for CommitterWatermark {
    fn from(checkpoint: &CheckpointData) -> Self {
        Self {
            current_epoch: checkpoint.checkpoint_summary.epoch,
            max_committed_cp: checkpoint.checkpoint_summary.sequence_number,
            max_committed_tx: checkpoint
                .checkpoint_summary
                .network_total_transactions
                .saturating_sub(1),
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
    ObjectsBackwardHistory,
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
    TxWrappedOrDeletedObjects,
    TxGlobalOrder,
    Checkpoints,
    PrunerCpWatermark,
}

/// Enum representing tables that are written by optimistic indexing, and not by
/// main pipeline
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
pub enum OptimisticIndexingTables {
    OptimisticTransactions,
}
