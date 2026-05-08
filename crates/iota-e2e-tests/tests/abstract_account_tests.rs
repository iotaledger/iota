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
    ed25519::{Ed25519KeyPair, Ed25519Signature},
    encoding::{Encoding, Hex},
    secp256k1::Secp256k1KeyPair,
    secp256r1::Secp256r1KeyPair,
    traits::{Authenticator, KeyPair as FastcryptoKeyPair, ToFromBytes},
};
use iota_core::authority_client::AuthorityAPI;
use iota_json_rpc_types::{
    DryRunTransactionBlockResponse, IotaTransactionBlockEffectsAPI, IotaTransactionBlockResponse,
};
use iota_keys::keystore::AccountKeystore;
use iota_macros::sim_test;
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::crypto::{Intent, IntentMessage};
use iota_test_transaction_builder::publish_package;
use iota_types::{
    IOTA_FRAMEWORK_PACKAGE_ID, TypeTag,
    base_types::{IotaAddress, ObjectID, ObjectRef},
    crypto::{IotaKeyPair, PublicKey, Signature as IotaSignature, SignatureScheme},
    effects::{TransactionEffects, TransactionEffectsAPI},
    error::{IotaError, UserInputError},
    execution_status::{ExecutionFailureStatus, MoveLocation},
    messages_grpc::{HandleCertificateRequestV1, HandleTransactionResponse},
    move_authenticator::MoveAuthenticator,
    move_package,
    multisig::{MultiSig, MultiSigPublicKey},
    object::Owner,
    passkey_authenticator::{PasskeyAuthenticator, to_signing_message},
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
use p256::pkcs8::DecodePublicKey;
use passkey_authenticator::{Authenticator as PasskeyClient, UserCheck, UserValidationMethod};
use passkey_client::Client as WebAuthnClient;
use passkey_types::{
    Bytes, Passkey,
    ctap2::{Aaguid, Ctap2Error},
    rand::random_vec,
    webauthn::{
        AttestationConveyancePreference, CredentialCreationOptions, CredentialRequestOptions,
        PublicKeyCredentialCreationOptions, PublicKeyCredentialParameters,
        PublicKeyCredentialRequestOptions, PublicKeyCredentialRpEntity, PublicKeyCredentialType,
        PublicKeyCredentialUserEntity, UserVerificationRequirement,
    },
};
use rand::{SeedableRng, rngs::StdRng};
use test_cluster::{TestCluster, TestClusterBuilder};
use url::Url;

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
const AA_AUTHENTICATE_FN_NAME_WITH_SPONSOR_AND_SENDER: &str =
    "authenticate_with_sponsor_and_sender";
const AA_AUTHENTICATE_FN_NAME_ED25519_VIA_SIGNING_DIGEST: &str =
    "authenticate_ed25519_via_signing_digest";
const AA_RECEIVE_OBJECT_FN_NAME: &str = "receive_object";
const AA_RECEIVE_OBJECT_FN_NAME_NO_SENDER_CHECK: &str = "receive_object_without_sender_check";

// Built-in authenticator module / function names (used by the new
// builtin_keyed_aa Move module).
const AA_BUILTIN_MODULE_NAME: &str = "builtin_keyed_aa";
const AA_BUILTIN_ED25519_CREATE_FN: &str = "create_with_ed25519";
const AA_BUILTIN_SECP256K1_CREATE_FN: &str = "create_with_secp256k1";
const AA_BUILTIN_SECP256R1_CREATE_FN: &str = "create_with_secp256r1";
const AA_BUILTIN_MULTISIG_CREATE_FN: &str = "create_with_multisig";
const AA_BUILTIN_PASSKEY_CREATE_FN: &str = "create_with_passkey";
const AA_BUILTIN_ED25519_AUTH_SECP256K1_KEY_CREATE_FN: &str =
    "create_with_ed25519_auth_and_secp256k1_key";

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
    let aa_sender = aa_ref.0.into();

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

    // sign_secure signs blake2b256(intent || bcs(TransactionData)), which is
    // exactly what auth_ctx.signing_digest() returns on the Move side.
    let signatures =
        vec![test_env.create_move_authenticator_for_ed25519_via_signing_digest(&tx_data)?];

    // Execute — the Move authenticator asserts all structural invariants
    // and verifies the ed25519 signature against signing_digest.
    let tx = Transaction::from_generic_sig_data(tx_data, signatures);
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

// ----------------------------------------------------------
// ----------- Sponsor Move authentication tests ------------
// ----------------------------------------------------------

/// A sponsored TX where both the sender (AA) and the sponsor (AA) carry a
/// MoveAuthenticator must succeed(enable_move_authentication_for_sponsor =
/// true).
#[sim_test]
async fn test_aa_sender_and_aa_sponsor_succeeded_with_enabled_move_auth_for_sponsor()
-> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    // Build the test environment and create the sender AA.
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519)
        .await?;
    let sender_aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: IotaAddress = sender_aa_ref.0.into();

    // Create a second AA that will act as the sponsor.
    let sponsor_aa_ref = test_env.create_extra_abstract_account().await?;
    let sponsor_addr: IotaAddress = sponsor_aa_ref.0.into();

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
    let tx_digest = tx_data.digest().into_inner();

    // Both sender and sponsor provide MoveAuthenticators.
    let sender_aa_sig = test_env.create_move_authenticator_for_ed25519(&tx_digest)?;
    let sponsor_aa_sig =
        test_env.create_move_authenticator_for_ed25519_for_ref(sponsor_aa_ref, &tx_digest)?;
    let tx = Transaction::from_generic_sig_data(tx_data, vec![sender_aa_sig, sponsor_aa_sig]);

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
    let sponsor_addr: IotaAddress = sponsor_aa_ref.0.into();

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
    let sender_sig = GenericSignature::Signature(
        test_env
            .test_cluster
            .wallet
            .config()
            .keystore()
            .sign_secure(&sender, &tx_data, Intent::iota_transaction())?,
    );
    let sponsor_aa_sig =
        test_env.create_move_authenticator_for_free_access_for_ref(sponsor_aa_ref)?;
    let tx = Transaction::from_generic_sig_data(tx_data, vec![sender_sig, sponsor_aa_sig]);

    // The TX must succeed when the sender is a regular account and AA sponsor.
    test_env.execute_and_check_tx_correctness(tx).await
}

