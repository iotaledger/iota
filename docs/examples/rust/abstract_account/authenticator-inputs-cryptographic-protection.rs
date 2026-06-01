// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Example demonstrating how to protect an abstract account authenticator
//! inputs from being tampered.

use std::str::FromStr;

use anyhow::{Result, bail};
use docs_examples::utils::{
    create_transaction_data, execute_ptb, execute_transaction, get_coin, publish_aa_package,
    request_tokens_from_faucet,
};
use fastcrypto::{
    ed25519::Ed25519Signature,
    encoding::{Encoding, Hex},
    hash::{HashFunction, Sha256},
    traits::Authenticator,
};
use iota_keys::keystore::{AccountKeystore, InMemKeystore};
use iota_sdk::{
    IotaClient, IotaClientBuilder,
    rpc_types::{IotaTransactionBlockEffectsAPI, ObjectChange},
    types::{
        base_types::ObjectID,
        crypto::SignatureScheme::ED25519,
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        transaction::{Argument, Transaction},
    },
};
use iota_sdk_types::{Identifier, TypeTag};
use iota_types::{
    base_types::{IotaAddress, ObjectRef},
    crypto::PublicKey,
    object::Owner,
    signature::GenericSignature,
    transaction::{CallArg, SharedObjectRef},
    utils::MoveAuthenticator,
};

/// Got from iota-genesis-builder/src/stardust/test_outputs/stardust_mix.rs
const MAIN_ADDRESS_MNEMONIC: &str = "okay pottery arch air egg very cave cash poem gown sorry mind poem crack dawn wet car pink extra crane hen bar boring salt";

/// Account example relative path.
const AA_PACKAGE_PATH: &str =
    "../move/abstract_account/authenticator-inputs-cryptographic-protection";

/// Account module name.
const AA_MODULE_NAME: &str = "account";
/// Account struct name.
const AA_ACCOUNT_NAME: &str = "Account";
/// Account create function name.
const AA_CREATE_ACCOUNT_FN_NAME: &str = "create";
/// Account authenticator function name.
const AA_AUTHENTICATE_FN_NAME: &str = "authenticate";

