// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Portfolio mix with vesting outputs.
//! 4-years, initial unlock, bi-weekly unlock.
//! One mnemonic/wallet, one account, multiple public and internal addresses.

use iota_sdk::{
    client::secret::{mnemonic::MnemonicSecretManager, GenerateAddressOptions, SecretManage},
    types::block::{
        address::Ed25519Address,
        output::{
            unlock_condition::AddressUnlockCondition, BasicOutputBuilder, Output,
            OUTPUT_INDEX_RANGE,
        },
    },
};
use iota_types::timelock::timelock::VESTED_REWARD_ID_PREFIX;
use rand::{random, rngs::StdRng, Rng, SeedableRng};

use crate::stardust::{
    test_outputs::{new_vested_output, MERGE_MILESTONE_INDEX, MERGE_TIMESTAMP_SECS},
    types::{output_header::OutputHeader, output_index::OutputIndex},
};

const IOTA_COIN_TYPE: u32 = 4218;
const VESTING_WEEKS: usize = 208;
const VESTING_WEEKS_FREQUENCY: usize = 2;
const MNEMONIC: &str = "axis art silk merit assist hour bright always day legal misery arm laundry mule ship upon oil ski cup hat skin wet old sea";
// bip path values for account, internal, address
const ADDRESSES: &'static [[u32; 3]] = &[
    // public
    [0, 0, 0],
    [0, 0, 1],
    [0, 0, 2],
    // internal
    [0, 1, 0],
    [0, 1, 1],
    [0, 1, 2],
];

pub(crate) async fn outputs(vested_index: &mut u32) -> anyhow::Result<Vec<(OutputHeader, Output)>> {
    let mut outputs = Vec::new();
    let secret_manager = MnemonicSecretManager::try_from_mnemonic(MNEMONIC)?;

    let randomness_seed = random::<u64>();
    let mut rng = StdRng::seed_from_u64(randomness_seed);
    println!("vesting_schedule_portfolio_mix randomness seed: {randomness_seed}");

    for [account_index, internal, address_index] in ADDRESSES {
        let address = secret_manager
            .generate_ed25519_addresses(
                IOTA_COIN_TYPE,
                *account_index,
                *address_index..address_index + 1,
                (*internal == 1).then_some(GenerateAddressOptions::internal()),
            )
            .await?[0];

        match address_index {
            0 => {
                add_random_basic_output(&mut outputs, &mut rng, address)?;
            }
            1 => {
                add_vested_outputs(&mut outputs, &mut rng, vested_index, address)?;
            }
            2 => {
                add_random_basic_output(&mut outputs, &mut rng, address)?;
                add_vested_outputs(&mut outputs, &mut rng, vested_index, address)?;
            }
            _ => unreachable!(),
        }
    }
    Ok(outputs)
}

fn random_output_header(rng: &mut StdRng) -> OutputHeader {
    OutputHeader::new_testing(
        rng.gen(),
        OutputIndex::new(rng.gen_range(OUTPUT_INDEX_RANGE))
            .expect("range is guaranteed to be valid"),
        rng.gen(),
        MERGE_MILESTONE_INDEX,
        MERGE_TIMESTAMP_SECS,
    )
}

fn add_random_basic_output(
    outputs: &mut Vec<(OutputHeader, Output)>,
    rng: &mut StdRng,
    address: Ed25519Address,
) -> anyhow::Result<()> {
    let basic_output_header = random_output_header(rng);

    let amount = rng.gen_range(1_000_000..10_000_000);
    let basic_output = BasicOutputBuilder::new_with_amount(amount)
        .add_unlock_condition(AddressUnlockCondition::new(address))
        .finish()?;

    outputs.push((basic_output_header, basic_output.into()));

    Ok(())
}

fn add_vested_outputs(
    outputs: &mut Vec<(OutputHeader, Output)>,
    rng: &mut StdRng,
    vested_index: &mut u32,
    address: Ed25519Address,
) -> anyhow::Result<()> {
    let (initial_unlock_amount, vested_amount) = initial_unlock_and_vested_amounts(rng);

    let mut transaction_id = [0; 32];

    // Prepare a transaction ID with the vested reward prefix.
    transaction_id[0..28]
        .copy_from_slice(&prefix_hex::decode::<[u8; 28]>(VESTED_REWARD_ID_PREFIX)?);

    outputs.push(new_vested_output(
        &mut transaction_id,
        vested_index,
        initial_unlock_amount,
        address,
        None,
    )?);

    for offset in (0..=VESTING_WEEKS).step_by(VESTING_WEEKS_FREQUENCY) {
        let timelock = MERGE_TIMESTAMP_SECS + offset as u32 * 604_800;

        outputs.push(new_vested_output(
            &mut transaction_id,
            vested_index,
            vested_amount,
            address,
            Some(timelock),
        )?);
    }

    Ok(())
}

fn initial_unlock_and_vested_amounts(rng: &mut StdRng) -> (u64, u64) {
    let amount = rng.gen_range(1_000_000..10_000_000)
        * (VESTING_WEEKS as u64 / VESTING_WEEKS_FREQUENCY as u64 * 10);
    // Initial unlock amount is 10% of the total address reward.
    let initial_unlock_amount = amount * 10 / 100;
    // Vested amount is 90% of the total address reward spread across the vesting
    // schedule.
    let vested_amount = amount * 90 / 100 / (VESTING_WEEKS as u64 / VESTING_WEEKS_FREQUENCY as u64);

    (initial_unlock_amount, vested_amount)
}
