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
use iota_config::transaction_deny_config::TransactionDenyConfigBuilder;
use iota_core::authority_client::validator::ValidatorAPI;
use iota_json_rpc_types::{DryRunTransactionBlockResponse, IotaTransactionBlockEffectsAPI};
use iota_keys::keystore::AccountKeystore;
use iota_macros::sim_test;
use iota_protocol_config::{PerObjectCongestionControlMode, ProtocolConfig};
use iota_sdk_types::{
    Address, Argument, ExecutionError, Identifier, MoveAuthenticatorV1, MoveLocation, ObjectId,
    ObjectReference, OwnedObjectReference, Owner, ProgrammableTransaction, SharedObjectReference,
    SignatureScheme, Transaction, TransactionEffects, TypeTag, UserSignature, WriteKind,
    crypto::{Intent, SimpleSignature},
};
use iota_test_transaction_builder::publish_package;
use iota_types::{
    IOTA_FRAMEWORK_PACKAGE_ID,
    base_types::AuthorityName,
    crypto::PublicKey,
    effects::{TransactionEffectsAPI, TransactionEffectsExt},
    error::{IotaError, UserInputError},
    messages_grpc::{HandleCertificateRequestV1, HandleTransactionResponse},
    move_package,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    quorum_driver_types::QuorumDriverResponse,
    transaction::{
        CallArg, TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionAPI,
        TransactionEnvelope,
    },
};
use move_command_line_common::error_bitset::ErrorBitset;
use test_cluster::{TestCluster, TestClusterBuilder, override_pcool_flow};

const AA_PACKAGE_PATH: &str = "tests/abstract_account/abstract_account";
const AA_MODULE_NAME: &str = "abstract_account";
const AA_ACCOUNT_NAME: &str = "AbstractAccount";
const AA_DELAYED_MODULE_NAME: &str = "delayed_abstract_account";
const AA_DELAYED_ACCOUNT_NAME: &str = "DelayedAbstractAccount";
const AA_CREATE_MODULE_NAME: &str = "abstract_account_keyed";
const AA_CREATE_FN_NAME: &str = "create";
const AA_AUTHENTICATE_MODULE_NAME: &str = "abstract_account_keyed";
const AA_DELAYED_CREATE_MODULE_NAME: &str = "delayed_abstract_account";
const AA_DELAYED_AUTHENTICATE_MODULE_NAME: &str = "delayed_abstract_account_keyed";
const AA_AUTHENTICATE_FN_NAME_ED25519: &str = "authenticate_ed25519";
const AA_AUTHENTICATE_FN_NAME_FREE_ACCESS: &str = "authenticate_free_access";
const AA_AUTHENTICATE_FN_NAME_WITH_SPONSOR_AND_SENDER: &str =
    "authenticate_with_sponsor_and_sender";
const AA_AUTHENTICATE_FN_NAME_ED25519_VIA_SIGNING_DIGEST: &str =
    "authenticate_ed25519_via_signing_digest";
const AA_RECEIVE_OBJECT_FN_NAME: &str = "receive_object";
const AA_RECEIVE_OBJECT_FN_NAME_NO_SENDER_CHECK: &str = "receive_object_without_sender_check";

// ------------------------------
// --- Abstract Account tests ---
// ------------------------------

/// Test the creation of an Abstract Account and the issuance of a simple
/// transaction from it using the Move-based Ed25519 signature authenticator.
#[sim_test]
async fn test_abstract_account_creation_and_issue_tx_pre_consensus_flow()
-> Result<(), anyhow::Error> {
    test_abstract_account_creation_and_issue_tx(false).await
}

#[sim_test]
async fn test_abstract_account_creation_and_issue_tx_pcool_flow() -> Result<(), anyhow::Error> {
    test_abstract_account_creation_and_issue_tx(true).await
}

async fn test_abstract_account_creation_and_issue_tx(pcool: bool) -> Result<(), anyhow::Error> {
    let _pcool_guard = override_pcool_flow(pcool);
    telemetry_subscribers::init_for_testing();

    // Build a test environment and create an abstract account
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519)
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();

    // Retrieve the sender
    let aa_sender = aa_ref.object_id.into();

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
    let tx_digest = tx_data.digest().into_bytes();

    // Create the MoveAuthenticator for the Ed25519 signature authenticator
    let signatures = vec![test_env.create_move_authenticator_for_ed25519(&tx_digest)?];

    // Create the TX envelope and execute it
    let aa_simple_tx = TransactionEnvelope::from_user_sig_data(tx_data, signatures);
    test_env
        .execute_and_check_tx_correctness(aa_simple_tx)
        .await
}

/// Test that the AuthContext byte fields are correctly populated during
/// authentication and that an ed25519 signature can be verified against
/// `signing_digest`.
///
/// The Move authenticator (`authenticate_ed25519_via_signing_digest`) asserts:
/// 1. `signing_digest` == `blake2b256(intent_tx_data_bytes)` and is 32 bytes
/// 2. ed25519 signature over `signing_digest` is valid
#[sim_test]
async fn test_auth_context_tx_bytes_and_signature() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    // Build a test environment and create an abstract account with the new
    // authenticator
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519_VIA_SIGNING_DIGEST)
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let aa_sender = aa_ref.object_id.into();

    // Fund the AbstractAccount with gas
    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let aa_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20000000000), aa_sender)
        .await;

    // Create a transaction from the AA
    let pt = test_env.craft_aa_simple_ptb(AA_MODULE_NAME)?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, aa_gas, aa_sender, None)
        .await?;

    // sign_secure signs blake2b256(intent || bcs(Transaction)), which is
    // exactly what auth_ctx.signing_digest() returns on the Move side.
    let signatures =
        vec![test_env.create_move_authenticator_for_ed25519_via_signing_digest(&tx_data)?];

    // Execute — the Move authenticator asserts all structural invariants
    // and verifies the ed25519 signature against signing_digest.
    let tx = TransactionEnvelope::from_user_sig_data(tx_data, signatures);
    test_env.execute_and_check_tx_correctness(tx).await
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
    let aa_sender = aa_ref.object_id.into();
    let tx_data = test_env
        .craft_tx_from_pt(pt, sponsor_gas, aa_sender, Some(sponsor))
        .await?;

    // Sponsor signature
    let sponsor_signature = UserSignature::Simple(keystore.sign_secure(
        &sponsor,
        &tx_data,
        Intent::iota_transaction(),
    )?);

    // AA signature
    let aa_signature = test_env.create_move_authenticator_for_free_access()?;

    // Create the TX envelope and execute it
    let aa_sponsored_tx =
        TransactionEnvelope::from_user_sig_data(tx_data, vec![aa_signature, sponsor_signature]);
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
        effects.status().is_success(),
        "Expected make_delayed_abstract_account to succeed, got: {:?}",
        effects.status()
    );
    // The AA account address is the same as the delayed object ID
    let aa_sender: Address = delayed_aa_ref.object_id.into();

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
    let tx = TransactionEnvelope::from_user_sig_data(tx_data, vec![aa_sig]);

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
    let aa_sender: Address = aa_ref.object_id.into();

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
    let tx = TransactionEnvelope::from_user_sig_data(tx_data, vec![aa_sig]);

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
        "Expected MoveAbort error, got: {error_string}"
    );

    Ok(())
}

/// Test in 3 steps the failure of an Abstract Account transaction
/// post-consensus:
/// 1) Create a TX certificate signed by the validators where the authentication is successful
/// 2) Tamper with the AA shared object state by creating a second TX altering the state by changing
///    the public key that allows the authentication to pass
/// 3) Submit the original certificate which should now fail during post-consensus, even though
///    validators originally run the authenticate and it passed
///
/// With `report_move_authentication_error` enabled (the latest protocol config)
/// the failure surfaces as a distinct `MoveAuthentication` error that wraps the
/// underlying abort and carries no command index.
#[sim_test]
async fn test_abstract_account_post_consensus_failure() -> Result<(), anyhow::Error> {
    let _pcool_guard = override_pcool_flow(false);
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
        .generate_and_add_new_key(SignatureScheme::Ed25519, None, None, None)
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
    let aa_sender = aa_ref.object_id.into();

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
    let tx_digest = tx_data.digest().into_bytes();
    // Create the MoveAuthenticator for the Ed25519 signature authenticator
    let signatures = vec![test_env.create_move_authenticator_for_ed25519(&tx_digest)?];
    // Create the TX envelope and send it for validators signing
    let aa_simple_tx = TransactionEnvelope::from_user_sig_data(tx_data, signatures);
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
    let tx_digest2 = tx_data2.digest().into_bytes();
    // Create the MoveAuthenticator for the Ed25519 signature authenticator
    let signatures2 = vec![test_env.create_move_authenticator_for_ed25519(&tx_digest2)?];
    // Create the TX envelope and send it for validators signing
    let aa_rotate_tx = TransactionEnvelope::from_user_sig_data(tx_data2, signatures2);
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

    assert!(
        summary.status.is_failure(),
        "Expected the TX execution to fail"
    );
    assert!(
        summary.gas_cost_summary.gas_used() == 3401600
            && summary.mutated_object_count == 2
            && summary.created_object_count == 0
            && summary.unwrapped_object_count == 0
            && summary.deleted_object_count == 0
            && summary.wrapped_object_count == 0,
        "Expected gas to be used in the failed transaction and that only the gas object was mutated and the TX input object was bumped in version",
    );

    let (error, command) = summary.status.unwrap_err();
    // The authenticator abort is reported as an authentication failure, not as
    // a command of the programmable transaction, so it carries no command index.
    assert!(
        command.is_none(),
        "Expected the authentication failure to carry no command index",
    );
    let ExecutionError::MoveAuthentication { error } = &error else {
        panic!("Expected a MoveAuthentication, got: {error:?}");
    };
    assert!(
        matches!(
            &**error,
            ExecutionError::MoveAbort{location: MoveLocation { module, function_name, .. }, code: abort_code}
            if module.as_str() == "basic_keyed_aa"
            && function_name.as_ref().is_some_and(|f|f.as_str() == "authenticate_ed25519")
            && ErrorBitset::from_u64(*abort_code).unwrap().error_code() == Some(0)
        ),
        "Expected failure to be a MoveAuthentication wrapping a Move abort in basic_keyed_aa::authenticate_ed25519",
    );

    Ok(())
}

/// Test in 3 steps the failure of an Abstract Account transaction
/// post-consensus:
/// 1) Create a TX certificate signed by the validators where the authentication is successful
/// 2) Tamper with the AA shared object state by creating a second TX altering the state by deleting
///    the AA shared object
/// 3) Submit the original certificate which should now fail during post-consensus, even though
///    validators originally run the authenticate and it passed
#[sim_test]
async fn test_abstract_account_post_consensus_deletion_failure() -> Result<(), anyhow::Error> {
    let _pcool_guard = override_pcool_flow(false);
    telemetry_subscribers::init_for_testing();
    let client_ip = SocketAddr::new([127, 0, 0, 1].into(), 0);

    // Build a test environment and create an abstract account. Step 2 below
    // reads the response of the transaction that deletes the abstract account,
    // so the fullnode has to keep the deleted object's previous version.
    let mut test_env = TestEnvironment::new_with_unpruned_fullnode().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519)
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let aa_sender = aa_ref.object_id.into();

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
    let tx_digest = tx_data.digest().into_bytes();
    // Create the MoveAuthenticator for the Ed25519 signature authenticator
    let signatures = vec![test_env.create_move_authenticator_for_ed25519(&tx_digest)?];
    // Create the TX envelope and send it for validators signing
    let aa_simple_tx = TransactionEnvelope::from_user_sig_data(tx_data, signatures);
    let cert = test_env
        .test_cluster
        .create_certificate(aa_simple_tx, Some(client_ip))
        .await
        .unwrap();

    // Step 2: tamper with the AA shared object state by deleting it, so the
    // original certificate becomes invalid post-consensus
    let aa_gas2 = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20000000000), aa_sender)
        .await;
    let pt2 = test_env.craft_aa_delete_object_ptb()?;
    let tx_data2 = test_env
        .craft_tx_from_pt(
            pt2, aa_gas2, aa_sender, None, // No sponsor
        )
        .await?;
    let tx_digest2 = tx_data2.digest().into_bytes();
    // Create the MoveAuthenticator for the Ed25519 signature authenticator
    let signatures2 = vec![test_env.create_move_authenticator_for_ed25519(&tx_digest2)?];
    // Create the TX envelope and send it for validators signing
    let aa_delete_tx = TransactionEnvelope::from_user_sig_data(tx_data2, signatures2);
    // Should succeed
    test_env
        .execute_and_check_tx_correctness(aa_delete_tx)
        .await?;

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

    assert!(
        summary.status.is_failure(),
        "Expected the TX execution to fail"
    );
    assert!(
        summary.gas_cost_summary.gas_used() == 1980400
            && summary.mutated_object_count == 1
            && summary.created_object_count == 0
            && summary.unwrapped_object_count == 0
            && summary.deleted_object_count == 0
            && summary.wrapped_object_count == 0,
        "Expected gas to be used in the failed transaction and that only the gas object was mutated",
    );

    let (error, command) = summary.status.unwrap_err();
    assert!(
        command.is_none(),
        "Expected the authentication failure to carry no command index",
    );
    let ExecutionError::InputObjectDeleted = &error else {
        panic!("Expected an InputObjectDeleted error, got: {error:?}");
    };

    Ok(())
}

/// A valid Abstract Account transaction executes successfully. Post-consensus
/// validation checks the authenticator's input objects, which include the
/// account, and keeps the transaction. Resolving the account and running the
/// authenticator are left to execution.
#[sim_test]
async fn test_valid_abstract_account_tx_executes() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519)
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let aa_sender = aa_ref.object_id.into();
    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let aa_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    let pt = test_env.craft_aa_simple_ptb(AA_MODULE_NAME)?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, aa_gas, aa_sender, None)
        .await?;
    let tx_digest = tx_data.digest().into_bytes();
    let signatures = vec![test_env.create_move_authenticator_for_ed25519(&tx_digest)?];
    test_env
        .execute_and_check_tx_correctness(TransactionEnvelope::from_user_sig_data(
            tx_data, signatures,
        ))
        .await
}

/// The protocol version the upgrade tests start at, with the P-COOL flow off.
#[cfg(msim)]
const PCOOL_FLOW_OFF_VERSION: u64 = PCOOL_FLOW_ON_VERSION - 1;

/// The protocol version the upgrade tests upgrade to, where the P-COOL flow is
/// on.
#[cfg(msim)]
const PCOOL_FLOW_ON_VERSION: u64 = iota_protocol_config::ProtocolVersion::MAX.as_u64();

/// How long the upgrade tests wait for each condition on the nodes.
#[cfg(msim)]
const UPGRADE_TEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(180);

/// The P-COOL flow reaches a network through a protocol upgrade at an epoch
/// boundary. The cluster starts with the flow off, upgrades to a version where
/// it is on, and one validator restarts in the first epoch with it on.
/// Transfers from a Move account and from a key account succeed before the
/// upgrade, after it, after the restart and in the next epoch; a validator
/// whose checkpoints differed from the certified ones would stop the test.
///
/// Simulator only: the protocol config override that turns the flag on is
/// thread-local, and only the simulator runs every node on the test's thread.
#[cfg(msim)]
#[sim_test]
async fn test_pcool_flow_turned_on_at_a_protocol_upgrade() -> Result<(), anyhow::Error> {
    let _guard = turn_on_the_pcool_flow_at_the_upgrade();
    telemetry_subscribers::init_for_testing();
    let test_env = start_before_the_pcool_flow(
        TestClusterBuilder::new()
            .with_num_validators(4)
            .with_epoch_duration_ms(60_000),
    )
    .await?;

    wait_for_the_pcool_flow(&test_env.test_cluster).await;
    transfer_from_move_account_and_key_account(&test_env).await?;

    let restarted = test_env.test_cluster.get_validator_pubkeys()[1];
    test_env.test_cluster.stop_node(&restarted);
    transfer_from_move_account_and_key_account(&test_env).await?;
    test_env.test_cluster.start_node(&restarted).await;
    transfer_from_move_account_and_key_account(&test_env).await?;

    check_the_next_epoch_with_the_pcool_flow(&test_env).await
}

