// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod alias_ownership;
mod stardust_mix;
mod vesting_schedule_entity;
mod vesting_schedule_iota_airdrop;
mod vesting_schedule_portfolio_mix;

use std::{fs::File, io::BufWriter, path::Path, str::FromStr};

use iota_sdk::types::block::{
    address::Ed25519Address,
    output::{
        unlock_condition::{AddressUnlockCondition, TimelockUnlockCondition},
        BasicOutputBuilder, Output, OutputId,
    },
};
use iota_types::{
    gas_coin::TOTAL_SUPPLY_IOTA,
    timelock::timelock::{self, VESTED_REWARD_ID_PREFIX},
};
use packable::{
    packer::{IoPacker, Packer},
    Packable,
};
use rand::{random, thread_rng, Rng};

use crate::stardust::{
    parse::HornetSnapshotParser,
    types::{output_header::OutputHeader, output_index::random_output_index},
};

const OUTPUT_TO_DECREASE_AMOUNT_FROM: &str =
    "0xb462c8b2595d40d3ff19924e3731f501aab13e215613ce3e248d0ed9f212db160000";
const MERGE_MILESTONE_INDEX: u32 = 7669900;
const MERGE_TIMESTAMP_SECS: u32 = 1696406475;
const A_WEEK_IN_SECONDS: u32 = 604_800;
const TIMELOCK_MAX_ENDING_TIME: u32 = A_WEEK_IN_SECONDS * 208;

const TO_MICROS: u64 = 1_000_000;
const DELEGATOR_GAS_COIN_NUM: u8 = 100;
const DELEGATOR_GAS_COIN_AMOUNT_PER_OUTPUT: u64 = 1_000_000 * TO_MICROS;
const DELEGATOR_TIMELOCKS_NUM: u8 = 100;
const DELEGATOR_TIMELOCKS_AMOUNT_PER_OUTPUT: u64 = 1_000_000 * TO_MICROS;

const WITH_SAMPLING: bool = true;
const PROBABILITY_OF_PICKING_A_BASIC_OUTPUT: f64 = 0.1;

