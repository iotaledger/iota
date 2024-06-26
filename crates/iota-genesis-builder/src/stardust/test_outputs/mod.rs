// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod vesting_schedule_iota_airdrop;

use std::{
    fs::{File, OpenOptions},
    io::BufWriter,
    path::Path,
    str::FromStr,
};

use iota_sdk::types::block::output::{BasicOutputBuilder, Output, OutputId, OUTPUT_INDEX_RANGE};
use packable::{packer::IoPacker, Packable};
use rand::Rng as _;

use crate::stardust::parse::FullSnapshotParser;

const OUTPUT_TO_DECREASE_AMOUNT_FROM: &str =
    "0xb462c8b2595d40d3ff19924e3731f501aab13e215613ce3e248d0ed9f212db160000";

/// Adds outputs to test specific and intricate scenario in the full snapshot.
pub async fn add_snapshot_test_outputs<P: AsRef<Path> + core::fmt::Debug>(
    current_path: P,
    new_path: P,
) -> anyhow::Result<()> {
    let current_file = File::open(current_path)?;
    let new_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(new_path)?;
    let mut writer = IoPacker::new(BufWriter::new(new_file));
    let parser = FullSnapshotParser::new(current_file)?;
    let output_to_decrease_amount_from = OutputId::from_str(OUTPUT_TO_DECREASE_AMOUNT_FROM)?;
    let mut new_header = parser.header.clone();
    let mut vested_index = u32::MAX;

    let new_outputs = vesting_schedule_iota_airdrop::outputs(&mut vested_index).await?;
    let new_amount = new_outputs.iter().map(|o| o.1.amount()).sum::<u64>();

    // Increments the output count according to newly generated outputs.
    new_header.output_count += new_outputs.len() as u64;

    // Writes the new header.
    new_header.pack(&mut writer)?;

    // Writes previous and new outputs.
    for (output_header, output) in parser.outputs().filter_map(|o| o.ok()).chain(new_outputs) {
        output_header.pack(&mut writer)?;

        if output_header.output_id() == output_to_decrease_amount_from {
            let basic = output.as_basic();
            let amount = basic
                .amount()
                .checked_sub(new_amount)
                .ok_or_else(|| anyhow::anyhow!("underflow decreasing new amount from output"))?;
            let output = Output::from(
                BasicOutputBuilder::from(basic)
                    .with_amount(amount)
                    .finish()?,
            );

            output.pack(&mut writer)?;
        } else {
            output.pack(&mut writer)?;
        }
    }

    Ok(())
}

#[derive(Copy, Clone, Debug, Default)]
pub struct OutputIndex(u16);

impl OutputIndex {
    pub fn new(index: u16) -> anyhow::Result<Self> {
        if !OUTPUT_INDEX_RANGE.contains(&index) {
            anyhow::bail!("index {index} out of range {OUTPUT_INDEX_RANGE:?}");
        }
        Ok(Self(index))
    }

    pub fn get(&self) -> u16 {
        self.0
    }
}

/// Generates a random, valid output index in the range [0..128)
pub fn random_output_index() -> OutputIndex {
    OutputIndex::new(rand::thread_rng().gen_range(OUTPUT_INDEX_RANGE))
        .expect("range is guaranteed to be valid")
}
