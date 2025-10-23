// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Example demonstrating how to create an abstracted account and send a TX
//! through it using a Move-based Ed25519 authentication. In order to work, it
//! requires a running local network.

use docs_examples::utils::{compile_package, get_coin, request_tokens};
use fastcrypto::{
    ed25519::Ed25519Signature,
    encoding::{Encoding, Hex},
    traits::Authenticator,
};
use iota_keys::keystore::{AccountKeystore, InMemKeystore};
use iota_sdk::{
    IotaClient, IotaClientBuilder,
    rpc_types::{IotaTransactionBlockResponseOptions, ObjectChange},
    types::{
        IOTA_FRAMEWORK_ADDRESS,
        base_types::ObjectID,
        crypto::SignatureScheme::ED25519,
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        quorum_driver_types::ExecuteTransactionRequestType,
        transaction::{Argument, Transaction, TransactionData},
    },
};
use iota_types::{
    TypeTag,
    base_types::{IotaAddress, ObjectRef},
    move_authenticator::MoveAuthenticator,
    object::Owner,
    signature::GenericSignature,
    transaction::{CallArg, ObjectArg},
};
use move_core_types::ident_str;
use shared_crypto::intent::Intent;

/// Got from iota-genesis-builder/src/stardust/test_outputs/stardust_mix.rs
const MAIN_ADDRESS_MNEMONIC: &str = "rain flip mad lamp owner siren tower buddy wolf shy tray exit glad come dry tent they pond wrist web cliff mixed seek drum";

pub const GAS_BUDGET: u64 = 100_000_000;

const IOTACCOUNT_PACKAGE_PATH: &str = "../../../examples/move/iotaccount/";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Build an iota client for a local network
    let iota_client = IotaClientBuilder::default().build_localnet().await?;

    // Setup the temporary keystore
    let mut keystore = InMemKeystore::new_insecure_for_tests(0);

    // Derive the address of the first account and set it as default
    let sender = keystore.import_from_mnemonic(MAIN_ADDRESS_MNEMONIC, ED25519, None, None)?;
    println!("Sender address: {sender}");

    // Request faucet coins sender
    request_tokens(&iota_client, sender).await?;

    // Publish the Move Account Abstraction package
    let iotaccount_package_id =
        publish_account_abstraction_package(&iota_client, sender, &keystore).await?;
    println!("IOTAccount package id: {iotaccount_package_id}");

    // Create an IOTAccount
    let iotaccount_ref =
        create_iota_account(&iota_client, sender, iotaccount_package_id, &keystore).await?;
    println!("IOTAccount object id: {}", iotaccount_ref.0);

    // Request faucet coins for iotaccount
    request_tokens(&iota_client, iotaccount_ref.0.into()).await?;

    // Create a simple transaction from the IOTA account
    let tx_data =
        iota_account_simple_tx(&iota_client, iotaccount_package_id, iotaccount_ref).await?;
    let tx_digest = tx_data.digest().into_inner();

    // Sign the tx data with the sender key
    let signature = keystore.sign_hashed(&sender, &tx_digest)?;
    let hex_encoded_signature: String = Hex::encode(signature)
        .chars()
        .skip(2) // flag prefix length
        .take(Ed25519Signature::LENGTH * 2)
        .collect();
    println!("Hex encoded signature: {hex_encoded_signature}");

    // Create the MoveAuthenticator for the Ed25519 signature authenticator:
    // public fun authenticate_ed25519(
    //    self: &IOTAccount,
    //    signature: vector<u8>,
    //    _: &AuthContext,
    //    ctx: &TxContext,
    let self_call_arg = CallArg::Object(ObjectArg::SharedObject {
        id: iotaccount_ref.0,
        initial_shared_version: iotaccount_ref.1,
        mutable: false,
    });
    let signature_call_arg = CallArg::Pure(bcs::to_bytes(&hex_encoded_signature)?);
    let signatures = vec![GenericSignature::MoveAuthenticator(
        MoveAuthenticator::new_for_testing(vec![signature_call_arg], vec![], self_call_arg),
    )];

    let transaction_response = iota_client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_generic_sig_data(tx_data, signatures),
            IotaTransactionBlockResponseOptions::full_content(),
            ExecuteTransactionRequestType::WaitForLocalExecution,
        )
        .await?;

    println!("Main AA TX digest: {}", transaction_response.digest);

    Ok(())
}

