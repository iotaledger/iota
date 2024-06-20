// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

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

use crate::stardust::types::snapshot::OutputHeader;

const MNEMONIC: &str = "sense silent picnic predict any public install educate trial depth faith voyage age exercise perfect hair favorite glimpse blame wood wave fiber maple receive";
const ACCOUNTS: u32 = 10;
const ADDRESSES_PER_ACCOUNT: u32 = 10;
const COIN_TYPE: u32 = 4218;
const VESTING_WEEKS: usize = 104;
const VESTING_WEEKS_FREQUENCY: usize = 2;
const MERGE_TIMESTAMP_SECS: u32 = 1696406475;

pub(crate) async fn outputs() -> anyhow::Result<Vec<(OutputHeader, Output)>> {
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

            // TODO add initial unlock
            for offset in (0..=VESTING_WEEKS).step_by(VESTING_WEEKS_FREQUENCY) {
                let timelock = MERGE_TIMESTAMP_SECS + offset as u32 * 604_800;
                let output_header = OutputHeader::new_testing(
                    // TODO randomize
                    *TransactionId::from_str(
                        "0xb191c4bc825ac6983789e50545d5ef07a1d293a98ad974fc9498cb1812345678",
                    )
                    .unwrap(),
                    rand::random(),
                    rand::random(),
                    rand::random(),
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

    Ok(outputs)
}
