// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Abstract Account tests
//!
//! The tests in this module are meant to test the creation of an abstracted
//! account and sending of a TX through it using a Move-based Ed25519
//! authentication.
//!
//! The tests make use of the `./tests/abstract_account/abstract_account` Move
//! package, which contains a basic implementation of an abstract account
//! inspired by the `examples/move/iotaccount` implementation. This is needed in
//! order to not depend on an external folder and to enable easier changes to
//! the Move code.
use std::{net::SocketAddr, str::FromStr};

use fastcrypto::{
    ed25519::Ed25519Signature,
    encoding::{Encoding, Hex},
    traits::Authenticator,
};
use iota_json_rpc_types::{
    DryRunTransactionBlockResponse, IotaTransactionBlockEffectsAPI, IotaTransactionBlockResponse,
};
use iota_keys::keystore::AccountKeystore;
use iota_macros::sim_test;
use iota_sdk_types::crypto::Intent;
use iota_test_transaction_builder::publish_package;
use iota_types::{
    IOTA_FRAMEWORK_ADDRESS, TypeTag,
    base_types::{IotaAddress, ObjectID, ObjectRef},
    crypto::{PublicKey, SignatureScheme},
    effects::{TransactionEffects, TransactionEffectsAPI},
    execution_status::{ExecutionFailureStatus, MoveLocation},
    messages_grpc::HandleCertificateRequestV1,
    move_authenticator::MoveAuthenticator,
    move_package,
    object::Owner,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    quorum_driver_types::QuorumDriverResponse,
    signature::GenericSignature,
    storage::WriteKind,
    transaction::{
        Argument, CallArg, ObjectArg, ProgrammableTransaction,
        TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, Transaction, TransactionData,
    },
};
use move_command_line_common::error_bitset::ErrorBitset;
use move_core_types::{ident_str, identifier::Identifier};
use test_cluster::{TestCluster, TestClusterBuilder};

const AA_PACKAGE_PATH: &str = "tests/abstract_account/abstract_account";
const AA_MODULE_NAME: &str = "abstract_account";
const AA_ACCOUNT_NAME: &str = "AbstractAccount";
const AA_DELAYED_MODULE_NAME: &str = "delayed_abstract_account";
const AA_DELAYED_ACCOUNT_NAME: &str = "DelayedAbstractAccount";
const AA_CREATE_MODULE_NAME: &str = "abstract_account_keyed";
const AA_AUTHENTICATE_MODULE_NAME: &str = "abstract_account_keyed";
const AA_DELAYED_CREATE_MODULE_NAME: &str = "delayed_abstract_account";
const AA_DELAYED_AUTHENTICATE_MODULE_NAME: &str = "delayed_abstract_account_keyed";
const AA_AUTHENTICATE_FN_NAME_ED25519: &str = "authenticate_ed25519";
const AA_AUTHENTICATE_FN_NAME_FREE_ACCESS: &str = "authenticate_free_access";
const AA_RECEIVE_OBJECT_FN_NAME: &str = "receive_object";
const AA_RECEIVE_OBJECT_FN_NAME_NO_SENDER_CHECK: &str = "receive_object_without_sender_check";

// ------------------------------
// --- Abstract Account tests ---
// ------------------------------

/// Test the creation of an Abstract Account and the issuance of a simple
/// transaction from it using the Move-based Ed25519 signature authenticator.
#[sim_test]
async fn test_abstract_account_creation_and_issue_tx() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    // Build a test environment and create an abstract account
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519)
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();

    // Retrieve the sender
    let aa_sender = aa_ref.0.into();

    // Request faucet coins for the AbstractAccount
    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let aa_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20000000000), aa_sender)
        .await;

    // Create a simple transaction from the IOTA account
    let pt = test_env.craft_aa_simple_ptb(AA_MODULE_NAME)?;
    let tx_data = test_env
        .craft_tx_from_pt(
            pt, aa_gas, aa_sender, None, // No sponsor
        )
        .await?;
    let tx_digest = tx_data.digest().into_inner();

    // Create the MoveAuthenticator for the Ed25519 signature authenticator
    let signatures = vec![test_env.create_move_authenticator_for_ed25519(&tx_digest)?];

    // Create the TX envelope and execute it
    let aa_simple_tx = Transaction::from_generic_sig_data(tx_data, signatures);
    test_env
        .execute_and_check_tx_correctness(aa_simple_tx)
        .await
}

/// Test the issuance of a sponsored transaction from an Abstract Account
/// using the free access authenticator. The sponsor is a regular IOTA account
/// that provides gas for the transaction.
#[sim_test]
async fn test_abstract_account_issues_sponsored_tx() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    // Build a test environment and create an abstract account
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_FREE_ACCESS)
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();

    // Retrieve the keystore and derive the address of the first account
    let keystore = test_env.test_cluster.wallet.config().keystore();
    let sponsor = keystore.addresses().first().cloned().unwrap();

    // Request faucet coins for the Sponsor
    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let sponsor_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20000000000), sponsor)
        .await;

    // Create a simple transaction from the IOTA account
    let pt = test_env.craft_aa_simple_ptb(AA_MODULE_NAME)?;
    let aa_sender = aa_ref.0.into();
    let tx_data = test_env
        .craft_tx_from_pt(pt, sponsor_gas, aa_sender, Some(sponsor))
        .await?;

    // Sponsor signature
    let sponsor_signature = GenericSignature::Signature(keystore.sign_secure(
        &sponsor,
        &tx_data,
        Intent::iota_transaction(),
    )?);

    // AA signature
    let aa_signature = test_env.create_move_authenticator_for_free_access()?;

    // Create the TX envelope and execute it
    let aa_sponsored_tx =
        Transaction::from_generic_sig_data(tx_data, vec![aa_signature, sponsor_signature]);
    test_env
        .execute_and_check_tx_correctness(aa_sponsored_tx)
        .await
}

/// Test predicting the id of an account using a dry run transaction.
#[sim_test]
async fn test_predict_abstract_account_id_dry_run() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    // Build a test environment and dry run the creation of an abstract account
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account_dry_run(AA_AUTHENTICATE_FN_NAME_FREE_ACCESS)
        .await?;

    // Create the AA account (after the dry run); it also checks that aa_ref_actual
    // is equal to aa_sender
    test_env.setup_abstract_account_after_dry_run().await?;

    Ok(())
}

