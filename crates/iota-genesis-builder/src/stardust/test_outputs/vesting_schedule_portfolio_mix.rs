// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Entity vesting schedule scenario.
//! 4-years, initial unlock, bi-weekly unlock.
//! One mnemonic, one account, one address.

use iota_sdk::{
    client::secret::{mnemonic::MnemonicSecretManager, GenerateAddressOptions, SecretManage},
    types::block::{
        address::Address,
        output::{
            unlock_condition::AddressUnlockCondition, BasicOutput, BasicOutputBuilder, Output,
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
// TODO: already used by other test data, so better use a different one
const MNEMONIC: &str = "rain flip mad lamp owner siren tower buddy wolf shy tray exit glad come dry tent they pond wrist web cliff mixed seek drum";
// bip path values for account, internal, address, then the scenario case
// identifier
const ADDRESSES: &'static [[u32; 3]] = &[
    // public
    [0, 0, 0],
    [0, 0, 1], // public, no coins, vested outputs
    [0, 0, 2], // public, coins, no vested outputs
    [0, 0, 3], // public, coins, vested outputs
    // internal
    [0, 1, 0], // internal, no coins, no vested outputs
    [0, 1, 1], // internal, no coins, vested outputs
    [0, 1, 2], // internal, coins, no vested outputs
    [0, 1, 3], // internal, coins, vested outputs
];

pub(crate) async fn outputs(vested_index: &mut u32) -> anyhow::Result<Vec<(OutputHeader, Output)>> {
    let mut outputs = Vec::new();
    let secret_manager = MnemonicSecretManager::try_from_mnemonic(MNEMONIC)?;
    let mut transaction_id = [0; 32];

    let randomness_seed = random::<u64>();
    let mut rng = StdRng::seed_from_u64(randomness_seed);
    println!("vesting_schedule_portfolio_mix randomness seed: {randomness_seed}");

    // Prepare a transaction ID with the vested reward prefix.
    transaction_id[0..28]
        .copy_from_slice(&prefix_hex::decode::<[u8; 28]>(VESTED_REWARD_ID_PREFIX)?);

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
                // just basic output
                let (header, basic) = random_basic_output(&mut rng, address);
                outputs.push((header, basic.into()));
            }
            1 => {
                // basic output and vested outputs
                let (header, basic) = random_basic_output(&mut rng, address);
                outputs.push((header, basic.into()));
                let (initial_unlock_amount, vested_amount) =
                    initial_unlock_and_vested_amounts(&mut rng);
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
            }
            2 => {
                // no funds at all
            }
            3 => {
                // no coins, but vested outputs
                let (initial_unlock_amount, vested_amount) =
                    initial_unlock_and_vested_amounts(&mut rng);
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

fn random_basic_output(rng: &mut StdRng, owner: impl Into<Address>) -> (OutputHeader, BasicOutput) {
    let basic_output_header = random_output_header(rng);

    let amount = rng.gen_range(1_000_000..10_000_000);
    let basic_output = BasicOutputBuilder::new_with_amount(amount)
        .add_unlock_condition(AddressUnlockCondition::new(owner))
        .finish()
        .unwrap();

    (basic_output_header, basic_output)
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