/// A validator that is down when the P-COOL flow turns on starts after the
/// upgrade in the last epoch with it off, catches up that epoch, moves to the
/// first epoch with it on while that epoch's commits are being replayed, and
/// then builds the same checkpoints as the others.
///
/// Simulator only, for the reason given on the test above.
#[cfg(msim)]
#[sim_test]
async fn test_validator_down_when_the_pcool_flow_turns_on_catches_up() -> Result<(), anyhow::Error>
{
    run_with_a_validator_down_across_the_upgrade(0).await
}

/// A validator that is down for the whole first epoch with the P-COOL flow on
/// starts in the second one. It catches up the last epoch with it off, moves to
/// the first epoch with it on after that epoch has already ended, and then
/// builds the same checkpoints as the others.
///
/// Simulator only, for the reason given on the tests above.
#[cfg(msim)]
#[sim_test]
async fn test_validator_down_for_the_first_pcool_flow_epoch_catches_up() -> Result<(), anyhow::Error>
{
    run_with_a_validator_down_across_the_upgrade(1).await
}

/// A validator killed while it reconfigures into the first epoch with the
/// P-COOL flow on, before it saves the new epoch's start configuration,
/// restarts in the last epoch with it off and reconfigures again.
///
/// Simulator only, for the reason given on the tests above and because the
/// kill is a fail point.
#[cfg(msim)]
#[sim_test]
async fn test_crash_before_saving_the_first_pcool_flow_epoch_start() -> Result<(), anyhow::Error> {
    run_with_a_crash_while_reconfiguring_into_the_pcool_flow("reconfig_delay", |crash| {
        iota_macros::register_fail_point_async("reconfig_delay", move || {
            crash();
            async {}
        })
    })
    .await
}

/// A validator killed while it reconfigures into the first epoch with the
/// P-COOL flow on, after it saved the new epoch's start configuration but
/// before it opened the new epoch's store, restarts directly in the new epoch.
///
/// Simulator only, for the reason given on the tests above and because the
/// kill is a fail point.
#[cfg(msim)]
#[sim_test]
async fn test_crash_before_opening_the_first_pcool_flow_epoch_store() -> Result<(), anyhow::Error> {
    run_with_a_crash_while_reconfiguring_into_the_pcool_flow(
        "before-open-new-epoch-store",
        |crash| iota_macros::register_fail_point("before-open-new-epoch-store", move || crash()),
    )
    .await
}

/// A Move-authenticated transaction without an expiration executes while the
/// P-COOL flow is off, and still executes once it is on, whether it was signed
/// before or after the upgrade. One that expires in the epoch before the
/// upgrade is refused as expired by every validator and does not execute, so
/// its coins stay spendable.
///
/// Simulator only, for the reason given on the tests above.
#[cfg(msim)]
#[sim_test]
async fn test_move_authenticated_transaction_without_expiration_after_the_upgrade()
-> Result<(), anyhow::Error> {
    use iota_core::authority_client::validator_v2::ValidatorV2API;
    use iota_types::{base_types::ConciseableName, messages_grpc::TxStatusUpdate};

    let _guard = turn_on_the_pcool_flow_at_the_upgrade();
    telemetry_subscribers::init_for_testing();
    let client_ip = SocketAddr::new([127, 0, 0, 1].into(), 0);
    let test_env = start_before_the_pcool_flow(
        TestClusterBuilder::new()
            .with_num_validators(4)
            .with_epoch_duration_ms(60_000),
    )
    .await?;
    let test_cluster = &test_env.test_cluster;
    let aa_sender: Address = test_env.aa_ref.unwrap().object_id.into();
    let rgp = test_cluster.get_reference_gas_price().await;
    let fund = async || {
        let gas = test_cluster
            .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
            .await;
        let coin = test_cluster
            .fund_address_and_return_gas(rgp, Some(1_000_000), aa_sender)
            .await;
        (coin, gas)
    };
    let executed_before = fund().await;
    let held_without_expiration = fund().await;
    let held_expiring_before = fund().await;
    let built_after = fund().await;

    let without_expiration = async |(coin, gas): (ObjectReference, ObjectReference)| {
        let pt = test_env.craft_object_transfer(coin, Address::random())?;
        let mut tx_data = test_env.craft_tx_from_pt(pt, gas, aa_sender, None).await?;
        *tx_data.expiration_mut_for_testing() = iota_sdk_types::TransactionExpiration::None;
        let signatures =
            vec![test_env.create_move_authenticator_for_ed25519(&tx_data.digest().into_bytes())?];
        anyhow::Ok(TransactionEnvelope::from_user_sig_data(tx_data, signatures))
    };
    anyhow::ensure!(
        !pcool_flow_on_running_validators(test_cluster)
            .into_iter()
            .any(|on| on),
        "the transactions must be built before the upgrade"
    );
    test_env
        .execute_and_check_tx_correctness(without_expiration(executed_before).await?)
        .await?;
    let held_none = without_expiration(held_without_expiration).await?;
    let epoch_before = test_cluster
        .fullnode_handle
        .iota_node
        .with(|node| node.state().epoch_store_for_testing().epoch());
    let held_expired = {
        let (coin, gas) = held_expiring_before;
        let pt = test_env.craft_object_transfer(coin, Address::random())?;
        let mut tx_data = test_env.craft_tx_from_pt(pt, gas, aa_sender, None).await?;
        *tx_data.expiration_mut_for_testing() =
            iota_sdk_types::TransactionExpiration::Epoch(epoch_before);
        let signatures =
            vec![test_env.create_move_authenticator_for_ed25519(&tx_data.digest().into_bytes())?];
        TransactionEnvelope::from_user_sig_data(tx_data, signatures)
    };

    let upgrade_epoch = wait_for_the_pcool_flow(test_cluster).await;
    anyhow::ensure!(
        upgrade_epoch > epoch_before,
        "the held transactions must be signed before the upgrade"
    );
    test_env.execute_and_check_tx_correctness(held_none).await?;
    test_env
        .execute_and_check_tx_correctness(without_expiration(built_after).await?)
        .await?;

    let aggregator = test_cluster.authority_aggregator();
    for (name, client) in aggregator.authority_clients.iter() {
        let answer = client
            .authority_client()
            .submit_tx(vec![held_expired.clone()], Some(client_ip))
            .await?;
        anyhow::ensure!(
            matches!(
                answer.as_slice(),
                [(
                    _,
                    TxStatusUpdate::Rejected {
                        error: IotaError::TransactionExpired
                    }
                )]
            ),
            "validator {} must refuse a transaction that expired before the upgrade, got \
             {answer:?}",
            name.concise()
        );
    }
    for handle in test_cluster.all_node_handles() {
        anyhow::ensure!(
            handle.with(|node| {
                node.state()
                    .get_transaction_cache_reader()
                    .get_executed_effects(held_expired.digest())
                    .is_none()
            }),
            "a refused transaction must not execute"
        );
    }
    let (coin, gas) = held_expiring_before;
    let pt = test_env.craft_object_transfer(coin, Address::random())?;
    let tx_data = test_env.craft_tx_from_pt(pt, gas, aa_sender, None).await?;
    let signatures =
        vec![test_env.create_move_authenticator_for_ed25519(&tx_data.digest().into_bytes())?];
    test_env
        .execute_and_check_tx_correctness(TransactionEnvelope::from_user_sig_data(
            tx_data, signatures,
        ))
        .await?;
    transfer_from_move_account_and_key_account(&test_env).await?;
    check_the_next_epoch_with_the_pcool_flow(&test_env).await
}

/// A fullnode stopped before the P-COOL flow turns on and started after the
/// upgrade catches up the last epoch with it off, executes the first epoch with
/// it on, including a transfer made while it was down, and takes transactions
/// again. A second fullnode serves the test while the first is down.
///
/// Simulator only, for the reason given on the tests above.
#[cfg(msim)]
#[sim_test]
async fn test_fullnode_restarted_across_the_pcool_flow_upgrade() -> Result<(), anyhow::Error> {
    use test_cluster::FullNodeHandle;

    let _guard = turn_on_the_pcool_flow_at_the_upgrade();
    telemetry_subscribers::init_for_testing();
    let mut test_env = start_before_the_pcool_flow(
        TestClusterBuilder::new()
            .with_num_validators(4)
            .with_epoch_duration_ms(60_000),
    )
    .await?;
    let restarted = test_env
        .test_cluster
        .swarm
        .fullnodes()
        .next()
        .unwrap()
        .name();
    let other_fullnode = test_env.test_cluster.spawn_new_fullnode().await;

    // Coins for the transfer made while the restarted fullnode is down: the
    // faucet is reached through the wallet, which only knows that fullnode.
    let aa_sender: Address = test_env.aa_ref.unwrap().object_id.into();
    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;
    let coin = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(1_000_000), aa_sender)
        .await;
    tokio::time::timeout(UPGRADE_TEST_TIMEOUT, async {
        while other_fullnode.iota_node.with(|node| {
            [gas.object_id, coin.object_id]
                .iter()
                .any(|id| node.state().get_object(id).is_none())
        }) {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    })
    .await
    .map_err(|_| anyhow::anyhow!("the other fullnode must sync the funded coins"))?;

    // The cluster's handle pins the fullnode's store, which a restart could
    // then not open, so the test uses the other fullnode until the restart.
    drop(std::mem::replace(
        &mut test_env.test_cluster.fullnode_handle,
        other_fullnode,
    ));
    test_env.test_cluster.stop_node(&restarted);
    anyhow::ensure!(
        !pcool_flow_on_running_validators(&test_env.test_cluster)
            .into_iter()
            .any(|on| on),
        "the fullnode must stop before the upgrade"
    );

    let upgrade_epoch = wait_for_the_pcool_flow(&test_env.test_cluster).await;
    let pt = test_env.craft_object_transfer(coin, Address::random())?;
    let tx_data = test_env.craft_tx_from_pt(pt, gas, aa_sender, None).await?;
    let signatures =
        vec![test_env.create_move_authenticator_for_ed25519(&tx_data.digest().into_bytes())?];
    let transfer = TransactionEnvelope::from_user_sig_data(tx_data, signatures);
    let transferred_while_down = *transfer.digest();
    test_env.execute_and_check_tx_correctness(transfer).await?;

    test_env.test_cluster.start_node(&restarted).await;
    let (restarted_handle, json_rpc_address) = {
        let node = test_env.test_cluster.swarm.node(&restarted).unwrap();
        (node.get_node_handle().unwrap(), node.json_rpc_address())
    };
    test_env.test_cluster.fullnode_handle =
        FullNodeHandle::new(restarted_handle, json_rpc_address).await;
    wait_for_running_nodes_to_reach_epoch(&test_env.test_cluster, upgrade_epoch).await;
    tokio::time::timeout(
        UPGRADE_TEST_TIMEOUT,
        test_env
            .test_cluster
            .fullnode_handle
            .iota_node
            .with_async(|node| async move {
                node.state()
                    .get_transaction_cache_reader()
                    .try_notify_read_executed_effects("", &[transferred_while_down])
                    .await
            }),
    )
    .await
    .map_err(|_| {
        anyhow::anyhow!("the restarted fullnode must execute the transfer made while it was down")
    })??;

    transfer_from_move_account_and_key_account(&test_env).await?;
    check_the_next_epoch_with_the_pcool_flow(&test_env).await
}

/// Runs a cluster of seven validators, one of them stopped before the P-COOL
/// flow turns on and started again `epochs_after_the_upgrade` epochs after the
/// first epoch with it on. With seven validators, the other six hold enough
/// stake to pass the upgrade (2f+1 plus half of f with the default buffer);
/// three of four would not.
///
/// Epochs last 30 seconds, so the test passes the epochs it waits for within
/// the test runner's time limit. The fullnode keeps every checkpoint: in the simulator, nodes prune
/// checkpoints older than two epochs, and there is no archive, so without it
/// a validator down for two epochs would find no peer to sync from.
#[cfg(msim)]
async fn run_with_a_validator_down_across_the_upgrade(
    epochs_after_the_upgrade: u64,
) -> Result<(), anyhow::Error> {
    let _guard = turn_on_the_pcool_flow_at_the_upgrade();
    telemetry_subscribers::init_for_testing();
    let test_env = start_before_the_pcool_flow(
        TestClusterBuilder::new()
            .with_num_validators(7)
            .with_epoch_duration_ms(30_000)
            .disable_fullnode_pruning(),
    )
    .await?;
    let test_cluster = &test_env.test_cluster;

    let stopped = test_cluster.get_validator_pubkeys()[1];
    test_cluster.stop_node(&stopped);
    anyhow::ensure!(
        !pcool_flow_on_running_validators(test_cluster)
            .into_iter()
            .any(|on| on),
        "the validator must stop before the upgrade"
    );
    transfer_from_move_account_and_key_account(&test_env).await?;

    let upgrade_epoch = wait_for_the_pcool_flow(test_cluster).await;
    transfer_from_move_account_and_key_account(&test_env).await?;
    let restart_epoch = upgrade_epoch + epochs_after_the_upgrade;
    if restart_epoch > upgrade_epoch {
        test_cluster
            .wait_for_epoch_with_timeout(Some(restart_epoch), UPGRADE_TEST_TIMEOUT)
            .await;
        wait_for_running_nodes_to_reach_epoch(test_cluster, restart_epoch).await;
        transfer_from_move_account_and_key_account(&test_env).await?;
    }

    test_cluster.start_node(&stopped).await;
    wait_for_validator_to_reach_epoch(test_cluster, &stopped, restart_epoch).await;
    transfer_from_move_account_and_key_account(&test_env).await?;
    assert_running_nodes_agree_on_checkpoints(test_cluster).await;
    check_the_next_epoch_with_the_pcool_flow(&test_env).await
}

/// Runs a cluster of four validators where one is killed once at `fail_point`,
/// while it reconfigures into the first epoch with the P-COOL flow on.
/// `register` registers the given kill at that fail point. The killed validator
/// restarts by itself.
#[cfg(msim)]
async fn run_with_a_crash_while_reconfiguring_into_the_pcool_flow(
    fail_point: &'static str,
    register: impl FnOnce(std::sync::Arc<dyn Fn() + Send + Sync>),
) -> Result<(), anyhow::Error> {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use iota_types::iota_system_state::IotaSystemStateTrait;

    let _guard = turn_on_the_pcool_flow_at_the_upgrade();
    telemetry_subscribers::init_for_testing();
    let test_env = start_before_the_pcool_flow(
        TestClusterBuilder::new()
            .with_num_validators(4)
            .with_epoch_duration_ms(60_000),
    )
    .await?;
    let test_cluster = &test_env.test_cluster;

    let victim = test_cluster.get_validator_pubkeys()[1];
    // A weak reference, so that the killed validator's store is not pinned
    // and the restart can open it.
    let (victim_sim_id, victim_state) = test_cluster
        .swarm
        .node(&victim)
        .unwrap()
        .get_node_handle()
        .unwrap()
        .with(|node| {
            (
                iota_simulator::current_simnode_id(),
                Arc::downgrade(&node.state()),
            )
        });
    let killed = Arc::new(AtomicBool::new(false));
    {
        let killed = killed.clone();
        register(Arc::new(move || {
            if iota_simulator::current_simnode_id() != victim_sim_id
                || killed.load(Ordering::SeqCst)
            {
                return;
            }
            // The epoch-change transaction has executed, so the system state
            // already carries the next epoch's protocol version.
            let into_the_pcool_flow = victim_state.upgrade().is_some_and(|state| {
                state
                    .get_iota_system_state_object_for_testing()
                    .is_ok_and(|system_state| {
                        system_state.protocol_version() == PCOOL_FLOW_ON_VERSION
                    })
            });
            if into_the_pcool_flow {
                killed.store(true, Ordering::SeqCst);
                iota_simulator::task::kill_current_node(None);
            }
        }));
    }

    let upgrade_epoch = wait_for_the_pcool_flow(test_cluster).await;
    wait_for_validator_to_reach_epoch(test_cluster, &victim, upgrade_epoch).await;
    iota_macros::clear_fail_point(fail_point);
    anyhow::ensure!(
        killed.load(Ordering::SeqCst),
        "the validator must be killed at {fail_point} while it reconfigures"
    );
    transfer_from_move_account_and_key_account(&test_env).await?;
    assert_running_nodes_agree_on_checkpoints(test_cluster).await;
    check_the_next_epoch_with_the_pcool_flow(&test_env).await
}

/// Turns the P-COOL flow off below `PCOOL_FLOW_ON_VERSION` and on from it,
/// until the guard is dropped.
#[cfg(msim)]
fn turn_on_the_pcool_flow_at_the_upgrade() -> iota_protocol_config::OverrideGuard {
    ProtocolConfig::apply_overrides_for_testing(|version, mut config| {
        config.set_enable_pcool_flow_for_testing(version.as_u64() >= PCOOL_FLOW_ON_VERSION);
        config
    })
}