/// Test the delayed creation of an Abstract Account and the issuance of a
/// simple transaction from it.
///
/// This test verifies that:
/// 1. A shared object can be created first (not yet an AA account)
/// 2. The shared object can later be converted into an actual AA account
/// 3. The AA account can then issue transactions normally
#[sim_test]
async fn test_abstract_account_delayed_creation() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    // Build a test environment and create a delayed abstract account object
    // (this creates a shared object that is NOT yet an AA account)
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_delayed_abstract_account_object(AA_AUTHENTICATE_FN_NAME_FREE_ACCESS)
        .await?;
    let delayed_aa_ref = test_env.aa_ref.unwrap();

    // Now convert the delayed object into an actual AA account
    let effects = test_env.make_delayed_abstract_account().await?;
    assert!(
        effects.status().is_ok(),
        "Expected make_delayed_abstract_account to succeed, got: {:?}",
        effects.status()
    );
    // The AA account address is the same as the delayed object ID
    let aa_sender: IotaAddress = delayed_aa_ref.0.into();

    // Fund the AA account with gas
    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let aa_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    // Create a simple transaction from the AA account
    let pt = test_env.craft_aa_simple_ptb(AA_DELAYED_MODULE_NAME)?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, aa_gas, aa_sender, None)
        .await?;

    // Create the MoveAuthenticator (free access - no signature needed)
    let aa_sig = test_env.create_move_authenticator_for_free_access()?;
    let tx = Transaction::from_generic_sig_data(tx_data, vec![aa_sig]);

    // Execute and verify the transaction succeeds
    test_env.execute_and_check_tx_correctness(tx).await
}

/// FAIL: receive in the main PT using
/// abstract_account::receive_object<T>(...).
#[sim_test]
async fn test_receive_object_in_main_tx_succeeds() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    // AA with free access (effect-free auth)
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_FREE_ACCESS)
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: IotaAddress = aa_ref.0.into();

    // Fund AA
    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let aa_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;
    let gas_to_send = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(10_000_000), aa_sender)
        .await;

    // Main PTB: actually receive the Gas into the AA
    let pt = test_env.craft_aa_receive_gas_ptb(
        gas_to_send,
        AA_MODULE_NAME,
        AA_RECEIVE_OBJECT_FN_NAME,
    )?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, aa_gas, aa_sender, None)
        .await?;

    // Authenticator: free-access (no object args)
    let aa_sig = test_env.create_move_authenticator_for_free_access()?;
    let tx = Transaction::from_generic_sig_data(tx_data, vec![aa_sig]);

    // Should fail
    let tx_result = test_env
        .test_cluster
        .wallet
        .execute_transaction_may_fail(tx)
        .await
        .unwrap()
        .effects
        .unwrap();

    // Assert received a MoveAbort error
    assert!(
        tx_result.status().is_err(),
        "Expected TX2 certificate creation to fail due to conflict on receiving object"
    );
    let error_string = format!("{:#?}", tx_result.status());
    assert!(
        error_string.contains("abort"),
        "Expected MoveAbort error, got: {}",
        error_string
    );

    Ok(())
}

/// Test in 3 steps the failure of an Abstract Account transaction
/// post-consensus:
/// 1) Create a TX certificate signed by the validators where the authentication
///    is successful
/// 2) Tamper with the AA shared object state by creating a second TX altering
///    the state by changing the public key that allows the authentication to
///    pass
/// 3) Submit the original certificate which should now fail during
///    post-consensus, even though validators originally run the authenticate
///    and it passed
#[sim_test]
async fn test_abstract_account_post_consensus_failure() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let client_ip = SocketAddr::new([127, 0, 0, 1].into(), 0);

    // Build a test environment and create an abstract account
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519)
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let rgp = test_env.test_cluster.get_reference_gas_price().await;

    // Retrieve the keystore and setup an account for rotating owner key
    let keystore = test_env.test_cluster.wallet.config_mut().keystore_mut();
    let new_aa_owner = keystore
        .generate_and_add_new_key(SignatureScheme::ED25519, None, None, None)
        .expect("ED25519 key generation should not fail")
        .0;
    assert!(new_aa_owner != test_env.owner.unwrap());
    let new_aa_owner_pk = test_env
        .test_cluster
        .wallet
        .config()
        .keystore()
        .get_key(&new_aa_owner)?
        .public();
    let aa_sender = aa_ref.0.into();

    // Step 1: create an AA TX and ask the validators to sign it
    // Create a simple transaction from the IOTA account
    let aa_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20000000000), aa_sender)
        .await;
    let pt = test_env.craft_aa_simple_ptb(AA_MODULE_NAME)?;
    let tx_data = test_env
        .craft_tx_from_pt(
            pt, aa_gas, aa_sender, None, // No sponsor
        )
        .await?;
    let tx_digest = tx_data.digest().into_inner();
    // Create the MoveAuthenticator for the Ed25519 signature authenticator
    let signatures = vec![test_env.create_move_authenticator_for_ed25519(&tx_digest)?];
    // Create the TX envelope and send it for validators signing
    let aa_simple_tx = Transaction::from_generic_sig_data(tx_data, signatures);
    let cert = test_env
        .test_cluster
        .create_certificate(aa_simple_tx, Some(client_ip))
        .await
        .unwrap();

    // Step 2: tamper with the certificate to make it invalid post-consensus; this
    // means creating a second transaction altering the AA shared object state
    let aa_gas2 = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20000000000), aa_sender)
        .await;
    let pt2 = test_env.craft_aa_rotate_owner_key_ptb(&new_aa_owner_pk)?;
    let tx_data2 = test_env
        .craft_tx_from_pt(
            pt2, aa_gas2, aa_sender, None, // No sponsor
        )
        .await?;
    let tx_digest2 = tx_data2.digest().into_inner();
    // Create the MoveAuthenticator for the Ed25519 signature authenticator
    let signatures2 = vec![test_env.create_move_authenticator_for_ed25519(&tx_digest2)?];
    // Create the TX envelope and send it for validators signing
    let aa_rotate_tx = Transaction::from_generic_sig_data(tx_data2, signatures2);
    // Should succeed
    test_env
        .execute_and_check_tx_correctness(aa_rotate_tx)
        .await?;
    // Update the test environment with the new owner (this is just for
    // completeness, not needed for this test)
    test_env.owner = Some(new_aa_owner);

    // Step 3: submit the original certificate which should now fail
    let QuorumDriverResponse { effects_cert, .. } = test_env
        .test_cluster
        .authority_aggregator()
        .process_certificate(
            HandleCertificateRequestV1::new(cert).with_events(),
            Some(client_ip),
        )
        .await
        .unwrap();
    let summary = effects_cert.summary_for_debug();

    assert!(summary.status.is_err(), "Expected the TX execution to fail");
    assert!(
        summary.gas_used.gas_used() == 3401600
            && summary.mutated_object_count == 2
            && summary.created_object_count == 0
            && summary.unwrapped_object_count == 0
            && summary.deleted_object_count == 0
            && summary.wrapped_object_count == 0,
        "Expected gas to be used in the failed transaction and that only the gas object was mutated and the TX input object was bumped in version",
    );

    assert!(
        matches!(
            summary.status.unwrap_err().0,
            ExecutionFailureStatus::MoveAbort(MoveLocation { module, function_name, .. }, abort_code)
            if module.name() == ident_str!("basic_keyed_aa")
            && function_name == Some("authenticate_ed25519".to_string())
            && ErrorBitset::from_u64(abort_code).unwrap().error_code() == Some(0)
        ),
        "Expected failure to be a Move abort in basic_keyed_aa::authenticate_ed25519",
    );

    Ok(())
}

