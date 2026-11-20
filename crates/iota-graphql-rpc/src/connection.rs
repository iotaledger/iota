// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_graphql::SimpleObject;
use iota_indexer::pruning::oldest_available_checkpoint::OldestAvailableCheckpoint;

use crate::types::uint53::UInt53;

/// The `oldestAvailableCheckpoint` field, to be added to a connection as its
/// additional fields.
#[derive(SimpleObject, Default)]
pub(crate) struct OldestAvailableCheckpointField {
    /// The oldest checkpoint this page can contain data from. Anything before
    /// it has been pruned and is not listed. Zero when nothing has been
    /// pruned, or when the historical fallback serves the whole history. Null
    /// when no bound applies to the page, such as a lookup by digest, or a
    /// page whose filters contradict each other.
    pub oldest_available_checkpoint: Option<UInt53>,
}

impl From<OldestAvailableCheckpoint> for OldestAvailableCheckpointField {
    fn from(oldest_available_checkpoint: OldestAvailableCheckpoint) -> Self {
        Self {
            oldest_available_checkpoint: Some(UInt53::new_unchecked(u64::from(
                oldest_available_checkpoint,
            ))),
        }
    }
}
