// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Ok, Result, anyhow};
use iota_keys::keystore::AccountKeystore;
use iota_sdk::{
    IotaClient,
    rpc_types::{IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions, ObjectChange},
    types::{
        Identifier,
        base_types::{IotaAddress, ObjectID, ObjectRef},
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        quorum_driver_types::ExecuteTransactionRequestType,
        signature::GenericSignature,
        transaction::{Command, ObjectArg, Transaction, TransactionData},
    },
};
use move_core_types::language_storage::StructTag;
use shared_crypto::intent::Intent;

use crate::{
    sig_utils::extract_pure_signature,
    utils::{GAS_BUDGET, THRESHOLD, WEIGHTS, get_coin},
};

const TX_MODULE_NAME: &str = "tx_flow";
const MULTISIG_PUBKEY_MODULE_NAME: &str = "multisig_pubkey";

/// Proposes a withdrawal transaction to the on-chain SmartAccount.
///
/// This function publishes a `ProposedTx` Move object, which includes:
/// - The transaction digest
/// - The raw transaction data (bcs bytes)
/// - A signature threshold (usually 2 for multisig)
///
/// # Returns
/// The `ObjectRef` of the created `ProposedTx` Move object.
pub async fn propose_tx_to_smart_account<K: AccountKeystore>(
    iota_client: &IotaClient,
    multisig_pubkeys: &[&[u8]],
    digest: Vec<u8>,
    withdraw_tx_data: &TransactionData,
    keystore: &K,
    alice_addr: IotaAddress,
    package_id: ObjectID,
    smart_account_object: ObjectRef,
) -> Result<ObjectRef> {
    let mut ptb_builder = ProgrammableTransactionBuilder::new();

    let arguments = vec![
        ptb_builder.pure(multisig_pubkeys)?,
        ptb_builder.pure(WEIGHTS)?,
        ptb_builder.pure(THRESHOLD)?,
    ];
    let multisig_pubkey = ptb_builder.command(Command::move_call(
        package_id,
        Identifier::new(MULTISIG_PUBKEY_MODULE_NAME)?,
        Identifier::new("new")?,
        vec![],
        arguments,
    ));

    let withdraw_tx_bytes = bcs::to_bytes(&withdraw_tx_data)?;
    let arguments = vec![
        ptb_builder.obj(ObjectArg::SharedObject {
            id: smart_account_object.0,
            initial_shared_version: smart_account_object.1,
            mutable: true,
        })?,
        ptb_builder.pure(digest)?,
        ptb_builder.pure(withdraw_tx_bytes)?,
        multisig_pubkey,
    ];
    ptb_builder.command(Command::move_call(
        package_id,
        Identifier::new(TX_MODULE_NAME)?,
        Identifier::new("entry_point")?,
        vec![],
        arguments,
    ));

    let gas_price = iota_client.read_api().get_reference_gas_price().await?;

    let alice_gas_coin = get_coin(iota_client, alice_addr).await?;
    let tx_data = TransactionData::new_programmable(
        alice_addr,
        vec![alice_gas_coin.object_ref()],
        ptb_builder.finish(),
        GAS_BUDGET,
        gas_price,
    );

    let signature = keystore.sign_secure(&alice_addr, &tx_data, Intent::iota_transaction())?;

    let transaction_response = iota_client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_data(tx_data, vec![signature]),
            IotaTransactionBlockResponseOptions::full_content(),
            ExecuteTransactionRequestType::WaitForLocalExecution,
        )
        .await?;

    print!("\n Proposed tx transaction info: ");
    println!("{}", transaction_response);

    // Extract the ProposedTx object from the response
    let proposed_tx_name = Identifier::new("ProposedTx")?;
    let proposed_tx_object = transaction_response
        .object_changes
        .as_ref()
        .and_then(|changes| {
            changes.iter().find_map(|change| match change {
                ObjectChange::Created {
                    object_type: StructTag { name, .. },
                    ..
                } if name == &proposed_tx_name => Some(change.object_ref()),
                _ => None,
            })
        })
        .ok_or_else(|| anyhow!("ProposedTx object not found in transaction"))?;

    Ok(proposed_tx_object)
}

/// Registers a signature for a `ProposedTx` using the signer's key and public
/// key.
///
/// Each call stores the signature along with the public key.
/// After reaching the required threshold (e.g. 2-of-2), the `SignedTx` will be
/// generated.
///
/// # Returns
/// The response containing all transaction and object updates.
pub async fn sign_proposed_tx<K: AccountKeystore>(
    iota_client: &IotaClient,
    proposed_tx_id: ObjectID,
    withdraw_tx_data: &TransactionData,
    keystore: &K,
    addr: IotaAddress,
    package_id: ObjectID,
    smart_account_object: ObjectRef,
) -> Result<IotaTransactionBlockResponse> {
    let signature: GenericSignature = keystore
        .sign_secure(&addr, &withdraw_tx_data, Intent::iota_transaction())?
        .into();
    let pub_key = keystore.get_key(&addr)?.public();
    let mut ptb_builder = ProgrammableTransactionBuilder::new();

    let pure_signature = extract_pure_signature(&signature);

    let arguments = vec![
        ptb_builder.obj(ObjectArg::SharedObject {
            id: smart_account_object.0,
            initial_shared_version: smart_account_object.1,
            mutable: true,
        })?,
        ptb_builder.pure(proposed_tx_id)?,
        ptb_builder.pure(pub_key.as_ref())?,
        ptb_builder.pure(pure_signature)?,
    ];
    ptb_builder.command(Command::move_call(
        package_id,
        Identifier::new(TX_MODULE_NAME)?,
        Identifier::new("sign_proposed_tx")?,
        vec![],
        arguments,
    ));

    let gas_price = iota_client.read_api().get_reference_gas_price().await?;

    let alice_gas_coin = get_coin(iota_client, addr).await?;
    let tx_data = TransactionData::new_programmable(
        addr,
        vec![alice_gas_coin.object_ref()],
        ptb_builder.finish(),
        GAS_BUDGET,
        gas_price,
    );

    let signature = keystore.sign_secure(&addr, &tx_data, Intent::iota_transaction())?;

    let transaction_response = iota_client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_data(tx_data, vec![signature]),
            IotaTransactionBlockResponseOptions::full_content(),
            ExecuteTransactionRequestType::WaitForLocalExecution,
        )
        .await?;

    print!("\n Sign proposed tx transaction info: ");
    println!("{}", transaction_response);

    Ok(transaction_response)
}
