use anyhow::{anyhow, Result};
use bee_block::output::OutputId;
use bee_block::payload::milestone::MilestoneIndex;
use bee_block::{output::Output, BlockId};
use bee_ledger::types::snapshot::FullSnapshotHeader;
use packable::{unpacker::Unpacker, Packable};

pub fn parse_full_snapshot() -> Result<()> {
    let Some(path) = std::env::args().nth(1) else {
        anyhow::bail!("please provide path to the full-snapshot file");
    };

    // TODO: Here we load the entire file in memory. We can optimize by using a `BufRead`.
    let mut slice = &std::fs::read(path)?[..];

    let full_header = FullSnapshotHeader::unpack::<_, true>(&mut slice)?;

    println!("Output count:\t\t\t{}", full_header.output_count());

    iterate_on_outputs(&mut slice, full_header.output_count())?;
    Ok(())
}

fn iterate_on_outputs<U: Unpacker>(unpacker: &mut U, output_count: u64) -> Result<()> {
    for _ in 0..output_count {
        let output_id = OutputId::unpack::<_, true>(unpacker)
            .map_err(|_| anyhow!("unpacking output id failed"))?;
        let _block_id =
            BlockId::unpack::<_, true>(unpacker).map_err(|_| anyhow!("unpacking block failed"))?;
        let _milestone_index_booked = MilestoneIndex::unpack::<_, true>(unpacker)
            .map_err(|_| anyhow!("unpacking milestone index failed"))?;
        let _milestone_timestamp_booked =
            u32::unpack::<_, true>(unpacker).map_err(|_| anyhow!("unpacking timestamp failed"))?;
        let _output_length =
            u32::unpack::<_, true>(unpacker).map_err(|_| anyhow!("unpacking length failed"))?;
        let _output = Output::unpack::<_, true>(unpacker)
            .map_err(|_| anyhow!("unpacking output {output_id} failed",))?;
    }
    Ok(())
}