/// A sponsored TX where both the sender (AA) and the sponsor (AA) carry a
/// MoveAuthenticator and use the same shared object must
/// succeed(enable_move_authentication_for_sponsor = true).
#[sim_test]
async fn test_aa_sender_and_aa_sponsor_use_the_same_shared_object_succeeded_with_enabled_move_auth_for_sponsor()
-> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    // Build the test environment and create the sender AA.
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519)
        .await?;
    let sender_aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: IotaAddress = sender_aa_ref.0.into();

    // Create a second AA that will act as the sponsor.
    let sponsor_aa_ref = test_env
        .create_extra_abstract_account_with(AA_AUTHENTICATE_FN_NAME_WITH_SPONSOR_AND_SENDER)
        .await?;
    let sponsor_addr: IotaAddress = sponsor_aa_ref.0.into();

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
    let tx_digest = tx_data.digest().into_inner();

    // Both sender and sponsor provide MoveAuthenticators.
    let sender_aa_sig = test_env.create_move_authenticator_for_ed25519(&tx_digest)?;
    // The sender object is used in both MoveAuthenticators.
    let sponsor_aa_sig =
        test_env.create_move_authenticator_with_sponsor_and_sender(sponsor_aa_ref)?;
    let tx = Transaction::from_generic_sig_data(tx_data, vec![sender_aa_sig, sponsor_aa_sig]);

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
    telemetry_subscribers::init_for_testing();

    // Disable Move authentication for the sponsor.
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_move_authentication_for_sponsor_for_testing(false);
        config
    });

    // Build the test environment and create the sender AA.
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_FREE_ACCESS)
        .await?;
    let sender_aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: IotaAddress = sender_aa_ref.0.into();

    // Create a second AA that will act as the sponsor.
    let sponsor_aa_ref = test_env.create_extra_abstract_account().await?;
    let sponsor_addr: IotaAddress = sponsor_aa_ref.0.into();

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
    let tx = Transaction::from_generic_sig_data(tx_data, vec![sender_aa_sig, sponsor_aa_sig]);

    // The TX must be rejected: >1 MoveAuthenticator is not allowed.
    let err = test_env.handle_tx(tx).await.unwrap_err();

    assert!(
        matches!(
            &err,
            IotaError::UserInput {
                error: UserInputError::Unsupported(msg)
            } if msg == "SenderSignedData with more than one MoveAuthenticator is not supported"
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
    telemetry_subscribers::init_for_testing();

    // Disable Move authentication for the sponsor.
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_move_authentication_for_sponsor_for_testing(false);
        config
    });

    // Build the test environment; the AA here will be the *sponsor*, not the
    // sender.
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_FREE_ACCESS)
        .await?;
    let sponsor_aa_ref = test_env.aa_ref.unwrap();
    let sponsor_addr: IotaAddress = sponsor_aa_ref.0.into();

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
    let sender_sig = GenericSignature::Signature(
        test_env
            .test_cluster
            .wallet
            .config()
            .keystore()
            .sign_secure(&sender, &tx_data, Intent::iota_transaction())?,
    );
    let sponsor_aa_sig =
        test_env.create_move_authenticator_for_free_access_for_ref(sponsor_aa_ref)?;
    let tx = Transaction::from_generic_sig_data(tx_data, vec![sender_sig, sponsor_aa_sig]);

    // The TX must be rejected: the single MoveAuthenticator belongs to the
    // sponsor, not the sender, which is not allowed.
    let err = test_env.handle_tx(tx).await.unwrap_err();

    assert!(
        matches!(
            &err,
            IotaError::UserInput {
                error: UserInputError::Unsupported(msg)
            } if msg == "SenderSignedData can have MoveAuthenticator only for the sender"
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
    telemetry_subscribers::init_for_testing();

    // Build the test environment and create the sender AA.
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_FREE_ACCESS)
        .await?;
    let sender_aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: IotaAddress = sender_aa_ref.0.into();

    // Create a second AA that will act as the sponsor.
    let sponsor_aa_ref = test_env.create_extra_abstract_account().await?;
    let sponsor_addr: IotaAddress = sponsor_aa_ref.0.into();

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
    let tx = Transaction::from_generic_sig_data(tx_data, vec![sender_aa_sig, unrelated_aa_sig]);

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
    telemetry_subscribers::init_for_testing();

    // Build the test environment and create the sender AA.
    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_FREE_ACCESS)
        .await?;
    let sender_aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: IotaAddress = sender_aa_ref.0.into();

    // Create a second AA that will act as the sponsor.
    let sponsor_aa_ref = test_env.create_extra_abstract_account().await?;
    let sponsor_addr: IotaAddress = sponsor_aa_ref.0.into();

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
    let tx_digest = tx_data.digest().into_inner();

    // Both sender and sponsor provide MoveAuthenticators.
    let sender_aa_sig = test_env.create_move_authenticator_for_free_access()?;
    // But the sponsor's signature is for ed25519 authentication, which does not
    // match the sponsor AA's actual free access authenticator.
    let sponsor_aa_sig =
        test_env.create_move_authenticator_for_ed25519_for_ref(sponsor_aa_ref, &tx_digest)?;
    let tx = Transaction::from_generic_sig_data(tx_data, vec![sender_aa_sig, sponsor_aa_sig]);

    // The TX must be rejected: the sponsor's signature is incorrect.
    let err = test_env.handle_tx(tx).await.unwrap_err();

    assert!(
        matches!(&err, IotaError::MoveAuthenticatorExecutionFailure { .. }),
        "Expected MoveAuthenticatorExecutionFailure for wrong sponsor MoveAuthenticator, got: {err:?}"
    );

    Ok(())
}

// ---------------------------------------------------
// --- Built-in authenticator tests ------------------
// ---------------------------------------------------

/// Test that the built-in Ed25519 authenticator lets the account's owner key
/// authorize a transaction. The wallet's existing Ed25519 keypair is reused.
#[sim_test]
async fn test_builtin_ed25519_authenticator() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    let mut test_env = TestEnvironment::new().await;
    test_env.init_abstract_account_state("").await;

    let owner = test_env.owner.unwrap();
    let kp = test_env
        .test_cluster
        .wallet
        .config()
        .keystore()
        .get_key(&owner)?
        .as_keypair()?
        .clone();
    let pk = kp.public();

    test_env
        .setup_builtin_account(
            pk.scheme(),
            pk.as_ref().to_vec(),
            AA_BUILTIN_ED25519_CREATE_FN,
        )
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: IotaAddress = aa_ref.0.into();

    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let aa_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    let pt = test_env.craft_aa_simple_ptb(AA_MODULE_NAME)?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, aa_gas, aa_sender, None)
        .await?;
    let sig = builtin_sig_for_keypair(&kp, &tx_data, aa_ref)?;
    let aa_tx = Transaction::from_generic_sig_data(tx_data, vec![sig]);
    test_env.execute_and_check_tx_correctness(aa_tx).await
}

/// Test that the built-in Secp256k1 authenticator lets a fresh Secp256k1
/// keypair authorize a transaction.
#[sim_test]
async fn test_builtin_secp256k1_authenticator() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    let mut test_env = TestEnvironment::new().await;
    test_env.init_abstract_account_state("").await;

    let kp = IotaKeyPair::Secp256k1(Secp256k1KeyPair::generate(&mut StdRng::from_seed(
        [1u8; 32],
    )));
    let pk = kp.public();

    test_env
        .setup_builtin_account(
            pk.scheme(),
            pk.as_ref().to_vec(),
            AA_BUILTIN_SECP256K1_CREATE_FN,
        )
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: IotaAddress = aa_ref.0.into();

    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let aa_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    let pt = test_env.craft_aa_simple_ptb(AA_MODULE_NAME)?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, aa_gas, aa_sender, None)
        .await?;
    let sig = builtin_sig_for_keypair(&kp, &tx_data, aa_ref)?;
    let aa_tx = Transaction::from_generic_sig_data(tx_data, vec![sig]);
    test_env.execute_and_check_tx_correctness(aa_tx).await
}

