// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod alias_ownership;
mod delegator_outputs;
mod stardust_mix;
mod vesting_schedule_entity;
mod vesting_schedule_iota_airdrop;
mod vesting_schedule_portfolio_mix;

use std::{
    fs::File,
    io::{BufWriter, Read},
    path::Path,
    str::FromStr,
};

use anyhow::anyhow;
pub(crate) use delegator_outputs::{new_simple_basic_output, new_vested_output};
use iota_sdk::types::block::{
    address::Ed25519Address,
    output::{BasicOutputBuilder, Output, OutputId},
};
use iota_types::{
    gas_coin::STARDUST_TOTAL_SUPPLY_IOTA,
    stardust::coin_type::CoinType,
    timelock::timelock::{self},
};
use packable::{
    packer::{IoPacker, Packer},
    Packable,
};
use rand::{rngs::StdRng, Rng, SeedableRng};

use crate::stardust::{parse::HornetSnapshotParser, types::output_header::OutputHeader};

pub const IOTA_COIN_TYPE: u32 = 4218;
const IOTA_OUTPUT_TO_DECREASE_AMOUNT_FROM: &str =
    "0xb462c8b2595d40d3ff19924e3731f501aab13e215613ce3e248d0ed9f212db160000";

pub const SHIMMER_COIN_TYPE: u32 = 4219;
pub const STARDUST_TOTAL_SUPPLY_SHIMMER_MICRO: u64 = 1_813_620_509_061_365;
const SHIMMER_OUTPUT_TO_DECREASE_AMOUNT_FROM: &str =
    "0x4c337ea67697cb8dd0267cced8d9b51c479eb61dea04842138dcef31218d63810000";

pub const MERGE_MILESTONE_INDEX: u32 = 7669900;
pub const MERGE_TIMESTAMP_SECS: u32 = 1696406475;

pub const fn to_micros(n: u64) -> u64 {
    match n.checked_mul(1_000_000) {
        Some(res) => res,
        None => panic!("should not overflow"),
    }
}

const PROBABILITY_OF_PICKING_A_BASIC_OUTPUT: f64 = 0.1;

/// Create outputs to test specific and intricate scenario in a new full
/// snapshot.
///
/// If a delegator address is NOT present, then some test outputs are added to
/// the previous full snapshot.
///
/// If a delegator address is present, then the resulting new full snapshot will
/// include only test outputs and some outputs dedicated to the delegator. If
/// `with_sampling` is true, then some samples from the previous snapshot are
/// taken too.
pub async fn add_snapshot_test_outputs<const VERIFY: bool>(
    current_path: impl AsRef<Path> + core::fmt::Debug,
    new_path: impl AsRef<Path> + core::fmt::Debug,
    coin_type: CoinType,
    randomness_seed: u64,
    delegator: impl Into<Option<Ed25519Address>>,
    with_sampling: bool,
) -> anyhow::Result<()> {
    let current_file = File::open(current_path)?;
    let new_file = File::create(new_path)?;

    let mut writer = IoPacker::new(BufWriter::new(new_file));
    let mut parser = HornetSnapshotParser::new::<VERIFY>(current_file)?;
    let mut new_header = parser.header.clone();
    let mut vested_index = u32::MAX;

    let mut rng = StdRng::seed_from_u64(randomness_seed);
    let mut new_outputs = [
        alias_ownership::outputs(&mut rng, coin_type).await?,
        stardust_mix::outputs(&mut rng, &mut vested_index, coin_type).await?,
        vesting_schedule_entity::outputs(&mut rng, &mut vested_index, coin_type).await?,
        vesting_schedule_iota_airdrop::outputs(&mut rng, &mut vested_index, coin_type).await?,
        vesting_schedule_portfolio_mix::outputs(&mut rng, &mut vested_index, coin_type).await?,
    ]
    .concat();

    let new_temp_amount = new_outputs.iter().map(|o| o.1.amount()).sum::<u64>();

    new_outputs.append(&mut if let Some(delegator) = delegator.into() {
        add_only_test_outputs(
            &mut rng,
            &mut vested_index,
            delegator,
            with_sampling.then_some(&mut parser),
            new_temp_amount,
            coin_type,
        )?
    } else {
        add_all_previous_outputs_and_test_outputs(&mut parser, new_temp_amount, coin_type)?
    });

    // Adjust the output count according to newly generated outputs.
    new_header.output_count = new_outputs.len() as u64;

    // Writes the new header.
    new_header.pack(&mut writer)?;

    // Writes only the new outputs.
    for (output_header, output) in new_outputs {
        output_header.pack(&mut writer)?;
        output.pack(&mut writer)?;
    }

    // Add the solid entry points from the snapshot
    writer.pack_bytes(parser.solid_entry_points_bytes()?)?;

    Ok(())
}

