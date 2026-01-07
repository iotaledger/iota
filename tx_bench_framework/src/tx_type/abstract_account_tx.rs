// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result};
use fastcrypto::{
    ed25519::Ed25519Signature,
    encoding::{Encoding, Hex},
    traits::Authenticator,
};
use iota_json_rpc_types::IotaTransactionBlockEffectsAPI;
use iota_keys::keystore::AccountKeystore;
use iota_sdk::{
    IotaClient,
    rpc_types::IotaTransactionBlockResponseOptions,
    types::{
        base_types::ObjectID,
        quorum_driver_types::ExecuteTransactionRequestType,
        transaction::{Transaction, TransactionData},
    },
};
use iota_types::{
    base_types::{IotaAddress, SequenceNumber},
    move_authenticator::MoveAuthenticator,
    signature::GenericSignature,
    transaction::{CallArg, ObjectArg},
};

use crate::{
    AuthenticatorKind, SubmitResult, get_two_distinct_coins, registry_state::AccountState,
    tx_type::build_split_and_transfer_pt,
};

pub async fn submit_aa_tx<K: AccountKeystore>(
    client: &IotaClient,
    keystore: &K,
    owner: IotaAddress,
    state: &AccountState,
    recipient: IotaAddress,
    gas_budget: u64,
    split_amount: u64,
    wait_mode: ExecuteTransactionRequestType,
) -> Result<SubmitResult> {
    let aa_addr: IotaAddress = state
        .aa_address
        .parse()
        .context("bad aa_address in state")?;
    let sender = aa_addr;

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

    let aa_obj_id: ObjectID = state
        .aa_account_object_id
        .parse()
        .context("bad aa_account_object_id")?;
    let init_ver = SequenceNumber::from_u64(state.aa_account_version);

    // println!(
    //     "AA data: obj_id={}, version={}",
    //     aa_obj_id,
    //     init_ver.value()
    // );
    let self_call_arg = CallArg::Object(ObjectArg::SharedObject {
        id: aa_obj_id,
        initial_shared_version: init_ver,
        mutable: false,
    });

    let mut auth_args: Vec<CallArg> = vec![];

    match state.authenticator {
        AuthenticatorKind::Ed25519 | AuthenticatorKind::Ed25519Heavy => {
            let digest = tx_data.digest().into_inner();
            let hex_encoded_signature: String = Hex::encode(keystore.sign_hashed(&owner, &digest)?)
                .chars()
                .skip(2)
                .take(Ed25519Signature::LENGTH * 2)
                .collect();
            auth_args.push(CallArg::Pure(bcs::to_bytes(&hex_encoded_signature)?));
        }
        AuthenticatorKind::HelloWorld => {
            auth_args.push(CallArg::Pure(bcs::to_bytes("HelloWorld")?));
        }
    }

    let signatures = vec![GenericSignature::MoveAuthenticator(
        MoveAuthenticator::new_for_testing(auth_args, vec![], self_call_arg),
    )];

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