/// Starts a cluster at `PCOOL_FLOW_OFF_VERSION` whose validators upgrade to
/// `PCOOL_FLOW_ON_VERSION` at an epoch change, creates the Move account and
/// checks that transfers succeed before the upgrade. The builder's epochs must
/// be long enough for that to happen in the first one.
#[cfg(msim)]
async fn start_before_the_pcool_flow(
    builder: TestClusterBuilder,
) -> Result<TestEnvironment, anyhow::Error> {
    use iota_types::supported_protocol_versions::SupportedProtocolVersions;

    let mut test_env = TestEnvironment::with_cluster(
        builder
            .with_protocol_version(PCOOL_FLOW_OFF_VERSION.into())
            .with_supported_protocol_versions(SupportedProtocolVersions::new_for_testing(
                PCOOL_FLOW_OFF_VERSION,
                PCOOL_FLOW_ON_VERSION,
            ))
            .build()
            .await,
    );
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519)
        .await?;
    anyhow::ensure!(
        !pcool_flow_on_running_validators(&test_env.test_cluster)
            .into_iter()
            .any(|on| on),
        "the upgrade must come after the first transfers"
    );
    transfer_from_move_account_and_key_account(&test_env).await?;
    Ok(test_env)
}

/// Whether each running validator has the P-COOL flow on in its current epoch.
#[cfg(msim)]
fn pcool_flow_on_running_validators(test_cluster: &TestCluster) -> Vec<bool> {
    test_cluster
        .swarm
        .validator_nodes()
        .filter_map(|node| node.get_node_handle())
        .map(|handle| {
            handle.with(|node| {
                node.state()
                    .epoch_store_for_testing()
                    .protocol_config()
                    .enable_pcool_flow()
            })
        })
        .collect()
}

