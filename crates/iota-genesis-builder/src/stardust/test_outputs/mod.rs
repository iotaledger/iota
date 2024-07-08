// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod alias_ownership;
mod stardust_mix;
mod vesting_schedule_entity;
mod vesting_schedule_iota_airdrop;

use std::{fs::File, io::BufWriter, path::Path, str::FromStr};

use iota_sdk::types::block::{
    address::Ed25519Address,
    output::{
        unlock_condition::{AddressUnlockCondition, TimelockUnlockCondition},
        BasicOutputBuilder, Output, OutputId,
    },
};
use packable::{
    packer::{IoPacker, Packer},
    Packable,
};

use crate::stardust::{
    parse::HornetSnapshotParser,
    types::{output_header::OutputHeader, output_index::random_output_index},
};

const OUTPUT_TO_DECREASE_AMOUNT_FROM: &str =
    "0xb462c8b2595d40d3ff19924e3731f501aab13e215613ce3e248d0ed9f212db160000";
const MERGE_MILESTONE_INDEX: u32 = 7669900;
const MERGE_TIMESTAMP_SECS: u32 = 1696406475;

pub(crate) fn new_vested_output(
    transaction_id: &mut [u8; 32],
    vested_index: &mut u32,
    amount: u64,
    address: Ed25519Address,
    timelock: Option<u32>,
) -> anyhow::Result<(OutputHeader, Output)> {
    transaction_id[28..32].copy_from_slice(&vested_index.to_le_bytes());
    *vested_index -= 1;

    let output_header = OutputHeader::new_testing(
        *transaction_id,
        random_output_index(),
        [0; 32],
        MERGE_MILESTONE_INDEX,
        MERGE_TIMESTAMP_SECS,
    );

    let mut builder = BasicOutputBuilder::new_with_amount(amount)
        .add_unlock_condition(AddressUnlockCondition::new(address));

    if let Some(timelock) = timelock {
        builder = builder.add_unlock_condition(TimelockUnlockCondition::new(timelock)?);
    }

    let output = Output::from(builder.finish().unwrap());

    Ok((output_header, output))
}

/// Adds outputs to test specific and intricate scenario in the full snapshot.
pub async fn add_snapshot_test_outputs<const VERIFY: bool>(
    current_path: impl AsRef<Path> + core::fmt::Debug,
    new_path: impl AsRef<Path> + core::fmt::Debug,
) -> anyhow::Result<()> {
    let current_file = File::open(current_path)?;
    let new_file = File::create(new_path)?;

    let mut writer = IoPacker::new(BufWriter::new(new_file));
    let mut parser = HornetSnapshotParser::new::<VERIFY>(current_file)?;
    let output_to_decrease_amount_from = OutputId::from_str(OUTPUT_TO_DECREASE_AMOUNT_FROM)?;
    let mut new_header = parser.header.clone();
    let mut vested_index = u32::MAX;

    let new_outputs = [
        alias_ownership::outputs().await?,
        stardust_mix::outputs(&mut vested_index).await?,
        vesting_schedule_entity::outputs(&mut vested_index).await?,
        vesting_schedule_iota_airdrop::outputs(&mut vested_index).await?,
    ]
    .concat();
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

    // Add the solid entry points from the snapshot
    writer.pack_bytes(parser.solid_entry_points_bytes()?)?;

    Ok(())
}