/// Test that the built-in Secp256r1 authenticator lets a fresh Secp256r1
/// keypair authorize a transaction.
#[sim_test]
async fn test_builtin_secp256r1_authenticator() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    let mut test_env = TestEnvironment::new().await;
    test_env.init_abstract_account_state("").await;

    let kp = IotaKeyPair::Secp256r1(Secp256r1KeyPair::generate(&mut StdRng::from_seed(
        [2u8; 32],
    )));
    let pk = kp.public();

    test_env
        .setup_builtin_account(
            pk.scheme(),
            pk.as_ref().to_vec(),
            AA_BUILTIN_SECP256R1_CREATE_FN,
        )
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: IotaAddress = aa_ref.0.into();

    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let aa_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    let pt = test_env.craft_aa_simple_ptb(AA_MODULE_NAME)?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, aa_gas, aa_sender, None)
        .await?;
    let sig = builtin_sig_for_keypair(&kp, &tx_data, aa_ref)?;
    let aa_tx = Transaction::from_generic_sig_data(tx_data, vec![sig]);
    test_env.execute_and_check_tx_correctness(aa_tx).await
}

/// Test that the built-in MultiSig authenticator accepts a multisig that meets
/// the threshold (2-of-2 Ed25519 keys).
#[sim_test]
async fn test_builtin_multisig_authenticator() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    let mut test_env = TestEnvironment::new().await;
    test_env.init_abstract_account_state("").await;

    // Build a 2-of-2 multisig key.
    let kp1 = IotaKeyPair::Ed25519(Ed25519KeyPair::generate(&mut StdRng::from_seed([3u8; 32])));
    let kp2 = IotaKeyPair::Ed25519(Ed25519KeyPair::generate(&mut StdRng::from_seed([4u8; 32])));
    let multisig_pk = MultiSigPublicKey::new(vec![kp1.public(), kp2.public()], vec![1, 1], 2)?;

    test_env
        .setup_builtin_account(
            SignatureScheme::MultiSig,
            bcs::to_bytes(&multisig_pk)?,
            AA_BUILTIN_MULTISIG_CREATE_FN,
        )
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: IotaAddress = aa_ref.0.into();

    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let aa_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    let pt = test_env.craft_aa_simple_ptb(AA_MODULE_NAME)?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, aa_gas, aa_sender, None)
        .await?;

    // Sign with both keys and combine into a MultiSig.
    let intent_msg = IntentMessage::new(Intent::iota_transaction(), tx_data.clone());
    let sig1 = GenericSignature::Signature(IotaSignature::new_secure(&intent_msg, &kp1));
    let sig2 = GenericSignature::Signature(IotaSignature::new_secure(&intent_msg, &kp2));
    let multisig = MultiSig::combine(vec![sig1, sig2], multisig_pk)?;
    let wire_bytes = GenericSignature::MultiSig(multisig).as_ref().to_vec();

    let object_arg = CallArg::Object(ObjectArg::SharedObject {
        id: aa_ref.0,
        initial_shared_version: aa_ref.1,
        mutable: false,
    });
    let auth_sig = GenericSignature::MoveAuthenticator(MoveAuthenticator::new_v1(
        vec![CallArg::Pure(bcs::to_bytes(&wire_bytes)?)],
        vec![],
        object_arg,
    ));
    let aa_tx = Transaction::from_generic_sig_data(tx_data, vec![auth_sig]);
    test_env.execute_and_check_tx_correctness(aa_tx).await
}

/// Test that the built-in Passkey authenticator accepts a WebAuthn credential
/// signature over the transaction intent message.
#[sim_test]
async fn test_builtin_passkey_authenticator() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    let mut test_env = TestEnvironment::new().await;
    test_env.init_abstract_account_state("").await;

    // Set up a mock WebAuthn authenticator and register a passkey credential.
    let store: Option<Passkey> = None;
    let my_authenticator = PasskeyClient::new(Aaguid::new_empty(), store, AlwaysApprove);
    let mut my_client = WebAuthnClient::new(my_authenticator);
    let origin = Url::parse("https://www.iota.org").unwrap();

    let creation_opts = CredentialCreationOptions {
        public_key: PublicKeyCredentialCreationOptions {
            rp: PublicKeyCredentialRpEntity {
                id: None,
                name: origin.domain().unwrap().into(),
            },
            user: PublicKeyCredentialUserEntity {
                id: random_vec(32).into(),
                display_name: "Test".into(),
                name: "test@iota.org".into(),
            },
            challenge: random_vec(32).into(),
            pub_key_cred_params: vec![PublicKeyCredentialParameters {
                ty: PublicKeyCredentialType::PublicKey,
                alg: coset::iana::Algorithm::ES256,
            }],
            timeout: None,
            exclude_credentials: None,
            authenticator_selection: None,
            hints: None,
            attestation: AttestationConveyancePreference::None,
            attestation_formats: None,
            extensions: None,
        },
    };
    let credential = my_client
        .register(&origin, creation_opts, None)
        .await
        .expect("passkey registration failed");

    // Derive the compressed Secp256r1 public key from the DER-encoded WebAuthn key.
    let verifying_key = p256::ecdsa::VerifyingKey::from_public_key_der(
        credential.response.public_key.unwrap().as_slice(),
    )?;
    let ep = verifying_key.to_encoded_point(false);
    let prefix = if ep.y().unwrap()[31] % 2 == 0 {
        0x02
    } else {
        0x03
    };
    let mut pk_bytes = vec![prefix];
    pk_bytes.extend_from_slice(ep.x().unwrap());

    test_env
        .setup_builtin_account(
            SignatureScheme::PasskeyAuthenticator,
            pk_bytes.clone(),
            AA_BUILTIN_PASSKEY_CREATE_FN,
        )
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: IotaAddress = aa_ref.0.into();

    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let aa_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    let pt = test_env.craft_aa_simple_ptb(AA_MODULE_NAME)?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, aa_gas, aa_sender, None)
        .await?;

    // The passkey challenge is the blake2b hash of bcs(IntentMessage(tx_data)).
    let intent_msg = IntentMessage::new(Intent::iota_transaction(), tx_data.clone());
    let passkey_challenge: Bytes = to_signing_message(&intent_msg).to_vec().into();

    let auth_request = CredentialRequestOptions {
        public_key: PublicKeyCredentialRequestOptions {
            challenge: passkey_challenge,
            timeout: None,
            rp_id: Some(origin.domain().unwrap().into()),
            allow_credentials: None,
            user_verification: UserVerificationRequirement::default(),
            attestation: Default::default(),
            attestation_formats: None,
            extensions: None,
            hints: None,
        },
    };
    let auth_cred = my_client
        .authenticate(&origin, auth_request, None)
        .await
        .expect("passkey authentication failed");

    // Build the Secp256r1 signature in wire format (flag || sig || pk).
    let sig_der = auth_cred.response.signature.as_slice();
    let sig = p256::ecdsa::Signature::from_der(sig_der)?;
    let sig_bytes = sig.normalize_s().unwrap_or(sig).to_bytes();
    let mut user_sig_bytes = vec![SignatureScheme::Secp256r1.flag()];
    user_sig_bytes.extend_from_slice(&sig_bytes);
    user_sig_bytes.extend_from_slice(&pk_bytes);

    let passkey_auth = PasskeyAuthenticator::new_for_testing(
        auth_cred.response.authenticator_data.as_slice().to_vec(),
        String::from_utf8_lossy(auth_cred.response.client_data_json.as_slice()).into(),
        IotaSignature::from_bytes(&user_sig_bytes)?,
    )?;
    let wire_bytes = GenericSignature::PasskeyAuthenticator(passkey_auth)
        .as_ref()
        .to_vec();

    let object_arg = CallArg::Object(ObjectArg::SharedObject {
        id: aa_ref.0,
        initial_shared_version: aa_ref.1,
        mutable: false,
    });
    let auth_sig = GenericSignature::MoveAuthenticator(MoveAuthenticator::new_v1(
        vec![CallArg::Pure(bcs::to_bytes(&wire_bytes)?)],
        vec![],
        object_arg,
    ));
    let aa_tx = Transaction::from_generic_sig_data(tx_data, vec![auth_sig]);
    test_env.execute_and_check_tx_correctness(aa_tx).await
}

