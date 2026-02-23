// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result};
use fastcrypto::{
    ed25519::Ed25519Signature,
    encoding::{Encoding, Hex},
    traits::Authenticator,
};
use iota_keys::keystore::AccountKeystore;
use iota_sdk::{
    IotaClient,
    types::{
        base_types::ObjectID, quorum_driver_types::ExecuteTransactionRequestType,
        transaction::TransactionData,
    },
};
use iota_types::{
    base_types::{IotaAddress, SequenceNumber},
    move_authenticator::MoveAuthenticator,
    signature::GenericSignature,
    transaction::{CallArg, ObjectArg},
};

use crate::{
    TxType,
    cli::AuthenticatorKind,
    registry_state::AccountState,
    tx_type::{
        CoinRefs, SubmitResult, build_request_add_stake_pt, build_split_and_transfer_pt,
        execute_and_measure,
    },
};

fn build_move_auth_args<K: AccountKeystore>(
    keystore: &K,
    owner: IotaAddress,
    tx_data: &TransactionData,
    state: &AccountState,
) -> Result<Vec<CallArg>> {
    let mut auth_args: Vec<CallArg> = Vec::new();

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
            auth_args.push(CallArg::Pure(
                bcs::to_bytes("HelloWorld").context("bcs::to_bytes(HelloWorld) failed")?,
            ));
        }
        AuthenticatorKind::MaxArgs128 | AuthenticatorKind::MaxArgs255 => {
            for bench_obj in state.bench_objects.iter() {
                let obj_ref = bench_obj
                    .to_object_ref()
                    .context("bench_obj.to_object_ref failed")?;
                auth_args.push(CallArg::Object(ObjectArg::ImmOrOwnedObject(obj_ref)));
            }
        }
    }

    Ok(auth_args)
}

pub async fn submit_aa_tx<K: AccountKeystore>(
    client: &IotaClient,
    keystore: &K,
    owner: IotaAddress,
    state: &AccountState,
    recipient: IotaAddress,
    gas_budget: u64,
    split_amount: u64,
    tx_type: TxType,
    wait_mode: ExecuteTransactionRequestType,
    coins: &mut CoinRefs,
) -> Result<SubmitResult> {
    let aa_addr: IotaAddress = state
        .aa_address
        .parse()
        .context("bad aa_address in state")?;
    let sender = aa_addr;

    let gas_price = client
        .read_api()
        .get_reference_gas_price()
        .await
        .context("get_reference_gas_price failed")?;

    let pt = match tx_type {
        TxType::OwnedObject => build_split_and_transfer_pt(coins.pay, recipient, split_amount)
            .context("build_split_and_transfer_pt failed")?,
        TxType::SharedObject => build_request_add_stake_pt(coins.pay, recipient)
            .context("build_request_add_stake_pt failed")?,
    };

    let tx_data =
        TransactionData::new_programmable(sender, vec![coins.gas], pt, gas_budget, gas_price);

    let aa_obj_id: ObjectID = state
        .aa_account_object_id
        .parse()
        .context("bad aa_account_object_id")?;

    let init_ver = SequenceNumber::from_u64(state.aa_account_version);

    let self_call_arg = CallArg::Object(ObjectArg::SharedObject {
        id: aa_obj_id,
        initial_shared_version: init_ver,
        mutable: false,
    });

    let auth_args = build_move_auth_args(keystore, owner, &tx_data, state)
        .context("build_move_auth_args failed")?;

    let signatures = vec![GenericSignature::MoveAuthenticator(MoveAuthenticator::new(
        auth_args,
        vec![],
        self_call_arg,
    ))];

    execute_and_measure(client, tx_data, signatures, wait_mode, coins).await
}