/// Blacklist module name.
const AA_BLACKLIST_MODULE_NAME: &str = "blacklist";
/// Create blacklist function name.
const AA_CREATE_BLACKLIST_FN_NAME: &str = "create";

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

    // Create an abstract account instance using the publisher public key for
    // simplicity
    let pub_key = keystore.keys()[0].public();
    let account_ref = create_account(
        &iota_client,
        &mut keystore,
        publisher,
        &package_id,
        metadata_ref,
        &pub_key,
    )
    .await?;
    let account_address = account_ref.object_id.into();

    // Top up the account address from the faucet
    request_tokens_from_faucet(&iota_client, account_address).await?;

    // Create an empty blacklist shared object instance
    let blacklist_ref =
        create_blacklist(&iota_client, &mut keystore, publisher, &package_id).await?;

    // Create an abstract account transaction
    let recipient_a = IotaAddress::random();

    println!("Recipient A address: {recipient_a}");

    let transaction = create_test_transaction(
        &iota_client,
        &mut keystore,
        publisher,
        recipient_a,
        &account_ref,
        &blacklist_ref,
    )
    .await?;

    // Execute the transaction
    let _ = execute_transaction(&iota_client, transaction).await?;

    // Get a transferred coin from the recipient address A to verify the
    // transaction succeeded
    let transferred_coin = get_coin(&iota_client, recipient_a).await?;
    println!("Recipient A coin: {transferred_coin:?}");

    // Create one more test transaction
    let recipient_b = IotaAddress::random();

    println!("Recipient B address: {recipient_b}");

    let transaction = create_test_transaction(
        &iota_client,
        &mut keystore,
        publisher,
        recipient_b,
        &account_ref,
        &blacklist_ref,
    )
    .await?;

    // Create a new empty blacklist shared object instance
    let new_blacklist_ref =
        create_blacklist(&iota_client, &mut keystore, publisher, &package_id).await?;

    // Swap the blacklist shared object in the transaction with the new one
    let hacked_transaction = swap_blacklist_in_transaction(transaction, &new_blacklist_ref);

    // Execute the hacked transaction.
    let transaction_response = execute_transaction(&iota_client, hacked_transaction).await;

    // The transaction is expected to be failed due to the blacklist object was
    // swapped
    match transaction_response {
        Ok(response) => {
            bail!("Transaction expected to fail, but got response: {response:?}");
        }
        Err(e) => {
            if !e
                .to_string()
                .contains("Failed to execute the Move authenticator")
            {
                bail!("Transaction failed with unexpected error: {e}");
            }
        }
    }

    // A transferred coin is not expected to be found at the recipient address
    match get_coin(&iota_client, recipient_b).await {
        Ok(coin) => {
            bail!("Transaction expected to fail, but got a transferred coin: {coin:?}");
        }
        Err(e) => {
            if !e.to_string().contains("No coin object found for address") {
                bail!("Getting a coin failed with unexpected error: {e}");
            }
        }
    }

    // Create one more test transaction
    let recipient_c = IotaAddress::random();

    println!("Recipient C address: {recipient_c}");

    let transaction = create_test_transaction(
        &iota_client,
        &mut keystore,
        publisher,
        recipient_c,
        &account_ref,
        &blacklist_ref,
    )
    .await?;

    // Swap the raw value in the transaction
    let hacked_transaction = swap_raw_value_in_transaction(transaction, 24);

    // Execute the hacked transaction.
    let transaction_response = execute_transaction(&iota_client, hacked_transaction).await;

    // The transaction is expected to be failed due to the raw value was swapped
    match transaction_response {
        Ok(response) => {
            bail!("Transaction expected to fail, but got response: {response:?}");
        }
        Err(e) => {
            if !e
                .to_string()
                .contains("Failed to execute the Move authenticator")
            {
                bail!("Transaction failed with unexpected error: {e}");
            }
        }
    }

    // A transferred coin is not expected to be found at the recipient address
    match get_coin(&iota_client, recipient_c).await {
        Ok(coin) => {
            bail!("Transaction expected to fail, but got a transferred coin: {coin:?}");
        }
        Err(e) => {
            if !e.to_string().contains("No coin object found for address") {
                bail!("Getting a coin failed with unexpected error: {e}");
            }
        }
    }

    Ok(())
}

/// Creates an abstract account instance.
pub async fn create_account(
    iota_client: &IotaClient,
    keystore: &mut InMemKeystore,
    publisher: IotaAddress,
    package_id: &ObjectID,
    package_metadata_ref: ObjectRef,
    pub_key: &PublicKey,
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
                builder.pure(pub_key.as_ref())?,
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

/// Creates a blacklist shared object instance.
pub async fn create_blacklist(
    iota_client: &IotaClient,
    keystore: &mut InMemKeystore,
    publisher: IotaAddress,
    package_id: &ObjectID,
) -> Result<ObjectRef> {
    // Create a PTB that creates a blacklist shared object instance
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();

        builder.programmable_move_call(
            *package_id,
            Identifier::from_static(AA_BLACKLIST_MODULE_NAME),
            Identifier::from_static(AA_CREATE_BLACKLIST_FN_NAME),
            vec![],
            vec![],
        );

        builder.finish()
    };

    // Execute the transaction
    let transaction_response = execute_ptb(iota_client, keystore, publisher, pt).await?;

    println!(
        "Blacklist creating transaction digest: {}",
        transaction_response.digest
    );

    let tx_effects = transaction_response
        .effects
        .expect("Transaction has no effects");
    let blacklist_ref = tx_effects
        .created()
        .first()
        .map(|blacklist| blacklist.reference)
        .expect("There are no created objects");

    println!("Blacklist Ref: {blacklist_ref:?}");

    Ok(blacklist_ref)
}