/// Test in 3 steps
/// 1) Create a valid TX1 certificate signed by validators where sender is an AA
///    account using a owned Coin as gas
/// 2) Tamper with the AA shared object by creating a second TX2, with sender
///    being a random Bob address, altering the state calling the “receive“
///    function for the Coin used as gas in TX1
/// 3) Submit the original certificate TX1 which should NOT fail during
///    post-consensus, because validators originally run the authenticate and it
///    passed. What fails is the execution of TX2 because of the conflict on the
///   receiving object
#[sim_test]
async fn test_receiving_gas_executing_aa_tx_first() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let client_ip = SocketAddr::new([127, 0, 0, 1].into(), 0);

    // Build a test environment and create an abstract account
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_FREE_ACCESS)
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let rgp = test_env.test_cluster.get_reference_gas_price().await;

    // AA account address
    let aa_sender: IotaAddress = aa_ref.0.into();

    // Retrieve the keystore and setup secondary random account (Bob)
    let bob = {
        let keystore = test_env.test_cluster.wallet.config_mut().keystore_mut();
        keystore
            .generate_and_add_new_key(SignatureScheme::ED25519, None, None, None)
            .expect("ED25519 key generation should not fail")
            .0
    };
    assert!(bob != aa_sender);

    // Fund AA and Bob with gas; AA account's gas coin is the conflicting one
    let bob_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), bob)
        .await;
    let conflict_coin_ref = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    // Step 1: create TX1 where the sender is the AA using the owned "conflict" Coin
    // as gas
    let pt1 = test_env.craft_aa_simple_ptb(AA_MODULE_NAME)?;
    let tx1_data = test_env
        .craft_tx_from_pt(pt1, conflict_coin_ref, aa_sender, None)
        .await?;
    // Create the MoveAuthenticator for the free access authenticator
    let signatures = vec![test_env.create_move_authenticator_for_free_access()?];
    // Create the TX envelope and send it for validators signing
    let tx1 = Transaction::from_generic_sig_data(tx1_data, signatures);
    let tx1_cert = test_env
        .test_cluster
        .create_certificate(tx1, Some(client_ip))
        .await
        .expect("TX1 certificate creation should succeed");

    // Step 2: create TX2 where the sender is Bob, calling the receiving function on
    // the same "conflict" Coin used by TX1
    let pt2 = test_env.craft_aa_receive_gas_ptb(
        conflict_coin_ref,
        AA_MODULE_NAME,
        AA_RECEIVE_OBJECT_FN_NAME_NO_SENDER_CHECK,
    )?;
    let tx2_data = test_env.craft_tx_from_pt(pt2, bob_gas, bob, None).await?;
    // Create the TX envelope and send it for validators signing
    let tx2 = test_env.test_cluster.wallet.sign_transaction(&tx2_data);
    let tx2_cert = test_env
        .test_cluster
        .create_certificate(tx2, Some(client_ip))
        .await
        .expect("TX2 certificate creation should succeed");
    // Submit the TX2 certificate which should fail during execution because of
    // trying to receive an object owned by an AA account
    let QuorumDriverResponse { effects_cert, .. } = test_env
        .test_cluster
        .authority_aggregator()
        .process_certificate(
            HandleCertificateRequestV1::new(tx2_cert).with_events(),
            Some(client_ip),
        )
        .await
        .unwrap();
    assert!(
        effects_cert.summary_for_debug().status.is_err(),
        "Expected the TX execution to fail due to receiving an object owned by an AA account"
    );

    // Step 3: submit the original certificate TX1 which should NOT fail during the
    // execution
    let QuorumDriverResponse { effects_cert, .. } = test_env
        .test_cluster
        .authority_aggregator()
        .process_certificate(
            HandleCertificateRequestV1::new(tx1_cert).with_events(),
            Some(client_ip),
        )
        .await
        .unwrap();
    assert!(
        effects_cert.summary_for_debug().status.is_ok(),
        "Expected the TX execution to succeed"
    );

    Ok(())
}