pub(crate) fn new_simple_basic_output(
    amount: u64,
    address: Ed25519Address,
) -> anyhow::Result<(OutputHeader, Output)> {
    let output_header = OutputHeader::new_testing(
        random::<[u8; 32]>(),
        random_output_index(),
        [0; 32],
        MERGE_MILESTONE_INDEX,
        MERGE_TIMESTAMP_SECS,
    );

    let builder = BasicOutputBuilder::new_with_amount(amount)
        .add_unlock_condition(AddressUnlockCondition::new(address));

    let output = Output::from(builder.finish().unwrap());

    Ok((output_header, output))
}

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
/// If a delegator address is present, then the resulting snapshot will include
/// only a sample of the original outputs and some dedicated outputs will be
/// created for the delegator to make it compliant with a timelocked staking.
pub async fn add_snapshot_test_outputs<const VERIFY: bool>(
    current_path: impl AsRef<Path> + core::fmt::Debug,
    new_path: impl AsRef<Path> + core::fmt::Debug,
    delegator: Option<Ed25519Address>,
) -> anyhow::Result<()> {
    let current_file = File::open(current_path)?;
    let new_file = File::create(new_path)?;

    let mut writer = IoPacker::new(BufWriter::new(new_file));
    let mut parser = HornetSnapshotParser::new::<VERIFY>(current_file)?;
    let output_to_decrease_amount_from = OutputId::from_str(OUTPUT_TO_DECREASE_AMOUNT_FROM)?;
    let mut new_header = parser.header.clone();
    let mut vested_index = u32::MAX;
    let mut output_count = new_header.output_count;

    let mut new_outputs = [
        alias_ownership::outputs().await?,
        stardust_mix::outputs(&mut vested_index).await?,
        vesting_schedule_entity::outputs(&mut vested_index).await?,
        vesting_schedule_iota_airdrop::outputs(&mut vested_index).await?,
        vesting_schedule_portfolio_mix::outputs(&mut vested_index).await?,
    ]
    .concat();

    // Delegator
    let mut create_with_only_test_outputs = false;
    if let Some(delegator) = delegator {
        create_with_only_test_outputs = true;
        // Add gas coins to delegator
        for _ in 0..DELEGATOR_GAS_COIN_NUM {
            new_outputs.push(new_simple_basic_output(
                DELEGATOR_GAS_COIN_AMOUNT_PER_OUTPUT,
                delegator,
            )?);
        }

        // Add timelocks to delegator
        let mut transaction_id = [0; 32];
        transaction_id[0..28]
            .copy_from_slice(&prefix_hex::decode::<[u8; 28]>(VESTED_REWARD_ID_PREFIX)?);
        for _ in 0..DELEGATOR_TIMELOCKS_NUM {
            new_outputs.push(new_vested_output(
                &mut transaction_id,
                &mut vested_index,
                DELEGATOR_TIMELOCKS_AMOUNT_PER_OUTPUT,
                delegator,
                Some(MERGE_TIMESTAMP_SECS + TIMELOCK_MAX_ENDING_TIME),
            )?);
        }

        if WITH_SAMPLING {
            // Samples without timelocks.
            // Writes previous outputs.
            let target_milestone_timestamp = parser.target_milestone_timestamp();
            let mut rng = thread_rng();
            for (output_header, output) in parser.outputs().filter_map(|o| o.ok()) {
                match output {
                    Output::Basic(ref basic) => {
                        if !timelock::is_timelocked_vested_reward(
                            output_header.output_id(),
                            &basic,
                            target_milestone_timestamp,
                        ) && rng.gen_bool(PROBABILITY_OF_PICKING_A_BASIC_OUTPUT)
                        {
                            new_outputs.push((output_header, output));
                        }
                    }
                    Output::Treasury(_)
                    | Output::Foundry(_)
                    | Output::Alias(_)
                    | Output::Nft(_) => new_outputs.push((output_header, output)),
                };
            }
        }

        output_count = 0;
    }

    // Compute the IOTA tokens amounk used by test outputs
    let new_amount = new_outputs.iter().map(|o| o.1.amount()).sum::<u64>();

    if create_with_only_test_outputs {
        // Add all the remainder tokens to the zero address
        let zero_address = Ed25519Address::new([0; 32]);
        let remainder = (TOTAL_SUPPLY_IOTA * TO_MICROS)
            .checked_sub(new_amount)
            .expect("new amount should not be higher than total supply");
        let remainder_per_output = remainder / 4;
        let difference = remainder % 4;
        for _ in 0..4 {
            new_outputs.push(new_simple_basic_output(remainder_per_output, zero_address)?);
        }
        if difference > 0 {
            new_outputs.push(new_simple_basic_output(difference, zero_address)?);
        }
    }

    // Adjust the output count according to newly generated outputs.
    new_header.output_count = output_count + new_outputs.len() as u64;

    // Writes the new header.
    new_header.pack(&mut writer)?;

    if !create_with_only_test_outputs {
        // Writes previous outputs.
        for (output_header, output) in parser.outputs().filter_map(|o| o.ok()) {
            output_header.pack(&mut writer)?;

            if output_header.output_id() == output_to_decrease_amount_from {
                let basic = output.as_basic();
                let amount = basic.amount().checked_sub(new_amount).ok_or_else(|| {
                    anyhow::anyhow!("underflow decreasing new amount from output")
                })?;
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
    }

    // Writes only the new outputs.
    for (output_header, output) in new_outputs {
        output_header.pack(&mut writer)?;
        output.pack(&mut writer)?;
    }

    // Add the solid entry points from the snapshot
    writer.pack_bytes(parser.solid_entry_points_bytes()?)?;

    Ok(())
}