// ---------------------------------------------------
// --- Built-in authenticator failure tests ----------
// ---------------------------------------------------

/// Test that the built-in Ed25519 authenticator rejects a transaction signed
/// by a key that does not match the account's registered public key.
#[sim_test]
async fn test_builtin_ed25519_authenticator_wrong_key() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    let mut test_env = TestEnvironment::new().await;
    test_env.init_abstract_account_state("").await;

    // Register the account with kp1.
    let kp1 = IotaKeyPair::Ed25519(Ed25519KeyPair::generate(&mut StdRng::from_seed([10u8; 32])));
    test_env
        .setup_builtin_account(
            kp1.public().scheme(),
            kp1.public().as_ref().to_vec(),
            AA_BUILTIN_ED25519_CREATE_FN,
        )
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: IotaAddress = aa_ref.0.into();

    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let aa_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    let pt = test_env.craft_aa_simple_ptb(AA_MODULE_NAME)?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, aa_gas, aa_sender, None)
        .await?;

    // Sign with kp2 (a different, unrelated Ed25519 key).
    let kp2 = IotaKeyPair::Ed25519(Ed25519KeyPair::generate(&mut StdRng::from_seed([11u8; 32])));
    let sig = builtin_sig_for_keypair(&kp2, &tx_data, aa_ref)?;
    let aa_tx = Transaction::from_generic_sig_data(tx_data, vec![sig]);

    let err = test_env.handle_tx(aa_tx).await.unwrap_err();
    let IotaError::MoveAuthenticatorExecutionFailure { error } = &err else {
        panic!("Expected MoveAuthenticatorExecutionFailure for wrong Ed25519 key, got: {err:?}");
    };
    assert!(
        error.contains("BuiltinAuthenticatorVerificationError"),
        "Expected BuiltinAuthenticatorVerificationError in error, got: {error}"
    );
    assert!(
        error.contains("Value was not signed by the correct sender"),
        "Expected 'Value was not signed by the correct sender' in error, got: {error}"
    );
    Ok(())
}

/// Test that the built-in Secp256k1 authenticator rejects a transaction signed
/// by a key that does not match the account's registered public key.
#[sim_test]
async fn test_builtin_secp256k1_authenticator_wrong_key() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    let mut test_env = TestEnvironment::new().await;
    test_env.init_abstract_account_state("").await;

    // Register the account with kp1.
    let kp1 = IotaKeyPair::Secp256k1(Secp256k1KeyPair::generate(&mut StdRng::from_seed(
        [20u8; 32],
    )));
    test_env
        .setup_builtin_account(
            kp1.public().scheme(),
            kp1.public().as_ref().to_vec(),
            AA_BUILTIN_SECP256K1_CREATE_FN,
        )
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: IotaAddress = aa_ref.0.into();

    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let aa_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    let pt = test_env.craft_aa_simple_ptb(AA_MODULE_NAME)?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, aa_gas, aa_sender, None)
        .await?;

    // Sign with kp2 (a different, unrelated Secp256k1 key).
    let kp2 = IotaKeyPair::Secp256k1(Secp256k1KeyPair::generate(&mut StdRng::from_seed(
        [21u8; 32],
    )));
    let sig = builtin_sig_for_keypair(&kp2, &tx_data, aa_ref)?;
    let aa_tx = Transaction::from_generic_sig_data(tx_data, vec![sig]);

    let err = test_env.handle_tx(aa_tx).await.unwrap_err();
    let IotaError::MoveAuthenticatorExecutionFailure { error } = &err else {
        panic!("Expected MoveAuthenticatorExecutionFailure for wrong Secp256k1 key, got: {err:?}");
    };
    assert!(
        error.contains("BuiltinAuthenticatorVerificationError"),
        "Expected BuiltinAuthenticatorVerificationError in error, got: {error}"
    );
    assert!(
        error.contains("Value was not signed by the correct sender"),
        "Expected 'Value was not signed by the correct sender' in error, got: {error}"
    );
    Ok(())
}

/// Test that the built-in Secp256r1 authenticator rejects a transaction signed
/// by a key that does not match the account's registered public key.
#[sim_test]
async fn test_builtin_secp256r1_authenticator_wrong_key() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    let mut test_env = TestEnvironment::new().await;
    test_env.init_abstract_account_state("").await;

    // Register the account with kp1.
    let kp1 = IotaKeyPair::Secp256r1(Secp256r1KeyPair::generate(&mut StdRng::from_seed(
        [30u8; 32],
    )));
    test_env
        .setup_builtin_account(
            kp1.public().scheme(),
            kp1.public().as_ref().to_vec(),
            AA_BUILTIN_SECP256R1_CREATE_FN,
        )
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: IotaAddress = aa_ref.0.into();

    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let aa_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    let pt = test_env.craft_aa_simple_ptb(AA_MODULE_NAME)?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, aa_gas, aa_sender, None)
        .await?;

    // Sign with kp2 (a different, unrelated Secp256r1 key).
    let kp2 = IotaKeyPair::Secp256r1(Secp256r1KeyPair::generate(&mut StdRng::from_seed(
        [31u8; 32],
    )));
    let sig = builtin_sig_for_keypair(&kp2, &tx_data, aa_ref)?;
    let aa_tx = Transaction::from_generic_sig_data(tx_data, vec![sig]);

    let err = test_env.handle_tx(aa_tx).await.unwrap_err();
    let IotaError::MoveAuthenticatorExecutionFailure { error } = &err else {
        panic!("Expected MoveAuthenticatorExecutionFailure for wrong Secp256r1 key, got: {err:?}");
    };
    assert!(
        error.contains("BuiltinAuthenticatorVerificationError"),
        "Expected BuiltinAuthenticatorVerificationError in error, got: {error}"
    );
    assert!(
        error.contains("Value was not signed by the correct sender"),
        "Expected 'Value was not signed by the correct sender' in error, got: {error}"
    );
    Ok(())
}