/// Creates a test transaction from the abstract account.
pub async fn create_test_transaction(
    iota_client: &IotaClient,
    keystore: &mut InMemKeystore,
    publisher: IotaAddress,
    recipient: IotaAddress,
    account_ref: &ObjectRef,
    blacklist_ref: &ObjectRef,
) -> Result<Transaction> {
    let account_address = account_ref.object_id.into();

    // Create a PTB that sends some IOTA from the abstract account to the recipient
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();

        builder.transfer_iota(recipient, Some(10));
        builder.finish()
    };

    let tx_data = create_transaction_data(iota_client, account_address, pt).await?;

    let tx_digest = tx_data.digest();

    // Create a transaction
    let account_call_arg = CallArg::Shared(SharedObjectRef::new(
        account_ref.object_id,
        account_ref.version,
        false,
    ));
    let blacklist_call_arg = CallArg::Shared(SharedObjectRef::new(
        blacklist_ref.object_id,
        blacklist_ref.version,
        false,
    ));

    let raw_value: u64 = 42;
    let raw_value_arg = CallArg::Pure(bcs::to_bytes(&raw_value)?);

    let mut message = vec![];
    message.extend_from_slice(tx_digest.as_ref());
    message.extend_from_slice(&blacklist_ref.object_id.into_bytes());
    message.extend_from_slice(bcs::to_bytes(&raw_value)?.as_slice());
    let message_hash = Sha256::digest(message.as_slice()).digest;

    let hex_encoded_signature: String =
        Hex::encode(keystore.sign_hashed(&publisher, &message_hash)?)
            .chars()
            .skip(2) // flag prefix length
            .take(Ed25519Signature::LENGTH * 2)
            .collect();
    let signature_call_arg = CallArg::Pure(bcs::to_bytes(&hex_encoded_signature)?);

    let signature = GenericSignature::MoveAuthenticator(MoveAuthenticator::new_v1(
        vec![blacklist_call_arg, raw_value_arg, signature_call_arg],
        vec![],
        account_call_arg.clone(),
    ));

    Ok(Transaction::from_generic_sig_data(tx_data, vec![signature]))
}

/// Swaps the blacklist shared object in the transaction with a new one.
pub fn swap_blacklist_in_transaction(
    mut transaction: Transaction,
    new_blacklist_ref: &ObjectRef,
) -> Transaction {
    let new_blacklist_ref_call_arg = CallArg::Shared(SharedObjectRef::new(
        new_blacklist_ref.object_id,
        new_blacklist_ref.version,
        false,
    ));

    let new_sig = match &transaction.inner_mut().tx_signatures[0] {
        GenericSignature::MoveAuthenticator(move_authenticator) => {
            let raw_value_call_arg = move_authenticator.call_args()[1].clone();
            let signature_call_arg = move_authenticator.call_args()[2].clone();

            let account_call_arg = move_authenticator.object_to_authenticate().clone();

            GenericSignature::MoveAuthenticator(MoveAuthenticator::new_v1(
                vec![
                    new_blacklist_ref_call_arg,
                    raw_value_call_arg,
                    signature_call_arg,
                ],
                vec![],
                account_call_arg,
            ))
        }
        _ => panic!("Expected MoveAuthenticator signature"),
    };

    transaction.inner_mut().tx_signatures[0] = new_sig;

    transaction
}

/// Swaps the raw value in the transaction with a new one.
pub fn swap_raw_value_in_transaction(
    mut transaction: Transaction,
    new_raw_value: u64,
) -> Transaction {
    let new_raw_value_call_arg = CallArg::Pure(bcs::to_bytes(&new_raw_value).unwrap());

    let new_sig = match &transaction.inner_mut().tx_signatures[0] {
        GenericSignature::MoveAuthenticator(move_authenticator) => {
            let blacklist_call_arg = move_authenticator.call_args()[0].clone();
            let signature_call_arg = move_authenticator.call_args()[2].clone();

            let account_call_arg = move_authenticator.object_to_authenticate().clone();

            GenericSignature::MoveAuthenticator(MoveAuthenticator::new_v1(
                vec![
                    blacklist_call_arg,
                    new_raw_value_call_arg,
                    signature_call_arg,
                ],
                vec![],
                account_call_arg,
            ))
        }
        _ => panic!("Expected MoveAuthenticator signature"),
    };

    transaction.inner_mut().tx_signatures[0] = new_sig;

    transaction
}

/// Utility function to get the TypeTag of the abstract account struct.
fn aa_type_tag(package_id: &ObjectID) -> TypeTag {
    TypeTag::from_str(format!("{package_id}::{AA_MODULE_NAME}::{AA_ACCOUNT_NAME}").as_str())
        .unwrap()
}
