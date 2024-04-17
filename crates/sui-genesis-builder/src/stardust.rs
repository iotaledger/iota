use std::fs::File;
use std::io::{BufRead, Read};
use std::mem::size_of;

use anyhow::Result;
use bee_block::output::OutputId;
use bee_block::payload::milestone::MilestoneIndex;
use bee_block::BlockId;
use bee_ledger::types::snapshot::FullSnapshotHeader;
use packable::Packable;

type SdkOutput = ();

const SNAPSHOT_HEADER_LENGTH: usize = std::mem::size_of::<FullSnapshotHeader>();

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

pub fn parse_full_snapshot() -> Result<()> {
    let Some(path) = std::env::args().nth(1) else {
        anyhow::bail!("please provide path to the full-snapshot file");
    };

    let snapshot_file = File::open(path)?;
    let mut reader = std::io::BufReader::new(snapshot_file);
    let mut buf = [0_u8; SNAPSHOT_HEADER_LENGTH];
    reader.read_exact(&mut buf)?;

    let full_header = FullSnapshotHeader::unpack::<_, true>(&mut buf.as_slice())?;

    println!("Output count:\t\t\t{}", full_header.output_count());

    for _ in iterate_on_outputs(&mut reader, full_header.output_count())? { /* do something */ }
    Ok(())
}

pub fn iterate_on_outputs(
    src: &mut impl BufRead,
    output_count: u64,
) -> Result<impl Iterator<Item = Result<SdkOutput, anyhow::Error>> + '_> {
    let mut header_buf = [0_u8; OutputHeader::LENGTH];
    let mut output_buf = [0_u8; u16::MAX as usize];

    let iter = (0..output_count).map(move |_| {
        src.read_exact(&mut header_buf)?;
        let header = OutputHeader::unpack::<_, false>(&mut header_buf.as_slice())?;
        println!("header {:?}", header);
        src.read_exact(&mut output_buf[0..header.length as usize])?;
        // TODO: Use the [iota_sdk::types::block::Output] to unpack the `output_buf`
        // let output = Output::unpack::<_, true>(&mut output_buf.as_slice())?;
        let output = ();
        Ok(output)
    });
    Ok(iter)
}
