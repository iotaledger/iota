// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! IOTA airdrop vesting schedule scenario.
//! 2-years, initial unlock, bi-weekly unlock.
//! One mnemonic, multi accounts, multi addresses.
//! Some addresses have initial unlock, some don't.
//! Some addresses have expired/unexpired timelocked outputs, some only have
//! unexpired.

use std::time::SystemTime;

use iota_sdk::{
    client::secret::{mnemonic::MnemonicSecretManager, SecretManage},
    types::block::{
        address::Ed25519Address,
        output::{
            unlock_condition::{AddressUnlockCondition, TimelockUnlockCondition},
            BasicOutputBuilder, Output,
        },
    },
};
use iota_types::timelock::timelock::VESTED_REWARD_ID_PREFIX;
use rand::{random, rngs::StdRng, Rng, SeedableRng};

use crate::stardust::types::{output_header::OutputHeader, output_index::random_output_index};

const MNEMONIC: &str = "sense silent picnic predict any public install educate trial depth faith voyage age exercise perfect hair favorite glimpse blame wood wave fiber maple receive";
const ACCOUNTS: u32 = 10;
const ADDRESSES_PER_ACCOUNT: u32 = 20;
const COIN_TYPE: u32 = 4218;
const VESTING_WEEKS: usize = 104;
const VESTING_WEEKS_FREQUENCY: usize = 2;
const MERGE_MILESTONE_INDEX: u32 = 7669900;
const MERGE_TIMESTAMP_SECS: u32 = 1696406475;

fn new_output(
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

pub(crate) async fn outputs(vested_index: &mut u32) -> anyhow::Result<Vec<(OutputHeader, Output)>> {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs() as u32;
    let mut outputs = Vec::new();
    let secret_manager = MnemonicSecretManager::try_from_mnemonic(MNEMONIC)?;
    let mut transaction_id = [0; 32];

    let randomness_seed = random::<u64>();
    let mut rng = StdRng::seed_from_u64(randomness_seed);
    println!("vesting_schedule_iota_airdrop randomness seed: {randomness_seed}");

    // Prepare a transaction ID with the vested reward prefix.
    transaction_id[0..28]
        .copy_from_slice(&prefix_hex::decode::<[u8; 28]>(VESTED_REWARD_ID_PREFIX)?);

    for account_index in 0..ACCOUNTS {
        for address_index in 0..ADDRESSES_PER_ACCOUNT {
            let address = secret_manager
                .generate_ed25519_addresses(
                    COIN_TYPE,
                    account_index,
                    address_index..address_index + 1,
                    None,
                )
                .await?[0];
            // VESTING_WEEKS / VESTING_WEEKS_FREQUENCY * 10 so that `vested_amount` doesn't
            // lose precision.
            let amount = rng.gen_range(1_000_000..10_000_000)
                * (VESTING_WEEKS as u64 / VESTING_WEEKS_FREQUENCY as u64 * 10);
            // Initial unlock amount is 10% of the total address reward.
            let initial_unlock_amount = amount * 10 / 100;
            // Vested amount is 90% of the total address reward spread across the vesting
            // schedule.
            let vested_amount =
                amount * 90 / 100 / (VESTING_WEEKS as u64 / VESTING_WEEKS_FREQUENCY as u64);

            // The modulos 3 and 5 are chosen because they create a pattern of
            // all possible combinations of having an initial unlock and having
            //  expired timelock outputs.

            // 2 addresses out of 3 have an initial unlock.
            if address_index % 3 != 0 {
                outputs.push(new_output(
                    &mut transaction_id,
                    vested_index,
                    initial_unlock_amount,
                    address,
                    None,
                )?);
            }

            for offset in (0..=VESTING_WEEKS).step_by(VESTING_WEEKS_FREQUENCY) {
                let timelock = MERGE_TIMESTAMP_SECS + offset as u32 * 604_800;

                // 4 addresses out of 5 have unexpired and expired timelocked vested outputs.
                // 1 address out of 4 only has unexpired timelocked vested outputs.
                if address_index % 5 != 0 || timelock > now {
                    outputs.push(new_output(
                        &mut transaction_id,
                        vested_index,
                        vested_amount,
                        address,
                        Some(timelock),
                    )?);
                }
            }
        }
    }

    Ok(outputs)
}
