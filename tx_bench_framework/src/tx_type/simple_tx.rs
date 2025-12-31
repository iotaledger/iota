use anyhow::Result;
use iota_keys::keystore::AccountKeystore;
use iota_sdk::{
    IotaClient,
    rpc_types::IotaTransactionBlockResponseOptions,
    types::{
        quorum_driver_types::ExecuteTransactionRequestType,
        transaction::{Transaction, TransactionData},
    },
};
use crate::build_split_and_transfer_pt;
use crate::get_two_distinct_coins;
use iota_json_rpc_types::{IotaTransactionBlockEffectsAPI};
use iota_types::{base_types::IotaAddress, signature::GenericSignature};
use iota_sdk_types::crypto::Intent;
use crate::SubmitResult;

pub async fn submit_standard_tx<K: AccountKeystore>(
    client: &IotaClient,
    keystore: &K,
    sender: IotaAddress,
    recipient: IotaAddress,
    gas_budget: u64,
    split_amount: u64,
) -> Result<SubmitResult> {
    let t0 = std::time::Instant::now();
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
        keystore.sign_secure(&sender, &tx_data, Intent::iota_transaction())?.into(),
    ];
    let resp = client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_generic_sig_data(tx_data, signatures),
            IotaTransactionBlockResponseOptions::full_content(),
            Some(ExecuteTransactionRequestType::WaitForEffectsCert),
        )
        .await?;
    Ok(SubmitResult {
        digest: resp.digest.to_string(),
        gas_used: resp.effects.as_ref().map(|e| format!("{:?}", e.gas_cost_summary())),
        elapsed_ms: t0.elapsed().as_millis(),
    })
}