/// Test that the built-in MultiSig authenticator rejects a transaction when the
/// combined signature weight does not meet the required threshold (1 of 2 sigs
/// provided for a 2-of-2 scheme).
#[sim_test]
async fn test_builtin_multisig_authenticator_threshold_not_met() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    let mut test_env = TestEnvironment::new().await;
    test_env.init_abstract_account_state("").await;

    // Build a 2-of-2 multisig key.
    let kp1 = IotaKeyPair::Ed25519(Ed25519KeyPair::generate(&mut StdRng::from_seed([40u8; 32])));
    let kp2 = IotaKeyPair::Ed25519(Ed25519KeyPair::generate(&mut StdRng::from_seed([41u8; 32])));
    let multisig_pk = MultiSigPublicKey::new(vec![kp1.public(), kp2.public()], vec![1, 1], 2)?;

    test_env
        .setup_builtin_account(
            SignatureScheme::MultiSig,
            bcs::to_bytes(&multisig_pk)?,
            AA_BUILTIN_MULTISIG_CREATE_FN,
        )
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: IotaAddress = aa_ref.0.into();

    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let aa_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    let pt = test_env.craft_aa_simple_ptb(AA_MODULE_NAME)?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, aa_gas, aa_sender, None)
        .await?;

    // Only provide kp1's signature — weight 1 is below the required threshold of 2.
    let intent_msg = IntentMessage::new(Intent::iota_transaction(), tx_data.clone());
    let sig1 = GenericSignature::Signature(IotaSignature::new_secure(&intent_msg, &kp1));
    let multisig = MultiSig::combine(vec![sig1], multisig_pk)?;
    let wire_bytes = GenericSignature::MultiSig(multisig).as_ref().to_vec();

    let object_arg = CallArg::Object(ObjectArg::SharedObject {
        id: aa_ref.0,
        initial_shared_version: aa_ref.1,
        mutable: false,
    });
    let auth_sig = GenericSignature::MoveAuthenticator(MoveAuthenticator::new_v1(
        vec![CallArg::Pure(bcs::to_bytes(&wire_bytes)?)],
        vec![],
        object_arg,
    ));
    let aa_tx = Transaction::from_generic_sig_data(tx_data, vec![auth_sig]);

    let err = test_env.handle_tx(aa_tx).await.unwrap_err();
    let IotaError::MoveAuthenticatorExecutionFailure { error } = &err else {
        panic!(
            "Expected MoveAuthenticatorExecutionFailure for insufficient MultiSig weight, got: \
             {err:?}"
        );
    };
    assert!(
        error.contains("BuiltinAuthenticatorVerificationError"),
        "Expected BuiltinAuthenticatorVerificationError in error, got: {error}"
    );
    assert!(
        error.contains("Insufficient weight"),
        "Expected 'Insufficient weight' in error, got: {error}"
    );
    Ok(())
}

/// Test that the built-in Passkey authenticator rejects a transaction signed by
/// a WebAuthn credential whose public key does not match the one registered in
/// the account.
#[sim_test]
async fn test_builtin_passkey_authenticator_wrong_key() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    let mut test_env = TestEnvironment::new().await;
    test_env.init_abstract_account_state("").await;

    let origin = Url::parse("https://www.iota.org").unwrap();

    let make_credential_creation_opts = || CredentialCreationOptions {
        public_key: PublicKeyCredentialCreationOptions {
            rp: PublicKeyCredentialRpEntity {
                id: None,
                name: origin.domain().unwrap().into(),
            },
            user: PublicKeyCredentialUserEntity {
                id: random_vec(32).into(),
                display_name: "Test".into(),
                name: "test@iota.org".into(),
            },
            challenge: random_vec(32).into(),
            pub_key_cred_params: vec![PublicKeyCredentialParameters {
                ty: PublicKeyCredentialType::PublicKey,
                alg: coset::iana::Algorithm::ES256,
            }],
            timeout: None,
            exclude_credentials: None,
            authenticator_selection: None,
            hints: None,
            attestation: AttestationConveyancePreference::None,
            attestation_formats: None,
            extensions: None,
        },
    };

    // Register credential A — this key will be stored in the account.
    let store_a: Option<Passkey> = None;
    let authenticator_a = PasskeyClient::new(Aaguid::new_empty(), store_a, AlwaysApprove);
    let mut client_a = WebAuthnClient::new(authenticator_a);
    let credential_a = client_a
        .register(&origin, make_credential_creation_opts(), None)
        .await
        .expect("passkey A registration failed");

    let verifying_key_a = p256::ecdsa::VerifyingKey::from_public_key_der(
        credential_a.response.public_key.unwrap().as_slice(),
    )?;
    let ep_a = verifying_key_a.to_encoded_point(false);
    let prefix_a = if ep_a.y().unwrap()[31] % 2 == 0 {
        0x02
    } else {
        0x03
    };
    let mut pk_bytes_a = vec![prefix_a];
    pk_bytes_a.extend_from_slice(ep_a.x().unwrap());

    test_env
        .setup_builtin_account(
            SignatureScheme::PasskeyAuthenticator,
            pk_bytes_a,
            AA_BUILTIN_PASSKEY_CREATE_FN,
        )
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: IotaAddress = aa_ref.0.into();

    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let aa_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    let pt = test_env.craft_aa_simple_ptb(AA_MODULE_NAME)?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, aa_gas, aa_sender, None)
        .await?;

    // Register credential B (a completely different passkey).
    let store_b: Option<Passkey> = None;
    let authenticator_b = PasskeyClient::new(Aaguid::new_empty(), store_b, AlwaysApprove);
    let mut client_b = WebAuthnClient::new(authenticator_b);
    let credential_b = client_b
        .register(&origin, make_credential_creation_opts(), None)
        .await
        .expect("passkey B registration failed");

    // Authenticate with credential B using the correct transaction challenge.
    let intent_msg = IntentMessage::new(Intent::iota_transaction(), tx_data.clone());
    let passkey_challenge: Bytes = to_signing_message(&intent_msg).to_vec().into();

    let auth_request = CredentialRequestOptions {
        public_key: PublicKeyCredentialRequestOptions {
            challenge: passkey_challenge,
            timeout: None,
            rp_id: Some(origin.domain().unwrap().into()),
            allow_credentials: None,
            user_verification: UserVerificationRequirement::default(),
            attestation: Default::default(),
            attestation_formats: None,
            extensions: None,
            hints: None,
        },
    };
    let auth_cred_b = client_b
        .authenticate(&origin, auth_request, None)
        .await
        .expect("passkey B authentication failed");

    // Derive credential B's compressed Secp256r1 public key.
    let verifying_key_b = p256::ecdsa::VerifyingKey::from_public_key_der(
        credential_b.response.public_key.unwrap().as_slice(),
    )?;
    let ep_b = verifying_key_b.to_encoded_point(false);
    let prefix_b = if ep_b.y().unwrap()[31] % 2 == 0 {
        0x02
    } else {
        0x03
    };
    let mut pk_bytes_b = vec![prefix_b];
    pk_bytes_b.extend_from_slice(ep_b.x().unwrap());

    // Build the Secp256r1 signature in wire format using credential B's key.
    let sig_der = auth_cred_b.response.signature.as_slice();
    let sig = p256::ecdsa::Signature::from_der(sig_der)?;
    let sig_bytes = sig.normalize_s().unwrap_or(sig).to_bytes();
    let mut user_sig_bytes = vec![SignatureScheme::Secp256r1.flag()];
    user_sig_bytes.extend_from_slice(&sig_bytes);
    user_sig_bytes.extend_from_slice(&pk_bytes_b);

    let passkey_auth = PasskeyAuthenticator::new_for_testing(
        auth_cred_b.response.authenticator_data.as_slice().to_vec(),
        String::from_utf8_lossy(auth_cred_b.response.client_data_json.as_slice()).into(),
        IotaSignature::from_bytes(&user_sig_bytes)?,
    )?;
    let wire_bytes = GenericSignature::PasskeyAuthenticator(passkey_auth)
        .as_ref()
        .to_vec();

    let object_arg = CallArg::Object(ObjectArg::SharedObject {
        id: aa_ref.0,
        initial_shared_version: aa_ref.1,
        mutable: false,
    });
    let auth_sig = GenericSignature::MoveAuthenticator(MoveAuthenticator::new_v1(
        vec![CallArg::Pure(bcs::to_bytes(&wire_bytes)?)],
        vec![],
        object_arg,
    ));
    let aa_tx = Transaction::from_generic_sig_data(tx_data, vec![auth_sig]);

    let err = test_env.handle_tx(aa_tx).await.unwrap_err();
    let IotaError::MoveAuthenticatorExecutionFailure { error } = &err else {
        panic!(
            "Expected MoveAuthenticatorExecutionFailure for wrong Passkey credential, got: {err:?}"
        );
    };
    assert!(
        error.contains("BuiltinAuthenticatorVerificationError"),
        "Expected BuiltinAuthenticatorVerificationError in error, got: {error}"
    );
    assert!(
        error.contains("Invalid author"),
        "Expected 'Invalid author' in error, got: {error}"
    );
    Ok(())
}

