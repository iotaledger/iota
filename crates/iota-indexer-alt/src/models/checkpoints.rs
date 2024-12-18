// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Result, anyhow};
use diesel::prelude::*;
use iota_field_count::FieldCount;
use iota_protocol_config::{Chain, ProtocolVersion};
use iota_types::digests::{ChainIdentifier, CheckpointDigest};

use crate::schema::{kv_checkpoints, kv_genesis};

#[derive(Insertable, Debug, Clone, FieldCount)]
#[diesel(table_name = kv_checkpoints)]
pub struct StoredCheckpoint {
    pub sequence_number: i64,
    /// BCS serialized CertifiedCheckpointSummary
    pub certified_checkpoint: Vec<u8>,
    /// BCS serialized CheckpointContents
    pub checkpoint_contents: Vec<u8>,
}

#[derive(Insertable, Selectable, Queryable, Debug, Clone)]
#[diesel(table_name = kv_genesis)]
pub struct StoredGenesis {
    pub genesis_digest: Vec<u8>,
    pub initial_protocol_version: i64,
}

impl StoredGenesis {
    /// Try and identify the chain that this indexer is idnexing based on its
    /// genesis checkpoint digest.
    pub fn chain(&self) -> Result<Chain> {
        let bytes: [u8; 32] = self
            .genesis_digest
            .clone()
            .try_into()
            .map_err(|_| anyhow!("Bad genesis digest"))?;

        let digest = CheckpointDigest::new(bytes);
        let identifier = ChainIdentifier::from(digest);

        Ok(identifier.chain())
    }

    /// The protocol version that the chain was started at.
    pub fn initial_protocol_version(&self) -> ProtocolVersion {
        ProtocolVersion::new(self.initial_protocol_version as u64)
    }
}
