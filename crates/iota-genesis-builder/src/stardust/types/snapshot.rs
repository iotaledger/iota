// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Types representing blocks of data in a Stardust snapshot.
use std::mem::size_of;

use anyhow::Result;
use iota_sdk::types::block::{
    output::OutputId,
    payload::milestone::{MilestoneId, MilestoneIndex, MilestoneOption},
    protocol::ProtocolParameters,
    BlockId,
};
use packable::{
    error::{UnpackError, UnpackErrorExt},
    packer::Packer,
    unpacker::Unpacker,
    Packable, PackableExt,
};

use crate::stardust::error::StardustError;

/// The snapshot version supported currently
const SNAPSHOT_VERSION: u8 = 2;
/// The total supply on the iota-mainnet
pub const TOTAL_SUPPLY_IOTA: u64 = 4_600_000_000_000_000;

/// The kind of a snapshot.
#[repr(u8)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, packable::Packable)]
#[packable(unpack_error = StardustError)]
pub enum SnapshotKind {
    /// Full is a snapshot which contains the full ledger entry for a given
    /// milestone plus the milestone diffs which subtracted to the ledger
    /// milestone reduce to the snapshot milestone ledger.
    Full = 0,
    /// Delta is a snapshot which contains solely diffs of milestones newer than
    /// a certain ledger milestone instead of the complete ledger state of a
    /// given milestone.
    Delta = 1,
}

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
        block_id_bytes: [u8; 32],
        milestone_index: u32,
        milestone_timestamp: u32,
    ) -> OutputHeader {
        use iota_sdk::types::block::payload::transaction::TransactionId;

        OutputHeader {
            output_id: OutputId::new(TransactionId::new(transaction_id_bytes), 0).unwrap(),
            block_id: BlockId::new(block_id_bytes),
            ms_index: MilestoneIndex::new(milestone_index),
            ms_ts: milestone_timestamp,
            length: 1,
        }
    }
}

/// Describes a snapshot header specific to full snapshots.
#[derive(Clone, Debug)]
pub struct FullSnapshotHeader {
    genesis_milestone_index: MilestoneIndex,
    target_milestone_index: MilestoneIndex,
    target_milestone_timestamp: u32,
    target_milestone_id: MilestoneId,
    ledger_milestone_index: MilestoneIndex,
    treasury_output_milestone_id: MilestoneId,
    treasury_output_amount: u64,
    parameters_milestone_option: MilestoneOption,
    output_count: u64,
    milestone_diff_count: u32,
    sep_count: u16,
}

impl FullSnapshotHeader {
    /// The length of the header in bytes
    pub const LENGTH: usize = std::mem::size_of::<Self>();

    /// Returns the genesis milestone index of a [`FullSnapshotHeader`].
    pub fn genesis_milestone_index(&self) -> MilestoneIndex {
        self.genesis_milestone_index
    }

    /// Returns the target milestone index of a [`FullSnapshotHeader`].
    pub fn target_milestone_index(&self) -> MilestoneIndex {
        self.target_milestone_index
    }

    /// Returns the target milestone timestamp of a [`FullSnapshotHeader`].
    pub fn target_milestone_timestamp(&self) -> u32 {
        self.target_milestone_timestamp
    }

    /// Returns the target milestone ID of a [`FullSnapshotHeader`].
    pub fn target_milestone_id(&self) -> &MilestoneId {
        &self.target_milestone_id
    }

    /// Returns the ledger milestone index of a [`FullSnapshotHeader`].
    pub fn ledger_milestone_index(&self) -> MilestoneIndex {
        self.ledger_milestone_index
    }

    /// Returns the treasury output milestone ID of a [`FullSnapshotHeader`].
    pub fn treasury_output_milestone_id(&self) -> &MilestoneId {
        &self.treasury_output_milestone_id
    }

    /// Returns the treasury output amount of a [`FullSnapshotHeader`].
    pub fn treasury_output_amount(&self) -> u64 {
        self.treasury_output_amount
    }

    /// Returns the parameters milestone option of a [`FullSnapshotHeader`].
    pub fn parameters_milestone_option(&self) -> &MilestoneOption {
        &self.parameters_milestone_option
    }

