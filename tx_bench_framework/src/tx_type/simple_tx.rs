// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result};
use iota_keys::keystore::AccountKeystore;
use iota_sdk::{
    IotaClient,
    types::{quorum_driver_types::ExecuteTransactionRequestType, transaction::TransactionData},
};
use iota_sdk_types::crypto::Intent;
use iota_types::{base_types::IotaAddress, signature::GenericSignature};

use crate::{
    tx_type::{SubmitResult, build_split_and_transfer_pt, execute_and_measure},
    utils::get_two_distinct_coins,
};

pub async fn submit_standard_tx<K: AccountKeystore>(
    client: &IotaClient,
    keystore: &K,
    sender: IotaAddress,
    recipient: IotaAddress,
    gas_budget: u64,
    split_amount: u64,
    wait_mode: ExecuteTransactionRequestType,
) -> Result<SubmitResult> {
    let gas_price = client
        .read_api()
        .get_reference_gas_price()
        .await
        .context("get_reference_gas_price failed")?;

    let (gas_coin, pay_coin) = get_two_distinct_coins(client, sender)
        .await
        .context("get_two_distinct_coins failed")?;

    let pt = build_split_and_transfer_pt(pay_coin.object_ref(), recipient, split_amount)
        .context("build_split_and_transfer_pt failed")?;

    let tx_data = TransactionData::new_programmable(
        sender,
        vec![gas_coin.object_ref()],
        pt,
        gas_budget,
        gas_price,
    );

    let signatures: Vec<GenericSignature> = vec![
        keystore
            .sign_secure(&sender, &tx_data, Intent::iota_transaction())
            .context("sign_secure failed")?
            .into(),
    ];

    execute_and_measure(client, tx_data, signatures, wait_mode).await
}