/// Waits until a running validator is in an epoch with the P-COOL flow on, then
/// until every running node is in that epoch or a later one, and returns that
/// epoch.
#[cfg(msim)]
async fn wait_for_the_pcool_flow(test_cluster: &TestCluster) -> u64 {
    let upgrade_epoch = tokio::time::timeout(UPGRADE_TEST_TIMEOUT, async {
        loop {
            let upgrade_epoch = test_cluster
                .swarm
                .validator_nodes()
                .filter_map(|node| node.get_node_handle())
                .find_map(|handle| {
                    handle.with(|node| {
                        let epoch_store = node.state().epoch_store_for_testing();
                        epoch_store
                            .protocol_config()
                            .enable_pcool_flow()
                            .then(|| epoch_store.epoch())
                    })
                });
            if let Some(upgrade_epoch) = upgrade_epoch {
                return upgrade_epoch;
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    })
    .await
    .expect("a validator must reach an epoch with the P-COOL flow on");
    wait_for_running_nodes_to_reach_epoch(test_cluster, upgrade_epoch).await;
    assert!(
        pcool_flow_on_running_validators(test_cluster)
            .into_iter()
            .all(|on| on),
        "every running validator must have the P-COOL flow on in epoch {upgrade_epoch}"
    );
    upgrade_epoch
}

/// Waits until every running node is in `epoch` or a later one, and every
/// running fullnode's client for the validators is too.
#[cfg(msim)]
async fn wait_for_running_nodes_to_reach_epoch(test_cluster: &TestCluster, epoch: u64) {
    let reached = |handle: &iota_node::IotaNodeHandle| {
        handle.with(|node| {
            node.state().epoch_store_for_testing().epoch() >= epoch
                && node
                    .clone_authority_aggregator()
                    .is_none_or(|aggregator| aggregator.committee.epoch() >= epoch)
        })
    };
    tokio::time::timeout(UPGRADE_TEST_TIMEOUT, async {
        while !test_cluster.all_node_handles().iter().all(&reached) {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("every running node must reach epoch {epoch}"));
}

/// Waits until the validator is running and in `epoch` or a later one.
#[cfg(msim)]
async fn wait_for_validator_to_reach_epoch(
    test_cluster: &TestCluster,
    name: &AuthorityName,
    epoch: u64,
) {
    use iota_types::base_types::ConciseableName;

    tokio::time::timeout(UPGRADE_TEST_TIMEOUT, async {
        while !test_cluster
            .swarm
            .node(name)
            .unwrap()
            .get_node_handle()
            .is_some_and(|handle| {
                handle.with(|node| node.state().epoch_store_for_testing().epoch() >= epoch)
            })
        {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("validator {} must reach epoch {epoch}", name.concise()));
}

/// Waits for the network's next epoch, checks that every running validator has
/// the P-COOL flow on in it, that transfers succeed in it, and that every
/// running node agrees on the checkpoints.
#[cfg(msim)]
async fn check_the_next_epoch_with_the_pcool_flow(
    test_env: &TestEnvironment,
) -> Result<(), anyhow::Error> {
    let test_cluster = &test_env.test_cluster;
    let next_epoch = test_cluster
        .fullnode_handle
        .iota_node
        .with(|node| node.state().epoch_store_for_testing().epoch())
        + 1;
    test_cluster
        .wait_for_epoch_with_timeout(Some(next_epoch), UPGRADE_TEST_TIMEOUT)
        .await;
    wait_for_running_nodes_to_reach_epoch(test_cluster, next_epoch).await;
    anyhow::ensure!(
        pcool_flow_on_running_validators(test_cluster)
            .into_iter()
            .all(|on| on),
        "every running validator must have the P-COOL flow on in epoch {next_epoch}"
    );
    transfer_from_move_account_and_key_account(test_env).await?;
    assert_running_nodes_agree_on_checkpoints(test_cluster).await;
    Ok(())
}

/// Waits until every running node has executed a checkpoint a few past the
/// one the test's fullnode has executed, so the network keeps making
/// checkpoints, and every running validator has built it. Then checks that
/// every running node holds the same certified checkpoint there, and that each
/// validator's own checkpoints match the certified ones where it still holds
/// both. A validator prunes its own checkpoints of earlier epochs, which its
/// fork check covered when it executed them.
#[cfg(msim)]
async fn assert_running_nodes_agree_on_checkpoints(test_cluster: &TestCluster) {
    use iota_types::base_types::ConciseableName;

    let highest_executed = |handle: &iota_node::IotaNodeHandle| {
        handle.with(|node| {
            node.state()
                .get_checkpoint_store()
                .get_highest_executed_checkpoint_seq_number()
                .unwrap()
                .unwrap_or_default()
        })
    };
    let last_built = |handle: &iota_node::IotaNodeHandle| {
        handle.with(|node| {
            node.state()
                .epoch_store_for_testing()
                .last_built_checkpoint_builder_summary()
                .unwrap()
                .map(|summary| summary.summary.sequence_number)
                .unwrap_or_default()
        })
    };
    let target = highest_executed(&test_cluster.fullnode_handle.iota_node) + 5;
    tokio::time::timeout(UPGRADE_TEST_TIMEOUT, async {
        while test_cluster
            .all_node_handles()
            .iter()
            .any(|handle| highest_executed(handle) < target)
            || test_cluster
                .swarm
                .validator_nodes()
                .filter_map(|node| node.get_node_handle())
                .any(|handle| last_built(&handle) < target)
        {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!("every running node must execute, and every running validator build, checkpoint {target}")
    });

    let certified_at_target: Vec<_> = test_cluster
        .all_node_handles()
        .iter()
        .map(|handle| {
            handle.with(|node| {
                *node
                    .state()
                    .get_checkpoint_store()
                    .get_checkpoint_by_sequence_number(target)
                    .unwrap()
                    .expect("an executed checkpoint is certified")
                    .digest()
            })
        })
        .collect();
    assert!(
        certified_at_target
            .iter()
            .all(|digest| *digest == certified_at_target[0]),
        "every running node must hold the same checkpoint {target}: {certified_at_target:?}"
    );
    for node in test_cluster.swarm.validator_nodes() {
        let Some(handle) = node.get_node_handle() else {
            continue;
        };
        handle.with(|iota_node| {
            let store = iota_node.state().get_checkpoint_store().clone();
            for sequence_number in 1..=target {
                let local = store
                    .get_locally_computed_checkpoint(sequence_number)
                    .unwrap();
                let certified = store
                    .get_checkpoint_by_sequence_number(sequence_number)
                    .unwrap();
                if let (Some(local), Some(certified)) = (local, certified) {
                    assert_eq!(
                        local.digest(),
                        *certified.digest(),
                        "validator {}'s checkpoint {sequence_number} must match the certified one",
                        node.name().concise(),
                    );
                }
            }
        });
    }
}

/// Executes a transfer of a fresh coin from the environment's Move account,
/// then a transfer from a key account. Panics unless both succeed.
#[cfg(msim)]
async fn transfer_from_move_account_and_key_account(
    test_env: &TestEnvironment,
) -> Result<(), anyhow::Error> {
    let Some(aa_ref) = test_env.aa_ref else {
        anyhow::bail!("Abstract account not created yet");
    };
    let aa_sender: Address = aa_ref.object_id.into();
    let test_cluster = &test_env.test_cluster;
    let rgp = test_cluster.get_reference_gas_price().await;
    let aa_gas = test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;
    let coin = test_cluster
        .fund_address_and_return_gas(rgp, Some(1_000_000), aa_sender)
        .await;
    let pt = test_env.craft_object_transfer(coin, Address::random())?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, aa_gas, aa_sender, None)
        .await?;
    let tx_digest = tx_data.digest().into_bytes();
    let signatures = vec![test_env.create_move_authenticator_for_ed25519(&tx_digest)?];
    test_env
        .execute_and_check_tx_correctness(TransactionEnvelope::from_user_sig_data(
            tx_data, signatures,
        ))
        .await?;

    let tx = test_cluster
        .test_transaction_builder_with_sender(test_cluster.get_address_0())
        .await
        .transfer_iota(Some(1_000_000), test_cluster.get_address_1())
        .build();
    test_cluster.sign_and_execute_transaction(&tx).await;
    Ok(())
}

/// Under P-COOL, a transaction whose authenticator fails at execution produces
/// failure effects on every validator. Such a transaction reaches consensus
/// through an honest validator whose view is behind: before submitting, it runs
/// the authenticator against its own older state, where it passes.
/// Post-consensus validation keeps the transaction without running the
/// authenticator, so validators at different points cannot disagree on it, and
/// execution runs the authenticator at the version consensus assigned, where it
/// fails. The account pays its own gas but did not authorize the transaction,
/// so nobody is charged, and the account can spend the gas coin afterwards at
/// its next version.
///
/// Staging: execution on one validator is paused before the owner key rotation
/// executes there, so that validator still admits a transaction signed with
/// the old key. Consensus orders it after the rotation, and every validator
/// fails it at execution.
///
/// Simulator only, because the pause is a fail point.
#[cfg(msim)]
#[sim_test]
async fn test_stale_abstract_account_tx_fails_with_effects_with_the_pcool_flow()
-> Result<(), anyhow::Error> {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };

    use iota_core::authority_client::validator_v2::ValidatorV2API;
    use iota_macros::{clear_fail_point, register_fail_point_async};
    use iota_sdk_types::{ExecutionStatus, GasCostSummary};

    telemetry_subscribers::init_for_testing();
    let client_ip = SocketAddr::new([127, 0, 0, 1].into(), 0);

    let mut test_env = TestEnvironment::with_cluster(
        TestClusterBuilder::new()
            .with_epoch_duration_ms(600_000)
            .build()
            .await,
    );
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519)
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: Address = aa_ref.object_id.into();
    let rgp = test_env.test_cluster.get_reference_gas_price().await;

    // Both gas coins exist on every validator before any execution is paused.
    let stale_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;
    let rotate_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    // The new owner key the rotation installs.
    let keystore = test_env.test_cluster.wallet.config_mut().keystore_mut();
    let new_owner = keystore
        .generate_and_add_new_key(SignatureScheme::Ed25519, None, None, None)
        .expect("ED25519 key generation should not fail")
        .0;
    let new_owner_pk = test_env
        .test_cluster
        .wallet
        .config()
        .keystore()
        .get_key(&new_owner)?
        .public();

    // Pause execution on one validator until the stale transaction has been
    // admitted and executed by the others.
    let paused_validator = test_env.test_cluster.get_validator_pubkeys()[0];
    let paused_sim_id = test_env
        .test_cluster
        .swarm
        .node(&paused_validator)
        .unwrap()
        .get_node_handle()
        .unwrap()
        .with(|_| iota_simulator::current_simnode_id());
    let paused = Arc::new(AtomicBool::new(true));
    {
        let paused = paused.clone();
        register_fail_point_async("transaction_execution_delay", move || {
            let paused = paused.clone();
            async move {
                if iota_simulator::current_simnode_id() == paused_sim_id {
                    while paused.load(Ordering::SeqCst) {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                }
            }
        });
    }

    // The rotation executes on every validator but the paused one.
    let pt = test_env.craft_aa_rotate_owner_key_ptb(&new_owner_pk)?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, rotate_gas, aa_sender, None)
        .await?;
    let tx_digest = tx_data.digest().into_bytes();
    let signatures = vec![test_env.create_move_authenticator_for_ed25519(&tx_digest)?];
    test_env
        .execute_and_check_tx_correctness(TransactionEnvelope::from_user_sig_data(
            tx_data, signatures,
        ))
        .await?;

    // A transaction signed with the old key, submitted to the paused validator.
    // Its view still has that key, so the authenticator it runs before
    // submitting passes, and it proposes the transaction.
    let pt = test_env.craft_aa_simple_ptb(AA_MODULE_NAME)?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, stale_gas, aa_sender, None)
        .await?;
    let tx_digest = tx_data.digest().into_bytes();
    let signatures = vec![test_env.create_move_authenticator_for_ed25519(&tx_digest)?];
    let stale_tx = TransactionEnvelope::from_user_sig_data(tx_data, signatures);
    let stale_digest = *stale_tx.digest();
    let paused_client = test_env
        .test_cluster
        .authority_aggregator()
        .authority_clients[&paused_validator]
        .clone();
    let submission = tokio::spawn(async move {
        paused_client
            .authority_client()
            .submit_tx(vec![stale_tx], Some(client_ip))
            .await
    });

    // Post-consensus validation keeps it on every validator, since none runs
    // the authenticator there. The others execute it and certify its
    // checkpoint, which the fullnode then executes.
    let effects = tokio::time::timeout(
        Duration::from_secs(60),
        test_env
            .test_cluster
            .fullnode_handle
            .iota_node
            .with_async(|node| async move {
                node.state()
                    .get_transaction_cache_reader()
                    .try_notify_read_executed_effects("", &[stale_digest])
                    .await
            }),
    )
    .await
    .expect("the stale transaction must execute within 60s")?
    .remove(0);

    // Resume the paused validator and let the submission finish, so the fail
    // point is cleared even if an assertion below fails.
    paused.store(false, Ordering::SeqCst);
    tokio::time::timeout(Duration::from_secs(60), submission)
        .await
        .expect("the paused validator must finish the submission within 60s")??;
    clear_fail_point("transaction_execution_delay");

    // Kept by every validator and failed at execution: an authentication
    // failure, not a drop.
    let ExecutionStatus::Failure { error, command } = effects.status() else {
        panic!(
            "expected the stale transaction to fail, got {:?}",
            effects.status()
        );
    };
    assert!(
        command.is_none(),
        "the authentication failure must carry no command index"
    );
    let ExecutionError::MoveAuthentication { error } = error else {
        panic!("expected a MoveAuthentication failure, got {error:?}");
    };
    assert!(
        matches!(
            &**error,
            ExecutionError::MoveAbort { location: MoveLocation { module, function_name, .. }, .. }
            if module.as_str() == "basic_keyed_aa"
            && function_name.as_ref().is_some_and(|f| f.as_str() == "authenticate_ed25519")
        ),
        "expected the abort of basic_keyed_aa::authenticate_ed25519, got {error:?}"
    );
    // The account pays its own gas but did not authorize the transaction, so
    // nobody is charged. Its gas coin moves to its next version with its
    // balance unchanged.
    assert_eq!(
        effects.gas_cost_summary(),
        &GasCostSummary::default(),
        "nobody may be charged for a transaction the gas owner did not authorize"
    );
    let next_gas = effects
        .mutated()
        .into_iter()
        .find(|mutated| mutated.reference().object_id == stale_gas.object_id)
        .map(|mutated| *mutated.reference())
        .expect("the gas coin must move to its next version");

    // The paused validator kept the transaction too: the checkpoint it builds
    // from its own consensus output matches the certified one. Its effects
    // alone would not show that, since a validator that dropped the
    // transaction still executes it from the certified checkpoint.
    let paused_node = test_env
        .test_cluster
        .swarm
        .node(&paused_validator)
        .unwrap()
        .get_node_handle()
        .unwrap();
    let (local, certified) = tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            let built = paused_node.with(|node| {
                let state = node.state();
                let seq = state
                    .epoch_store_for_testing()
                    .get_transaction_checkpoint(&stale_digest)
                    .unwrap()?;
                let store = state.get_checkpoint_store();
                Some((
                    store.get_locally_computed_checkpoint(seq).unwrap()?,
                    store.get_checkpoint_by_sequence_number(seq).unwrap()?,
                ))
            });
            if let Some(built) = built {
                break built;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("the paused validator must build the stale transaction's checkpoint within 60s");
    assert_eq!(
        &local,
        certified.data(),
        "the paused validator's own checkpoint must match the certified one"
    );

    // The account can spend the gas coin at its next version with its new
    // key, here by sending all of it away.
    // Signing the stale transaction again would not do: a digest does not
    // cover signatures, so that would be the stale transaction, which has
    // already executed.
    test_env.owner = Some(new_owner);
    let mut builder = ProgrammableTransactionBuilder::new();
    builder.pay_all_iota(Address::random());
    let tx_data = test_env
        .craft_tx_from_pt(builder.finish(), next_gas, aa_sender, None)
        .await?;
    let tx_digest = tx_data.digest().into_bytes();
    let signatures = vec![test_env.create_move_authenticator_for_ed25519(&tx_digest)?];
    test_env
        .execute_and_check_tx_correctness(TransactionEnvelope::from_user_sig_data(
            tx_data, signatures,
        ))
        .await?;

    Ok(())
}

/// Under P-COOL, a transaction from an account whose authenticate function
/// lives in a package the deny rules name is dropped at the commit on every
/// validator, even when the validator that submits it skips its admission
/// checks, as a faulty one can. Nobody executes it, so the denied package's
/// authenticate function does not run, and the validators keep building the
/// certified checkpoints, which the fullnode keeps executing. The same
/// transaction from an account whose authenticate function lives in a second
/// copy of the package, which is not denied, executes.
///
/// The deny rules in force under P-COOL are the ones the validators agree on
/// through consensus, from their local deny configs. The package only exists
/// once the cluster runs, so every validator is restarted with a config that
/// denies it, and an epoch change follows, after which the fullnode's authority
/// clients reach the restarted validators again.
///
/// Simulator only, because the admission skip is a fail point.
#[cfg(msim)]
#[sim_test]
async fn test_denied_authenticator_package_is_dropped_at_the_commit() -> Result<(), anyhow::Error> {
    use std::time::Duration;

    use iota_core::authority_client::validator_v2::ValidatorV2API;
    use iota_macros::{clear_fail_point, register_fail_point_if};
    use iota_sdk_types::ExecutionStatus;
    use iota_types::messages_grpc::TxStatusUpdate;

    telemetry_subscribers::init_for_testing();
    let client_ip = SocketAddr::new([127, 0, 0, 1].into(), 0);

    let mut test_env = TestEnvironment::with_cluster(
        TestClusterBuilder::new()
            .with_epoch_duration_ms(600_000)
            .build()
            .await,
    );
    for handle in test_env.test_cluster.all_validator_handles() {
        handle.with(|node| {
            let epoch_store = node.state().epoch_store_for_testing();
            let protocol_config = epoch_store.protocol_config();
            assert!(
                protocol_config.enable_pcool_flow()
                    && protocol_config.deny_authenticator_packages(),
                "the P-COOL flow and the authenticator package deny check must be on"
            );
        });
    }

    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519)
        .await?;
    let owner = test_env.owner.unwrap();
    let denied_package = test_env.aa_package_id.unwrap();
    let denied_account = test_env.aa_ref.unwrap();
    let (allowed_package, allowed_package_metadata_ref) =
        test_env.publish_account_abstraction_package().await;
    let effects = test_env
        .create_abstract_account_with(
            owner,
            AA_AUTHENTICATE_FN_NAME_ED25519,
            allowed_package,
            allowed_package_metadata_ref,
        )
        .await?;
    let allowed_account = abstract_account_from_all_changed_objects(&effects.all_changed_objects());

    // A gas coin and a coin to transfer for each account.
    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let fund = |account: ObjectReference, amount: u64| {
        test_env.test_cluster.fund_address_and_return_gas(
            rgp,
            Some(amount),
            account.object_id.into(),
        )
    };
    let denied_gas = fund(denied_account, 20_000_000_000).await;
    let denied_coin = fund(denied_account, 1_000_000).await;
    let allowed_gas = fund(allowed_account, 20_000_000_000).await;
    let allowed_coin = fund(allowed_account, 1_000_000).await;

    for name in test_env.test_cluster.get_validator_pubkeys() {
        test_env
            .test_cluster
            .update_transaction_deny_config_on(
                &name,
                TransactionDenyConfigBuilder::new()
                    .add_denied_package(denied_package)
                    .build(),
            )
            .await;
    }
    test_env.test_cluster.force_new_epoch().await;
    tokio::time::timeout(Duration::from_secs(60), async {
        while !test_env
            .test_cluster
            .all_validator_handles()
            .iter()
            .all(|handle| {
                handle.with(|node| {
                    node.state()
                        .epoch_store_for_testing()
                        .get_active_transaction_deny_rules()
                        .denied_packages
                        .contains(&denied_package)
                })
            })
        {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    })
    .await
    .expect("the deny rules must deny the package on every validator within 60s");

    // A plain transfer, so the account's package is named by the authenticator
    // alone.
    let account_tx = |account: ObjectReference, gas: ObjectReference, coin: ObjectReference| {
        let test_env = &test_env;
        async move {
            let pt = test_env.craft_object_transfer(coin, owner)?;
            let tx_data = test_env
                .craft_tx_from_pt(pt, gas, account.object_id.into(), None)
                .await?;
            let tx_digest = tx_data.digest().into_bytes();
            let signatures =
                vec![test_env.create_move_authenticator_for_ed25519_for_ref(account, &tx_digest)?];
            anyhow::Ok(TransactionEnvelope::from_user_sig_data(tx_data, signatures))
        }
    };

    // One validator skips its admission checks and submits both transactions,
    // as a faulty one can.
    let faulty = test_env.test_cluster.get_validator_pubkeys()[0];
    let faulty_sim_id = test_env
        .test_cluster
        .swarm
        .node(&faulty)
        .unwrap()
        .get_node_handle()
        .unwrap()
        .with(|_| iota_simulator::current_simnode_id());
    register_fail_point_if("admit-without-input-checks", move || {
        iota_simulator::current_simnode_id() == faulty_sim_id
    });
    let faulty_client = test_env
        .test_cluster
        .authority_aggregator()
        .authority_clients[&faulty]
        .clone();
    let submit = |tx: TransactionEnvelope| {
        let faulty_client = faulty_client.clone();
        async move {
            let digest = *tx.digest();
            let submitted = tokio::time::timeout(
                Duration::from_secs(60),
                faulty_client
                    .authority_client()
                    .submit_tx(vec![tx], Some(client_ip)),
            )
            .await
            .expect("the submission must be answered within 60s")
            .unwrap();
            assert!(
                matches!(
                    submitted.as_slice(),
                    [(submitted_digest, TxStatusUpdate::Submitted)] if *submitted_digest == digest
                ),
                "the validator skipping its admission checks must submit the transaction, got \
                 {submitted:?}"
            );
            digest
        }
    };

    let allowed_digest =
        submit(account_tx(allowed_account, allowed_gas, allowed_coin).await?).await;
    let denied_digest = submit(account_tx(denied_account, denied_gas, denied_coin).await?).await;
    clear_fail_point("admit-without-input-checks");

    let allowed_effects = tokio::time::timeout(
        Duration::from_secs(60),
        test_env
            .test_cluster
            .fullnode_handle
            .iota_node
            .with_async(|node| async move {
                node.state()
                    .get_transaction_cache_reader()
                    .try_notify_read_executed_effects("", &[allowed_digest])
                    .await
            }),
    )
    .await
    .expect("the transaction from the account of the package not denied must execute within 60s")?
    .remove(0);
    assert_eq!(
        allowed_effects.status(),
        &ExecutionStatus::Success,
        "the transaction from the account of the package not denied must succeed"
    );

    // Every validator drops the transaction from the account of the denied
    // package, with the error the deny check gives at admission.
    for handle in test_env.test_cluster.all_validator_handles() {
        let outcome = tokio::time::timeout(
            Duration::from_secs(60),
            handle.with_async(|node| async move {
                let state = node.state();
                let cache = state.get_transaction_cache_reader();
                let epoch_store = state.epoch_store_for_testing();
                let digests = [denied_digest];
                tokio::select! {
                    error = epoch_store.notify_read_dropped_digests(denied_digest) => Ok(error),
                    effects = cache.try_notify_read_executed_effects("", &digests) => {
                        Err(effects)
                    }
                }
            }),
        )
        .await
        .expect("every validator must drop or execute the transaction within 60s");
        let error = match outcome {
            Ok(error) => error,
            Err(effects) => panic!(
                "a validator executed the transaction from the account of the denied package: \
                 {:?}",
                effects.map(|effects| effects[0].status().clone())
            ),
        };
        let IotaError::UserInput {
            error: UserInputError::TransactionDenied { error },
        } = &error
        else {
            panic!("expected TransactionDenied, got: {error:?}");
        };
        assert!(
            error.contains(&denied_package.to_string()),
            "expected the denied authenticator package to be named, got: {error}"
        );
    }

    // Every validator keeps executing checkpoints well past the horizon
    // distance after the drop, and the ones it builds itself are the certified
    // ones.
    let highest_executed = |handle: &iota_node::IotaNodeHandle| {
        handle.with(|node| {
            node.state()
                .get_checkpoint_store()
                .get_highest_executed_checkpoint_seq_number()
                .unwrap()
                .unwrap_or_default()
        })
    };
    let last_built = |handle: &iota_node::IotaNodeHandle| {
        handle.with(|node| {
            node.state()
                .epoch_store_for_testing()
                .last_built_checkpoint_builder_summary()
                .unwrap()
                .map(|summary| summary.summary.sequence_number)
                .unwrap_or_default()
        })
    };
    let validators = test_env.test_cluster.all_validator_handles();
    let first_compared = validators.iter().map(highest_executed).min().unwrap();
    let target = validators.iter().map(highest_executed).max().unwrap() + 20;
    for handle in &validators {
        tokio::time::timeout(Duration::from_secs(180), async {
            while highest_executed(handle) < target || last_built(handle) < target {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        })
        .await
        .expect("every validator must execute and build checkpoints past the drop");
        handle.with(|node| {
            let store = node.state().get_checkpoint_store().clone();
            for sequence_number in first_compared..=target {
                let local = store
                    .get_locally_computed_checkpoint(sequence_number)
                    .unwrap();
                let certified = store
                    .get_checkpoint_by_sequence_number(sequence_number)
                    .unwrap();
                if let (Some(local), Some(certified)) = (local, certified) {
                    assert_eq!(
                        local.digest(),
                        *certified.digest(),
                        "a validator's checkpoint {sequence_number} must match the certified one"
                    );
                }
            }
        });
    }
    tokio::time::timeout(Duration::from_secs(60), async {
        while highest_executed(&test_env.test_cluster.fullnode_handle.iota_node) < target {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    })
    .await
    .expect("the fullnode must keep executing checkpoints past the drop");

    // Nobody executed the dropped transaction, and its gas coin is untouched.
    // The authenticate function runs at admission, which the submitting
    // validator skipped, and at execution, so it ran nowhere.
    for handle in test_env.test_cluster.all_node_handles() {
        let executed = handle.with(|node| {
            node.state()
                .get_transaction_cache_reader()
                .try_get_executed_effects(&denied_digest)
                .unwrap()
        });
        assert!(
            executed.is_none(),
            "no node may execute the transaction from the account of the denied package"
        );
    }
    assert_eq!(
        test_env
            .test_cluster
            .get_latest_object_ref(&denied_gas.object_id)
            .await,
        denied_gas,
        "the dropped transaction's gas coin must keep its version"
    );

    Ok(())
}

/// Under P-COOL, a transaction that names, as an owned input, a version that a
/// receive already used never reaches execution: every validator reports it
/// missing, and drops it once resubmitted.
///
/// An object can be received from its parent only while the parent is not an
/// account, but the parent can become one afterwards and then send
/// transactions whose owned inputs are the objects sent to its address. Bob
/// receives a coin from the delayed account object, the object becomes an
/// account, and the account names the coin's received version as an owned
/// input. Admission rejects that transaction on an honest validator, so the
/// test has one validator skip its input checks and submit it to consensus, as
/// a faulty validator can. Post-consensus validation reports the account
/// missing until the horizon reaches the commit that made it one. By then the
/// receive is at or below the horizon too, so a resubmission drops on the
/// received version.
///
/// Simulator only, because the admission bypass is a fail point.
#[cfg(msim)]
#[sim_test]
async fn test_received_version_named_as_owned_input_never_executes() -> Result<(), anyhow::Error> {
    use std::time::Duration;

    use iota_core::authority_client::validator_v2::ValidatorV2API;
    use iota_macros::{clear_fail_point, register_fail_point_if};
    use iota_types::messages_grpc::{GetTxStatusRequest, TxStatusQuery, TxStatusUpdate};

    // Wide enough that the receive, the account creation and the first
    // submission fit within it.
    const HORIZON_DISTANCE: u64 = 50;

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_pcool_deterministic_validation_horizon_distance_for_testing(HORIZON_DISTANCE);
        config
    });
    telemetry_subscribers::init_for_testing();
    let client_ip = SocketAddr::new([127, 0, 0, 1].into(), 0);

    let mut test_env = TestEnvironment::with_cluster(
        TestClusterBuilder::new()
            .with_epoch_duration_ms(600_000)
            .build()
            .await,
    );
    test_env
        .setup_delayed_abstract_account_object(AA_AUTHENTICATE_FN_NAME_FREE_ACCESS)
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: Address = aa_ref.object_id.into();
    let rgp = test_env.test_cluster.get_reference_gas_price().await;

    let bob = {
        let keystore = test_env.test_cluster.wallet.config_mut().keystore_mut();
        keystore
            .generate_and_add_new_key(SignatureScheme::Ed25519, None, None, None)
            .expect("ED25519 key generation should not fail")
            .0
    };
    let bob_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), bob)
        .await;
    let received_coin = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;
    let account_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    // Every input above must be older than the horizon distance, or post-
    // consensus validation reports it missing for that reason instead.
    let stale_validator = test_env.test_cluster.get_validator_pubkeys()[0];
    let stale_node = test_env
        .test_cluster
        .swarm
        .node(&stale_validator)
        .unwrap()
        .get_node_handle()
        .unwrap();
    let flushed_commit = || {
        stale_node.with(|node| {
            node.state()
                .epoch_store_for_testing()
                .tables()
                .unwrap()
                .get_last_consensus_index()
                .unwrap()
                .map(|indices| indices.sub_dag_index)
                .unwrap_or_default()
        })
    };
    let wait_for_commit = |target: u64| async move {
        tokio::time::timeout(Duration::from_secs(300), async {
            while flushed_commit() < target {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        })
        .await
        .expect("the commits must advance within 300s");
    };
    wait_for_commit(flushed_commit() + HORIZON_DISTANCE + 5).await;

    // Bob receives the coin while the object is not an account yet.
    let pt = test_env.craft_aa_receive_gas_ptb(
        received_coin,
        AA_DELAYED_MODULE_NAME,
        AA_RECEIVE_OBJECT_FN_NAME_NO_SENDER_CHECK,
    )?;
    let tx_data = test_env.craft_tx_from_pt(pt, bob_gas, bob, None).await?;
    let receive_tx = test_env.test_cluster.wallet.sign_transaction(&tx_data);
    let effects = test_env
        .test_cluster
        .wallet
        .execute_transaction_may_fail(receive_tx)
        .await?
        .effects
        .unwrap();
    assert!(
        effects.status().is_ok(),
        "the receive must succeed while the object is not an account, got {:?}",
        effects.status()
    );

    let effects = test_env.make_delayed_abstract_account().await?;
    assert!(
        effects.status().is_success(),
        "making the object an account must succeed, got {:?}",
        effects.status()
    );
    let made_account_commit = flushed_commit();

    // The account names the received version as an owned input.
    let pt = test_env.craft_object_transfer(received_coin, Address::ZERO)?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, account_gas, aa_sender, None)
        .await?;
    let signatures = vec![test_env.create_move_authenticator_for_free_access()?];
    let stale_tx = TransactionEnvelope::from_user_sig_data(tx_data, signatures);
    let stale_digest = *stale_tx.digest();

    let stale_sim_id = stale_node.with(|_| iota_simulator::current_simnode_id());
    let authority_clients = test_env
        .test_cluster
        .authority_aggregator()
        .authority_clients
        .clone();
    // Submits the transaction through the validator that skips its input
    // checks, and waits for every validator to reject it on the input that
    // decided: with the "object not found" error when the input is missing,
    // and with the error for an input not available at the commit when it
    // drops. A missing verdict is not recorded as the transaction's status,
    // only sent to the queries waiting for it, so the queries start before
    // the submission.
    let submit_and_expect_rejection = |object_id: ObjectId, dropped: bool| {
        let stale_tx = stale_tx.clone();
        let authority_clients = authority_clients.clone();
        async move {
            let queries = authority_clients
                .iter()
                .map(|(validator, client)| {
                    let (validator, client) = (*validator, client.clone());
                    tokio::spawn(async move {
                        let statuses = tokio::time::timeout(
                            Duration::from_secs(60),
                            client.authority_client().get_tx_status(
                                GetTxStatusRequest {
                                    queries: vec![TxStatusQuery {
                                        transaction_digest: stale_digest,
                                        include_details: false,
                                    }],
                                },
                                Some(client_ip),
                            ),
                        )
                        .await
                        .unwrap_or_else(|_| {
                            panic!(
                                "validator {validator:?} must decide the stale transaction within \
                                 60s"
                            )
                        })
                        .unwrap();
                        (validator, statuses)
                    })
                })
                .collect::<Vec<_>>();

            register_fail_point_if("admit-without-input-checks", move || {
                iota_simulator::current_simnode_id() == stale_sim_id
            });
            let submitted = tokio::time::timeout(
                Duration::from_secs(60),
                authority_clients[&stale_validator]
                    .authority_client()
                    .submit_tx(vec![stale_tx], Some(client_ip)),
            )
            .await;
            clear_fail_point("admit-without-input-checks");
            let submitted = submitted
                .expect("the submission must be answered within 60s")
                .unwrap();
            assert!(
                matches!(
                    submitted.as_slice(),
                    [(digest, TxStatusUpdate::Submitted)] if *digest == stale_digest
                ),
                "the validator skipping its input checks must submit the stale transaction, \
                 got {submitted:?}"
            );

            for query in queries {
                let (validator, statuses) = query.await.unwrap();
                let [(digest, update)] = <[_; 1]>::try_from(statuses).unwrap_or_else(|statuses| {
                    panic!("validator {validator:?} must report one status, got {statuses:?}")
                });
                assert_eq!(digest, stale_digest);
                assert!(
                    match &update {
                        TxStatusUpdate::Rejected {
                            error: IotaError::UserInput { error },
                        } => match error {
                            UserInputError::ObjectNotFound { object_id: id, .. } => {
                                !dropped && *id == object_id
                            }
                            UserInputError::InputObjectNotAvailableAtCommit {
                                object_id: id,
                                ..
                            } => dropped && *id == object_id,
                            _ => false,
                        },
                        _ => false,
                    },
                    "expected validator {validator:?} to reject the stale transaction on \
                     {object_id}, got {update:?}"
                );
            }
        }
    };

    // Submitted while the account's creation is above the horizon: the
    // account is missing on every validator.
    submit_and_expect_rejection(aa_ref.object_id, false).await;
    assert!(
        flushed_commit() < made_account_commit + HORIZON_DISTANCE,
        "the first submission must be decided before the horizon reaches the account's creation"
    );

    // Resubmitted once the horizon is past the account's creation, and with
    // it past the receive: the received version drops on every validator.
    wait_for_commit(made_account_commit + HORIZON_DISTANCE + 5).await;
    submit_and_expect_rejection(received_coin.object_id, true).await;

    // No validator executed it, and the coin is where the receive left it.
    for validator in test_env.test_cluster.get_validator_pubkeys() {
        let executed = test_env
            .test_cluster
            .swarm
            .node(&validator)
            .unwrap()
            .get_node_handle()
            .unwrap()
            .with(|node| {
                node.state()
                    .get_transaction_cache_reader()
                    .try_get_executed_effects(&stale_digest)
                    .unwrap()
            });
        assert!(
            executed.is_none(),
            "validator {validator:?} executed the stale transaction"
        );
    }
    let coin_now = test_env
        .test_cluster
        .get_latest_object_ref(&received_coin.object_id)
        .await;
    assert!(
        coin_now.version > received_coin.version,
        "the receive must have moved the coin past the version the stale transaction named"
    );

    Ok(())
}

/// Test that a certified Abstract Account transaction is cancelled
/// post-consensus when the (shared) AA object it touches is congested.
///
/// Shared-object congestion control is forced on via protocol-config overrides:
/// with `TotalGasBudget` accounting the transaction's gas budget alone
/// overflows a per-object commit limit of 1, so it is deferred, and with zero
/// allowed deferral rounds the first deferral is turned into a cancellation.
/// Validators originally sign the transaction (authentication runs
/// pre-consensus and passes), but post-consensus the AA object is read as a
/// cancelled shared object and the transaction fails with
/// `ExecutionCancelledDueToSharedObjectCongestionV2`.
#[sim_test]
async fn test_abstract_account_shared_object_congestion_cancellation() -> Result<(), anyhow::Error>
{
    let _pcool_guard = override_pcool_flow(false);
    telemetry_subscribers::init_for_testing();
    let client_ip = SocketAddr::new([127, 0, 0, 1].into(), 0);

    // Force shared-object congestion so any transaction touching a shared object
    // is cancelled on the first deferral.
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_per_object_congestion_control_mode_for_testing(
            PerObjectCongestionControlMode::TotalGasBudget,
        );
        config.set_max_accumulated_txn_cost_per_object_in_mysticeti_commit_for_testing(1);
        config.set_max_congestion_limit_overshoot_per_commit_for_testing(0);
        config.set_max_deferral_rounds_for_congestion_control_for_testing(0);
        // Selects the V2 error (carrying `suggested_gas_price`); enabled by
        // default at the max protocol version, set explicitly for robustness.
        config.set_congestion_control_gas_price_feedback_mechanism_for_testing(true);
        config
    });

    // Build a test environment and create an abstract account
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519)
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let aa_sender = aa_ref.object_id.into();

    // Create an AA TX that touches the (shared) AA object and ask the validators
    // to sign it. Authentication runs and passes here, pre-consensus.
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
    let tx_digest = tx_data.digest().into_bytes();
    // Create the MoveAuthenticator for the Ed25519 signature authenticator
    let signatures = vec![test_env.create_move_authenticator_for_ed25519(&tx_digest)?];
    // Create the TX envelope and send it for validators signing
    let aa_simple_tx = TransactionEnvelope::from_user_sig_data(tx_data, signatures);
    let cert = test_env
        .test_cluster
        .create_certificate(aa_simple_tx, Some(client_ip))
        .await
        .unwrap();

    // Submit the certificate: consensus defers the congested transaction and,
    // with zero deferral rounds allowed, cancels it.
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

    assert!(
        summary.status.is_failure(),
        "Expected the TX execution to fail"
    );

    let (error, command) = summary.status.unwrap_err();
    assert!(
        command.is_none(),
        "Expected the congestion cancellation to carry no command index",
    );
    let ExecutionError::ExecutionCanceledDueToSharedObjectCongestionV2 {
        congested_objects, ..
    } = &error
    else {
        panic!("Expected an ExecutionCancelledDueToSharedObjectCongestionV2 error, got: {error:?}");
    };
    assert!(
        congested_objects.contains(&aa_ref.object_id),
        "Expected the AA shared object to be reported as congested, got: {congested_objects:?}",
    );

    Ok(())
}

