// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use anyhow::{Ok, Result};
use bip32::DerivationPath;
use iota_keys::keystore::{AccountKeystore, InMemKeystore};
use iota_sdk::{
    IotaClientBuilder,
    rpc_types::IotaTransactionBlockResponseOptions,
    types::{
        base_types::IotaAddress,
        crypto::SignatureScheme::ED25519,
        quorum_driver_types::ExecuteTransactionRequestType,
        signature::GenericSignature,
        transaction::{Transaction, TransactionData},
    },
};

use crate::{
    faucet::request_tokens,
    sig_utils::{build_multisig, build_multisig_pub_key, restore_signagure_bytes_to_generic},
    signed_tx::SignedTx,
    smart_account::{
        delete_smart_account, init_smart_account, make_deposit_to_smart_account,
        prepare_withdraw_tx_data, publish_account_abstraction_package, smart_account_data,
    },
    tx_flow::{propose_tx_to_smart_account, sign_proposed_tx},
    utils::{MAIN_MNEMONIC, THRESHOLD, WEIGHTS, check_recipient_balance, package_id},
};
mod faucet;
mod sig_utils;
mod signed_tx;
mod smart_account;
mod tx_flow;
mod utils;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let iota_client = IotaClientBuilder::default().build_localnet().await?;
    println!("Iota local network version: {}", iota_client.api_version());

    // Setup the temporary keystore
    let mut keystore = InMemKeystore::new_insecure_for_tests(0);

    // Setup actors addresses
    let alice_addr = keystore.import_from_mnemonic(
        MAIN_MNEMONIC,
        ED25519,
        Some(DerivationPath::from_str("m/44'/4218'/0'/0'/0'")?),
        None,
    )?;
    println!("Alice address: {alice_addr}");
    let bob_addr = keystore.import_from_mnemonic(
        MAIN_MNEMONIC,
        ED25519,
        Some(DerivationPath::from_str("m/44'/4218'/0'/0'/1'")?),
        None,
    )?;
    println!("Bob address: {bob_addr}");

    // Setup multisig address with Alice and Bob as signers
    let multisig_addr =
        (&build_multisig_pub_key(&keystore, &[alice_addr, bob_addr], WEIGHTS, THRESHOLD)?).into();
    println!("Multisig address: {multisig_addr}");

    // Hardcoded recipient address
    let coin_recipient_addr = IotaAddress::from_str(
        "0x7b4a34f6a011794f0ecbe5e5beb96102d3eef6122eb929b9f50a8d757bfbdd67",
    )?;

    // Request faucet coins for all parties involved
    request_tokens(&iota_client, multisig_addr).await?;
    request_tokens(&iota_client, alice_addr).await?;
    request_tokens(&iota_client, bob_addr).await?;

    // Publish the Account Abstraction (AA) package
    // For instance as a publisher we use Alice address.
    let aa_package = publish_account_abstraction_package(
        &iota_client,
        alice_addr,
        bob_addr,
        multisig_addr,
        &keystore,
    )
    .await?;
    println!("AA package publishing tx info: {:#?}", aa_package);

    let package_id = package_id(aa_package);

    // Initialize a new smart account object owned by the multisig
    let smart_account = init_smart_account(
        &iota_client,
        package_id,
        alice_addr,
        multisig_addr,
        &keystore,
    )
    .await?;
    println!("Smart account creation tx info: {smart_account}");

    // Extract the SmartAccount and OwnerCap object references
    let (smart_account_object, owner_cap_object) = smart_account_data(smart_account)?;
    println!("Smart Account ID: {}", smart_account_object.0);
    println!("OwnerCap ID: {}", owner_cap_object.0);

    // Deposit tokens to the smart account from Alice
    // This function includes two transactions:
    // one for depositing (transferring tokens to the smart account)
    // and one for receiving the transferred tokens.
    make_deposit_to_smart_account(
        &iota_client,
        &keystore,
        alice_addr,
        bob_addr,
        multisig_addr,
        package_id,
        smart_account_object,
    )
    .await?;

    // Prepare a withdrawal transaction from the smart account to an external
    // recipient
    let withdraw_amount = 1_000_005u128;
    let (withdraw_digest, withdraw_tx_data) = prepare_withdraw_tx_data(
        &iota_client,
        alice_addr,
        multisig_addr,
        package_id,
        smart_account_object,
        owner_cap_object,
        coin_recipient_addr,
        withdraw_amount as u64,
    )
    .await?;

    // Submit a ProposedTx to the chain, to be later multisigned
    let proposed_tx_object = propose_tx_to_smart_account(
        &iota_client,
        withdraw_digest,
        &withdraw_tx_data,
        &keystore,
        alice_addr,
        package_id,
        smart_account_object,
    )
    .await?;

    // Register the Signatures On-chain
    // Register Alice's signature on-chain
    sign_proposed_tx(
        &iota_client,
        proposed_tx_object.0,
        &withdraw_tx_data,
        &keystore,
        alice_addr,
        package_id,
        smart_account_object,
    )
    .await?;

    // Register Bob's signature on-chain and extract the resulting SignedTx
    let tx_response = sign_proposed_tx(
        &iota_client,
        proposed_tx_object.0,
        &withdraw_tx_data,
        &keystore,
        bob_addr,
        package_id,
        smart_account_object,
    )
    .await?;

    // Extract signed tx data
    let signed_tx = SignedTx::from_tx_response(&iota_client, tx_response).await?;
    println!("Signed Tx object: {:?}", signed_tx);

    // Reconstruct and validate the multisignature from the verified individual
    // signatures
    let withdraw_tx_data = bcs::from_bytes::<TransactionData>(&signed_tx.tx_bytes)?;

    let alice_signature = restore_signagure_bytes_to_generic(
        &keystore,
        alice_addr,
        &signed_tx.verified_signatures[0],
    )?;

    let sigs: Vec<GenericSignature> = vec![
        alice_signature.clone(),
        restore_signagure_bytes_to_generic(&keystore, bob_addr, &signed_tx.verified_signatures[1])?,
    ];

    let multisig = build_multisig(&keystore, &[alice_addr, bob_addr], WEIGHTS, THRESHOLD, sigs)?;

    // Execute the final withdrawal transaction using the both multisignature and
    // alice signature
    let withdraw_tx_response = iota_client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_generic_sig_data(withdraw_tx_data, vec![multisig, alice_signature]),
            IotaTransactionBlockResponseOptions::full_content(),
            ExecuteTransactionRequestType::WaitForLocalExecution,
        )
        .await?;

    print!("\n Withdraw signature execution info: {withdraw_tx_response}");

    // Verify recipient received expected amount
    assert!(
        check_recipient_balance(&iota_client, coin_recipient_addr, withdraw_amount).await?,
        "Recipient did not receive expected balance"
    );

    println!("Recipient has received - {withdraw_amount}");

    // Deletiton of Smart Account initiated by Bob and approved by Alice

    let delete_sm_tx_resp = delete_smart_account(
        &iota_client,
        package_id,
        multisig_addr,
        bob_addr,
        alice_addr,
        smart_account_object,
        owner_cap_object.0,
        &keystore,
    )
    .await?;

    print!("\n Delete Smart Contract Transaction: {delete_sm_tx_resp}");

    Ok(())
}