/// Test that the built-in Ed25519 authenticator rejects a transaction signed
/// with a Secp256k1 key: the submitted signature's scheme does not match the
/// Ed25519 authenticator function ref.
#[sim_test]
async fn test_builtin_ed25519_authenticator_signature_scheme_mismatch() -> Result<(), anyhow::Error>
{
    telemetry_subscribers::init_for_testing();

    let mut test_env = TestEnvironment::new().await;
    test_env.init_abstract_account_state("").await;

    let owner = test_env.owner.unwrap();
    let ed25519_kp = test_env
        .test_cluster
        .wallet
        .config()
        .keystore()
        .get_key(&owner)?
        .as_keypair()?
        .clone();
    let pk = ed25519_kp.public();

    test_env
        .setup_builtin_account(
            pk.scheme(),
            pk.as_ref().to_vec(),
            AA_BUILTIN_ED25519_CREATE_FN,
        )
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: IotaAddress = aa_ref.0.into();

    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let aa_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    let pt = test_env.craft_aa_simple_ptb(AA_MODULE_NAME)?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, aa_gas, aa_sender, None)
        .await?;

    // Sign with a Secp256k1 key — wrong scheme for an Ed25519 authenticator.
    let secp256k1_kp = IotaKeyPair::Secp256k1(Secp256k1KeyPair::generate(&mut StdRng::from_seed(
        [50u8; 32],
    )));
    let sig = builtin_sig_for_keypair(&secp256k1_kp, &tx_data, aa_ref)?;
    let aa_tx = Transaction::from_generic_sig_data(tx_data, vec![sig]);

    let err = test_env.handle_tx(aa_tx).await.unwrap_err();
    let IotaError::MoveAuthenticatorExecutionFailure { error } = &err else {
        panic!(
            "Expected MoveAuthenticatorExecutionFailure for Secp256k1 sig on Ed25519 account, \
             got: {err:?}"
        );
    };
    assert!(
        error.contains("BuiltinAuthenticatorVerificationError"),
        "Expected BuiltinAuthenticatorVerificationError in error, got: {error}"
    );
    assert!(
        error.contains("Signature scheme mismatch"),
        "Expected 'Signature scheme mismatch' in error, got: {error}"
    );
    Ok(())
}

/// Test that the built-in Ed25519 authenticator rejects a transaction when
/// the on-chain public key was registered with a Secp256k1 scheme.
///
/// The account is deliberately misconfigured:
/// `ed25519_authenticator_function_ref_v1` is used but a Secp256k1 public key
/// is attached. The signature is Ed25519 (so the signature scheme check
/// passes), but `verify_builtin_signature` catches the mismatch between the
/// authenticator's expected scheme (Ed25519) and the stored key's scheme
/// (Secp256k1).
#[sim_test]
async fn test_builtin_ed25519_authenticator_public_key_scheme_mismatch() -> Result<(), anyhow::Error>
{
    telemetry_subscribers::init_for_testing();

    let mut test_env = TestEnvironment::new().await;
    test_env.init_abstract_account_state("").await;

    let secp256k1_kp = IotaKeyPair::Secp256k1(Secp256k1KeyPair::generate(&mut StdRng::from_seed(
        [60u8; 32],
    )));
    let secp256k1_pk = secp256k1_kp.public();

    test_env
        .setup_builtin_account(
            secp256k1_pk.scheme(),
            secp256k1_pk.as_ref().to_vec(),
            AA_BUILTIN_ED25519_AUTH_SECP256K1_KEY_CREATE_FN,
        )
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: IotaAddress = aa_ref.0.into();

    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let aa_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    let pt = test_env.craft_aa_simple_ptb(AA_MODULE_NAME)?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, aa_gas, aa_sender, None)
        .await?;

    // Sign with an Ed25519 key so the signature scheme check passes. The failure
    // comes next: the on-chain public key's scheme (Secp256k1) does not match the
    // Ed25519 authenticator function ref.
    let ed25519_kp =
        IotaKeyPair::Ed25519(Ed25519KeyPair::generate(&mut StdRng::from_seed([61u8; 32])));
    let sig = builtin_sig_for_keypair(&ed25519_kp, &tx_data, aa_ref)?;
    let aa_tx = Transaction::from_generic_sig_data(tx_data, vec![sig]);

    let err = test_env.handle_tx(aa_tx).await.unwrap_err();
    let IotaError::MoveAuthenticatorExecutionFailure { error } = &err else {
        panic!(
            "Expected MoveAuthenticatorExecutionFailure for public key scheme mismatch, \
             got: {err:?}"
        );
    };
    assert!(
        error.contains("BuiltinAuthenticatorVerificationError"),
        "Expected BuiltinAuthenticatorVerificationError in error, got: {error}"
    );
    assert!(
        error.contains("Public key scheme mismatch"),
        "Expected 'Public key scheme mismatch' in error, got: {error}"
    );
    Ok(())
}