/// Same scenario as [`test_abstract_account_post_consensus_failure`], but with
/// `report_move_authentication_error` disabled: the authenticator abort
/// surfaces as a bare Move abort attributed to the authenticator's own command
/// index 0 -- indistinguishable from a failure of the programmable
/// transaction's first command.
#[sim_test]
async fn test_abstract_account_post_consensus_failure_without_report_flag()
-> Result<(), anyhow::Error> {
    let _pcool_guard = override_pcool_flow(false);
    telemetry_subscribers::init_for_testing();
    let client_ip = SocketAddr::new([127, 0, 0, 1].into(), 0);

    // Disable reporting the authentication failure as a distinct error.
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_report_move_authentication_error_for_testing(false);
        config
    });

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
        .generate_and_add_new_key(SignatureScheme::Ed25519, None, None, None)
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
    let aa_sender = aa_ref.object_id.into();

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
    let tx_digest = tx_data.digest().into_bytes();
    // Create the MoveAuthenticator for the Ed25519 signature authenticator
    let signatures = vec![test_env.create_move_authenticator_for_ed25519(&tx_digest)?];
    // Create the TX envelope and send it for validators signing
    let aa_simple_tx = TransactionEnvelope::from_user_sig_data(tx_data, signatures);
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
    let tx_digest2 = tx_data2.digest().into_bytes();
    // Create the MoveAuthenticator for the Ed25519 signature authenticator
    let signatures2 = vec![test_env.create_move_authenticator_for_ed25519(&tx_digest2)?];
    // Create the TX envelope and send it for validators signing
    let aa_rotate_tx = TransactionEnvelope::from_user_sig_data(tx_data2, signatures2);
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

    assert!(
        summary.status.is_failure(),
        "Expected the TX execution to fail"
    );
    assert!(
        summary.gas_cost_summary.gas_used() == 3401600
            && summary.mutated_object_count == 2
            && summary.created_object_count == 0
            && summary.unwrapped_object_count == 0
            && summary.deleted_object_count == 0
            && summary.wrapped_object_count == 0,
        "Expected gas to be used in the failed transaction and that only the gas object was mutated and the TX input object was bumped in version",
    );

    let (error, command) = summary.status.unwrap_err();
    assert_eq!(
        command,
        Some(0),
        "Without the flag, the authenticator abort is reported at command index 0",
    );
    assert!(
        matches!(
            &error,
            ExecutionError::MoveAbort{location: MoveLocation { module, function_name, .. }, code: abort_code}
            if module.as_str() == "basic_keyed_aa"
            && function_name.as_ref().is_some_and(|f|f.as_str() == "authenticate_ed25519")
            && ErrorBitset::from_u64(*abort_code).unwrap().error_code() == Some(0)
        ),
        "Expected failure to be a bare Move abort in basic_keyed_aa::authenticate_ed25519, got: {error:?}",
    );

    Ok(())
}

/// The pre-consensus authentication path (`authenticate_transaction`, run when
/// a validator signs a transaction) reports a failing authenticator the same
/// way as the execution path. With `report_move_authentication_error` enabled,
/// the resulting `MoveAuthenticatorExecutionFailure` reads as a Move
/// authentication failure and carries no command index.
#[sim_test]
async fn test_pre_consensus_authentication_failure() -> Result<(), anyhow::Error> {
    let _pcool_guard = override_pcool_flow(false);
    telemetry_subscribers::init_for_testing();

    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519)
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let aa_sender = aa_ref.object_id.into();
    let rgp = test_env.test_cluster.get_reference_gas_price().await;

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

    // Sign the authenticator over a digest that does not match the transaction,
    // so `authenticate_ed25519` aborts while the validator signs (pre-consensus).
    let signatures = vec![test_env.create_move_authenticator_for_ed25519(&[0u8; 32])?];
    let tx = TransactionEnvelope::from_user_sig_data(tx_data, signatures);

    let err = test_env.handle_tx(tx).await.unwrap_err();
    let IotaError::MoveAuthenticatorExecutionFailure { error } = &err else {
        panic!("Expected MoveAuthenticatorExecutionFailure, got: {err:?}");
    };
    assert!(
        error.contains("Move authentication failed"),
        "Expected the failure to be reported as a Move authentication error, got: {error}",
    );
    assert!(
        !error.contains("command index"),
        "Expected an authentication failure to carry no command index, got: {error}",
    );

    Ok(())
}

/// Denying the package that holds the authenticate function must stop the
/// transaction, even though that package is named by neither a command nor a
/// linkage table.
///
/// The second half pins the ordering: with a signature that cannot verify, the
/// deny check has to win over the authenticator execution that would otherwise
/// report the Move abort. Reaching that abort would mean code from a denied
/// package ran before the transaction was turned away.
#[sim_test]
async fn test_authenticator_package_denied() -> Result<(), anyhow::Error> {
    let _pcool_guard = override_pcool_flow(false);
    telemetry_subscribers::init_for_testing();

    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519)
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let validator = test_env.test_cluster.get_validator_pubkeys()[0];
    let aa_sender = aa_ref.object_id.into();
    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let aa_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20000000000), aa_sender)
        .await;
    let aa_coin = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(1000000), aa_sender)
        .await;

    // The package id only exists once the package is published, which is after
    // the cluster was built, so the deny list is installed by restarting. Only
    // the validator this test queries needs it.
    test_env
        .test_cluster
        .update_transaction_deny_config_on(
            &validator,
            TransactionDenyConfigBuilder::new()
                .add_denied_package(test_env.aa_package_id.unwrap())
                .build(),
        )
        .await;

    // A plain transfer, so the account's package is named by the authenticator
    // alone. A transaction that called into that package would be turned away
    // by the command-level check even without the one under test.
    let pt = test_env.craft_object_transfer(aa_coin, test_env.owner.unwrap())?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, aa_gas, aa_sender, None)
        .await?;

    // A transaction that would otherwise succeed.
    let tx_digest = tx_data.digest().into_bytes();
    let signatures = vec![test_env.create_move_authenticator_for_ed25519(&tx_digest)?];
    let tx = TransactionEnvelope::from_user_sig_data(tx_data.clone(), signatures);

    let err = test_env
        .handle_tx_in_process(&validator, tx)
        .await
        .unwrap_err();
    let IotaError::UserInput {
        error: UserInputError::TransactionDenied { error },
    } = &err
    else {
        panic!("Expected TransactionDenied, got: {err:?}");
    };
    assert!(
        error.contains(&test_env.aa_package_id.unwrap().to_string()),
        "Expected the denied authenticator package to be named, got: {error}",
    );

    // A signature over the wrong digest, which `authenticate_ed25519` would
    // abort on. The deny check must answer first.
    let signatures = vec![test_env.create_move_authenticator_for_ed25519(&[0u8; 32])?];
    let tx = TransactionEnvelope::from_user_sig_data(tx_data, signatures);

    let err = test_env
        .handle_tx_in_process(&validator, tx)
        .await
        .unwrap_err();
    assert!(
        matches!(
            &err,
            IotaError::UserInput {
                error: UserInputError::TransactionDenied { .. }
            }
        ),
        "Expected the deny check to precede authenticator execution, got: {err:?}",
    );

    Ok(())
}

/// Same scenario as [`test_authenticator_package_denied`] with
/// `deny_authenticator_packages` disabled: the transaction goes through, which
/// is both the pre-37 behaviour and the proof that the other test's rejection
/// comes from the check under test rather than from somewhere else.
#[sim_test]
async fn test_authenticator_package_denied_without_flag() -> Result<(), anyhow::Error> {
    let _pcool_guard = override_pcool_flow(false);
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_deny_authenticator_packages_for_testing(false);
        config
    });

    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519)
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let validator = test_env.test_cluster.get_validator_pubkeys()[0];
    let aa_sender = aa_ref.object_id.into();
    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let aa_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20000000000), aa_sender)
        .await;
    let aa_coin = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(1000000), aa_sender)
        .await;

    test_env
        .test_cluster
        .update_transaction_deny_config_on(
            &validator,
            TransactionDenyConfigBuilder::new()
                .add_denied_package(test_env.aa_package_id.unwrap())
                .build(),
        )
        .await;

    let pt = test_env.craft_object_transfer(aa_coin, test_env.owner.unwrap())?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, aa_gas, aa_sender, None)
        .await?;
    let tx_digest = tx_data.digest().into_bytes();
    let signatures = vec![test_env.create_move_authenticator_for_ed25519(&tx_digest)?];
    let tx = TransactionEnvelope::from_user_sig_data(tx_data, signatures);

    test_env.handle_tx_in_process(&validator, tx).await?;

    Ok(())
}

