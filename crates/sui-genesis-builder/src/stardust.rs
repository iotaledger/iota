use anyhow::Result;
use iota_sdk::types::block::output::Output;
use iota_sdk::types::block::output::OutputId;
use iota_sdk::types::block::payload::milestone::MilestoneId;
use iota_sdk::types::block::payload::milestone::MilestoneIndex;
use iota_sdk::types::block::payload::milestone::MilestoneOption;
use iota_sdk::types::block::protocol::ProtocolParameters;
use iota_sdk::types::block::BlockId;
use packable::error::UnknownTagError;
use packable::error::UnpackError;
use packable::error::UnpackErrorExt;
use packable::packer::Packer;
use packable::unpacker::SliceUnpacker;
use packable::unpacker::Unpacker;
use packable::Packable;
use packable::PackableExt;
use std::convert::Infallible;
use std::fs::File;
use std::io::{BufRead, Read};
use std::mem::size_of;
use thiserror::Error;

const SNAPSHOT_HEADER_LENGTH: usize = std::mem::size_of::<FullSnapshotHeader>();
const SNAPSHOT_VERSION: u8 = 2;

#[derive(Debug, Error)]
pub enum StardustError {
    #[error("unsupported snapshot version: expected {0}, got {1}")]
    UnsupportedSnapshotVersion(u8, u8),
    #[error("invalid snapshot kind: {0}")]
    InvalidSnapshotKind(u8),
    #[error("block error: {0}")]
    BlockError(#[from] iota_sdk::types::block::Error),
    #[error("unknown tag: {0}")]
    UnknownTag(u8),
}

impl From<Infallible> for StardustError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

impl From<UnknownTagError<u8>> for StardustError {
    fn from(err: UnknownTagError<u8>) -> Self {
        Self::UnknownTag(err.0)
    }
}

/// The kind of a snapshot.
#[repr(u8)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, packable::Packable)]
#[packable(unpack_error = StardustError)]
pub enum SnapshotKind {
    /// Full is a snapshot which contains the full ledger entry for a given milestone plus the milestone diffs which
    /// subtracted to the ledger milestone reduce to the snapshot milestone ledger.
    Full = 0,
    /// Delta is a snapshot which contains solely diffs of milestones newer than a certain ledger milestone instead of
    /// the complete ledger state of a given milestone.
    Delta = 1,
}

#[derive(Debug, Clone, Packable)]
pub struct OutputHeader {
    output_id: OutputId,
    block_id: BlockId,
    ms_index: MilestoneIndex,
    ms_ts: u32,
    length: u32,
}

impl OutputHeader {
    pub const LENGTH: usize = OutputId::LENGTH
        + size_of::<BlockId>()
        + size_of::<MilestoneIndex>()
        + 2 * size_of::<u32>();
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
        // TODO: Verify that the provided protocol parameters milestone are valid.
        let parameters_milestone_option =
            MilestoneOption::unpack::<_, false>(unpacker, &ProtocolParameters::default())
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

pub fn parse_full_snapshot() -> Result<()> {
    let Some(path) = std::env::args().nth(1) else {
        anyhow::bail!("please provide path to the full-snapshot file");
    };

    let snapshot_file = File::open(path)?;
    let mut reader = std::io::BufReader::new(snapshot_file);
    let mut buf = [0_u8; SNAPSHOT_HEADER_LENGTH];
    reader.read_exact(&mut buf)?;

    let full_header =
        FullSnapshotHeader::unpack::<_, true>(&mut SliceUnpacker::new(buf.as_slice()), &())?;

    println!("Output count:\t\t\t{}", full_header.output_count());

    for _ in iterate_on_outputs(&mut reader, full_header.output_count())? { /* do something */ }
    Ok(())
}

pub fn iterate_on_outputs(
    src: &mut impl BufRead,
    output_count: u64,
) -> Result<impl Iterator<Item = Result<Output, anyhow::Error>> + '_> {
    let mut header_buf = [0_u8; OutputHeader::LENGTH];
    let mut output_buf = [0_u8; u16::MAX as usize];

    let iter = (0..output_count).map(move |_| {
        src.read_exact(&mut header_buf)?;
        let header =
            OutputHeader::unpack::<_, false>(&mut SliceUnpacker::new(header_buf.as_slice()), &())?;
        println!("header {:?}", header);
        src.read_exact(&mut output_buf[0..header.length as usize])?;
        // TODO: Use the [iota_sdk::types::block::Output] to unpack the `output_buf`
        // let output = Output::unpack::<_, true>(&mut output_buf.as_slice())?;
        let output = unimplemented!();
        Ok(output)
    });
    Ok(iter)
}
