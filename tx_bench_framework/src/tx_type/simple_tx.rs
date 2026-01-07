// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use iota_json_rpc_types::IotaTransactionBlockEffectsAPI;
use iota_keys::keystore::AccountKeystore;
use iota_sdk::{
    IotaClient,
    rpc_types::IotaTransactionBlockResponseOptions,
    types::{
        quorum_driver_types::ExecuteTransactionRequestType,
        transaction::{Transaction, TransactionData},
    },
};
use iota_sdk_types::crypto::Intent;
use iota_types::{base_types::IotaAddress, signature::GenericSignature};

use crate::{SubmitResult, get_two_distinct_coins, tx_type::build_split_and_transfer_pt};

pub async fn submit_standard_tx<K: AccountKeystore>(
    client: &IotaClient,
    keystore: &K,
    sender: IotaAddress,
    recipient: IotaAddress,
    gas_budget: u64,
    split_amount: u64,
    wait_mode: ExecuteTransactionRequestType,
) -> Result<SubmitResult> {
    let gas_price = client.read_api().get_reference_gas_price().await?;
    let (gas_coin, pay_coin) = get_two_distinct_coins(client, sender).await?;
    let pt = build_split_and_transfer_pt(pay_coin.object_ref(), recipient, split_amount)?;
    let tx_data = TransactionData::new_programmable(
        sender,
        vec![gas_coin.object_ref()],
        pt,
        gas_budget,
        gas_price,
    );
    let signatures: Vec<GenericSignature> = vec![
        keystore
            .sign_secure(&sender, &tx_data, Intent::iota_transaction())?
            .into(),
    ];
    let start = std::time::Instant::now();
    let resp = client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_generic_sig_data(tx_data, signatures),
            IotaTransactionBlockResponseOptions::full_content(),
            Some(wait_mode),
        )
        .await?;
    let end = start.elapsed().as_millis();
    Ok(SubmitResult {
        digest: resp.digest.to_string(),
        gas_used: resp
            .effects
            .as_ref()
            .map(|e| format!("{:?}", e.gas_cost_summary())),
        elapsed_ms: end,
    })
}