/// Same scenario as [`test_pre_consensus_authentication_failure`], but with
/// `report_move_authentication_error` disabled: the pre-consensus path must
/// still gate on the flag and surface the authenticator's bare Move abort,
/// keeping the (misleading) command index 0.
#[sim_test]
async fn test_pre_consensus_authentication_failure_without_report_flag() -> Result<(), anyhow::Error>
{
    let _pcool_guard = override_pcool_flow(false);
    telemetry_subscribers::init_for_testing();

    // Disable reporting the authentication failure as a distinct error.
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_report_move_authentication_error_for_testing(false);
        config
    });

    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519)
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let aa_sender = aa_ref.object_id.into();
    let rgp = test_env.test_cluster.get_reference_gas_price().await;

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

    // Sign the authenticator over a digest that does not match the transaction,
    // so `authenticate_ed25519` aborts while the validator signs (pre-consensus).
    let signatures = vec![test_env.create_move_authenticator_for_ed25519(&[0u8; 32])?];
    let tx = TransactionEnvelope::from_user_sig_data(tx_data, signatures);

    let err = test_env.handle_tx(tx).await.unwrap_err();
    let IotaError::MoveAuthenticatorExecutionFailure { error } = &err else {
        panic!("Expected MoveAuthenticatorExecutionFailure, got: {err:?}");
    };
    // The flag gates the pre-consensus path too: the abort is reported bare,
    // keeping the authenticator's internal command index 0 and no wrapping.
    assert!(
        error.contains("command index: 0"),
        "Without the flag, the authenticator abort must keep command index 0, got: {error}",
    );
    assert!(
        !error.contains("Move authentication failed"),
        "Without the flag, the failure must not be wrapped as an authentication error, got: {error}",
    );

    Ok(())
}

/// Test in 3 steps
/// 1) Create a valid TX1 certificate signed by validators where sender is an AA account using a
///    owned Coin as gas
/// 2) Tamper with the AA shared object by creating a second TX2, with sender being a random Bob
///    address, altering the state calling the “receive“ function for the Coin used as gas in TX1
/// 3) Submit the original certificate TX1 which should NOT fail during post-consensus, because
///    validators originally run the authenticate and it passed. What fails is the execution of TX2
///    because of the conflict on the
///   receiving object
#[sim_test]
async fn test_receiving_gas_executing_aa_tx_first() -> Result<(), anyhow::Error> {
    let _pcool_guard = override_pcool_flow(false);
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
    let aa_sender: Address = aa_ref.object_id.into();

    // Retrieve the keystore and setup secondary random account (Bob)
    let bob = {
        let keystore = test_env.test_cluster.wallet.config_mut().keystore_mut();
        keystore
            .generate_and_add_new_key(SignatureScheme::Ed25519, None, None, None)
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
    let tx1 = TransactionEnvelope::from_user_sig_data(tx1_data, signatures);
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
        effects_cert.summary_for_debug().status.is_failure(),
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
        effects_cert.summary_for_debug().status.is_success(),
        "Expected the TX execution to succeed"
    );

    Ok(())
}

/// Test in 4 steps:
/// 1) Create TX1 where Bob calls the receiving function on a coin owned by an AA account.
/// 2) Create TX2 where the AA sender tries to use the conflict coin as input.
/// 3) Submit the original TX1 certificate. This fails with an execution abort.
/// 4) Submit the original TX2 certificate. This should now succeed.
#[sim_test]
async fn test_receiving_gas_executing_aa_tx_later() -> Result<(), anyhow::Error> {
    let _pcool_guard = override_pcool_flow(false);
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
    let aa_sender: Address = aa_ref.object_id.into();

    // Retrieve the keystore and setup secondary random account (Bob)
    let bob = {
        let keystore = test_env.test_cluster.wallet.config_mut().keystore_mut();
        keystore
            .generate_and_add_new_key(SignatureScheme::Ed25519, None, None, None)
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
    let pt2 = test_env.craft_object_transfer(conflict_coin_ref, Address::ZERO)?;
    let tx2_data = test_env
        .craft_tx_from_pt(pt2, second_gas_coin, aa_sender, None)
        .await?;
    // Create the MoveAuthenticator for the free access authenticator
    let signatures = vec![test_env.create_move_authenticator_for_free_access()?];
    // Create the TX envelope and send it for validators signing
    let tx2 = TransactionEnvelope::from_user_sig_data(tx2_data, signatures);
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
        summary.status.is_failure(),
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
        summary.status.is_success(),
        "Expected the TX2 execution to succeed"
    );

    Ok(())
}

/// Test in 5 steps:
/// 1) Create TX1 where Bob calls the receiving function on a coin owned by the AA object (before
///    the AA account is actually created). The AA object is NOT an account yet (just a shared
///    object).
/// 2) Make the AA become the actual account (delayed AA creation).
/// 3) Create TX2 where the AA sender tries to use the conflict coin as input.
/// 4) Submit the original TX1 certificate. This fails with an execution abort.
/// 5) Submit the original TX2 certificate. This should now succeed.
#[sim_test]
async fn test_failing_receiving_gas_then_create_account() -> Result<(), anyhow::Error> {
    let _pcool_guard = override_pcool_flow(false);
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
    let aa_sender: Address = aa_ref.object_id.into();

    // Retrieve the keystore and setup secondary random account (Bob)
    let bob = {
        let keystore = test_env.test_cluster.wallet.config_mut().keystore_mut();
        keystore
            .generate_and_add_new_key(SignatureScheme::Ed25519, None, None, None)
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
        effects.status().is_success(),
        "Expected make_delayed_abstract_account to succeed, got: {:?}",
        effects.status()
    );

    // Step 3: create a TX2 which uses the conflict Coin owned by the AA as gas
    let pt2 = test_env.craft_object_transfer(conflict_coin_ref, Address::ZERO)?;
    let tx2_data = test_env
        .craft_tx_from_pt(pt2, second_gas_coin, aa_sender, None)
        .await?;
    // Create the MoveAuthenticator for the free access authenticator
    let signatures = vec![test_env.create_move_authenticator_for_free_access()?];
    // Create the TX envelope and send it for validators signing
    let tx2 = TransactionEnvelope::from_user_sig_data(tx2_data, signatures);
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
        summary.status.is_failure(),
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
        summary.status.is_success(),
        "Expected the TX2 execution to succeed"
    );

    Ok(())
}

/// Test in 4 steps:
/// 1) Create TX1 where Bob calls the receiving function on a coin owned by the AA object (before
///    the AA account is actually created). The AA object is NOT an account yet (just a shared
///    object).
/// 2) Submit the original TX1 certificate. This is successful because the AA is not yet an account.
/// 3) Make the AA become the actual account (delayed AA creation).
/// 4) Create and submit a TX2 where the AA sender tries to use the conflict coin using the latest
///    reference, this should now succeed.
#[sim_test]
async fn test_successful_receiving_gas_then_create_account() -> Result<(), anyhow::Error> {
    let _pcool_guard = override_pcool_flow(false);
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
    let aa_sender: Address = aa_ref.object_id.into();

    // Retrieve the keystore and setup secondary random account (Bob)
    let bob = {
        let keystore = test_env.test_cluster.wallet.config_mut().keystore_mut();
        keystore
            .generate_and_add_new_key(SignatureScheme::Ed25519, None, None, None)
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
        summary.status.is_success(),
        "Expected the TX1 execution to succeed"
    );
    let conflict_coin_ref = *effects_cert
        .all_changed_objects()
        .iter()
        .find(|(changed, _)| changed.reference().object_id == conflict_coin_ref.object_id)
        .expect("Expected to find the updated conflict coin object")
        .0
        .reference();

    // Step 3: create the AA account (from the delayed abstract account object)
    let effects = test_env.make_delayed_abstract_account().await?;
    assert!(
        effects.status().is_success(),
        "Expected make_delayed_abstract_account to succeed, got: {:?}",
        effects.status()
    );

    // Step 4: create a TX2 which uses the conflict Coin owned by the AA as gas
    let pt2 = test_env.craft_object_transfer(conflict_coin_ref, Address::ZERO)?;
    let tx2_data = test_env
        .craft_tx_from_pt(pt2, second_gas_coin, aa_sender, None)
        .await?;
    // Create the MoveAuthenticator for the free access authenticator
    let signatures = vec![test_env.create_move_authenticator_for_free_access()?];
    // Create the TX envelope and send it for validators signing
    let tx2 = TransactionEnvelope::from_user_sig_data(tx2_data, signatures);
    // Submit TX2 for execution and expect success
    test_env.execute_and_check_tx_correctness(tx2).await
}

// ----------------------------------------------------------
// ----------- Sponsor Move authentication tests ------------
// ----------------------------------------------------------

/// A sponsored TX where both the sender (AA) and the sponsor (AA) carry a
/// MoveAuthenticator must succeed(enable_move_authentication_for_sponsor =
/// true).
#[sim_test]
async fn test_aa_sender_and_aa_sponsor_succeeded_with_enabled_move_auth_for_sponsor_pre_consensus_flow()
-> Result<(), anyhow::Error> {
    test_aa_sender_and_aa_sponsor_succeeded_with_enabled_move_auth_for_sponsor(false).await
}

#[sim_test]
async fn test_aa_sender_and_aa_sponsor_succeeded_with_enabled_move_auth_for_sponsor_pcool_flow()
-> Result<(), anyhow::Error> {
    test_aa_sender_and_aa_sponsor_succeeded_with_enabled_move_auth_for_sponsor(true).await
}

async fn test_aa_sender_and_aa_sponsor_succeeded_with_enabled_move_auth_for_sponsor(
    pcool: bool,
) -> Result<(), anyhow::Error> {
    let _pcool_guard = override_pcool_flow(pcool);
    telemetry_subscribers::init_for_testing();

    // Build the test environment and create the sender AA.
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519)
        .await?;
    let sender_aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: Address = sender_aa_ref.object_id.into();

    // Create a second AA that will act as the sponsor.
    let sponsor_aa_ref = test_env.create_extra_abstract_account().await?;
    let sponsor_addr: Address = sponsor_aa_ref.object_id.into();

    // Fund the sponsor AA so it can provide gas.
    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let sponsor_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), sponsor_addr)
        .await;

    // Build a simple sponsored PTB: sender = AA, sponsor = AA.
    let pt = test_env.craft_aa_simple_ptb(AA_MODULE_NAME)?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, sponsor_gas, aa_sender, Some(sponsor_addr))
        .await?;
    let tx_digest = tx_data.digest().into_bytes();

    // Both sender and sponsor provide MoveAuthenticators.
    let sender_aa_sig = test_env.create_move_authenticator_for_ed25519(&tx_digest)?;
    let sponsor_aa_sig =
        test_env.create_move_authenticator_for_ed25519_for_ref(sponsor_aa_ref, &tx_digest)?;
    let tx = TransactionEnvelope::from_user_sig_data(tx_data, vec![sender_aa_sig, sponsor_aa_sig]);

    // The TX must succeed with both AA sender and AA sponsor.
    test_env.execute_and_check_tx_correctness(tx).await
}

/// A sponsored TX where the sender is a regular account but the sponsor carries
/// a MoveAuthenticator must succeed(enable_move_authentication_for_sponsor
/// = true).
#[sim_test]
async fn test_sponsor_only_move_auth_succeeded_with_enabled_move_auth_for_sponsor()
-> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    // Build the test environment; the AA here will be the *sponsor*, not the
    // sender.
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_FREE_ACCESS)
        .await?;
    let sponsor_aa_ref = test_env.aa_ref.unwrap();
    let sponsor_addr: Address = sponsor_aa_ref.object_id.into();

    // The sender is a regular IOTA account from the keystore.
    let sender = test_env
        .test_cluster
        .wallet
        .config()
        .keystore()
        .addresses()
        .first()
        .cloned()
        .unwrap();

    // Fund the sponsor AA so it can provide gas.
    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let sponsor_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), sponsor_addr)
        .await;

    // Build a sponsored PTB: sender = regular account, sponsor = AA.
    let mut builder = ProgrammableTransactionBuilder::new();
    builder.transfer_iota(sender, None);
    let pt = builder.finish();
    let tx_data = test_env
        .craft_tx_from_pt(pt, sponsor_gas, sender, Some(sponsor_addr))
        .await?;

    // Sender signs with a regular key; sponsor provides a MoveAuthenticator.
    let sender_sig = UserSignature::Simple(
        test_env
            .test_cluster
            .wallet
            .config()
            .keystore()
            .sign_secure(&sender, &tx_data, Intent::iota_transaction())?,
    );
    let sponsor_aa_sig =
        test_env.create_move_authenticator_for_free_access_for_ref(sponsor_aa_ref)?;
    let tx = TransactionEnvelope::from_user_sig_data(tx_data, vec![sender_sig, sponsor_aa_sig]);

    // The TX must succeed when the sender is a regular account and AA sponsor.
    test_env.execute_and_check_tx_correctness(tx).await
}

/// A sponsored TX where both the sender (AA) and the sponsor (AA) carry a
/// MoveAuthenticator and use the same shared object must
/// succeed(enable_move_authentication_for_sponsor = true).
#[sim_test]
async fn test_aa_sender_and_aa_sponsor_use_the_same_shared_object_succeeded_with_enabled_move_auth_for_sponsor_pre_consensus_flow()
-> Result<(), anyhow::Error> {
    test_aa_sender_and_aa_sponsor_use_the_same_shared_object_succeeded_with_enabled_move_auth_for_sponsor(false).await
}

#[sim_test]
async fn test_aa_sender_and_aa_sponsor_use_the_same_shared_object_succeeded_with_enabled_move_auth_for_sponsor_pcool_flow()
-> Result<(), anyhow::Error> {
    test_aa_sender_and_aa_sponsor_use_the_same_shared_object_succeeded_with_enabled_move_auth_for_sponsor(true).await
}

async fn test_aa_sender_and_aa_sponsor_use_the_same_shared_object_succeeded_with_enabled_move_auth_for_sponsor(
    pcool: bool,
) -> Result<(), anyhow::Error> {
    let _pcool_guard = override_pcool_flow(pcool);
    telemetry_subscribers::init_for_testing();

    // Build the test environment and create the sender AA.
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519)
        .await?;
    let sender_aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: Address = sender_aa_ref.object_id.into();

    // Create a second AA that will act as the sponsor.
    let sponsor_aa_ref = test_env
        .create_extra_abstract_account_with(AA_AUTHENTICATE_FN_NAME_WITH_SPONSOR_AND_SENDER)
        .await?;
    let sponsor_addr: Address = sponsor_aa_ref.object_id.into();

    // Fund the sponsor AA so it can provide gas.
    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let sponsor_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), sponsor_addr)
        .await;

    // Build a simple sponsored PTB.
    let pt = test_env.craft_aa_simple_ptb(AA_MODULE_NAME)?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, sponsor_gas, aa_sender, Some(sponsor_addr))
        .await?;
    let tx_digest = tx_data.digest().into_bytes();

    // Both sender and sponsor provide MoveAuthenticators.
    let sender_aa_sig = test_env.create_move_authenticator_for_ed25519(&tx_digest)?;
    // The sender object is used in both MoveAuthenticators.
    let sponsor_aa_sig =
        test_env.create_move_authenticator_with_sponsor_and_sender(sponsor_aa_ref)?;
    let tx = TransactionEnvelope::from_user_sig_data(tx_data, vec![sender_aa_sig, sponsor_aa_sig]);

    // The TX must succeed with both AA sender and AA sponsor.
    test_env.execute_and_check_tx_correctness(tx).await
}

