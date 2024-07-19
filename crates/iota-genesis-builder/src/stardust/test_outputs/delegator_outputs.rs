// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk::types::block::{
    address::Ed25519Address,
    output::{
        unlock_condition::{AddressUnlockCondition, TimelockUnlockCondition},
        BasicOutputBuilder, Output,
    },
};
use iota_types::timelock::timelock::VESTED_REWARD_ID_PREFIX;
use rand::{rngs::StdRng, Rng, SeedableRng};

use crate::stardust::types::{
    output_header::OutputHeader, output_index::random_output_index_with_rng,
};

const MERGE_MILESTONE_INDEX: u32 = 7669900;
const MERGE_TIMESTAMP_SECS: u32 = 1696406475;
const A_WEEK_IN_SECONDS: u32 = 604_800;
const TIMELOCK_MAX_ENDING_TIME: u32 = A_WEEK_IN_SECONDS * 208;

const TO_MICROS: u64 = 1_000_000;
const DELEGATOR_GAS_COIN_NUM: u8 = 100;
const DELEGATOR_GAS_COIN_AMOUNT_PER_OUTPUT: u64 = 1_000_000 * TO_MICROS;
const DELEGATOR_TIMELOCKS_NUM: u8 = 100;
const DELEGATOR_TIMELOCKS_AMOUNT_PER_OUTPUT: u64 = 1_000_000 * TO_MICROS;

pub(crate) fn new_simple_basic_output(
    amount: u64,
    address: Ed25519Address,
    rng: &mut StdRng,
) -> anyhow::Result<(OutputHeader, Output)> {
    let output_header = OutputHeader::new_testing(
        rng.gen::<[u8; 32]>(),
        random_output_index_with_rng(rng),
        [0; 32],
        MERGE_MILESTONE_INDEX,
        MERGE_TIMESTAMP_SECS,
    );

    let output = Output::from(
        BasicOutputBuilder::new_with_amount(amount)
            .add_unlock_condition(AddressUnlockCondition::new(address))
            .finish()?,
    );

    Ok((output_header, output))
}

pub(crate) fn new_vested_output(
    vested_index: &mut u32,
    amount: u64,
    address: Ed25519Address,
    timelock: Option<u32>,
    rng: &mut StdRng,
) -> anyhow::Result<(OutputHeader, Output)> {
    let mut transaction_id = [0; 32];
    transaction_id[0..28]
        .copy_from_slice(&prefix_hex::decode::<[u8; 28]>(VESTED_REWARD_ID_PREFIX)?);
    transaction_id[28..32].copy_from_slice(&vested_index.to_le_bytes());
    *vested_index -= 1;

    let output_header = OutputHeader::new_testing(
        transaction_id,
        random_output_index_with_rng(rng),
        [0; 32],
        MERGE_MILESTONE_INDEX,
        MERGE_TIMESTAMP_SECS,
    );

    let mut builder = BasicOutputBuilder::new_with_amount(amount)
        .add_unlock_condition(AddressUnlockCondition::new(address));

    if let Some(timelock) = timelock {
        builder = builder.add_unlock_condition(TimelockUnlockCondition::new(timelock)?);
    }

    let output = Output::from(builder.finish()?);

    Ok((output_header, output))
}

pub fn outputs(
    randomness_seed: u64,
    vested_index: &mut u32,
    delegator: Ed25519Address,
) -> anyhow::Result<Vec<(OutputHeader, Output)>> {
    println!("delegator_outputs randomness seed: {randomness_seed}");
    let mut rng = StdRng::seed_from_u64(randomness_seed);
    let mut new_outputs = Vec::new();

    // Add gas coins to delegator
    for _ in 0..DELEGATOR_GAS_COIN_NUM {
        new_outputs.push(new_simple_basic_output(
            DELEGATOR_GAS_COIN_AMOUNT_PER_OUTPUT,
            delegator,
            &mut rng,
        )?);
    }

    // Add timelocks to delegator
    for _ in 0..DELEGATOR_TIMELOCKS_NUM {
        new_outputs.push(new_vested_output(
            vested_index,
            DELEGATOR_TIMELOCKS_AMOUNT_PER_OUTPUT,
            delegator,
            Some(MERGE_TIMESTAMP_SECS + TIMELOCK_MAX_ENDING_TIME),
            &mut rng,
        )?);
    }

    Ok(new_outputs)
}