/// Test that supplying a built-in-format signature to an account backed by a
/// custom Ed25519 authenticator is rejected.
///
/// The custom `authenticate_ed25519` expects its `signature` arg to be a
/// hex-encoded 64-byte value (it calls `iota::hex::decode` on it).
/// `builtin_sig_for_keypair` instead produces flag-prefixed wire bytes
/// (`0x00 | 64_sig_bytes | 32_pk_bytes`) which are not valid hex, so the
/// Move authenticator aborts.
#[sim_test]
async fn test_builtin_sig_rejected_by_custom_ed25519_authenticator() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    let mut test_env = TestEnvironment::new().await;
    test_env
        .setup_abstract_account(AA_AUTHENTICATE_FN_NAME_ED25519)
        .await?;
    let aa_ref = test_env.aa_ref.unwrap();
    let aa_sender: IotaAddress = aa_ref.0.into();

    let rgp = test_env.test_cluster.get_reference_gas_price().await;
    let aa_gas = test_env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    let pt = test_env.craft_aa_simple_ptb(AA_MODULE_NAME)?;
    let tx_data = test_env
        .craft_tx_from_pt(pt, aa_gas, aa_sender, None)
        .await?;

    // Use the owner keypair (the one registered in the account) but wrap it in
    // the builtin wire format — incompatible with the custom authenticator.
    let owner = test_env.owner.unwrap();
    let kp = test_env
        .test_cluster
        .wallet
        .config()
        .keystore()
        .get_key(&owner)?
        .as_keypair()?
        .clone();
    let sig = builtin_sig_for_keypair(&kp, &tx_data, aa_ref)?;
    let aa_tx = Transaction::from_generic_sig_data(tx_data, vec![sig]);

    let err = test_env.handle_tx(aa_tx).await.unwrap_err();
    let IotaError::MoveAuthenticatorExecutionFailure { error } = &err else {
        panic!(
            "Expected MoveAuthenticatorExecutionFailure when a builtin-format signature is used \
             with a custom authenticator, got: {err:?}"
        );
    };
    // The custom authenticator calls `iota::hex::decode` on the raw wire bytes.
    // The wire format is 97 bytes (odd), so `hex::decode` aborts immediately with
    // `EInvalidHexLength` (code 0) before any signature verification is attempted.
    assert!(
        error.contains("MoveAbort"),
        "Expected a MoveAbort (not a builtin verification error), got: {error}"
    );
    assert!(
        error.contains("hex"),
        "Expected abort to originate in the `hex` module, got: {error}"
    );
    Ok(())
}