/// A sponsored TX where both sender (AA) and sponsor (AA) each carry a
/// MoveAuthenticator must be rejected because having more than one
/// MoveAuthenticator is not supported(enable_move_authentication_for_sponsor =
/// false).
#[sim_test]
async fn test_two_move_authenticators_rejected_with_disabled_move_auth_for_sponsor()
-> Result<(), anyhow::Error> {
    let _pcool_guard = override_pcool_flow(false);
    telemetry_subscribers::init_for_testing();

    // Disable Move authentication for the sponsor.
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_move_authentication_for_sponsor_for_testing(false);
        config.set_pre_consensus_sponsor_only_move_authentication_for_testing(false);
        config
    });

    // Build the test environment and create the sender AA.
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_FREE_ACCESS)
        .await?;
    let sender_aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: Address = sender_aa_ref.object_id.into();

    // Create a second AA that will act as the sponsor.
    let sponsor_aa_ref = test_env.create_extra_abstract_account().await?;
    let sponsor_addr: Address = sponsor_aa_ref.object_id.into();

    // Fund the sponsor AA so it can provide gas.
    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let sponsor_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), sponsor_addr)
        .await;

    // Build a simple sponsored PTB: sender = AA, sponsor = AA.
    let pt = test_env.craft_aa_simple_ptb(AA_MODULE_NAME)?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, sponsor_gas, aa_sender, Some(sponsor_addr))
        .await?;

    // Both sender and sponsor provide MoveAuthenticators.
    let sender_aa_sig = test_env.create_move_authenticator_for_free_access()?;
    let sponsor_aa_sig =
        test_env.create_move_authenticator_for_free_access_for_ref(sponsor_aa_ref)?;
    let tx = TransactionEnvelope::from_user_sig_data(tx_data, vec![sender_aa_sig, sponsor_aa_sig]);

    // The TX must be rejected: >1 MoveAuthenticator is not allowed.
    let err = test_env.handle_tx(tx).await.unwrap_err();

    assert!(
        matches!(
            &err,
            IotaError::UserInput {
                error: UserInputError::Unsupported(msg)
            } if msg == "SenderSignedTransaction with more than one MoveAuthenticator is not supported"
        ),
        "Expected Unsupported error for >1 MoveAuthenticator, got: {err:?}"
    );

    Ok(())
}

/// A sponsored TX where the sender is a regular account but the sponsor carries
/// a MoveAuthenticator must be rejected because MoveAuthenticator is only
/// allowed for the sender(enable_move_authentication_for_sponsor = false).
#[sim_test]
async fn test_sponsor_only_move_auth_rejected_with_disabled_move_auth_for_sponsor()
-> Result<(), anyhow::Error> {
    let _pcool_guard = override_pcool_flow(false);
    telemetry_subscribers::init_for_testing();

    // Disable Move authentication for the sponsor.
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_move_authentication_for_sponsor_for_testing(false);
        config.set_pre_consensus_sponsor_only_move_authentication_for_testing(false);
        config
    });

    // Build the test environment; the AA here will be the *sponsor*, not the
    // sender.
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_FREE_ACCESS)
        .await?;
    let sponsor_aa_ref = test_env.aa_ref.unwrap();
    let sponsor_addr: Address = sponsor_aa_ref.object_id.into();

    // The sender is a regular IOTA account from the keystore.
    let sender = test_env
        .test_cluster
        .wallet
        .config()
        .keystore()
        .addresses()
        .first()
        .cloned()
        .unwrap();

    // Fund the sponsor AA so it can provide gas.
    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let sponsor_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), sponsor_addr)
        .await;

    // Build a sponsored PTB: sender = regular account, sponsor = AA.
    let mut builder = ProgrammableTransactionBuilder::new();
    builder.transfer_iota(sender, None);
    let pt = builder.finish();
    let tx_data = test_env
        .craft_tx_from_pt(pt, sponsor_gas, sender, Some(sponsor_addr))
        .await?;

    // Sender signs with a regular key; sponsor provides a MoveAuthenticator.
    let sender_sig = UserSignature::Simple(
        test_env
            .test_cluster
            .wallet
            .config()
            .keystore()
            .sign_secure(&sender, &tx_data, Intent::iota_transaction())?,
    );
    let sponsor_aa_sig =
        test_env.create_move_authenticator_for_free_access_for_ref(sponsor_aa_ref)?;
    let tx = TransactionEnvelope::from_user_sig_data(tx_data, vec![sender_sig, sponsor_aa_sig]);

    // The TX must be rejected: the single MoveAuthenticator belongs to the
    // sponsor, not the sender, which is not allowed.
    let err = test_env.handle_tx(tx).await.unwrap_err();

    assert!(
        matches!(
            &err,
            IotaError::UserInput {
                error: UserInputError::Unsupported(msg)
            } if msg == "SenderSignedTransaction can have MoveAuthenticator only for the sender"
        ),
        "Expected Unsupported error for sponsor-only MoveAuthenticator, got: {err:?}"
    );

    Ok(())
}

/// A sponsored TX where one MoveAuthenticator is for a third AA (neither
/// sender nor sponsor) must be rejected(enable_move_authentication_for_sponsor
/// = true).
#[sim_test]
async fn test_wrong_signer_move_auth_rejected_with_enabled_move_auth_for_sponsor()
-> Result<(), anyhow::Error> {
    let _pcool_guard = override_pcool_flow(false);
    telemetry_subscribers::init_for_testing();

    // Build the test environment and create the sender AA.
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_FREE_ACCESS)
        .await?;
    let sender_aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: Address = sender_aa_ref.object_id.into();

    // Create a second AA that will act as the sponsor.
    let sponsor_aa_ref = test_env.create_extra_abstract_account().await?;
    let sponsor_addr: Address = sponsor_aa_ref.object_id.into();

    // Create a third AA that is unrelated to this transaction.
    let unrelated_aa_ref = test_env.create_extra_abstract_account().await?;

    // Fund the sponsor AA so it can provide gas.
    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let sponsor_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), sponsor_addr)
        .await;

    // Build a simple sponsored PTB: sender = AA, sponsor = AA.
    let pt = test_env.craft_aa_simple_ptb(AA_MODULE_NAME)?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, sponsor_gas, aa_sender, Some(sponsor_addr))
        .await?;

    // Sender provides a valid MoveAuthenticator; sponsor provides one for an
    // unrelated AA instead of its own.
    let sender_aa_sig = test_env.create_move_authenticator_for_free_access()?;
    let unrelated_aa_sig =
        test_env.create_move_authenticator_for_free_access_for_ref(unrelated_aa_ref)?;
    let tx =
        TransactionEnvelope::from_user_sig_data(tx_data, vec![sender_aa_sig, unrelated_aa_sig]);

    // The TX must be rejected: the sponsor's signature is absent.
    let err = test_env.handle_tx(tx).await.unwrap_err();

    assert!(
        matches!(&err, IotaError::SignerSignatureAbsent { .. }),
        "Expected SignerSignatureAbsent for wrong signer MoveAuthenticator, got: {err:?}"
    );

    Ok(())
}

/// A sponsored TX where both the sender (AA) and the sponsor (AA) carry a
/// MoveAuthenticator must succeed(enable_move_authentication_for_sponsor =
/// true), but the sponsor authenticator fails.
#[sim_test]
async fn test_aa_sender_and_aa_sponsor_rejected_when_sponsor_aa_fails_with_enabled_move_auth_for_sponsor()
-> Result<(), anyhow::Error> {
    let _pcool_guard = override_pcool_flow(false);
    telemetry_subscribers::init_for_testing();

    // Build the test environment and create the sender AA.
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_FREE_ACCESS)
        .await?;
    let sender_aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: Address = sender_aa_ref.object_id.into();

    // Create a second AA that will act as the sponsor.
    let sponsor_aa_ref = test_env.create_extra_abstract_account().await?;
    let sponsor_addr: Address = sponsor_aa_ref.object_id.into();

    // Fund the sponsor AA so it can provide gas.
    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let sponsor_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), sponsor_addr)
        .await;

    // Build a simple sponsored PTB: sender = AA, sponsor = AA.
    let pt = test_env.craft_aa_simple_ptb(AA_MODULE_NAME)?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, sponsor_gas, aa_sender, Some(sponsor_addr))
        .await?;
    let tx_digest = tx_data.digest().into_bytes();

    // Both sender and sponsor provide MoveAuthenticators.
    let sender_aa_sig = test_env.create_move_authenticator_for_free_access()?;
    // But the sponsor's signature is for ed25519 authentication, which does not
    // match the sponsor AA's actual free access authenticator.
    let sponsor_aa_sig =
        test_env.create_move_authenticator_for_ed25519_for_ref(sponsor_aa_ref, &tx_digest)?;
    let tx = TransactionEnvelope::from_user_sig_data(tx_data, vec![sender_aa_sig, sponsor_aa_sig]);

    // The TX must be rejected: the sponsor's signature is incorrect.
    let err = test_env.handle_tx(tx).await.unwrap_err();

    assert!(
        matches!(&err, IotaError::MoveAuthenticatorExecutionFailure { .. }),
        "Expected MoveAuthenticatorExecutionFailure for wrong sponsor MoveAuthenticator, got: {err:?}"
    );

    Ok(())
}

// ------------------------------------------------------------------
// --- pre_consensus_sponsor_only_move_authentication flag tests  ---
// ------------------------------------------------------------------

/// With `pre_consensus_sponsor_only_move_authentication` enabled, only the
/// sponsor's MA runs pre-consensus for a sponsored TX. A sender MA signed over
/// the wrong digest is therefore accepted pre-consensus (the sponsor's
/// free-access MA passes) but fails post-consensus when the sender's MA is
/// finally executed and the signature doesn't match.
#[sim_test]
async fn test_sponsored_tx_sender_aa_fails_post_consensus_when_only_sponsor_runs_pre_consensus()
-> Result<(), anyhow::Error> {
    let _pcool_guard = override_pcool_flow(false);
    telemetry_subscribers::init_for_testing();
    let client_ip = SocketAddr::new([127, 0, 0, 1].into(), 0);

    // Enable the flag so only the sponsor's MA runs pre-consensus.
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_pre_consensus_sponsor_only_move_authentication_for_testing(true);
        config
    });

    // Sender is an AA with ED25519 authentication; sponsor is an AA with
    // free-access authentication.
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519)
        .await?;
    let sender_aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: Address = sender_aa_ref.object_id.into();

    let sponsor_aa_ref = test_env
        .create_extra_abstract_account_with(AA_AUTHENTICATE_FN_NAME_FREE_ACCESS)
        .await?;
    let sponsor_addr: Address = sponsor_aa_ref.object_id.into();

    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let sponsor_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), sponsor_addr)
        .await;

    let pt = test_env.craft_aa_simple_ptb(AA_MODULE_NAME)?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, sponsor_gas, aa_sender, Some(sponsor_addr))
        .await?;

    // Sender's MA is signed over the wrong digest — it will fail post-consensus.
    let wrong_digest = [0u8; 32];
    let sender_aa_sig = test_env.create_move_authenticator_for_ed25519(&wrong_digest)?;
    let sponsor_aa_sig =
        test_env.create_move_authenticator_for_free_access_for_ref(sponsor_aa_ref)?;
    let tx = TransactionEnvelope::from_user_sig_data(tx_data, vec![sender_aa_sig, sponsor_aa_sig]);

    // Pre-consensus: only the sponsor's MA is executed (free-access → passes).
    // The validator must sign the TX, producing a certificate.
    let cert = test_env
        .test_cluster
        .create_certificate(tx, Some(client_ip))
        .await?;

    // Post-consensus: both MAs are executed → sender's ED25519 MA fails.
    let QuorumDriverResponse { effects_cert, .. } = test_env
        .test_cluster
        .authority_aggregator()
        .process_certificate(
            HandleCertificateRequestV1::new(cert).with_events(),
            Some(client_ip),
        )
        .await?;

    let summary = effects_cert.summary_for_debug();
    assert!(
        summary.status.is_failure(),
        "Expected TX to fail post-consensus due to the sender's MA failure"
    );
    let (error, _) = summary.status.unwrap_err();
    assert!(
        matches!(
            &error,
            ExecutionError::MoveAuthentication { error }
            if matches!(&**error, ExecutionError::MoveAbort { .. })
        ),
        "Expected a Move authentication error wrapping the failed ED25519 authentication's abort, got: {error:?}"
    );

    // Even though the TX failed, the sponsor must have paid gas. Verify that
    // computation was charged and that the correct gas object (the sponsor's coin)
    // was debited.
    assert!(
        summary.gas_cost_summary.computation_cost > 0,
        "Expected computation cost > 0: the sponsor must pay gas even for a post-consensus failure"
    );
    assert_eq!(
        effects_cert.data().gas_object().reference().object_id,
        sponsor_gas.object_id,
        "Expected the sponsor's gas coin to be used for the failed TX"
    );

    Ok(())
}

/// With `pre_consensus_sponsor_only_move_authentication` disabled, both the
/// sender's and the sponsor's MAs run pre-consensus. A sender MA signed over
/// the wrong digest is therefore rejected immediately, before consensus.
#[sim_test]
async fn test_sponsored_tx_sender_aa_rejected_pre_consensus_without_sponsor_only_flag()
-> Result<(), anyhow::Error> {
    let _pcool_guard = override_pcool_flow(false);
    telemetry_subscribers::init_for_testing();

    // Disable the flag so ALL MAs run pre-consensus.
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_pre_consensus_sponsor_only_move_authentication_for_testing(false);
        config
    });

    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519)
        .await?;
    let sender_aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: Address = sender_aa_ref.object_id.into();

    let sponsor_aa_ref = test_env
        .create_extra_abstract_account_with(AA_AUTHENTICATE_FN_NAME_FREE_ACCESS)
        .await?;
    let sponsor_addr: Address = sponsor_aa_ref.object_id.into();

    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let sponsor_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), sponsor_addr)
        .await;

    let pt = test_env.craft_aa_simple_ptb(AA_MODULE_NAME)?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, sponsor_gas, aa_sender, Some(sponsor_addr))
        .await?;

    let wrong_digest = [0u8; 32];
    let sender_aa_sig = test_env.create_move_authenticator_for_ed25519(&wrong_digest)?;
    let sponsor_aa_sig =
        test_env.create_move_authenticator_for_free_access_for_ref(sponsor_aa_ref)?;
    let tx = TransactionEnvelope::from_user_sig_data(tx_data, vec![sender_aa_sig, sponsor_aa_sig]);

    // Pre-consensus: both MAs are executed → sender's MA fails → rejected
    // immediately.
    let err = test_env.handle_tx(tx).await.unwrap_err();

    assert!(
        matches!(&err, IotaError::MoveAuthenticatorExecutionFailure { .. }),
        "Expected MoveAuthenticatorExecutionFailure from the sender's MA, got: {err:?}"
    );

    Ok(())
}

/// With `pre_consensus_sponsor_only_move_authentication` enabled, the flag
/// only skips the sender's MA pre-consensus check for *sponsored* transactions.
/// For non-sponsored transactions, the sender's MA still runs pre-consensus, so
/// a bad sender MA is still rejected before consensus.
#[sim_test]
async fn test_non_sponsored_tx_sender_aa_rejected_pre_consensus_with_sponsor_only_flag()
-> Result<(), anyhow::Error> {
    let _pcool_guard = override_pcool_flow(false);
    telemetry_subscribers::init_for_testing();

    // Enable the flag; it must not affect non-sponsored transactions.
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_pre_consensus_sponsor_only_move_authentication_for_testing(true);
        config
    });

    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519)
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: Address = aa_ref.object_id.into();

    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let aa_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    // Non-sponsored TX — sender pays its own gas.
    let pt = test_env.craft_aa_simple_ptb(AA_MODULE_NAME)?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, aa_gas, aa_sender, None)
        .await?;

    // Sender's MA is signed over the wrong digest.
    let wrong_digest = [0u8; 32];
    let sender_aa_sig = test_env.create_move_authenticator_for_ed25519(&wrong_digest)?;
    let tx = TransactionEnvelope::from_user_sig_data(tx_data, vec![sender_aa_sig]);

    // Pre-consensus: non-sponsored TX → sender's MA always runs pre-consensus even
    // with the sponsor-only flag → sender's MA fails → rejected immediately.
    let err = test_env.handle_tx(tx).await.unwrap_err();

    assert!(
        matches!(&err, IotaError::MoveAuthenticatorExecutionFailure { .. }),
        "Expected MoveAuthenticatorExecutionFailure for non-sponsored TX, got: {err:?}"
    );

    Ok(())
}

// ---------------------------------------------------
// --- Test Environment for Abstract Account tests ---
// ---------------------------------------------------

/// Test environment for Abstract Account tests
struct TestEnvironment {
    test_cluster: TestCluster,
    owner: Option<Address>,
    authenticate_fn_name: Option<String>,
    aa_package_id: Option<ObjectId>,
    aa_package_metadata_ref: Option<ObjectReference>,
    aa_ref: Option<ObjectReference>,
    aa_create_transaction: Option<TransactionEnvelope>,
}

impl TestEnvironment {
    async fn new() -> Self {
        Self::with_cluster(TestClusterBuilder::new().build().await)
    }

