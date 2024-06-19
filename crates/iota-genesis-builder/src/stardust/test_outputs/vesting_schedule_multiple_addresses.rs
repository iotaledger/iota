// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use iota_sdk::{
    client::secret::{mnemonic::MnemonicSecretManager, SecretManage},
    types::block::{
        address::Ed25519Address,
        output::{unlock_condition::AddressUnlockCondition, BasicOutputBuilder, Output},
        payload::transaction::TransactionId,
    },
};

use crate::stardust::types::snapshot::OutputHeader;

const MNEMONIC: &str = "sense silent picnic predict any public install educate trial depth faith voyage age exercise perfect hair favorite glimpse blame wood wave fiber maple receive";
const ACCOUNTS: u32 = 10;
const ADDRESSES_PER_ACCOUNT: u32 = 10;
const IOTA_COIN_TYPE: u32 = 4218;

pub(crate) async fn outputs() -> anyhow::Result<Vec<(OutputHeader, Output)>> {
    let mut outputs = Vec::new();
    let secret_manager = MnemonicSecretManager::try_from_mnemonic(MNEMONIC)?;

    for account_index in 0..ACCOUNTS {
        for address_index in 0..ADDRESSES_PER_ACCOUNT {
            let address = secret_manager
                .generate_ed25519_addresses(
                    IOTA_COIN_TYPE,
                    account_index,
                    address_index..address_index + 1,
                    None,
                )
                .await?[0];
            println!("{address:?}");
        }
    }

    // let output_header = OutputHeader::new_testing(
    //     *TransactionId::from_str(
    //         "0xb191c4bc825ac6983789e50545d5ef07a1d293a98ad974fc9498cb1812345678",
    //     )
    //     .unwrap(),
    //     rand::random(),
    //     rand::random(),
    //     rand::random(),
    // );
    // let output = Output::from(
    //     BasicOutputBuilder::new_with_amount(1_000_000)
    //         .add_unlock_condition(AddressUnlockCondition::new(Ed25519Address::from(
    //             rand::random::<[u8; Ed25519Address::LENGTH]>(),
    //         )))
    //         .finish()
    //         .unwrap(),
    // );

    // outputs.push((output_header, output));

    Ok(outputs)
}