fn add_all_previous_outputs_and_test_outputs<R: Read>(
    parser: &mut HornetSnapshotParser<R>,
    new_amount: u64,
    coin_type: CoinType,
) -> anyhow::Result<Vec<(OutputHeader, Output)>> {
    let mut new_outputs = Vec::new();

    let target_output = match coin_type {
        CoinType::Iota => IOTA_OUTPUT_TO_DECREASE_AMOUNT_FROM,
        CoinType::Shimmer => SHIMMER_OUTPUT_TO_DECREASE_AMOUNT_FROM,
    };

    // Writes previous outputs.
    for (output_header, output) in parser.outputs().filter_map(|o| o.ok()) {
        if output_header.output_id() == OutputId::from_str(target_output)? {
            let basic = output.as_basic();
            let amount = basic
                .amount()
                .checked_sub(new_amount)
                .ok_or_else(|| anyhow::anyhow!("underflow decreasing new amount from output"))?;
            new_outputs.push((
                output_header,
                Output::from(
                    BasicOutputBuilder::from(basic)
                        .with_amount(amount)
                        .finish()?,
                ),
            ));
        } else {
            new_outputs.push((output_header, output));
        }
    }

    Ok(new_outputs)
}

/// Creates and return a vector of outputs that includes only test outputs
/// together with some outputs dedicated to the delegator to make it compliant
/// with a timelocked staking. Optionally some samples from the previous
/// snapshot are taken too.
fn add_only_test_outputs<R: Read>(
    rng: &mut StdRng,
    vested_index: &mut u32,
    delegator: Ed25519Address,
    parser: Option<&mut HornetSnapshotParser<R>>,
    new_temp_amount: u64,
    coin_type: CoinType,
) -> anyhow::Result<Vec<(OutputHeader, Output)>> {
    // Needed outputs for delegator
    let mut new_outputs = delegator_outputs::outputs(rng, vested_index, delegator)?;

    // Additional sample outputs
    if let Some(parser) = parser {
        new_outputs.append(&mut with_sampling(parser, rng)?)
    }

    // Add all the remainder tokens to the zero address
    let zero_address = Ed25519Address::new([0; 32]);
    let network_total_supply = match coin_type {
        CoinType::Iota => to_micros(STARDUST_TOTAL_SUPPLY_IOTA),
        CoinType::Shimmer => STARDUST_TOTAL_SUPPLY_SHIMMER_MICRO,
    };
    let remainder = network_total_supply
        .checked_sub(new_temp_amount + new_outputs.iter().map(|o| o.1.amount()).sum::<u64>())
        .ok_or_else(|| anyhow!("new amount should not be higher than total supply"))?;
    let remainder_per_output = remainder / 4;
    let difference = remainder % 4;
    for _ in 0..4 {
        new_outputs.push(new_simple_basic_output(
            remainder_per_output,
            zero_address,
            rng,
        )?);
    }
    if difference > 0 {
        new_outputs.push(new_simple_basic_output(difference, zero_address, rng)?);
    }

    Ok(new_outputs)
}

/// Get samples of the previous Hornet snapshot without timelocks and with a
/// certain probability of picking basic outputs.
fn with_sampling<R: Read>(
    parser: &mut HornetSnapshotParser<R>,
    rng: &mut StdRng,
) -> anyhow::Result<Vec<(OutputHeader, Output)>> {
    let mut new_outputs = Vec::new();
    let target_milestone_timestamp = parser.target_milestone_timestamp();

    for (output_header, output) in parser.outputs().filter_map(|o| o.ok()) {
        match output {
            Output::Basic(ref basic) => {
                if !timelock::is_timelocked_vested_reward(
                    output_header.output_id(),
                    basic,
                    target_milestone_timestamp,
                ) && rng.gen_bool(PROBABILITY_OF_PICKING_A_BASIC_OUTPUT)
                {
                    new_outputs.push((output_header, output));
                }
            }
            Output::Treasury(_) | Output::Foundry(_) | Output::Alias(_) | Output::Nft(_) => {
                new_outputs.push((output_header, output))
            }
        };
    }

    Ok(new_outputs)
}
