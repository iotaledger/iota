// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! IOTA airdrop vesting schedule scenario.
//! 2-years, initial unlock, bi-weekly unlock.
//! One mnemonic, multi accounts, multi addresses.
//! Some addresses have initial unlock, some don't.
//! Some addresses have expired/unexpired timelocked outputs, some only have unexpired.

use std::time::SystemTime;

use iota_sdk::{
    client::secret::{mnemonic::MnemonicSecretManager, SecretManage},
    types::block::{
        output::{
            unlock_condition::{AddressUnlockCondition, TimelockUnlockCondition},
            BasicOutputBuilder, Output,
        },
        payload::transaction::TransactionId,
    },
};

use crate::stardust::types::output_header::OutputHeader;

const MNEMONIC: &str = "sense silent picnic predict any public install educate trial depth faith voyage age exercise perfect hair favorite glimpse blame wood wave fiber maple receive";
const ACCOUNTS: u32 = 10;
const ADDRESSES_PER_ACCOUNT: u32 = 20;
const COIN_TYPE: u32 = 4218;
const VESTING_WEEKS: usize = 104;
const VESTING_WEEKS_FREQUENCY: usize = 2;
const MERGE_MILESTONE_INDEX: u32 = 7669900;
const MERGE_TIMESTAMP_SECS: u32 = 1696406475;

pub(crate) async fn outputs() -> anyhow::Result<Vec<(OutputHeader, Output)>> {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs() as u32;
    let mut outputs = Vec::new();
    let secret_manager = MnemonicSecretManager::try_from_mnemonic(MNEMONIC)?;

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

            // The modulos 3 and 5 are chosen because they create a pattern of all possible combinations of having an initial unlock and having expired timelock outputs.

            if address_index % 3 != 0 {
                let output_header = OutputHeader::new_testing(
                    *TransactionId::from(rand::random::<[u8; 32]>()),
                    0,
                    [0; 32],
                    MERGE_MILESTONE_INDEX,
                    MERGE_TIMESTAMP_SECS,
                );
                let output = Output::from(
                    BasicOutputBuilder::new_with_amount(10_000_000)
                        .add_unlock_condition(AddressUnlockCondition::new(address))
                        .finish()
                        .unwrap(),
                );

                outputs.push((output_header, output));
            }

            for offset in (0..=VESTING_WEEKS).step_by(VESTING_WEEKS_FREQUENCY) {
                let timelock = MERGE_TIMESTAMP_SECS + offset as u32 * 604_800;

                if address_index % 5 == 0 && timelock > now {
                    let output_header = OutputHeader::new_testing(
                        *TransactionId::from(rand::random::<[u8; 32]>()),
                        0,
                        [0; 32],
                        MERGE_MILESTONE_INDEX,
                        MERGE_TIMESTAMP_SECS,
                    );
                    let output = Output::from(
                        BasicOutputBuilder::new_with_amount(1_000_000)
                            .add_unlock_condition(AddressUnlockCondition::new(address))
                            .add_unlock_condition(TimelockUnlockCondition::new(timelock)?)
                            .finish()
                            .unwrap(),
                    );

                    outputs.push((output_header, output));
                }
            }
        }
    }

    Ok(outputs)
}
