// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod dummy;

use std::{
    fs::{File, OpenOptions},
    io::BufWriter,
    path::Path,
    str::FromStr,
};

use iota_sdk::types::block::{
    output::{BasicOutputBuilder, Output, OutputId},
    payload::milestone::{MilestoneOption, ParametersMilestoneOption},
    protocol::ProtocolParameters,
};
use packable::{packer::IoPacker, Packable, PackableExt};

use crate::stardust::parse::FullSnapshotParser;

fn decreased_amount_output(output: Output, amount: u64) -> anyhow::Result<Output> {
    let basic = output.as_basic();
    let amount = basic.amount().checked_sub(amount).unwrap();

    Ok(Output::from(
        BasicOutputBuilder::from(basic)
            .with_amount(amount)
            .finish()?,
    ))
}

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
    let output_id_if = OutputId::from_str(
        "0xb462c8b2595d40d3ff19924e3731f501aab13e215613ce3e248d0ed9f212db160000",
    )?;

    let new_outputs = dummy::outputs();

    // Increments the output count according to newly generated outputs.
    parser.header.output_count += new_outputs.len() as u64;

    // Creates new protocol parameters to increase the total supply according to
    // newly generated outputs.
    let params = parser.protocol_parameters()?;
    let new_amount = new_outputs.iter().map(|o| o.1.amount()).sum::<u64>();
    let new_params = ProtocolParameters::new(
        params.protocol_version(),
        params.network_name().to_owned(),
        params.bech32_hrp(),
        params.min_pow_score(),
        params.below_max_depth(),
        *params.rent_structure(),
        params.token_supply() + new_amount,
    )?;
    if let MilestoneOption::Parameters(params) = &parser.header.parameters_milestone_option {
        parser.header.parameters_milestone_option =
            MilestoneOption::Parameters(ParametersMilestoneOption::new(
                params.target_milestone_index(),
                params.protocol_version(),
                new_params.pack_to_vec(),
            )?);
    }

    // Writes the new header.
    parser.header.pack(&mut writer)?;

    // Writes previous and new outputs.
    for (output_header, output) in parser.outputs().filter_map(|o| o.ok()).chain(new_outputs) {
        output_header.pack(&mut writer).unwrap();

        if output_header.output_id() == output_id_if {
            decreased_amount_output(output, new_amount)?.pack(&mut writer)?;
        } else {
            output.pack(&mut writer)?;
        }
    }

    Ok(())
}