/// Test in 4 steps:
/// 1) Create TX1 where Bob calls the receiving function on a coin owned by an
///    AA account.
/// 2) Create TX2 where the AA sender tries to use the conflict coin as input.
/// 3) Submit the original TX1 certificate. This fails with an execution abort.
/// 4) Submit the original TX2 certificate. This should now succeed.
#[sim_test]
async fn test_receiving_gas_executing_aa_tx_later() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let client_ip = SocketAddr::new([127, 0, 0, 1].into(), 0);

    // Build a test environment and create an abstract account
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_FREE_ACCESS)
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let rgp = test_env.test_cluster.get_reference_gas_price().await;

    // AA account address
    let aa_sender: IotaAddress = aa_ref.0.into();

    // Retrieve the keystore and setup secondary random account (Bob)
    let bob = {
        let keystore = test_env.test_cluster.wallet.config_mut().keystore_mut();
        keystore
            .generate_and_add_new_key(SignatureScheme::ED25519, None, None, None)
            .expect("ED25519 key generation should not fail")
            .0
    };
    assert!(bob != aa_sender);

    // Fund AA and Bob with gas; AA account's gas coin is the conflicting one
    let bob_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), bob)
        .await;
    let conflict_coin_ref = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;
    let second_gas_coin = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    // Step 1: create TX1 where the sender is Bob, calling the receiving function on
    // a coin owned by the AA account
    let pt1 = test_env.craft_aa_receive_gas_ptb(
        conflict_coin_ref,
        AA_MODULE_NAME,
        AA_RECEIVE_OBJECT_FN_NAME_NO_SENDER_CHECK,
    )?;
    let tx1_data = test_env.craft_tx_from_pt(pt1, bob_gas, bob, None).await?;
    // Create the TX envelope and send it for validators signing
    let tx1 = test_env.test_cluster.wallet.sign_transaction(&tx1_data);
    // This must NOT fail during signing
    let tx1_cert = test_env
        .test_cluster
        .create_certificate(tx1, Some(client_ip))
        .await;
    assert!(
        tx1_cert.is_ok(),
        "Expected TX1 certificate creation to success"
    );

    // Step 2: create a TX2 which uses the conflict Coin owned by the AA account as
    // input
    let pt2 = test_env.craft_object_transfer(conflict_coin_ref, IotaAddress::ZERO)?;
    let tx2_data = test_env
        .craft_tx_from_pt(pt2, second_gas_coin, aa_sender, None)
        .await?;
    // Create the MoveAuthenticator for the free access authenticator
    let signatures = vec![test_env.create_move_authenticator_for_free_access()?];
    // Create the TX envelope and send it for validators signing
    let tx2 = Transaction::from_generic_sig_data(tx2_data, signatures);
    let tx2_cert = test_env
        .test_cluster
        .create_certificate(tx2, Some(client_ip))
        .await;
    assert!(
        tx2_cert.is_ok(),
        "Expected TX2 certificate creation to succeed"
    );

    // Step 3: submit the original certificate TX1 which should fail
    let QuorumDriverResponse { effects_cert, .. } = test_env
        .test_cluster
        .authority_aggregator()
        .process_certificate(
            HandleCertificateRequestV1::new(tx1_cert.unwrap()).with_events(),
            Some(client_ip),
        )
        .await
        .unwrap();
    let summary = effects_cert.summary_for_debug();
    assert!(
        summary.status.is_err(),
        "Expected the TX1 execution to fail execution"
    );

    // Step 4: Submit the original certificate TX2 which should now succeed
    let QuorumDriverResponse { effects_cert, .. } = test_env
        .test_cluster
        .authority_aggregator()
        .process_certificate(
            HandleCertificateRequestV1::new(tx2_cert.unwrap()).with_events(),
            Some(client_ip),
        )
        .await
        .unwrap();
    let summary = effects_cert.summary_for_debug();
    assert!(
        summary.status.is_ok(),
        "Expected the TX2 execution to succeed"
    );

    Ok(())
}

/// Test in 5 steps:
/// 1) Create TX1 where Bob calls the receiving function on a coin owned by the
///    AA object (before the AA account is actually created). The AA object is
///    NOT an account yet (just a shared object).
/// 2) Make the AA become the actual account (delayed AA creation).
/// 3) Create TX2 where the AA sender tries to use the conflict coin as input.
/// 4) Submit the original TX1 certificate. This fails with an execution abort.
/// 5) Submit the original TX2 certificate. This should now succeed.
#[sim_test]
async fn test_failing_receiving_gas_then_create_account() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let client_ip = SocketAddr::new([127, 0, 0, 1].into(), 0);

    // Build a test environment and create a delayed abstract account object (still
    // not account)
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_delayed_abstract_account_object(AA_AUTHENTICATE_FN_NAME_FREE_ACCESS)
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let rgp = test_env.test_cluster.get_reference_gas_price().await;

    // AA account address
    let aa_sender: IotaAddress = aa_ref.0.into();

    // Retrieve the keystore and setup secondary random account (Bob)
    let bob = {
        let keystore = test_env.test_cluster.wallet.config_mut().keystore_mut();
        keystore
            .generate_and_add_new_key(SignatureScheme::ED25519, None, None, None)
            .expect("ED25519 key generation should not fail")
            .0
    };
    assert!(bob != aa_sender);

    // Fund AA and Bob with gas; AA account's gas coin is the conflicting one
    let bob_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), bob)
        .await;
    let conflict_coin_ref = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;
    let second_gas_coin = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    // Step 1: create TX1 where the sender is Bob, calling the receiving function on
    // a coin owned by the AA object
    let pt1 = test_env.craft_aa_receive_gas_ptb(
        conflict_coin_ref,
        AA_MODULE_NAME,
        AA_RECEIVE_OBJECT_FN_NAME_NO_SENDER_CHECK,
    )?;
    let tx1_data = test_env.craft_tx_from_pt(pt1, bob_gas, bob, None).await?;
    // Create the TX envelope and send it for validators signing
    let tx1 = test_env.test_cluster.wallet.sign_transaction(&tx1_data);
    // This must NOT fail during signing
    let tx1_cert = test_env
        .test_cluster
        .create_certificate(tx1, Some(client_ip))
        .await;
    assert!(
        tx1_cert.is_ok(),
        "Expected TX1 certificate creation to success"
    );

    // Step 2: create the AA account (from the delayed abstract account object)
    let effects = test_env.make_delayed_abstract_account().await?;
    assert!(
        effects.status().is_ok(),
        "Expected make_delayed_abstract_account to succeed, got: {:?}",
        effects.status()
    );

    // Step 3: create a TX2 which uses the conflict Coin owned by the AA as gas
    let pt2 = test_env.craft_object_transfer(conflict_coin_ref, IotaAddress::ZERO)?;
    let tx2_data = test_env
        .craft_tx_from_pt(pt2, second_gas_coin, aa_sender, None)
        .await?;
    // Create the MoveAuthenticator for the free access authenticator
    let signatures = vec![test_env.create_move_authenticator_for_free_access()?];
    // Create the TX envelope and send it for validators signing
    let tx2 = Transaction::from_generic_sig_data(tx2_data, signatures);
    let tx2_cert = test_env
        .test_cluster
        .create_certificate(tx2, Some(client_ip))
        .await;
    assert!(
        tx2_cert.is_ok(),
        "Expected TX2 certificate creation to succeed"
    );

    // Step 4: submit the original certificate TX1 which should fail
    let QuorumDriverResponse { effects_cert, .. } = test_env
        .test_cluster
        .authority_aggregator()
        .process_certificate(
            HandleCertificateRequestV1::new(tx1_cert.unwrap()).with_events(),
            Some(client_ip),
        )
        .await
        .unwrap();
    let summary = effects_cert.summary_for_debug();
    assert!(
        summary.status.is_err(),
        "Expected the TX1 execution to fail execution"
    );

    // Step 5: Submit the original certificate TX2 which should succeed
    let QuorumDriverResponse { effects_cert, .. } = test_env
        .test_cluster
        .authority_aggregator()
        .process_certificate(
            HandleCertificateRequestV1::new(tx2_cert.unwrap()).with_events(),
            Some(client_ip),
        )
        .await
        .unwrap();
    let summary = effects_cert.summary_for_debug();
    assert!(
        summary.status.is_ok(),
        "Expected the TX2 execution to succeed"
    );

    Ok(())
}

