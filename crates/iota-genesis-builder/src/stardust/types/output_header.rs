// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Type representing output header for a Stardust snapshot.
use std::mem::size_of;

use anyhow::Result;
use iota_sdk::types::block::{output::OutputId, payload::milestone::MilestoneIndex, BlockId};
use iota_types::base_types::ObjectID;
use packable::Packable;

/// The total supply on the iota-mainnet
pub const TOTAL_SUPPLY_IOTA: u64 = 4_600_000_000_000_000;

/// The header of an [`Output`] in the snapshot
#[derive(Debug, Clone, Packable)]
pub struct OutputHeader {
    output_id: OutputId,
    block_id: BlockId,
    ms_index: MilestoneIndex,
    ms_ts: u32,
    length: u32,
}

impl OutputHeader {
    /// The length of the header in bytes
    pub const LENGTH: usize = OutputId::LENGTH
        + size_of::<BlockId>()
        + size_of::<MilestoneIndex>()
        + 2 * size_of::<u32>();

    pub fn output_id(&self) -> OutputId {
        self.output_id
    }

    pub fn new_object_id(&self) -> ObjectID {
        ObjectID::new(self.output_id.hash())
    }

    pub fn block_id(&self) -> BlockId {
        self.block_id
    }

    /// Get the milestone index
    pub fn ms_index(&self) -> MilestoneIndex {
        self.ms_index
    }

    /// Get the milestone timestamp in Unix time
    pub fn ms_timestamp(&self) -> u32 {
        self.ms_ts
    }

    /// The length of the output in bytes.
    pub fn length(&self) -> u32 {
        self.length
    }

    /// Creates a new OutputHeader for testing.
    pub fn new_testing(
        transaction_id_bytes: [u8; 32],
        output_index: u16,
        block_id_bytes: [u8; 32],
        milestone_index: u32,
        milestone_timestamp: u32,
    ) -> OutputHeader {
        use iota_sdk::types::block::payload::transaction::TransactionId;

        OutputHeader {
            output_id: OutputId::new(TransactionId::new(transaction_id_bytes), output_index)
                .unwrap(),
            block_id: BlockId::new(block_id_bytes),
            ms_index: MilestoneIndex::new(milestone_index),
            ms_ts: milestone_timestamp,
            length: 1,
        }
    }
}