    /// Returns the output count of a [`FullSnapshotHeader`].
    pub fn output_count(&self) -> u64 {
        self.output_count
    }

    /// Returns the milestone diff count of a [`FullSnapshotHeader`].
    pub fn milestone_diff_count(&self) -> u32 {
        self.milestone_diff_count
    }

    /// Returns the SEP count of a [`FullSnapshotHeader`].
    pub fn sep_count(&self) -> u16 {
        self.sep_count
    }
}

impl Packable for FullSnapshotHeader {
    type UnpackVisitor = ();
    type UnpackError = StardustError;

    fn pack<P: Packer>(&self, packer: &mut P) -> Result<(), P::Error> {
        SNAPSHOT_VERSION.pack(packer)?;
        SnapshotKind::Full.pack(packer)?;

        self.genesis_milestone_index.pack(packer)?;
        self.target_milestone_index.pack(packer)?;
        self.target_milestone_timestamp.pack(packer)?;
        self.target_milestone_id.pack(packer)?;
        self.ledger_milestone_index.pack(packer)?;
        self.treasury_output_milestone_id.pack(packer)?;
        self.treasury_output_amount.pack(packer)?;
        // This is only required in Hornet.
        (self.parameters_milestone_option.packed_len() as u16).pack(packer)?;
        self.parameters_milestone_option.pack(packer)?;
        self.output_count.pack(packer)?;
        self.milestone_diff_count.pack(packer)?;
        self.sep_count.pack(packer)?;

        Ok(())
    }

    fn unpack<U: Unpacker, const VERIFY: bool>(
        unpacker: &mut U,
        _: &(),
    ) -> Result<Self, UnpackError<Self::UnpackError, U::Error>> {
        let version = u8::unpack::<_, VERIFY>(unpacker, &()).coerce()?;

        if VERIFY && SNAPSHOT_VERSION != version {
            return Err(UnpackError::Packable(
                StardustError::UnsupportedSnapshotVersion(SNAPSHOT_VERSION, version),
            ));
        }

        let kind = SnapshotKind::unpack::<_, VERIFY>(unpacker, &()).coerce()?;

        if VERIFY && kind != SnapshotKind::Full {
            return Err(UnpackError::Packable(StardustError::InvalidSnapshotKind(
                kind as u8,
            )));
        }

        let genesis_milestone_index =
            MilestoneIndex::unpack::<_, VERIFY>(unpacker, &()).coerce()?;
        let target_milestone_index = MilestoneIndex::unpack::<_, VERIFY>(unpacker, &()).coerce()?;
        let target_milestone_timestamp = u32::unpack::<_, VERIFY>(unpacker, &()).coerce()?;
        let target_milestone_id = MilestoneId::unpack::<_, VERIFY>(unpacker, &()).coerce()?;
        let ledger_milestone_index = MilestoneIndex::unpack::<_, VERIFY>(unpacker, &()).coerce()?;
        let treasury_output_milestone_id =
            MilestoneId::unpack::<_, VERIFY>(unpacker, &()).coerce()?;
        let treasury_output_amount = u64::unpack::<_, VERIFY>(unpacker, &()).coerce()?;
        // This is only required in Hornet.
        let _parameters_milestone_option_length =
            u16::unpack::<_, VERIFY>(unpacker, &()).coerce()?;
        let parameters_milestone_option =
            MilestoneOption::unpack::<_, true>(unpacker, &ProtocolParameters::default())
                .coerce()?;
        let output_count = u64::unpack::<_, VERIFY>(unpacker, &()).coerce()?;
        let milestone_diff_count = u32::unpack::<_, VERIFY>(unpacker, &()).coerce()?;
        let sep_count = u16::unpack::<_, VERIFY>(unpacker, &()).coerce()?;

        Ok(Self {
            genesis_milestone_index,
            target_milestone_index,
            target_milestone_timestamp,
            target_milestone_id,
            ledger_milestone_index,
            treasury_output_milestone_id,
            treasury_output_amount,
            parameters_milestone_option,
            output_count,
            milestone_diff_count,
            sep_count,
        })
    }
}