/// Test in 4 steps:
/// 1) Create TX1 where Bob calls the receiving function on a coin owned by the
///    AA object (before the AA account is actually created). The AA object is
///    NOT an account yet (just a shared object).
/// 2) Submit the original TX1 certificate. This is successful because the AA is
///    not yet an account.
/// 3) Make the AA become the actual account (delayed AA creation).
/// 4) Create and submit a TX2 where the AA sender tries to use the conflict
///    coin using the latest reference, this should now succeed.
#[sim_test]
async fn test_successful_receiving_gas_then_create_account() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let client_ip = SocketAddr::new([127, 0, 0, 1].into(), 0);

    // Build a test environment and create a delayed abstract account object (still
    // not account)
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_delayed_abstract_account_object(AA_AUTHENTICATE_FN_NAME_FREE_ACCESS)
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let rgp = test_env.test_cluster.get_reference_gas_price().await;

    // AA account address
    let aa_sender: IotaAddress = aa_ref.0.into();

    // Retrieve the keystore and setup secondary random account (Bob)
    let bob = {
        let keystore = test_env.test_cluster.wallet.config_mut().keystore_mut();
        keystore
            .generate_and_add_new_key(SignatureScheme::ED25519, None, None, None)
            .expect("ED25519 key generation should not fail")
            .0
    };
    assert!(bob != aa_sender);

    // Fund AA and Bob with gas; AA account's gas coin is the conflicting one
    let bob_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), bob)
        .await;
    let conflict_coin_ref = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;
    let second_gas_coin = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    // Step 1: create TX1 where the sender is Bob, calling the receiving function on
    // a coin owned by the AA object
    let pt1 = test_env.craft_aa_receive_gas_ptb(
        conflict_coin_ref,
        AA_DELAYED_MODULE_NAME,
        AA_RECEIVE_OBJECT_FN_NAME_NO_SENDER_CHECK,
    )?;
    let tx1_data = test_env.craft_tx_from_pt(pt1, bob_gas, bob, None).await?;
    // Create the TX envelope and send it for validators signing
    let tx1 = test_env.test_cluster.wallet.sign_transaction(&tx1_data);
    // This must NOT fail during signing
    let tx1_cert = test_env
        .test_cluster
        .create_certificate(tx1, Some(client_ip))
        .await;
    assert!(
        tx1_cert.is_ok(),
        "Expected TX1 certificate creation to success"
    );

    // Step 2: submit the original certificate TX1 which should succeed because the
    // AA object is not yet an account
    let QuorumDriverResponse { effects_cert, .. } = test_env
        .test_cluster
        .authority_aggregator()
        .process_certificate(
            HandleCertificateRequestV1::new(tx1_cert.unwrap()).with_events(),
            Some(client_ip),
        )
        .await
        .unwrap();
    let summary = effects_cert.summary_for_debug();
    assert!(
        summary.status.is_ok(),
        "Expected the TX1 execution to succeed"
    );
    let conflict_coin_ref = effects_cert
        .all_changed_objects()
        .iter()
        .find(|obj| obj.0.0 == conflict_coin_ref.0)
        .expect("Expected to find the updated conflict coin object")
        .0;

    // Step 3: create the AA account (from the delayed abstract account object)
    let effects = test_env.make_delayed_abstract_account().await?;
    assert!(
        effects.status().is_ok(),
        "Expected make_delayed_abstract_account to succeed, got: {:?}",
        effects.status()
    );

    // Step 4: create a TX2 which uses the conflict Coin owned by the AA as gas
    let pt2 = test_env.craft_object_transfer(conflict_coin_ref, IotaAddress::ZERO)?;
    let tx2_data = test_env
        .craft_tx_from_pt(pt2, second_gas_coin, aa_sender, None)
        .await?;
    // Create the MoveAuthenticator for the free access authenticator
    let signatures = vec![test_env.create_move_authenticator_for_free_access()?];
    // Create the TX envelope and send it for validators signing
    let tx2 = Transaction::from_generic_sig_data(tx2_data, signatures);
    // Submit TX2 for execution and expect success
    test_env.execute_and_check_tx_correctness(tx2).await
}

// ---------------------------------------------------
// --- Test Environment for Abstract Account tests ---
// ---------------------------------------------------

/// Test environment for Abstract Account tests
struct TestEnvironment {
    test_cluster: TestCluster,
    owner: Option<IotaAddress>,
    authenticate_fn_name: Option<String>,
    aa_package_id: Option<ObjectID>,
    aa_package_metadata_ref: Option<ObjectRef>,
    aa_ref: Option<ObjectRef>,
    aa_create_transaction: Option<Transaction>,
}

impl TestEnvironment {
    async fn new() -> Self {
        let test_cluster = TestClusterBuilder::new().build().await;

        Self {
            test_cluster,
            owner: None,
            authenticate_fn_name: None,
            aa_package_id: None,
            aa_package_metadata_ref: None,
            aa_ref: None,
            aa_create_transaction: None,
        }
    }

    // -----------------------------------------------
    // --- Setup methods -----------------------------
    // -----------------------------------------------

    /// Common initialization for AA tests:
    /// - store authenticate fn name
    /// - derive owner from keystore
    /// - publish AA package and store its ID
    async fn init_abstract_account_state(&mut self, authenticate_fn_name: &str) {
        // Store the authenticate function name
        self.authenticate_fn_name = Some(authenticate_fn_name.to_string());

        // Retrieve the keystore and derive the address of the first account
        self.owner = Some(
            self.test_cluster
                .wallet
                .config()
                .keystore()
                .addresses()
                .first()
                .cloned()
                .unwrap(),
        );

        // Publish the Move Account Abstraction package
        let (aa_package_id, aa_package_metadata_ref) =
            self.publish_account_abstraction_package().await;
        self.aa_package_id = Some(aa_package_id);
        self.aa_package_metadata_ref = Some(aa_package_metadata_ref);
    }

    /// Setup an Abstract Account that must be created successfully. This method
    /// is the one to be used for most tests.
    async fn setup_abstract_account(
        &mut self,
        authenticate_fn_name: &str,
    ) -> Result<(), anyhow::Error> {
        // Common initialization
        self.init_abstract_account_state(authenticate_fn_name).await;

        // Create an AbstractAccount (must succeed in this variant)
        let effects = self.create_abstract_account().await?;
        self.aa_ref = Some(abstract_account_from_all_changed_objects(
            &effects.all_changed_objects(),
        ));

        Ok(())
    }

