// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Context, Result};
use crate::utils::{get_coin};
use iota_keys::keystore::{AccountKeystore};
use iota_sdk::{
    IotaClient,
    rpc_types::{IotaTransactionBlockResponseOptions, ObjectChange},
    types::{
        base_types::ObjectID,
        quorum_driver_types::ExecuteTransactionRequestType,
        transaction::{Transaction},
    },
};
use iota_types::{
    base_types::ObjectRef,
    object::Owner,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    signature::GenericSignature,
    transaction::{ObjectArg, TransactionData},
};
use iota_types::base_types::IotaAddress;
use move_core_types::ident_str;
use iota_sdk_types::crypto::Intent;

use crate::AuthenticatorKind;

pub async fn create_abstract_account<K: AccountKeystore>(
    client: &IotaClient,
    sender: IotaAddress,
    keystore: &K,
    aa_package_id: ObjectID,
    aa_package_metadata_ref: ObjectRef,
    authenticator: AuthenticatorKind,
    gas_budget: u64,
) -> Result<(ObjectRef, IotaAddress)> {
    let sender_pk = keystore.get_key(&sender)?.public();

    let gas_coin = get_coin(client, sender).await.context("get_coin failed")?;
    let gas_price = client.read_api().get_reference_gas_price().await?;

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();

        let args = vec![
            builder.obj(ObjectArg::ImmOrOwnedObject(aa_package_metadata_ref))?,
            builder.pure(authenticator.module_name())?,
            builder.pure(authenticator.function_name())?,
            builder.pure(sender_pk.as_ref())?,
        ];

        builder.programmable_move_call(
            aa_package_id,
            ident_str!("abstract_account").to_owned(),
            ident_str!("create").to_owned(),
            vec![],
            args,
        );

        builder.finish()
    };

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

    let resp = client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_generic_sig_data(tx_data, signatures),
            IotaTransactionBlockResponseOptions::full_content(),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .context("execute_transaction_block(create AbstractAccount) failed")?;

    println!("\n--- Raw create-account response (pretty) ---");
    println!("{}", serde_json::to_string_pretty(&resp)?);

    let aa_ref: ObjectRef = resp
        .object_changes
        .as_ref()
        .and_then(|changes| {
            changes.iter().find_map(|change| match change {
                ObjectChange::Created {
                    object_type, owner, ..
                } => {
                    let ty = object_type.to_string();
                    let is_aa = ty.contains("::abstract_account::AbstractAccount");
                    let is_shared = matches!(owner, Owner::Shared { .. });
                    if is_aa && is_shared {
                        Some(change.object_ref())
                    } else {
                        None
                    }
                }
                _ => None,
            })
        })
        .ok_or_else(|| anyhow!("Created shared AbstractAccount not found in object_changes"))?;

    let aa_addr: IotaAddress = aa_ref.0.into();
    Ok((aa_ref, aa_addr))
}
