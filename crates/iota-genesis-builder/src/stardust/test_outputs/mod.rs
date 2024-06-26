// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod dummy;

use std::{
    fs::{File, OpenOptions},
    io::BufWriter,
    path::Path,
    str::FromStr,
};

use iota_sdk::types::block::output::{BasicOutputBuilder, Output, OutputId};
use iota_types::stardust::error::StardustError;
use packable::{packer::IoPacker, Packable};

use crate::stardust::parse::FullSnapshotParser;

const OUTPUT_TO_DECREASE_AMOUNT_FROM: &str =
    "0xb462c8b2595d40d3ff19924e3731f501aab13e215613ce3e248d0ed9f212db160000";

/// Adds outputs to test specific and intricate scenario in the full snapshot.
pub fn add_snapshot_test_outputs<P: AsRef<Path> + core::fmt::Debug>(
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
    let mut parser = FullSnapshotParser::new(current_file)?;
    let output_to_decrease_amount_from =
        OutputId::from_str(OUTPUT_TO_DECREASE_AMOUNT_FROM).map_err(StardustError::BlockError)?;

    let new_outputs = dummy::outputs();
    let new_amount = new_outputs.iter().map(|o| o.1.amount()).sum::<u64>();

    // Increments the output count according to newly generated outputs.
    parser.header.output_count += new_outputs.len() as u64;

    // Writes the new header.
    parser.header.pack(&mut writer)?;

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
                    .finish()
                    .map_err(StardustError::BlockError)?,
            );

            output.pack(&mut writer)?;
        } else {
            output.pack(&mut writer)?;
        }
    }

    Ok(())
}