    /// Setup an Abstract Account via dry run that must be created successfully.
    /// It updates the stored AA object reference and saves the transaction for
    /// later use, but it does not alter the ledger.
    async fn setup_abstract_account_dry_run(
        &mut self,
        authenticate_fn_name: &str,
    ) -> Result<(), anyhow::Error> {
        // Common initialization
        self.init_abstract_account_state(authenticate_fn_name).await;

        // Create an AbstractAccount (must succeed in this variant)
        let (dry_run_res, transaction) = self.create_abstract_account_dry_run().await?;
        self.aa_ref = Some(abstract_account_from_all_changed_objects(
            &dry_run_res
                .effects
                .all_changed_objects()
                .iter()
                .map(|e| (e.0.reference.to_object_ref(), e.0.owner, e.1))
                .collect::<Vec<(ObjectRef, Owner, WriteKind)>>(),
        ));
        self.aa_create_transaction = Some(transaction);

        Ok(())
    }

    /// Setup an Abstract Account after a dry run. This method uses the stored
    /// transaction from the dry run to actually create the AA on the ledger,
    /// and checks that the created AA object reference matches the one from
    /// the dry run. See `setup_abstract_account_dry_run`.
    async fn setup_abstract_account_after_dry_run(&mut self) -> Result<(), anyhow::Error> {
        if self.aa_create_transaction.is_none() {
            anyhow::bail!("No AA create transaction stored from dry run");
        };

        // Create an AbstractAccount (must succeed in this variant)
        let effects = self.create_abstract_account().await?;
        let actual_aa_ref =
            abstract_account_from_all_changed_objects(&effects.all_changed_objects());

        assert!(
            actual_aa_ref == self.aa_ref.unwrap(),
            "AA object ref from actual creation does not match the one from dry run"
        );

        Ok(())
    }

    /// Setup a delayed Abstract Account that must be created successfully. This
    /// method first creates the delayed AA object, which is still not an
    /// account. The actual creation of the AA account must be done later by
    /// calling `make_delayed_abstract_account`.
    async fn setup_delayed_abstract_account_object(
        &mut self,
        authenticate_fn_name: &str,
    ) -> Result<(), anyhow::Error> {
        // Common initialization
        self.init_abstract_account_state(authenticate_fn_name).await;

        // Create an AbstractAccount (must succeed in this variant)
        let effects = self.create_delayed_abstract_account_object().await?;
        self.aa_ref = Some(abstract_account_from_all_changed_objects(
            &effects.all_changed_objects(),
        ));

        Ok(())
    }

    // -----------------------------------------------
    // --- Create/Publish Account methods ------------
    // -----------------------------------------------

    /// Publish the Account Abstraction Move package and return its ID and
    /// metadata object reference.
    async fn publish_account_abstraction_package(&mut self) -> (ObjectID, ObjectRef) {
        let path = [env!("CARGO_MANIFEST_DIR"), AA_PACKAGE_PATH]
            .iter()
            .collect();
        let aa_package_id = publish_package(self.test_cluster.wallet(), path).await.0;

        let aa_package_metadata_id = move_package::derive_package_metadata_id(aa_package_id);
        let aa_package_metadata_ref = self
            .test_cluster
            .get_latest_object_ref(&aa_package_metadata_id)
            .await;

        (aa_package_id, aa_package_metadata_ref)
    }

    /// Main method to create an Abstract Account on the ledger. Can be invoked
    /// for a normal account setup or after a dry run.
    async fn create_abstract_account(&self) -> anyhow::Result<TransactionEffects> {
        let (
            Some(owner),
            Some(authenticate_fn_name),
            Some(aa_package_id),
            Some(aa_package_metadata_ref),
        ) = (
            self.owner,
            &self.authenticate_fn_name,
            self.aa_package_id,
            self.aa_package_metadata_ref,
        )
        else {
            anyhow::bail!("Owner or authenticate function name or package id not set");
        };

        let transaction = if let Some(transaction) = &self.aa_create_transaction {
            transaction.clone()
        } else {
            self.craft_create_abstract_account(
                owner,
                authenticate_fn_name,
                aa_package_id,
                aa_package_metadata_ref,
            )
            .await?
        };

        let (effects, _) = self
            .test_cluster
            .execute_transaction_return_raw_effects(transaction)
            .await?;

        Ok(effects)
    }

    /// Create the delayed abstract account object, which is not yet an account.
    async fn create_delayed_abstract_account_object(&self) -> anyhow::Result<TransactionEffects> {
        let Some(aa_package_id) = self.aa_package_id else {
            anyhow::bail!("Owner or authenticate function name or package id not set");
        };

        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();

            // Create the delayed abstract account object.
            builder.programmable_move_call(
                aa_package_id,
                ident_str!(AA_DELAYED_CREATE_MODULE_NAME).to_owned(),
                ident_str!("create").to_owned(),
                vec![],
                vec![],
            );

            builder.finish()
        };

        let tx_data = self
            .test_cluster
            .test_transaction_builder()
            .await
            .programmable(pt)
            .build();

        let transaction = self.test_cluster.wallet.sign_transaction(&tx_data);
        let (effects, _) = self
            .test_cluster
            .execute_transaction_return_raw_effects(transaction)
            .await?;