/// Test that a built-in account cannot be created when
/// `enable_builtin_move_authenticators` is disabled in the protocol config.
/// Ed25519 is used as a representative scheme.
///
/// Each `*_authenticator_function_ref_v1` function calls
/// `check_builtin_authenticators_enabled()` and aborts with
/// `EBuiltinAuthenticatorsNotEnabled` (code 0) before an
/// `AuthenticatorFunctionRefV1` is ever produced, so the account creation
/// transaction itself fails — it never reaches the authentication step.
#[sim_test]
async fn test_builtin_move_gate_blocks_account_creation() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_builtin_move_authenticators_for_testing(false);
        config
    });

    let mut test_env = TestEnvironment::new().await;
    test_env.init_abstract_account_state("").await;

    let owner = test_env.owner.unwrap();
    let kp = test_env
        .test_cluster
        .wallet
        .config()
        .keystore()
        .get_key(&owner)?
        .as_keypair()?
        .clone();
    let pk = kp.public();

    // With the feature disabled, `ed25519_authenticator_function_ref_v1` aborts
    // with EBuiltinAuthenticatorsNotEnabled (code 0) inside account creation.
    let transaction = test_env
        .craft_create_builtin_account(pk.scheme(), pk.as_ref(), AA_BUILTIN_ED25519_CREATE_FN)
        .await?;
    let (effects, _) = test_env
        .test_cluster
        .execute_transaction_return_raw_effects(transaction)
        .await?;

    let status = effects.into_status();
    assert!(
        status.is_err(),
        "Expected account creation to fail when built-in authenticators are disabled, \
         got: {status:?}"
    );
    assert!(
        matches!(
            status.unwrap_err().0,
            ExecutionFailureStatus::MoveAbort(MoveLocation { module, .. }, abort_code)
            if module.name() == ident_str!("builtin_authenticator_functions")
                && ErrorBitset::from_u64(abort_code).unwrap().error_code() == Some(0)
        ),
        "Expected MoveAbort in builtin_authenticator_functions with code 0 \
         (EBuiltinAuthenticationsNotEnabled)"
    );

    Ok(())
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
                .map(|e| (e.0.reference, e.0.owner, e.1))
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
                IOTA_FRAMEWORK_PACKAGE_ID,
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
    fn create_move_authenticator_for_free_access(&self) -> anyhow::Result<GenericSignature> {
        let Some(aa_ref) = self.aa_ref else {
            anyhow::bail!("Abstract account not created yet");
        };

        self.create_move_authenticator_for_free_access_for_ref(aa_ref)
    }

    fn create_move_authenticator_with_sponsor_and_sender(
        &self,
        aa_sponsor_ref: ObjectRef,
    ) -> anyhow::Result<GenericSignature> {
        let Some(aa_ref) = self.aa_ref else {
            anyhow::bail!("Abstract account not created yet");
        };
        let self_call_arg = CallArg::Object(ObjectArg::SharedObject {
            id: aa_ref.0,
            initial_shared_version: aa_ref.1,
            mutable: false,
        });
        let sponsor_call_arg = CallArg::Object(ObjectArg::SharedObject {
            id: aa_sponsor_ref.0,
            initial_shared_version: aa_sponsor_ref.1,
            mutable: false,
        });
        Ok(GenericSignature::MoveAuthenticator(
            MoveAuthenticator::new_v1(vec![self_call_arg], vec![], sponsor_call_arg),
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
            IOTA_FRAMEWORK_PACKAGE_ID,
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
                IOTA_FRAMEWORK_PACKAGE_ID,
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

    /// Creates an extra AA (not stored in `aa_ref`) and returns its object ref.
    /// This requires if it is necessary to create more AAs in a test.
    async fn create_extra_abstract_account(&self) -> anyhow::Result<ObjectRef> {
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
    ) -> anyhow::Result<ObjectRef> {
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
        owner: IotaAddress,
        authenticate_fn_name: &str,
        aa_package_id: ObjectID,
        aa_package_metadata_ref: ObjectRef,
    ) -> anyhow::Result<TransactionEffects> {
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

    /// Create a free-access MoveAuthenticator for an explicit object reference
    /// (not necessarily the stored `aa_ref`).
    fn create_move_authenticator_for_free_access_for_ref(
        &self,
        aa_obj_ref: ObjectRef,
    ) -> anyhow::Result<GenericSignature> {
        let self_call_arg = CallArg::Object(ObjectArg::SharedObject {
            id: aa_obj_ref.0,
            initial_shared_version: aa_obj_ref.1,
            mutable: false,
        });
        Ok(GenericSignature::MoveAuthenticator(
            MoveAuthenticator::new_v1(vec![], vec![], self_call_arg),
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
        aa_obj_ref: ObjectRef,
        tx_digest: &[u8; 32],
    ) -> anyhow::Result<GenericSignature> {
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
    /// `blake2b256(intent || bcs(TransactionData))` — exactly what
    /// `signing_digest` returns on the Move side.
    fn create_move_authenticator_for_ed25519_via_signing_digest(
        &self,
        tx_data: &TransactionData,
    ) -> anyhow::Result<GenericSignature> {
        let Some(aa_ref) = self.aa_ref else {
            anyhow::bail!("Abstract account not created yet");
        };
        let Some(owner) = self.owner else {
            anyhow::bail!("Abstract account not created yet");
        };
        let signature = self.test_cluster.wallet.config().keystore().sign_secure(
            &owner,
            tx_data,
            Intent::iota_transaction(),
        )?;
        Self::move_authenticator_from_ed25519_sig(aa_ref, signature)
    }

    /// Build a `GenericSignature::MoveAuthenticator` from a raw ed25519
    /// `Signature` and the abstract-account object reference.
    fn move_authenticator_from_ed25519_sig(
        aa_obj_ref: ObjectRef,
        signature: iota_types::crypto::Signature,
    ) -> anyhow::Result<GenericSignature> {
        let self_call_arg = CallArg::Object(ObjectArg::SharedObject {
            id: aa_obj_ref.0,
            initial_shared_version: aa_obj_ref.1,
            mutable: false,
        });
        let hex_encoded_signature: String = Hex::encode(signature)
            .chars()
            .skip(2) // flag prefix length
            .take(Ed25519Signature::LENGTH * 2)
            .collect();
        let signature_call_arg = CallArg::Pure(bcs::to_bytes(&hex_encoded_signature)?);
        Ok(GenericSignature::MoveAuthenticator(
            MoveAuthenticator::new_v1(vec![signature_call_arg], vec![], self_call_arg),
        ))
    }

    // -----------------------------------------------
    // --- Built-in authenticator helpers ------------
    // -----------------------------------------------

    /// Publishes the AA package (assumed already done via
    /// `init_abstract_account_state`), then creates an `AbstractAccount`
    /// shared object backed by a built-in authenticator.
    /// Publishes the AA package (assumed already done via
    /// `init_abstract_account_state`), then creates an `AbstractAccount`
    /// shared object backed by a built-in authenticator.
    ///
    /// `scheme` and `raw_bytes` are passed directly to Move's
    /// `public_key::create`. `create_fn_name` is one of the functions defined
    /// in `builtin_keyed_aa`.
    async fn setup_builtin_account(
        &mut self,
        scheme: SignatureScheme,
        raw_bytes: Vec<u8>,
        create_fn_name: &str,
    ) -> anyhow::Result<()> {
        let transaction = self
            .craft_create_builtin_account(scheme, &raw_bytes, create_fn_name)
            .await?;
        let (effects, _) = self
            .test_cluster
            .execute_transaction_return_raw_effects(transaction)
            .await?;
        self.aa_ref = Some(abstract_account_from_all_changed_objects(
            &effects.all_changed_objects(),
        ));
        Ok(())
    }

    /// Craft a signed transaction that calls
    /// `builtin_keyed_aa::<create_fn_name>` with the given public key.
    async fn craft_create_builtin_account(
        &self,
        scheme: SignatureScheme,
        raw_bytes: &[u8],
        create_fn_name: &str,
    ) -> anyhow::Result<Transaction> {
        let Some(aa_package_id) = self.aa_package_id else {
            anyhow::bail!("AA package id not set — call init_abstract_account_state first");
        };

        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            // Pure args can only carry primitives / vector<u8>.  Construct the Move
            // PublicKey by calling signature_scheme::<scheme>() and public_key::create()
            // in the PTB, then forward the result to the account create function.
            let scheme_fn = match scheme {
                SignatureScheme::ED25519 => "ed25519",
                SignatureScheme::Secp256k1 => "secp256k1",
                SignatureScheme::Secp256r1 => "secp256r1",
                SignatureScheme::MultiSig => "multisig",
                SignatureScheme::PasskeyAuthenticator => "passkey",
                _ => anyhow::bail!("Unsupported scheme for built-in account: {scheme:?}"),
            };
            let scheme_arg = builder.programmable_move_call(
                IOTA_FRAMEWORK_PACKAGE_ID,
                Identifier::new("signature_scheme")?,
                Identifier::new(scheme_fn)?,
                vec![],
                vec![],
            );
            let raw_arg = builder.pure(raw_bytes.to_vec())?;
            let public_key = builder.programmable_move_call(
                IOTA_FRAMEWORK_PACKAGE_ID,
                Identifier::new("public_key")?,
                Identifier::new("create")?,
                vec![],
                vec![scheme_arg, raw_arg],
            );
            builder.programmable_move_call(
                aa_package_id,
                Identifier::new(AA_BUILTIN_MODULE_NAME)?,
                Identifier::new(create_fn_name)?,
                vec![],
                vec![public_key],
            );
            builder.finish()
        };

        let tx_data = self
            .test_cluster
            .test_transaction_builder()
            .await
            .programmable(pt)
            .build();
        Ok(self.test_cluster.wallet.sign_transaction(&tx_data))
    }

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

    async fn handle_tx(&self, tx: Transaction) -> Result<HandleTransactionResponse, IotaError> {
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

// ---------------------------------------------------
// --- Built-in authenticator utilities --------------
// ---------------------------------------------------

/// Sign `tx_data` with the intent message using `kp` and wrap the resulting
/// `GenericSignature` wire bytes in a `MoveAuthenticator` that authenticates
/// against `aa_ref`.
fn builtin_sig_for_keypair(
    kp: &IotaKeyPair,
    tx_data: &TransactionData,
    aa_ref: ObjectRef,
) -> anyhow::Result<GenericSignature> {
    let intent_msg = IntentMessage::new(Intent::iota_transaction(), tx_data.clone());
    let generic_sig = GenericSignature::Signature(IotaSignature::new_secure(&intent_msg, kp));
    let wire_bytes = generic_sig.as_ref().to_vec();
    let object_arg = CallArg::Object(ObjectArg::SharedObject {
        id: aa_ref.0,
        initial_shared_version: aa_ref.1,
        mutable: false,
    });
    Ok(GenericSignature::MoveAuthenticator(
        MoveAuthenticator::new_v1(
            vec![CallArg::Pure(bcs::to_bytes(&wire_bytes)?)],
            vec![],
            object_arg,
        ),
    ))
}

// ---------------------------------------------------
// --- Passkey test helper ---------------------------
// ---------------------------------------------------

/// Minimal `UserValidationMethod` that always approves — used by the mock
/// WebAuthn client in `test_builtin_passkey_authenticator`.
struct AlwaysApprove;

#[async_trait::async_trait]
impl UserValidationMethod for AlwaysApprove {
    type PasskeyItem = Passkey;

    async fn check_user<'a>(
        &self,
        _credential: Option<&'a Self::PasskeyItem>,
        _presence: bool,
        _verification: bool,
    ) -> Result<UserCheck, Ctap2Error> {
        Ok(UserCheck {
            presence: true,
            verification: true,
        })
    }

    fn is_verification_enabled(&self) -> Option<bool> {
        Some(true)
    }

    fn is_presence_enabled(&self) -> bool {
        true
    }
}