    /// Builds the environment on a cluster whose fullnode keeps every object
    /// version.
    ///
    /// Reading a transaction response that carries balance or object changes
    /// resolves the transaction's input objects at their previous version, and
    /// the fullnode prunes those versions once their checkpoint is executed.
    async fn new_with_unpruned_fullnode() -> Self {
        Self::with_cluster(
            TestClusterBuilder::new()
                .disable_fullnode_pruning()
                .build()
                .await,
        )
    }

    fn with_cluster(test_cluster: TestCluster) -> Self {
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
                .map(|(changed, kind)| {
                    (
                        OwnedObjectReference::new(changed.reference, changed.owner),
                        *kind,
                    )
                })
                .collect::<Vec<_>>(),
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
    async fn publish_account_abstraction_package(&mut self) -> (ObjectId, ObjectReference) {
        let path = [env!("CARGO_MANIFEST_DIR"), AA_PACKAGE_PATH]
            .iter()
            .collect();
        let aa_package_id = publish_package(self.test_cluster.wallet(), path)
            .await
            .object_id;

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

        self.create_abstract_account_with(
            owner,
            authenticate_fn_name,
            aa_package_id,
            aa_package_metadata_ref,
        )
        .await
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
                Identifier::from_static(AA_DELAYED_CREATE_MODULE_NAME),
                Identifier::from_static("create"),
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
                builder.obj(CallArg::ImmutableOrOwned(aa_package_metadata_ref))?,
                builder.pure(AA_DELAYED_AUTHENTICATE_MODULE_NAME)?,
                builder.pure(authenticate_fn_name)?,
            ];
            if let Argument::Result(authenticator_function_ref_v1) = builder.programmable_move_call(
                IOTA_FRAMEWORK_PACKAGE_ID,
                Identifier::from_static("authenticator_function"),
                Identifier::from_static("create_auth_function_ref_v1"),
                vec![delayed_abstract_account_type_tag(&aa_package_id)],
                arguments,
            ) {
                // Create the delayed abstract account.
                let arguments = vec![
                    builder.obj(CallArg::Shared(SharedObjectReference::new(
                        delayed_aa_ref.object_id,
                        delayed_aa_ref.version,
                        true,
                    )))?,
                    builder.pure(aa_owner_pk.as_ref())?,
                    Argument::Result(authenticator_function_ref_v1),
                ];
                builder.programmable_move_call(
                    aa_package_id,
                    Identifier::from_static(AA_DELAYED_AUTHENTICATE_MODULE_NAME),
                    Identifier::from_static("create"),
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
    ) -> anyhow::Result<(DryRunTransactionBlockResponse, TransactionEnvelope)> {
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
                AA_CREATE_FN_NAME,
                aa_package_id,
                aa_package_metadata_ref,
            )
            .await?;

        let dry_run_res = self
            .test_cluster
            .iota_client()
            .read_api()
            .dry_run_transaction_block(transaction.transaction().clone())
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
    ) -> anyhow::Result<UserSignature> {
        let Some(aa_ref) = self.aa_ref else {
            anyhow::bail!("Abstract account not created yet");
        };

        self.create_move_authenticator_for_ed25519_for_ref(aa_ref, tx_digest)
    }

    // Create the MoveAuthenticator for the free access authenticator:
    // public fun authenticate_free_access(
    //    self: &AbstractAccount,
    //    _: &AuthContext,
    //    ctx: &TxContext,
    fn create_move_authenticator_for_free_access(&self) -> anyhow::Result<UserSignature> {
        let Some(aa_ref) = self.aa_ref else {
            anyhow::bail!("Abstract account not created yet");
        };

        self.create_move_authenticator_for_free_access_for_ref(aa_ref)
    }

    fn create_move_authenticator_with_sponsor_and_sender(
        &self,
        aa_sponsor_ref: ObjectReference,
    ) -> anyhow::Result<UserSignature> {
        let Some(aa_ref) = self.aa_ref else {
            anyhow::bail!("Abstract account not created yet");
        };
        let self_call_arg = CallArg::Shared(SharedObjectReference::new(
            aa_ref.object_id,
            aa_ref.version,
            false,
        ));

        Ok(UserSignature::MoveAuthenticator(
            MoveAuthenticatorV1::new(
                vec![self_call_arg],
                vec![],
                SharedObjectReference::new(aa_sponsor_ref.object_id, aa_sponsor_ref.version, false),
            )
            .into(),
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
            builder.obj(CallArg::Shared(SharedObjectReference::new(
                aa_ref.object_id,
                aa_ref.version,
                true,
            )))?,
            builder.pure(1_u8)?,
            builder.pure(2_u8)?,
        ];
        builder.programmable_move_call(
            aa_package_id,
            Identifier::new(module_name)?,
            Identifier::from_static("add_field"),
            vec![TypeTag::U8, TypeTag::U8],
            arguments,
        );
        Ok(builder.finish())
    }

    fn craft_object_transfer(
        &self,
        object_ref: ObjectReference,
        recipient: Address,
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
            builder.obj(CallArg::ImmutableOrOwned(aa_package_metadata_ref))?,
            builder.pure(AA_AUTHENTICATE_MODULE_NAME)?,
            builder.pure(authenticate_fn_name)?,
        ];
        if let Argument::Result(authenticator_function_ref_v1) = builder.programmable_move_call(
            IOTA_FRAMEWORK_PACKAGE_ID,
            Identifier::from_static("authenticator_function"),
            Identifier::from_static("create_auth_function_ref_v1"),
            vec![abstract_account_type_tag(&aa_package_id)],
            arguments,
        ) {
            // rotate the key in the abstract account.
            let arguments = vec![
                builder.obj(CallArg::Shared(SharedObjectReference::new(
                    aa_ref.object_id,
                    aa_ref.version,
                    true,
                )))?,
                builder.pure(new_aa_owner_pk.as_ref())?,
                Argument::Result(authenticator_function_ref_v1),
            ];
            builder.programmable_move_call(
                aa_package_id,
                Identifier::from_static(AA_CREATE_MODULE_NAME),
                Identifier::from_static("rotate_public_key"),
                vec![],
                arguments,
            );
        }
        Ok(builder.finish())
    }

    fn craft_aa_delete_object_ptb(&mut self) -> anyhow::Result<ProgrammableTransaction> {
        let (Some(aa_ref), Some(aa_package_id)) = (self.aa_ref, self.aa_package_id) else {
            anyhow::bail!("Abstract account not created yet");
        };

        let mut builder = ProgrammableTransactionBuilder::new();

        // Delete the abstract account shared object.
        let arguments = vec![builder.obj(CallArg::Shared(SharedObjectReference::new(
            aa_ref.object_id,
            aa_ref.version,
            true,
        )))?];
        builder.programmable_move_call(
            aa_package_id,
            Identifier::from_static(AA_MODULE_NAME),
            Identifier::from_static("delete_account"),
            vec![],
            arguments,
        );
        Ok(builder.finish())
    }

    // Utilities

    async fn craft_tx_from_pt(
        &self,
        pt: ProgrammableTransaction,
        gas_coin: ObjectReference,
        sender: Address,
        sponsor: Option<Address>,
    ) -> anyhow::Result<Transaction> {
        let gas_price = self.test_cluster.get_reference_gas_price().await;

        // Create the transaction data that will be sent to the network
        Ok(Transaction::new_programmable_allow_sponsor(
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
        owner: Address,
        authenticate_fn_name: &str,
        create_fn_name: &str,
        aa_package_id: ObjectId,
        aa_package_metadata_ref: ObjectReference,
    ) -> anyhow::Result<TransactionEnvelope> {
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
                builder.obj(CallArg::ImmutableOrOwned(aa_package_metadata_ref))?,
                builder.pure(AA_AUTHENTICATE_MODULE_NAME)?,
                builder.pure(authenticate_fn_name)?,
            ];
            if let Argument::Result(authenticator_function_ref_v1) = builder.programmable_move_call(
                ObjectId::FRAMEWORK,
                Identifier::from_static("authenticator_function"),
                Identifier::from_static("create_auth_function_ref_v1"),
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
                    Identifier::from_static(AA_CREATE_MODULE_NAME),
                    Identifier::new(create_fn_name)?,
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
        gas_ref: ObjectReference,
        module_name: &str,
        receive_fn_name: &str,
    ) -> anyhow::Result<ProgrammableTransaction> {
        let (Some(aa_ref), Some(aa_package_id)) = (self.aa_ref, self.aa_package_id) else {
            anyhow::bail!("Abstract account not created yet");
        };
        let mut b = ProgrammableTransactionBuilder::new();

        let args = vec![
            b.obj(CallArg::Shared(SharedObjectReference::new(
                aa_ref.object_id,
                aa_ref.version,
                true,
            )))?,
            // IMPORTANT: passing an object ref *in the position of* `Receiving<T>`
            // yields a Receiving PTB arg (SDK converts when building the call).
            b.obj(CallArg::Receiving(gas_ref))?,
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

    /// Creates an extra AA (not stored in `aa_ref`) and returns its object ref.
    /// This requires if it is necessary to create more AAs in a test.
    async fn create_extra_abstract_account(&self) -> anyhow::Result<ObjectReference> {
        let effects = self.create_abstract_account().await?;
        Ok(abstract_account_from_all_changed_objects(
            &effects.all_changed_objects(),
        ))
    }

    /// Creates an extra AA with the specified parameters (not stored in
    /// `aa_ref`) and returns its object ref.
    /// This requires if it is necessary to create more AAs in a test.
    async fn create_extra_abstract_account_with(
        &self,
        authenticate_fn_name: &str,
    ) -> anyhow::Result<ObjectReference> {
        let (Some(owner), Some(aa_package_id), Some(aa_package_metadata_ref)) =
            (self.owner, self.aa_package_id, self.aa_package_metadata_ref)
        else {
            anyhow::bail!("Owner or authenticate function name or package id not set");
        };

        let effects = self
            .create_abstract_account_with(
                owner,
                authenticate_fn_name,
                aa_package_id,
                aa_package_metadata_ref,
            )
            .await?;
        Ok(abstract_account_from_all_changed_objects(
            &effects.all_changed_objects(),
        ))
    }

    /// Create an Abstract Account on the ledger with the specified parameters.
    async fn create_abstract_account_with(
        &self,
        owner: Address,
        authenticate_fn_name: &str,
        aa_package_id: ObjectId,
        aa_package_metadata_ref: ObjectReference,
    ) -> anyhow::Result<TransactionEffects> {
        let transaction = if let Some(transaction) = &self.aa_create_transaction {
            transaction.clone()
        } else {
            self.craft_create_abstract_account(
                owner,
                authenticate_fn_name,
                AA_CREATE_FN_NAME,
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

    /// Create a free-access MoveAuthenticator for an explicit object reference
    /// (not necessarily the stored `aa_ref`).
    fn create_move_authenticator_for_free_access_for_ref(
        &self,
        aa_obj_ref: ObjectReference,
    ) -> anyhow::Result<UserSignature> {
        Ok(UserSignature::MoveAuthenticator(
            MoveAuthenticatorV1::new(
                vec![],
                vec![],
                SharedObjectReference::new(aa_obj_ref.object_id, aa_obj_ref.version, false),
            )
            .into(),
        ))
    }

    // Create the MoveAuthenticator for the Ed25519 signature authenticator for an
    // explicit object reference (not necessarily the stored `aa_ref`):
    // public fun authenticate_ed25519(
    //    self: &AbstractAccount,
    //    signature: vector<u8>,
    //    _: &AuthContext,
    //    ctx: &TxContext,
    fn create_move_authenticator_for_ed25519_for_ref(
        &self,
        aa_obj_ref: ObjectReference,
        tx_digest: &[u8; 32],
    ) -> anyhow::Result<UserSignature> {
        let Some(owner) = self.owner else {
            anyhow::bail!("Abstract account not created yet");
        };
        let signature = self
            .test_cluster
            .wallet
            .config()
            .keystore()
            .sign_hashed(&owner, tx_digest)?;
        Self::move_authenticator_from_ed25519_sig(aa_obj_ref, signature)
    }

    /// Create the MoveAuthenticator for the ed25519 authenticator that verifies
    /// against `auth_ctx.signing_digest()`. Uses `sign_secure` which signs
    /// `blake2b256(intent || bcs(Transaction))` — exactly what
    /// `signing_digest` returns on the Move side.
    fn create_move_authenticator_for_ed25519_via_signing_digest(
        &self,
        tx: &Transaction,
    ) -> anyhow::Result<UserSignature> {
        let Some(aa_ref) = self.aa_ref else {
            anyhow::bail!("Abstract account not created yet");
        };
        let Some(owner) = self.owner else {
            anyhow::bail!("Abstract account not created yet");
        };
        let signature = self.test_cluster.wallet.config().keystore().sign_secure(
            &owner,
            tx,
            Intent::iota_transaction(),
        )?;
        Self::move_authenticator_from_ed25519_sig(aa_ref, signature)
    }

    /// Build a `UserSignature::MoveAuthenticator` from a raw ed25519
    /// `SimpleSignature` and the abstract-account object reference.
    fn move_authenticator_from_ed25519_sig(
        aa_obj_ref: ObjectReference,
        signature: SimpleSignature,
    ) -> anyhow::Result<UserSignature> {
        let hex_encoded_signature: String = Hex::encode(signature.to_bytes())
            .chars()
            .skip(2) // flag prefix length
            .take(Ed25519Signature::LENGTH * 2)
            .collect();
        let signature_call_arg = CallArg::Pure(bcs::to_bytes(&hex_encoded_signature)?);
        Ok(UserSignature::MoveAuthenticator(
            MoveAuthenticatorV1::new(
                vec![signature_call_arg],
                vec![],
                SharedObjectReference::new(aa_obj_ref.object_id, aa_obj_ref.version, false),
            )
            .into(),
        ))
    }

    async fn execute_and_check_tx_correctness(
        &self,
        tx: TransactionEnvelope,
    ) -> anyhow::Result<()> {
        // `execute_transaction` panics unless execution succeeded on the
        // fullnode.
        self.test_cluster.execute_transaction(tx).await;
        Ok(())
    }

    /// Runs the given validator's signing checks in process.
    ///
    /// Unlike [`Self::handle_tx`] this needs no network, so it still works
    /// after that validator has been restarted — restarting leaves the
    /// fullnode's cached authority clients pointing at dead connections.
    async fn handle_tx_in_process(
        &self,
        authority: &AuthorityName,
        tx: TransactionEnvelope,
    ) -> Result<HandleTransactionResponse, IotaError> {
        self.test_cluster
            .swarm
            .node(authority)
            .unwrap()
            .get_node_handle()
            .unwrap()
            .with_async(|node| async move {
                let state = node.state();
                let epoch_store = state.epoch_store_for_testing();
                let tx = epoch_store.verify_transaction(tx)?;
                state.handle_transaction(&epoch_store, tx).await
            })
            .await
    }

    async fn handle_tx(
        &self,
        tx: TransactionEnvelope,
    ) -> Result<HandleTransactionResponse, IotaError> {
        let aggregator = self.test_cluster.authority_aggregator();
        aggregator
            .authority_clients
            .values()
            .next()
            .unwrap()
            .authority_client()
            .handle_transaction(tx, Some(SocketAddr::new([127, 0, 0, 1].into(), 0)))
            .await
    }
}

// ---------------------------------------------------
// --- Utilities -------------------------------------
// ---------------------------------------------------

fn abstract_account_type_tag(aa_package_id: &ObjectId) -> TypeTag {
    TypeTag::from_str(format!("{aa_package_id}::{AA_MODULE_NAME}::{AA_ACCOUNT_NAME}").as_str())
        .unwrap()
}

fn delayed_abstract_account_type_tag(aa_package_id: &ObjectId) -> TypeTag {
    TypeTag::from_str(
        format!("{aa_package_id}::{AA_DELAYED_MODULE_NAME}::{AA_DELAYED_ACCOUNT_NAME}").as_str(),
    )
    .unwrap()
}

fn abstract_account_from_all_changed_objects(
    all_changed_objects: &[(OwnedObjectReference, WriteKind)],
) -> ObjectReference {
    // Extract the only created shared or immutable object which is the abstract
    // account. The other object the creation transaction writes is the
    // authenticator function ref, which is owned by the account object.
    *all_changed_objects
        .iter()
        .find_map(|(changed, kind)| {
            matches!(
                (changed.owner(), kind),
                (Owner::Shared(_) | Owner::Immutable, WriteKind::Create)
            )
            .then_some(changed.reference())
        })
        .expect("Expected a shared or immutable object in the transaction response")
}