        Ok(effects)
    }

    /// Make the delayed abstract account object become an actual Abstract
    /// Account on the ledger. To be invoked after
    /// `create_delayed_abstract_account_object`.
    async fn make_delayed_abstract_account(&self) -> anyhow::Result<TransactionEffects> {
        let (
            Some(delayed_aa_ref),
            Some(owner),
            Some(authenticate_fn_name),
            Some(aa_package_id),
            Some(aa_package_metadata_ref),
        ) = (
            self.aa_ref,
            self.owner,
            &self.authenticate_fn_name,
            self.aa_package_id,
            self.aa_package_metadata_ref,
        )
        else {
            anyhow::bail!("Owner or authenticate function name or package id not set");
        };

        let aa_owner_pk = self
            .test_cluster
            .wallet
            .config()
            .keystore()
            .get_key(&owner)?
            .public();

        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();

            // create auth function ref
            let arguments = vec![
                builder.obj(ObjectArg::ImmOrOwnedObject(aa_package_metadata_ref))?,
                builder.pure(AA_DELAYED_AUTHENTICATE_MODULE_NAME)?,
                builder.pure(authenticate_fn_name)?,
            ];
            if let Argument::Result(authenticator_function_ref_v1) = builder.programmable_move_call(
                IOTA_FRAMEWORK_ADDRESS.into(),
                ident_str!("authenticator_function").to_owned(),
                ident_str!("create_auth_function_ref_v1").to_owned(),
                vec![delayed_abstract_account_type_tag(&aa_package_id)],
                arguments,
            ) {
                // Create the delayed abstract account.
                let arguments = vec![
                    builder.obj(ObjectArg::SharedObject {
                        id: delayed_aa_ref.0,
                        initial_shared_version: delayed_aa_ref.1,
                        mutable: true,
                    })?,
                    builder.pure(aa_owner_pk.as_ref())?,
                    Argument::Result(authenticator_function_ref_v1),
                ];
                builder.programmable_move_call(
                    aa_package_id,
                    ident_str!(AA_DELAYED_AUTHENTICATE_MODULE_NAME).to_owned(),
                    ident_str!("create").to_owned(),
                    vec![],
                    arguments,
                );
            }
            builder.finish()
        };

        let tx_data = self
            .test_cluster
            .test_transaction_builder_with_sender(owner)
            .await
            .programmable(pt)
            .build();

        let transaction = self.test_cluster.wallet.sign_transaction(&tx_data);
        let (effects, _) = self
            .test_cluster
            .execute_transaction_return_raw_effects(transaction)
            .await?;

        Ok(effects)
    }

    /// This method only performs a dry run of the Abstract Account creation,
    /// it does not alter the ledger.
    async fn create_abstract_account_dry_run(
        &self,
    ) -> anyhow::Result<(DryRunTransactionBlockResponse, Transaction)> {
        let (
            Some(owner),
            Some(authenticate_fn_name),
            Some(aa_package_id),
            Some(aa_package_metadata_ref),
        ) = (
            self.owner,
            &self.authenticate_fn_name,
            self.aa_package_id,
            self.aa_package_metadata_ref,
        )
        else {
            anyhow::bail!("Owner or authenticate function name or package id not set");
        };

        let transaction = self
            .craft_create_abstract_account(
                owner,
                authenticate_fn_name,
                aa_package_id,
                aa_package_metadata_ref,
            )
            .await?;

        let dry_run_res = self
            .test_cluster
            .iota_client()
            .read_api()
            .dry_run_transaction_block(transaction.transaction_data().clone())
            .await?;

        Ok((dry_run_res, transaction))
    }

    // -----------------------------------------------
    // --- Authenticators methods --------------------
    // -----------------------------------------------

    // Create the MoveAuthenticator for the Ed25519 signature authenticator:
    // public fun authenticate_ed25519(
    //    self: &AbstractAccount,
    //    signature: vector<u8>,
    //    _: &AuthContext,
    //    ctx: &TxContext,
    fn create_move_authenticator_for_ed25519(
        &self,
        tx_digest: &[u8; 32],
    ) -> anyhow::Result<GenericSignature> {
        let (Some(owner), Some(aa_ref)) = (self.owner, self.aa_ref) else {
            anyhow::bail!("Abstract account not created yet");
        };
        let self_call_arg = CallArg::Object(ObjectArg::SharedObject {
            id: aa_ref.0,
            initial_shared_version: aa_ref.1,
            mutable: false,
        });
        // Sign the tx data with the owner key
        let hex_encoded_signature: String = Hex::encode(
            self.test_cluster
                .wallet
                .config()
                .keystore()
                .sign_hashed(&owner, tx_digest)?,
        )
        .chars()
        .skip(2) // flag prefix length
        .take(Ed25519Signature::LENGTH * 2)
        .collect();
        let signature_call_arg = CallArg::Pure(bcs::to_bytes(&hex_encoded_signature)?);
        Ok(GenericSignature::MoveAuthenticator(
            MoveAuthenticator::new_v1(vec![signature_call_arg], vec![], self_call_arg),
        ))
    }

    // Create the MoveAuthenticator for the free access authenticator:
    // public fun authenticate_free_access(
    //    self: &AbstractAccount,
    //    _: &AuthContext,
    //    ctx: &TxContext,
    fn create_move_authenticator_for_free_access(&self) -> anyhow::Result<GenericSignature> {
        let Some(aa_ref) = self.aa_ref else {
            anyhow::bail!("Abstract account not created yet");
        };

        let self_call_arg = CallArg::Object(ObjectArg::SharedObject {
            id: aa_ref.0,
            initial_shared_version: aa_ref.1,
            mutable: false,
        });
        Ok(GenericSignature::MoveAuthenticator(
            MoveAuthenticator::new_v1(vec![], vec![], self_call_arg),
        ))
    }

    // -----------------------------------------------
    // --- PTB crafting methods ----------------------
    // -----------------------------------------------

    fn craft_aa_simple_ptb(&self, module_name: &str) -> anyhow::Result<ProgrammableTransaction> {
        let (Some(aa_ref), Some(aa_package_id)) = (self.aa_ref, self.aa_package_id) else {
            anyhow::bail!("Abstract account not created yet");
        };
        let mut builder = ProgrammableTransactionBuilder::new();

        // Random IOTA account command.
        let arguments = vec![
            builder.obj(ObjectArg::SharedObject {
                id: aa_ref.0,
                initial_shared_version: aa_ref.1,
                mutable: true,
            })?,
            builder.pure(1_u8)?,
            builder.pure(2_u8)?,
        ];
        builder.programmable_move_call(
            aa_package_id,
            Identifier::new(module_name)?,
            ident_str!("add_field").to_owned(),
            vec![TypeTag::U8, TypeTag::U8],
            arguments,
        );
        Ok(builder.finish())
    }

    fn craft_object_transfer(
        &self,
        object_ref: ObjectRef,
        recipient: IotaAddress,
    ) -> anyhow::Result<ProgrammableTransaction> {
        let mut builder = ProgrammableTransactionBuilder::new();

        // Transfer command.
        builder.transfer_object(recipient, object_ref)?;
        Ok(builder.finish())
    }

    fn craft_aa_rotate_owner_key_ptb(
        &mut self,
        new_aa_owner_pk: &PublicKey,
    ) -> anyhow::Result<ProgrammableTransaction> {
        let (
            Some(aa_ref),
            Some(aa_package_id),
            Some(aa_package_metadata_ref),
            Some(authenticate_fn_name),
        ) = (
            self.aa_ref,
            self.aa_package_id,
            self.aa_package_metadata_ref,
            &self.authenticate_fn_name,
        )
        else {
            anyhow::bail!("Abstract account not created yet");
        };
        assert!(
            authenticate_fn_name == AA_AUTHENTICATE_FN_NAME_ED25519,
            "Key rotation is only supported for Ed25519 authentication"
        );

        let mut builder = ProgrammableTransactionBuilder::new();

        // create auth function ref
        let arguments = vec![
            builder.obj(ObjectArg::ImmOrOwnedObject(aa_package_metadata_ref))?,
            builder.pure(AA_AUTHENTICATE_MODULE_NAME)?,
            builder.pure(authenticate_fn_name)?,
        ];
        if let Argument::Result(authenticator_function_ref_v1) = builder.programmable_move_call(
            IOTA_FRAMEWORK_ADDRESS.into(),
            ident_str!("authenticator_function").to_owned(),
            ident_str!("create_auth_function_ref_v1").to_owned(),
            vec![abstract_account_type_tag(&aa_package_id)],
            arguments,
        ) {
            // rotate the key in the abstract account.
            let arguments = vec![
                builder.obj(ObjectArg::SharedObject {
                    id: aa_ref.0,
                    initial_shared_version: aa_ref.1,
                    mutable: true,
                })?,
                builder.pure(new_aa_owner_pk.as_ref())?,
                Argument::Result(authenticator_function_ref_v1),
            ];
            builder.programmable_move_call(
                aa_package_id,
                ident_str!(AA_CREATE_MODULE_NAME).to_owned(),
                ident_str!("rotate_public_key").to_owned(),
                vec![],
                arguments,
            );
        }
        Ok(builder.finish())
    }

    // Utilities

    async fn craft_tx_from_pt(
        &self,
        pt: ProgrammableTransaction,
        gas_coin: ObjectRef,
        sender: IotaAddress,
        sponsor: Option<IotaAddress>,
    ) -> anyhow::Result<TransactionData> {
        let gas_price = self.test_cluster.get_reference_gas_price().await;

        // Create the transaction data that will be sent to the network
        Ok(TransactionData::new_programmable_allow_sponsor(
            sender,
            vec![gas_coin],
            pt,
            gas_price * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
            gas_price,
            sponsor.unwrap_or(sender),
        ))
    }

    async fn craft_create_abstract_account(
        &self,
        owner: IotaAddress,
        authenticate_fn_name: &str,
        aa_package_id: ObjectID,
        aa_package_metadata_ref: ObjectRef,
    ) -> anyhow::Result<Transaction> {
        let aa_owner_pk = self
            .test_cluster
            .wallet
            .config()
            .keystore()
            .get_key(&owner)?
            .public();

        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();

            // create auth function ref
            let arguments = vec![
                builder.obj(ObjectArg::ImmOrOwnedObject(aa_package_metadata_ref))?,
                builder.pure(AA_AUTHENTICATE_MODULE_NAME)?,
                builder.pure(authenticate_fn_name)?,
            ];
            if let Argument::Result(authenticator_function_ref_v1) = builder.programmable_move_call(
                IOTA_FRAMEWORK_ADDRESS.into(),
                ident_str!("authenticator_function").to_owned(),
                ident_str!("create_auth_function_ref_v1").to_owned(),
                vec![abstract_account_type_tag(&aa_package_id)],
                arguments,
            ) {
                // Create the abstract account.
                let arguments = vec![
                    builder.pure(aa_owner_pk.as_ref())?,
                    Argument::Result(authenticator_function_ref_v1),
                ];
                builder.programmable_move_call(
                    aa_package_id,
                    ident_str!(AA_CREATE_MODULE_NAME).to_owned(),
                    ident_str!("create").to_owned(),
                    vec![],
                    arguments,
                );
            }
            builder.finish()
        };

        let tx_data = self
            .test_cluster
            .test_transaction_builder()
            .await
            .programmable(pt)
            .build();

        let transaction = self.test_cluster.wallet.sign_transaction(&tx_data);

        Ok(transaction)
    }

    /// PTB to receive the Gas in the main transaction:
    /// abstract_account::receive_object<Coin<IOTA>>(&mut account,
    /// Receiving<Gas>, ctx)
    fn craft_aa_receive_gas_ptb(
        &self,
        gas_ref: ObjectRef,
        module_name: &str,
        receive_fn_name: &str,
    ) -> anyhow::Result<ProgrammableTransaction> {
        let (Some(aa_ref), Some(aa_package_id)) = (self.aa_ref, self.aa_package_id) else {
            anyhow::bail!("Abstract account not created yet");
        };
        let mut b = ProgrammableTransactionBuilder::new();

        let args = vec![
            b.obj(ObjectArg::SharedObject {
                id: aa_ref.0,
                initial_shared_version: aa_ref.1,
                mutable: true,
            })?,
            // IMPORTANT: passing an object ref *in the position of* `Receiving<T>`
            // yields a Receiving PTB arg (SDK converts when building the call).
            b.obj(ObjectArg::Receiving(gas_ref))?,
        ];
        b.programmable_move_call(
            aa_package_id,
            Identifier::new(module_name)?, // abstract_account
            Identifier::new(receive_fn_name)?,
            vec![],
            args,
        );
        Ok(b.finish())
    }

    // -----------------------------------------------
    // --- Utilities ---------------------------------
    // -----------------------------------------------

    async fn execute_and_check_tx_correctness(&self, tx: Transaction) -> anyhow::Result<()> {
        let transaction_response = self.test_cluster.execute_transaction(tx).await;

        // Check correctness
        let IotaTransactionBlockResponse {
            confirmed_local_execution,
            errors,
            ..
        } = transaction_response;

        // The transaction must be successful
        assert!(confirmed_local_execution.unwrap());
        assert!(errors.is_empty());
        Ok(())
    }
}

// ---------------------------------------------------
// --- Utilities -------------------------------------
// ---------------------------------------------------

fn abstract_account_type_tag(aa_package_id: &ObjectID) -> TypeTag {
    TypeTag::from_str(format!("{aa_package_id}::{AA_MODULE_NAME}::{AA_ACCOUNT_NAME}").as_str())
        .unwrap()
}

fn delayed_abstract_account_type_tag(aa_package_id: &ObjectID) -> TypeTag {
    TypeTag::from_str(
        format!("{aa_package_id}::{AA_DELAYED_MODULE_NAME}::{AA_DELAYED_ACCOUNT_NAME}").as_str(),
    )
    .unwrap()
}

fn abstract_account_from_all_changed_objects(
    all_changed_objects: &[(ObjectRef, Owner, WriteKind)],
) -> ObjectRef {
    // Extract the only created shared object which is the abstract account
    all_changed_objects
        .iter()
        .find_map(|change| match change {
            (_, Owner::Shared { .. }, WriteKind::Create) => Some(change.0),
            _ => None,
        })
        .expect("Expected a shared object in the transaction response")
}
