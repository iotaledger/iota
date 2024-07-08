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
    types::block::output::Output,
};
use iota_types::timelock::timelock::VESTED_REWARD_ID_PREFIX;
use rand::{random, rngs::StdRng, Rng, SeedableRng};

use crate::stardust::{
    test_outputs::{new_vested_output, MERGE_TIMESTAMP_SECS},
    types::output_header::OutputHeader,
};

const MNEMONIC: &str = "mesh dose off wage gas tent key light help girl faint catch sock trouble guard moon talk pill enemy hawk gain mix sad mimic";
const ACCOUNTS: u32 = 10;
const ADDRESSES_PER_ACCOUNT: u32 = 20;
const COIN_TYPE: u32 = 4218;
const VESTING_WEEKS: usize = 104;
const VESTING_WEEKS_FREQUENCY: usize = 2;

pub(crate) async fn outputs(vested_index: &mut u32) -> anyhow::Result<Vec<(OutputHeader, Output)>> {
    let randomness_seed = random::<u64>();
    let mut rng = StdRng::seed_from_u64(randomness_seed);

    println!("------------------------------");
    println!("vesting_schedule_iota_airdrop");
    println!("Randomness seed: {randomness_seed}");
    println!("Mnemonic: {MNEMONIC}");
    println!(
        "{ADDRESSES_PER_ACCOUNT} accounts, {ADDRESSES_PER_ACCOUNT} addresses per account, coin type {COIN_TYPE}, {VESTING_WEEKS} vesting weeks, frequency of {VESTING_WEEKS_FREQUENCY} weeks"
    );
    println!("------------------------------\n");

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs() as u32;
    let mut outputs = Vec::new();
    let secret_manager = MnemonicSecretManager::try_from_mnemonic(MNEMONIC)?;
    let mut transaction_id = [0; 32];

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
            // expired timelock outputs.

            // 2 addresses out of 3 have an initial unlock.
            if address_index % 3 != 0 {
                outputs.push(new_vested_output(
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
                    outputs.push(new_vested_output(
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
