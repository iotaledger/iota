// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use diesel::prelude::*;

use crate::{schema::raw_checkpoints, types::IndexedCheckpoint};

#[derive(Queryable, Insertable, Selectable, Debug, Clone, Default)]
#[diesel(table_name = raw_checkpoints)]
pub struct StoredRawCheckpoint {
    pub sequence_number: i64,
    /// BCS serialized CertifiedCheckpointSummary
    pub certified_checkpoint: Vec<u8>,
    /// BCS serialized CheckpointContents
    pub checkpoint_contents: Vec<u8>,
}

impl From<&IndexedCheckpoint> for StoredRawCheckpoint {
    fn from(c: &IndexedCheckpoint) -> Self {
        Self {
            sequence_number: c.sequence_number as i64,
            certified_checkpoint: bcs::to_bytes(c.certified_checkpoint.as_ref().unwrap()).unwrap(),
            checkpoint_contents: bcs::to_bytes(c.checkpoint_contents.as_ref().unwrap()).unwrap(),
        }
    }
}
