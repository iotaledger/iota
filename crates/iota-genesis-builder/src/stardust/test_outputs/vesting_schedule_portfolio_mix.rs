// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Portfolio mix with vesting outputs.
//! 4-years, initial unlock, bi-weekly unlock.
//! One mnemonic/wallet, one account, multiple public and internal addresses.

use iota_sdk::{
    client::secret::{GenerateAddressOptions, SecretManage, mnemonic::MnemonicSecretManager},
    types::block::{
        address::Ed25519Address,
        output::{
            BasicOutputBuilder, OUTPUT_INDEX_RANGE, Output,
            unlock_condition::AddressUnlockCondition,
        },
    },
};
use rand::{Rng, rngs::StdRng};

use crate::stardust::{
    test_outputs::{MERGE_MILESTONE_INDEX, MERGE_TIMESTAMP_SECS, new_vested_output},
    types::{output_header::OutputHeader, output_index::OutputIndex},
};

const VESTING_WEEKS: usize = 208;
const VESTING_WEEKS_FREQUENCY: usize = 2;
const MNEMONIC: &str = "axis art silk merit assist hour bright always day legal misery arm laundry mule ship upon oil ski cup hat skin wet old sea";
// bip path values for account, internal, address
const ADDRESSES: &[[u32; 3]] = &[
    // public
    [0, 0, 0],
    [0, 0, 1],
    [0, 0, 2],
    // internal
    [0, 1, 0],
    [0, 1, 1],
    [0, 1, 2],
];

pub(crate) async fn outputs(
    rng: &mut StdRng,
    vested_index: &mut u32,
    coin_type: u32,
) -> anyhow::Result<Vec<(OutputHeader, Output)>> {
    let mut outputs = Vec::new();
    let secret_manager = MnemonicSecretManager::try_from_mnemonic(MNEMONIC)?;

    for [account_index, internal, address_index] in ADDRESSES {
        let address = secret_manager
            .generate_ed25519_addresses(
                coin_type,
                *account_index,
                *address_index..address_index + 1,
                (*internal == 1).then_some(GenerateAddressOptions::internal()),
            )
            .await?[0];

        match address_index {
            0 => {
                add_random_basic_output(&mut outputs, rng, address)?;
            }
            1 => {
                add_vested_outputs(&mut outputs, rng, vested_index, address)?;
            }
            2 => {
                add_random_basic_output(&mut outputs, rng, address)?;
                add_vested_outputs(&mut outputs, rng, vested_index, address)?;
            }
            _ => unreachable!(),
        }
    }
    Ok(outputs)
}

fn random_output_header(rng: &mut StdRng) -> OutputHeader {
    OutputHeader::new(
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

    outputs.push(new_vested_output(
        *vested_index,
        initial_unlock_amount,
        address,
        None,
        rng,
    )?);
    *vested_index -= 1;

    for offset in (0..=VESTING_WEEKS).step_by(VESTING_WEEKS_FREQUENCY) {
        let timelock = MERGE_TIMESTAMP_SECS + offset as u32 * 604_800;

        outputs.push(new_vested_output(
            *vested_index,
            vested_amount,
            address,
            Some(timelock),
            rng,
        )?);
        *vested_index -= 1;
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