/// Publishes the Move Account Abstraction (AA) package on-chain.
///
/// # Arguments
/// - `iota_client`: Initialized IOTA client instance.
/// - `sender`: Address used to pay for package deployment.
/// - `keystore`: Source of secret keys for signing.
///
/// # Returns
/// The id of the published package.
pub async fn publish_account_abstraction_package<K: AccountKeystore>(
    iota_client: &IotaClient,
    sender: IotaAddress,
    keystore: &K,
) -> anyhow::Result<ObjectID> {
    // Build the Move package from source
    let compiled_package = compile_package(IOTACCOUNT_PACKAGE_PATH)?;

    // Get multisig_addr coin for payment
    let sender_gas_coin = get_coin(iota_client, sender).await?;

    // Prepare publish transaction
    let tx_data = iota_client
        .transaction_builder()
        .publish(
            sender,
            compiled_package.get_package_bytes(false),
            compiled_package.get_dependency_storage_package_ids(),
            sender_gas_coin.coin_object_id,
            GAS_BUDGET,
        )
        .await?;

    let signatures: Vec<GenericSignature> = vec![
        keystore
            .sign_secure(&sender, &tx_data, Intent::iota_transaction())?
            .into(),
    ];

    let transaction_response = iota_client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_generic_sig_data(tx_data, signatures),
            IotaTransactionBlockResponseOptions::full_content(),
            ExecuteTransactionRequestType::WaitForLocalExecution,
        )
        .await?;

    Ok(transaction_response
        .object_changes
        .as_ref()
        .and_then(|changes| {
            changes.iter().find_map(|change| match change {
                ObjectChange::Published { .. } => Some(change.object_ref().0),
                _ => None,
            })
        })
        .expect("Expected a Published object in the transaction response"))
}

pub async fn create_iota_account<K: AccountKeystore>(
    iota_client: &IotaClient,
    sender: IotaAddress,
    iotaccount_package_id: ObjectID,
    keystore: &K,
) -> anyhow::Result<ObjectRef> {
    let sender_pk = keystore.get_key(&sender)?.public();

    let gas_coin = get_coin(iota_client, sender).await?;

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();

        // create auth info
        let arguments = vec![
            builder.pure(iotaccount_package_id)?,
            builder.pure("keyed_iotaccount")?,
            builder.pure("authenticate_ed25519")?,
        ];
        if let Argument::Result(authenticator_info_v1) = builder.programmable_move_call(
            IOTA_FRAMEWORK_ADDRESS.into(),
            ident_str!("account").to_owned(),
            ident_str!("create_auth_info_v1").to_owned(),
            vec![],
            arguments,
        ) {
            // Create the IOTA account.
            let arguments = vec![
                builder.pure(sender_pk.as_ref())?,
                Argument::Result(authenticator_info_v1),
            ];
            builder.programmable_move_call(
                iotaccount_package_id,
                ident_str!("keyed_iotaccount").to_owned(),
                ident_str!("create").to_owned(),
                vec![],
                arguments,
            );
        }
        builder.finish()
    };

    // Setup gas budget and gas price
    let gas_price = iota_client.read_api().get_reference_gas_price().await?;

    // Create the transaction data that will be sent to the network
    let tx_data = TransactionData::new_programmable(
        sender,
        vec![gas_coin.object_ref()],
        pt,
        GAS_BUDGET,
        gas_price,
    );

    // Sign the transaction
    let signature = keystore.sign_secure(&sender, &tx_data, Intent::iota_transaction())?;

    // Execute transaction
    let transaction_response = iota_client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_data(tx_data, vec![signature]),
            IotaTransactionBlockResponseOptions::full_content(),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;

    println!(
        "Create IOTAccount TX digest: {}",
        transaction_response.digest
    );

    Ok(transaction_response
        .object_changes
        .as_ref()
        .and_then(|changes| {
            changes.iter().find_map(|change| match change {
                ObjectChange::Created { owner, .. } => {
                    if matches!(owner, Owner::Shared { .. }) {
                        Some(change.object_ref())
                    } else {
                        None
                    }
                }
                _ => None,
            })
        })
        .expect("Expected a shared object in the transaction response"))
}

pub async fn iota_account_simple_tx(
    iota_client: &IotaClient,
    iotaccount_package_id: ObjectID,
    iotaccount_ref: ObjectRef,
) -> anyhow::Result<TransactionData> {
    let sender = iotaccount_ref.0.into();
    let gas_coin = get_coin(iota_client, sender).await?;

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();

        // Random IOTA account command.
        let arguments = vec![
            builder.obj(ObjectArg::SharedObject {
                id: iotaccount_ref.0,
                initial_shared_version: iotaccount_ref.1,
                mutable: true,
            })?,
            builder.pure(1_u8)?,
            builder.pure(2_u8)?,
        ];
        builder.programmable_move_call(
            iotaccount_package_id,
            ident_str!("iota_account").to_owned(),
            ident_str!("add_field").to_owned(),
            vec![TypeTag::U8, TypeTag::U8],
            arguments,
        );
        builder.finish()
    };

    // Setup gas budget and gas price
    let gas_price = iota_client.read_api().get_reference_gas_price().await?;

    // Create the transaction data that will be sent to the network
    Ok(TransactionData::new_programmable(
        sender,
        vec![gas_coin.object_ref()],
        pt,
        GAS_BUDGET,
        gas_price,
    ))
}
