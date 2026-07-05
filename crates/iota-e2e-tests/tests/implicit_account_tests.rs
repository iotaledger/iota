// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Implicit account tests
//!
//! End-to-end tests for the implicit account authentication flow with
//! all `GenericSignature` types (Ed25519, Secp256k1, Secp256r1, MultiSig and
//! Passkey), in both its variants:
//!
//! - IMPLICIT: no account object exists on chain; the node derives the expected
//!   scheme and public key from the transaction signature itself and
//!   authenticates through a synthetic `MoveAuthenticator`.
//! - EXPLICIT: a `iota::smart_account::SmartAccount` object exists at the
//!   sender address (created via the framework claim flow) and the on-chain
//!   public key dynamic field is authoritative for authentication.
//! - IMPLICIT-ACCOUNT PINNING: an implicit account (a plain-signed sender with
//!   no on-chain object) is assigned a per-transaction read-only version, so
//!   its read stays implicit and deterministic across validators even when a
//!   claim of the same address lands in a different commit.
//! - WHITE-FLAG CLAIM-CONFLICT INVALIDATION: when a `smart_account::build_v1`
//!   claim of an address and a plain-signed transaction from that same address
//!   are sequenced into the same consensus commit, the claim is validated first
//!   and the racing plain transaction is deterministically dropped with
//!   `AccountClaimConflict` before it executes — every validator agrees, so the
//!   network does not fork and the dropped transaction is retryable later.
//!
//! These tests intentionally do NOT duplicate `abstract_account_tests.rs`,
//! which covers explicit accounts created through the custom `builtin_keyed_aa`
//! test package and authenticated with hand-crafted `MoveAuthenticator`s.
//!
//! The test functions come first; signing actors, transaction helpers and
//! shared flows live at the bottom of the file.

use std::{future::Future, net::SocketAddr, pin::Pin};

use fastcrypto::{
    ed25519::Ed25519KeyPair, secp256k1::Secp256k1KeyPair, secp256r1::Secp256r1KeyPair,
    traits::KeyPair as FastcryptoKeyPair,
};
use iota_core::authority_client::validator::ValidatorAPI;
use iota_macros::sim_test;
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{
    Address, ExecutionError, ExecutionStatus, Identifier, MoveLocation, ObjectId, Owner,
    ProgrammableTransaction, SimpleSignature,
    crypto::{Intent, IntentMessage, PublicKey as SdkPublicKey, UserSignature},
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    base_types::ObjectRef,
    crypto::{
        EncodeDecodeBase64, IotaKeyPair, PublicKey, Signature as IotaSignature, SignatureScheme,
    },
    effects::{TransactionEffects, TransactionEffectsAPI, TransactionEffectsExt},
    error::IotaError,
    messages_grpc::HandleTransactionResponse,
    move_authenticator::MoveAuthenticator,
    multisig::{MultiSig, MultiSigPublicKey, MultisigMember},
    object::Object,
    passkey_authenticator::PasskeyAuthenticator,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    signature::GenericSignature,
    storage::WriteKind,
    transaction::{
        CallArg, SharedObjectRef, TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, Transaction,
        TransactionData, TransactionDataAPI,
    },
};
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

use crate::passkey_util::{passkey_actor, passkey_sign, register_passkey};

const GAS_AMOUNT: u64 = 20_000_000_000;
const SMART_ACCOUNT_MODULE: &str = "smart_account";
const PUBLIC_KEY_MODULE: &str = "public_key";

// ---------------------------------------------------
// --- A. Implicit happy-path matrix -----------------
// ---------------------------------------------------

/// A plain Ed25519 signature from a fresh key authorizes a transaction with
/// no account object on chain (implicit builtin authentication).
#[sim_test]
async fn test_implicit_builtin_ed25519() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    run_implicit_happy_path(Actor::ed25519(1)).await
}

/// Same as above for a Secp256k1 signature.
#[sim_test]
async fn test_implicit_builtin_secp256k1() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    run_implicit_happy_path(Actor::secp256k1(2)).await
}

/// Same as above for a Secp256r1 signature.
#[sim_test]
async fn test_implicit_builtin_secp256r1() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    run_implicit_happy_path(Actor::secp256r1(3)).await
}

/// Same as above for a MultiSig signature from a 2-of-3 mixed-scheme
/// committee (exercises the multisig arm of the signature-derived
/// `PreloadedBuiltinAuthenticatorData`).
#[sim_test]
async fn test_implicit_builtin_multisig() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    run_implicit_happy_path(Actor::multisig_mixed(4)).await
}

/// Same as above for a Passkey (WebAuthn) signature.
#[sim_test]
async fn test_implicit_builtin_passkey() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    run_implicit_happy_path(passkey_actor!()).await
}

// ---------------------------------------------------
// --- B. Implicit -> explicit claim transition ------
// ---------------------------------------------------

/// Claim the sender's address as a `SmartAccount` (the claim tx itself runs
/// through the implicit branch), then transact again via the explicit
/// on-chain-pk branch — Ed25519.
#[sim_test]
async fn test_claim_smart_account_ed25519() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    run_claim_transition(Actor::ed25519(10), false).await
}

/// Same as above for Secp256k1.
#[sim_test]
async fn test_claim_smart_account_secp256k1() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    run_claim_transition(Actor::secp256k1(11), false).await
}

/// Same as above for Secp256r1.
#[sim_test]
async fn test_claim_smart_account_secp256r1() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    run_claim_transition(Actor::secp256r1(12), false).await
}

/// Same as above for MultiSig (2-of-3 mixed-scheme committee; the Move-side
/// address derivation for Ed25519 members matches the node since #11869).
#[sim_test]
async fn test_claim_smart_account_multisig() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    run_claim_transition(Actor::multisig_mixed(13), false).await
}

/// Same as above for Passkey: the passkey signs both the claim transaction
/// and the follow-up explicit-branch transaction.
#[sim_test]
async fn test_claim_smart_account_passkey() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    run_claim_transition(passkey_actor!(), false).await
}

/// Claim finalized with `build_immutable_v1`: the follow-up transaction
/// exercises the immutable-account (`ImmOrOwnedObject`) branch of the
/// synthetic authenticator.
#[sim_test]
async fn test_claim_smart_account_immutable() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    run_claim_transition(Actor::ed25519(14), true).await
}

// ---------------------------------------------------
// --- C. Explicit on-chain pk is authoritative ------
// ---------------------------------------------------

/// Once an account is claimed (it has an on-chain object), a plain signature is
/// never accepted — only a `MoveAuthenticator` works. After claiming and
/// rotating the stored key to key2:
/// (a) the OLD key's plain signature is rejected: claimed accounts forbid plain
///     signatures (`PlainSignatureOnClaimedAccount`);
/// (b) the NEW key's plain signature is rejected upfront because it derives a
///     different address than the account (so the sender's signature is
/// absent); (c) the NEW key unlocks the account via a hand-crafted
/// `MoveAuthenticator`     targeting the account object.
#[sim_test]
async fn test_explicit_rotated_pk_old_key_fails() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    let test_cluster = TestClusterBuilder::new().build().await;
    let mut actor = Actor::ed25519(20);
    let sender = actor.address();

    claim_account(&test_cluster, &mut actor, false).await?;

    let new_key =
        IotaKeyPair::Ed25519(Ed25519KeyPair::generate(&mut StdRng::from_seed([21u8; 32])));
    rotate_account_pk(&test_cluster, &mut actor, prefixed_pk_of(&new_key)).await?;

    // (a) Old key, plain signature: rejected because the account is claimed —
    // claimed accounts can only be authenticated via a `MoveAuthenticator`.
    let gas = fund(&test_cluster, sender).await;
    let tx_data = transfer_tx_data(&test_cluster, sender, gas).await;
    let old_key_sig = actor.sign(&tx_data).await;
    let err = handle_tx(
        &test_cluster,
        Transaction::from_generic_sig_data(tx_data.clone(), vec![old_key_sig]),
    )
    .await
    .expect_err("a plain signature on a claimed account must be rejected");
    assert!(
        format!("{err:?}").contains("PlainSignatureOnClaimedAccount"),
        "expected PlainSignatureOnClaimedAccount, got {err:?}"
    );

    // (b) New key, plain signature: derives a different address than the
    // sender, so the required signer is missing.
    let intent_msg = IntentMessage::new(Intent::iota_transaction(), tx_data.clone());
    let new_key_sig = GenericSignature::Signature(IotaSignature::new_secure(&intent_msg, &new_key));
    let err = handle_tx(
        &test_cluster,
        Transaction::from_generic_sig_data(tx_data.clone(), vec![new_key_sig]),
    )
    .await
    .expect_err("a plain signature from the new key derives the wrong address");
    assert!(
        format!("{err:?}").contains("SignerSignatureAbsent"),
        "expected SignerSignatureAbsent, got {err:?}"
    );

    // (c) New key wrapped in a hand-crafted MoveAuthenticator targeting the
    // account object: succeeds.
    let wire_bytes =
        GenericSignature::Signature(IotaSignature::new_secure(&intent_msg, &new_key)).to_bytes();
    let account = shared_account_arg(&test_cluster, sender, false).await;
    let auth_sig = builtin_move_authenticator(&wire_bytes, account)?;
    execute_and_assert_success(
        &test_cluster,
        Transaction::from_generic_sig_data(tx_data, vec![auth_sig]),
    )
    .await?;
    Ok(())
}

/// After claiming and rotating the stored key (here to a different scheme), a
/// plain signature is rejected outright: a claimed account can only be
/// authenticated via a `MoveAuthenticator`, regardless of the signing key.
#[sim_test]
async fn test_explicit_rotated_pk_cross_scheme_mismatch() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    let test_cluster = TestClusterBuilder::new().build().await;
    let mut actor = Actor::ed25519(22);
    let sender = actor.address();

    claim_account(&test_cluster, &mut actor, false).await?;

    let secp_key = IotaKeyPair::Secp256k1(Secp256k1KeyPair::generate(&mut StdRng::from_seed(
        [23u8; 32],
    )));
    rotate_account_pk(&test_cluster, &mut actor, prefixed_pk_of(&secp_key)).await?;

    let gas = fund(&test_cluster, sender).await;
    let tx_data = transfer_tx_data(&test_cluster, sender, gas).await;
    let old_key_sig = actor.sign(&tx_data).await;
    let err = handle_tx(
        &test_cluster,
        Transaction::from_generic_sig_data(tx_data, vec![old_key_sig]),
    )
    .await
    .expect_err("a plain signature on a claimed account must be rejected");
    assert!(
        format!("{err:?}").contains("PlainSignatureOnClaimedAccount"),
        "expected PlainSignatureOnClaimedAccount, got {err:?}"
    );
    Ok(())
}

/// After detaching the public key from a claimed account, plain-signed
/// transactions are rejected because the account's public key cannot be
/// loaded.
#[sim_test]
async fn test_explicit_detached_pk_fails() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    let test_cluster = TestClusterBuilder::new().build().await;
    let mut actor = Actor::ed25519(24);
    let sender = actor.address();

    claim_account(&test_cluster, &mut actor, false).await?;

    // Detach the public key via a `MoveAuthenticator` (authorized by the
    // still-attached key); the account is claimed, so a plain signature would be
    // rejected.
    let account = shared_account_arg(&test_cluster, sender, true).await;
    let pt = detach_pk_ptb(account)?;
    let gas = fund(&test_cluster, sender).await;
    let tx_data = ptb_tx_data(&test_cluster, sender, gas, pt).await;
    let auth_account = shared_account_arg(&test_cluster, sender, false).await;
    let auth_sig = move_auth_sign(&mut actor, &tx_data, auth_account).await?;
    execute_and_assert_success(
        &test_cluster,
        Transaction::from_generic_sig_data(tx_data, vec![auth_sig]),
    )
    .await?;

    // A plain signature on the claimed account is rejected up front, regardless
    // of the (now detached) public key state.
    let gas = fund(&test_cluster, sender).await;
    let tx_data = transfer_tx_data(&test_cluster, sender, gas).await;
    let sig = actor.sign(&tx_data).await;
    let err = handle_tx(
        &test_cluster,
        Transaction::from_generic_sig_data(tx_data, vec![sig]),
    )
    .await
    .expect_err("a plain signature on a claimed account must be rejected");
    assert!(
        format!("{err:?}").contains("PlainSignatureOnClaimedAccount"),
        "expected PlainSignatureOnClaimedAccount, got {err:?}"
    );
    Ok(())
}

// ---------------------------------------------------
// --- D. Sponsored transactions ----------------------
// ---------------------------------------------------

/// Both the sender and the sponsor are implicit implicit accounts: each
/// plain signature flows through its own synthetic builtin authenticator.
#[sim_test]
async fn test_sponsored_tx_implicit_sender_and_sponsor() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    let test_cluster = TestClusterBuilder::new().build().await;
    let mut sender_actor = Actor::ed25519(30);
    let mut sponsor_actor = Actor::secp256k1(31);
    let sender = sender_actor.address();
    let sponsor = sponsor_actor.address();

    assert!(account_object(&test_cluster, sender).await.is_none());
    assert!(account_object(&test_cluster, sponsor).await.is_none());

    let sponsor_gas = fund(&test_cluster, sponsor).await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.pay_iota(vec![Address::ZERO], vec![1])?;
        builder.finish()
    };
    let tx_data = TransactionData::new_programmable_allow_sponsor(
        sender,
        vec![sponsor_gas],
        pt,
        rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        rgp,
        sponsor,
    );
    let sigs = vec![
        sender_actor.sign(&tx_data).await,
        sponsor_actor.sign(&tx_data).await,
    ];
    execute_and_assert_success(
        &test_cluster,
        Transaction::from_generic_sig_data(tx_data, sigs),
    )
    .await?;
    Ok(())
}

/// The sponsor is a claimed account: a sponsored transaction carrying the
/// sponsor's plain signature is rejected, because a claimed account (sender or
/// sponsor) can only be authenticated via a `MoveAuthenticator`.
#[sim_test]
async fn test_sponsored_tx_sponsor_explicit_rotated_fails() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    let test_cluster = TestClusterBuilder::new().build().await;
    let mut sender_actor = Actor::ed25519(32);
    let mut sponsor_actor = Actor::ed25519(33);
    let sender = sender_actor.address();
    let sponsor = sponsor_actor.address();

    claim_account(&test_cluster, &mut sponsor_actor, false).await?;
    let new_key =
        IotaKeyPair::Ed25519(Ed25519KeyPair::generate(&mut StdRng::from_seed([34u8; 32])));
    rotate_account_pk(&test_cluster, &mut sponsor_actor, prefixed_pk_of(&new_key)).await?;

    let sponsor_gas = fund(&test_cluster, sponsor).await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.pay_iota(vec![Address::ZERO], vec![1])?;
        builder.finish()
    };
    let tx_data = TransactionData::new_programmable_allow_sponsor(
        sender,
        vec![sponsor_gas],
        pt,
        rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        rgp,
        sponsor,
    );
    let sigs = vec![
        sender_actor.sign(&tx_data).await,
        sponsor_actor.sign(&tx_data).await,
    ];
    let err = handle_tx(
        &test_cluster,
        Transaction::from_generic_sig_data(tx_data, sigs),
    )
    .await
    .expect_err("the sponsor's plain signature must be rejected: the sponsor account is claimed");
    assert!(
        format!("{err:?}").contains("PlainSignatureOnClaimedAccount"),
        "expected PlainSignatureOnClaimedAccount, got {err:?}"
    );
    Ok(())
}

// ---------------------------------------------------
// --- E. Feature-gate contrasts ----------------------
// ---------------------------------------------------

/// With `enable_implicit_accounts` OFF, plain-signed transactions
/// are verified by the legacy path only and still succeed.
#[sim_test]
async fn test_implicit_flag_off_plain_tx_succeeds() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_implicit_accounts_for_testing(false);
        config
    });

    let test_cluster = TestClusterBuilder::new().build().await;
    let mut actor = Actor::secp256k1(40);
    let sender = actor.address();

    let gas = fund(&test_cluster, sender).await;
    let tx_data = transfer_tx_data(&test_cluster, sender, gas).await;
    let sig = actor.sign(&tx_data).await;
    execute_and_assert_success(
        &test_cluster,
        Transaction::from_generic_sig_data(tx_data, vec![sig]),
    )
    .await?;
    Ok(())
}

/// With the flag OFF, the explicit on-chain public key of a claimed account
/// is BYPASSED: a plain signature from the old (rotated-away) key still
/// succeeds. Direct contrast with
/// `test_explicit_rotated_pk_old_key_fails` — this pins the gate semantics.
#[sim_test]
async fn test_implicit_flag_off_rotated_explicit_account_bypassed() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_implicit_accounts_for_testing(false);
        config
    });

    let test_cluster = TestClusterBuilder::new().build().await;
    let mut actor = Actor::ed25519(41);
    let sender = actor.address();

    claim_account(&test_cluster, &mut actor, false).await?;
    let new_key =
        IotaKeyPair::Ed25519(Ed25519KeyPair::generate(&mut StdRng::from_seed([42u8; 32])));
    rotate_account_pk(&test_cluster, &mut actor, prefixed_pk_of(&new_key)).await?;

    // The old key still works: with the implicit flag off, no implicit
    // account objects are loaded and only legacy signature verification runs.
    let gas = fund(&test_cluster, sender).await;
    let tx_data = transfer_tx_data(&test_cluster, sender, gas).await;
    let sig = actor.sign(&tx_data).await;
    execute_and_assert_success(
        &test_cluster,
        Transaction::from_generic_sig_data(tx_data, vec![sig]),
    )
    .await?;
    Ok(())
}

// ---------------------------------------------------
// --- F. Claim failure modes -------------------------
// ---------------------------------------------------

/// Claiming with a public key that does not derive the sender's address
/// aborts in `iota::claim_registry` (`EAddressMismatch`).
#[sim_test]
async fn test_claim_wrong_public_key_aborts() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    let test_cluster = TestClusterBuilder::new().build().await;
    let mut actor = Actor::ed25519(50);
    let other = Actor::ed25519(51);
    let sender = actor.address();

    let gas = fund(&test_cluster, sender).await;
    let registry = claim_registry_arg(&test_cluster).await;
    // The pk belongs to `other`, not to the sender.
    let pt = claim_ptb(registry, other.prefixed_pk_bytes(), false)?;
    let tx_data = ptb_tx_data(&test_cluster, sender, gas, pt).await;
    let sig = actor.sign(&tx_data).await;
    execute_and_assert_claim_registry_abort(
        &test_cluster,
        Transaction::from_generic_sig_data(tx_data, vec![sig]),
    )
    .await
}

/// Once an address is claimed, a second `build_v1` claim is rejected at
/// authentication: the account now has an on-chain object, so its plain
/// signature is no longer accepted. The re-claim never reaches the
/// `EAlreadyClaimed` Move abort.
#[sim_test]
async fn test_claim_twice_rejected() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    let test_cluster = TestClusterBuilder::new().build().await;
    let mut actor = Actor::ed25519(52);
    let sender = actor.address();

    claim_account(&test_cluster, &mut actor, false).await?;

    let gas = fund(&test_cluster, sender).await;
    let registry = claim_registry_arg(&test_cluster).await;
    let pt = claim_ptb(registry, actor.prefixed_pk_bytes(), false)?;
    let tx_data = ptb_tx_data(&test_cluster, sender, gas, pt).await;
    let sig = actor.sign(&tx_data).await;
    let err = handle_tx(
        &test_cluster,
        Transaction::from_generic_sig_data(tx_data, vec![sig]),
    )
    .await
    .expect_err("a second claim is a plain signature on a now-claimed account");
    assert!(
        format!("{err:?}").contains("PlainSignatureOnClaimedAccount"),
        "expected PlainSignatureOnClaimedAccount, got {err:?}"
    );
    Ok(())
}

/// `smart_account::builtin_auth_builder_v1` creates an explicit account with
/// a FRESH object ID (unrelated to the key's address): a plain signature
/// cannot unlock it, only a hand-crafted `MoveAuthenticator` targeting the
/// account object can.
#[sim_test]
async fn test_builtin_auth_builder_v1_fresh_account() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    let test_cluster = TestClusterBuilder::new().build().await;
    let kp = IotaKeyPair::Ed25519(Ed25519KeyPair::generate(&mut StdRng::from_seed([53u8; 32])));

    // A wallet account creates the SmartAccount with a fresh UID.
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        let pk_bytes_arg = builder.pure(prefixed_pk_of(&kp))?;
        let pk = builder.programmable_move_call(
            ObjectId::FRAMEWORK,
            ident(PUBLIC_KEY_MODULE),
            ident("from_prefixed_bytes"),
            vec![],
            vec![pk_bytes_arg],
        );
        let account_builder = builder.programmable_move_call(
            ObjectId::FRAMEWORK,
            ident(SMART_ACCOUNT_MODULE),
            ident("builtin_auth_builder_v1"),
            vec![],
            vec![pk],
        );
        builder.programmable_move_call(
            ObjectId::FRAMEWORK,
            ident(SMART_ACCOUNT_MODULE),
            ident("build_v1"),
            vec![],
            vec![account_builder],
        );
        builder.finish()
    };
    let tx_data = test_cluster
        .test_transaction_builder()
        .await
        .programmable(pt)
        .build();
    let tx = test_cluster.wallet.sign_transaction(&tx_data);
    let effects = execute_and_assert_success(&test_cluster, tx).await?;

    let account_ref = created_shared_object(&effects);
    let account_address: Address = account_ref.object_id.into();
    assert_ne!(
        account_address,
        Address::from(&kp.public()),
        "builtin_auth_builder_v1 must create the account at a fresh object ID"
    );

    // A plain signature from the key cannot unlock the account: the key
    // derives its own address, not the account's.
    let gas = fund(&test_cluster, account_address).await;
    let tx_data = transfer_tx_data(&test_cluster, account_address, gas).await;
    let intent_msg = IntentMessage::new(Intent::iota_transaction(), tx_data.clone());
    let plain_sig = GenericSignature::Signature(IotaSignature::new_secure(&intent_msg, &kp));
    let err = handle_tx(
        &test_cluster,
        Transaction::from_generic_sig_data(tx_data.clone(), vec![plain_sig]),
    )
    .await
    .expect_err("a plain signature cannot unlock a fresh-UID account");
    assert!(
        format!("{err:?}").contains("SignerSignatureAbsent"),
        "expected SignerSignatureAbsent, got {err:?}"
    );

    // A hand-crafted MoveAuthenticator wrapping the key's wire signature
    // unlocks it.
    let wire_bytes =
        GenericSignature::Signature(IotaSignature::new_secure(&intent_msg, &kp)).to_bytes();
    let account = CallArg::Shared(SharedObjectRef::new(
        account_ref.object_id,
        account_ref.version,
        false,
    ));
    let auth_sig = builtin_move_authenticator(&wire_bytes, account)?;
    execute_and_assert_success(
        &test_cluster,
        Transaction::from_generic_sig_data(tx_data, vec![auth_sig]),
    )
    .await?;
    Ok(())
}

// ---------------------------------------------------
// --- G. P-COOL (white-flag) flow --------------------
// ---------------------------------------------------
//
// These run under `with_pcool_enabled()`, where transactions are sequenced
// through consensus before execution and built-in account objects participate
// in shared-object version assignment (the path the implicit-account
// determinism fix targets). Transactions are submitted via the fullnode
// orchestrator (`execute_transaction`), which routes through the
// `TransactionDriver` under P-COOL — the legacy `handle_transaction` /
// authority-aggregator helpers are disabled in this flow.

/// Implicit built-in authentication under P-COOL. A fresh key has no on-chain
/// account, so it is assigned no version (Weak) and the read stays implicit on
/// every validator.
#[sim_test]
async fn test_implicit_builtin_ed25519_pcool() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let test_cluster = TestClusterBuilder::new().with_pcool_enabled().build().await;
    let mut actor = Actor::ed25519(40);
    let sender = actor.address();

    let gas = fund(&test_cluster, sender).await;
    let tx_data = transfer_tx_data(&test_cluster, sender, gas).await;
    let sig = actor.sign(&tx_data).await;
    // `execute_transaction` asserts execution success internally.
    test_cluster
        .execute_transaction(Transaction::from_generic_sig_data(tx_data, vec![sig]))
        .await;

    assert!(
        account_object(&test_cluster, sender).await.is_none(),
        "implicit authentication must not create an account object"
    );
    Ok(())
}

/// Claim → explicit transition under P-COOL. The claim creates the account;
/// afterwards a plain-signed transfer is rejected (claimed accounts require a
/// `MoveAuthenticator`), while the same transfer via a `MoveAuthenticator`
/// succeeds.
#[sim_test]
async fn test_claim_then_plain_pcool() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let test_cluster = TestClusterBuilder::new().with_pcool_enabled().build().await;
    let mut actor = Actor::ed25519(41);
    let sender = actor.address();

    // Claim the sender's address (the claim itself authenticates implicitly).
    let gas = fund(&test_cluster, sender).await;
    let registry = claim_registry_arg(&test_cluster).await;
    let pt = claim_ptb(registry, actor.prefixed_pk_bytes(), false)?;
    let tx_data = ptb_tx_data(&test_cluster, sender, gas, pt).await;
    let sig = actor.sign(&tx_data).await;
    test_cluster
        .execute_transaction(Transaction::from_generic_sig_data(tx_data, vec![sig]))
        .await;
    assert!(
        account_object(&test_cluster, sender).await.is_some(),
        "the account object must exist after the claim"
    );

    // A follow-up plain transfer from the now-claimed account, in a later
    // commit, is rejected: a claimed account can only be authenticated via a
    // `MoveAuthenticator`.
    let gas = fund(&test_cluster, sender).await;
    let tx_data = transfer_tx_data(&test_cluster, sender, gas).await;
    let sig = actor.sign(&tx_data).await;
    let res = test_cluster
        .wallet
        .execute_transaction_may_fail(Transaction::from_generic_sig_data(tx_data, vec![sig]))
        .await;
    let succeeded = res.as_ref().ok().and_then(|r| r.status_ok()) == Some(true);
    assert!(
        !succeeded,
        "a plain signature on a claimed account must be rejected, got {res:?}"
    );
    Ok(())
}

/// Determinism race (claim-then-plain): a `claim` of A and a plain transfer
/// from A, both signed with A's key, submitted concurrently and sequenced into
/// the same consensus commit.
///
/// Under the current white-flag fix (issue #11900) the claim is validated first
/// and the racing plain transfer — whose sender matches the claimed address —
/// is deterministically dropped with `AccountClaimConflict` before it executes.
/// Every validator agrees, so the network does not fork; the dropped transfer
/// never reaches finality and is retryable in a later commit.
#[sim_test]
async fn test_claim_plain_race_pcool() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let test_cluster = TestClusterBuilder::new().with_pcool_enabled().build().await;
    let mut actor = Actor::ed25519(42);
    let sender = actor.address();

    // Independent gas coins so the two transactions do not conflict on gas.
    let claim_gas = fund(&test_cluster, sender).await;
    let transfer_gas = fund(&test_cluster, sender).await;

    let registry = claim_registry_arg(&test_cluster).await;
    let claim_pt = claim_ptb(registry, actor.prefixed_pk_bytes(), false)?;
    let claim_data = ptb_tx_data(&test_cluster, sender, claim_gas, claim_pt).await;
    let claim_sig = actor.sign(&claim_data).await;
    let claim_tx = Transaction::from_generic_sig_data(claim_data, vec![claim_sig]);

    let transfer_data = transfer_tx_data(&test_cluster, sender, transfer_gas).await;
    let transfer_sig = actor.sign(&transfer_data).await;
    let transfer_tx = Transaction::from_generic_sig_data(transfer_data, vec![transfer_sig]);

    let (claim_res, transfer_res) = tokio::join!(
        test_cluster.wallet.execute_transaction_may_fail(claim_tx),
        test_cluster
            .wallet
            .execute_transaction_may_fail(transfer_tx),
    );

    // The claim is a white-flag "claimer": validated first, never dropped.
    assert_eq!(claim_res?.status_ok(), Some(true), "claim must succeed");

    // The racing plain transfer shares the claimed sender, so it is dropped
    // (`AccountClaimConflict`) and never finalizes.
    let transfer_succeeded = transfer_res.as_ref().ok().and_then(|r| r.status_ok()) == Some(true);
    assert!(
        !transfer_succeeded,
        "plain transfer racing the claim must be dropped, not finalized, got {transfer_res:?}"
    );

    assert!(
        account_object(&test_cluster, sender).await.is_some(),
        "the account must exist after the claim"
    );
    Ok(())
}

/// Determinism race with an IMMUTABLE claim: a `claim` of A finalized with
/// `build_immutable_v1` and a plain transfer from A, submitted concurrently into
/// the same consensus commit. A claim is detected by its use of the
/// `ClaimRegistry` (not the finalizer name), so the immutable claim is
/// recognized and the racing transfer is dropped with `AccountClaimConflict`.
/// Regression for the previous `build_v1`-name detection, under which an
/// immutable claim went undetected and the racing transfer would finalize.
#[sim_test]
async fn test_claim_immutable_plain_race_pcool() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let test_cluster = TestClusterBuilder::new().with_pcool_enabled().build().await;
    let mut actor = Actor::ed25519(45);
    let sender = actor.address();

    // Independent gas coins so the two transactions do not conflict on gas.
    let claim_gas = fund(&test_cluster, sender).await;
    let transfer_gas = fund(&test_cluster, sender).await;

    let registry = claim_registry_arg(&test_cluster).await;
    // `true` => finalize the claim with `build_immutable_v1`.
    let claim_pt = claim_ptb(registry, actor.prefixed_pk_bytes(), true)?;
    let claim_data = ptb_tx_data(&test_cluster, sender, claim_gas, claim_pt).await;
    let claim_sig = actor.sign(&claim_data).await;
    let claim_tx = Transaction::from_generic_sig_data(claim_data, vec![claim_sig]);

    let transfer_data = transfer_tx_data(&test_cluster, sender, transfer_gas).await;
    let transfer_sig = actor.sign(&transfer_data).await;
    let transfer_tx = Transaction::from_generic_sig_data(transfer_data, vec![transfer_sig]);

    let (claim_res, transfer_res) = tokio::join!(
        test_cluster.wallet.execute_transaction_may_fail(claim_tx),
        test_cluster
            .wallet
            .execute_transaction_may_fail(transfer_tx),
    );

    assert_eq!(
        claim_res?.status_ok(),
        Some(true),
        "immutable claim must succeed"
    );

    // The racing plain transfer shares the claimed sender, so it is dropped
    // (`AccountClaimConflict`) and never finalizes — even though the claim was
    // finalized with `build_immutable_v1`.
    let transfer_succeeded = transfer_res.as_ref().ok().and_then(|r| r.status_ok()) == Some(true);
    assert!(
        !transfer_succeeded,
        "plain transfer racing the immutable claim must be dropped, not finalized, got {transfer_res:?}"
    );

    assert!(
        account_object(&test_cluster, sender).await.is_some(),
        "the account must exist after the immutable claim"
    );
    Ok(())
}

/// rotate-then-plain (sequential) under P-COOL: after claiming A and rotating
/// its on-chain key (via a `MoveAuthenticator`), a plain transfer signed with
/// the OLD key is rejected — once claimed, A only accepts a
/// `MoveAuthenticator`.
#[sim_test]
async fn test_rotated_old_key_rejected_pcool() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let test_cluster = TestClusterBuilder::new().with_pcool_enabled().build().await;
    let mut actor = Actor::ed25519(43);
    let sender = actor.address();

    // Claim, then rotate the on-chain key to a fresh one (the rotation is
    // authenticated by the current/old key, so it succeeds).
    let gas = fund(&test_cluster, sender).await;
    let registry = claim_registry_arg(&test_cluster).await;
    let claim_pt = claim_ptb(registry, actor.prefixed_pk_bytes(), false)?;
    let claim_data = ptb_tx_data(&test_cluster, sender, gas, claim_pt).await;
    let claim_sig = actor.sign(&claim_data).await;
    test_cluster
        .execute_transaction(Transaction::from_generic_sig_data(
            claim_data,
            vec![claim_sig],
        ))
        .await;

    let new_key =
        IotaKeyPair::Ed25519(Ed25519KeyPair::generate(&mut StdRng::from_seed([44u8; 32])));
    let rotate_gas = fund(&test_cluster, sender).await;
    let account_arg = shared_account_arg(&test_cluster, sender, true).await;
    let rotate_pt = rotate_pk_ptb(account_arg, prefixed_pk_of(&new_key))?;
    let rotate_data = ptb_tx_data(&test_cluster, sender, rotate_gas, rotate_pt).await;
    // The account is claimed, so the rotation is authenticated via a
    // `MoveAuthenticator` (a plain signature would be rejected).
    let rotate_auth_account = shared_account_arg(&test_cluster, sender, false).await;
    let rotate_auth = move_auth_sign(&mut actor, &rotate_data, rotate_auth_account).await?;
    test_cluster
        .execute_transaction(Transaction::from_generic_sig_data(
            rotate_data,
            vec![rotate_auth],
        ))
        .await;

    // Plain transfer signed with the OLD key (the actor still holds it).
    let transfer_gas = fund(&test_cluster, sender).await;
    let transfer_data = transfer_tx_data(&test_cluster, sender, transfer_gas).await;
    let old_key_sig = actor.sign(&transfer_data).await;
    let res = test_cluster
        .wallet
        .execute_transaction_may_fail(Transaction::from_generic_sig_data(
            transfer_data,
            vec![old_key_sig],
        ))
        .await;

    let succeeded = res
        .as_ref()
        .map(|r| r.status_ok() == Some(true))
        .unwrap_or(false);
    assert!(
        !succeeded,
        "the old key must be rejected after rotation, got {res:?}"
    );
    Ok(())
}

// ---------------------------------------------------
// --- Signing actors --------------------------------
// ---------------------------------------------------

/// Type-erased passkey signing closure: the mock WebAuthn client's concrete
/// type is unnameable here (its TLD-provider type parameter comes from a
/// non-dependency crate), so `passkey_actor!` captures it inside this boxed
/// closure instead.
type PasskeySigner =
    Box<dyn FnMut(TransactionData) -> Pin<Box<dyn Future<Output = GenericSignature>>>>;

/// A signer for one of the built-in schemes. Collapses the per-scheme test
/// matrices: address derivation, Move-side flag-prefixed public key bytes and
/// transaction signing.
enum Actor {
    Simple(Box<IotaKeyPair>),
    MultiSig {
        keys: Vec<IotaKeyPair>,
        multisig_pk: MultiSigPublicKey,
    },
    Passkey {
        address: Address,
        prefixed_pk: Vec<u8>,
        signer: PasskeySigner,
    },
}

impl Actor {
    fn ed25519(seed: u8) -> Self {
        Self::Simple(Box::new(IotaKeyPair::Ed25519(Ed25519KeyPair::generate(
            &mut StdRng::from_seed([seed; 32]),
        ))))
    }

    fn secp256k1(seed: u8) -> Self {
        Self::Simple(Box::new(IotaKeyPair::Secp256k1(
            Secp256k1KeyPair::generate(&mut StdRng::from_seed([seed; 32])),
        )))
    }

    fn secp256r1(seed: u8) -> Self {
        Self::Simple(Box::new(IotaKeyPair::Secp256r1(
            Secp256r1KeyPair::generate(&mut StdRng::from_seed([seed; 32])),
        )))
    }

    /// 2-of-3 committee mixing all three simple schemes.
    fn multisig_mixed(seed: u8) -> Self {
        let keys = vec![
            IotaKeyPair::Ed25519(Ed25519KeyPair::generate(&mut StdRng::from_seed([seed; 32]))),
            IotaKeyPair::Secp256k1(Secp256k1KeyPair::generate(&mut StdRng::from_seed(
                [seed.wrapping_add(1); 32],
            ))),
            IotaKeyPair::Secp256r1(Secp256r1KeyPair::generate(&mut StdRng::from_seed(
                [seed.wrapping_add(2); 32],
            ))),
        ];
        let multisig_pk = MultiSigPublicKey::new(
            keys.iter()
                .map(|k| MultisigMember::new(to_sdk_public_key(&k.public()), 1))
                .collect(),
            2,
        )
        .expect("valid multisig committee");
        Self::MultiSig { keys, multisig_pk }
    }

    fn address(&self) -> Address {
        match self {
            Self::Simple(kp) => Address::from(&kp.public()),
            Self::MultiSig { multisig_pk, .. } => Address::from(multisig_pk),
            Self::Passkey { address, .. } => *address,
        }
    }

    /// Flag-prefixed public key bytes as expected by Move's
    /// `public_key::from_prefixed_bytes`. For MultiSig the raw bytes are the
    /// BCS-encoded `MultiSigPublicKey`.
    fn prefixed_pk_bytes(&self) -> Vec<u8> {
        match self {
            Self::Simple(kp) => {
                let pk = kp.public();
                let mut bytes = vec![pk.scheme().flag()];
                bytes.extend_from_slice(pk.as_ref());
                bytes
            }
            Self::MultiSig { multisig_pk, .. } => {
                let mut bytes = vec![SignatureScheme::MultiSig.flag()];
                bytes.extend_from_slice(
                    &bcs::to_bytes(multisig_pk).expect("MultiSigPublicKey is BCS-serializable"),
                );
                bytes
            }
            Self::Passkey { prefixed_pk, .. } => prefixed_pk.clone(),
        }
    }

    /// Signs `tx_data` and returns the plain (non-MoveAuthenticator)
    /// `GenericSignature` that the node maps to an implicit account
    /// object.
    ///
    /// Takes `&mut self` because the passkey signer drives a mock WebAuthn
    /// client.
    async fn sign(&mut self, tx_data: &TransactionData) -> GenericSignature {
        let intent_msg = IntentMessage::new(Intent::iota_transaction(), tx_data.clone());
        match self {
            Self::Simple(kp) => {
                GenericSignature::Signature(IotaSignature::new_secure(&intent_msg, kp.as_ref()))
            }
            Self::MultiSig { keys, multisig_pk } => {
                // Sign with the first `threshold` keys.
                let sigs = keys
                    .iter()
                    .take(multisig_pk.threshold() as usize)
                    .map(|kp| {
                        let sig =
                            GenericSignature::Signature(IotaSignature::new_secure(&intent_msg, kp));
                        UserSignature::try_from(sig).expect("simple signature is convertible")
                    })
                    .collect();
                GenericSignature::MultiSig(
                    MultiSig::new(sigs, multisig_pk.clone())
                        .expect("multisig combination must succeed"),
                )
            }
            Self::Passkey { signer, .. } => signer(tx_data.clone()).await,
        }
    }
}

/// Flag-prefixed public key bytes for a single keypair (see
/// `Actor::prefixed_pk_bytes`).
fn prefixed_pk_of(kp: &IotaKeyPair) -> Vec<u8> {
    let pk = kp.public();
    let mut bytes = vec![pk.scheme().flag()];
    bytes.extend_from_slice(pk.as_ref());
    bytes
}

// ---------------------------------------------------
// --- Cluster / transaction helpers -----------------
// ---------------------------------------------------

fn ident(name: &str) -> Identifier {
    Identifier::new(name).expect("valid identifier")
}

/// Converts a node-internal [`PublicKey`] into the SDK public key used by the
/// multisig committee types.
fn to_sdk_public_key(pk: &PublicKey) -> SdkPublicKey {
    SdkPublicKey::from_base64(&pk.encode_base64()).expect("valid public key")
}

async fn fund(test_cluster: &TestCluster, address: Address) -> ObjectRef {
    let rgp = test_cluster.get_reference_gas_price().await;
    test_cluster
        .fund_address_and_return_gas(rgp, Some(GAS_AMOUNT), address)
        .await
}

/// Builds a minimal transfer `TransactionData` for `sender`.
async fn transfer_tx_data(
    test_cluster: &TestCluster,
    sender: Address,
    gas: ObjectRef,
) -> TransactionData {
    let rgp = test_cluster.get_reference_gas_price().await;
    TestTransactionBuilder::new(sender, gas, rgp)
        .transfer_iota(Some(1), Address::ZERO)
        .build()
}

/// Builds a `TransactionData` running `pt` with `sender`'s gas.
async fn ptb_tx_data(
    test_cluster: &TestCluster,
    sender: Address,
    gas: ObjectRef,
    pt: ProgrammableTransaction,
) -> TransactionData {
    let rgp = test_cluster.get_reference_gas_price().await;
    TestTransactionBuilder::new(sender, gas, rgp)
        .programmable(pt)
        .build()
}

/// Returns the object at the implicit account ID derived from `address`,
/// if any.
async fn account_object(test_cluster: &TestCluster, address: Address) -> Option<Object> {
    test_cluster
        .get_object_from_fullnode_store(&ObjectId::from(address))
        .await
}

/// The genesis `ClaimRegistry` shared object as a mutable PTB input.
async fn claim_registry_arg(test_cluster: &TestCluster) -> CallArg {
    let registry = test_cluster
        .get_object_from_fullnode_store(&ObjectId::CLAIM_REGISTRY)
        .await
        .expect("ClaimRegistry must exist at genesis");
    let initial_shared_version = match &registry.owner {
        Owner::Shared(initial_shared_version) => *initial_shared_version,
        owner => panic!("ClaimRegistry must be shared, found {owner:?}"),
    };
    CallArg::Shared(SharedObjectRef::new(
        ObjectId::CLAIM_REGISTRY,
        initial_shared_version,
        true,
    ))
}

/// The shared `SmartAccount` object at `address` as a PTB / authenticator
/// input.
async fn shared_account_arg(
    test_cluster: &TestCluster,
    address: Address,
    mutable: bool,
) -> CallArg {
    let account = account_object(test_cluster, address)
        .await
        .expect("SmartAccount must exist");
    let initial_shared_version = match &account.owner {
        Owner::Shared(initial_shared_version) => *initial_shared_version,
        owner => panic!("SmartAccount must be shared, found {owner:?}"),
    };
    CallArg::Shared(SharedObjectRef::new(
        ObjectId::from(address),
        initial_shared_version,
        mutable,
    ))
}

/// Builds the `MoveAuthenticator` auth-object `CallArg` for a claimed account,
/// handling both the shared (read-only) and immutable cases.
async fn account_auth_arg(
    test_cluster: &TestCluster,
    address: Address,
    immutable: bool,
) -> CallArg {
    if immutable {
        let account = account_object(test_cluster, address)
            .await
            .expect("SmartAccount must exist");
        CallArg::ImmutableOrOwned(account.object_ref())
    } else {
        shared_account_arg(test_cluster, address, false).await
    }
}

/// PTB claiming the sender's own address as a `SmartAccount`:
/// `public_key::from_prefixed_bytes` -> `smart_account::claim_builder_v1` ->
/// `smart_account::{build_v1|build_immutable_v1}`.
fn claim_ptb(
    registry: CallArg,
    prefixed_pk: Vec<u8>,
    immutable: bool,
) -> anyhow::Result<ProgrammableTransaction> {
    let mut builder = ProgrammableTransactionBuilder::new();
    let pk_bytes_arg = builder.pure(prefixed_pk)?;
    let pk = builder.programmable_move_call(
        ObjectId::FRAMEWORK,
        ident(PUBLIC_KEY_MODULE),
        ident("from_prefixed_bytes"),
        vec![],
        vec![pk_bytes_arg],
    );
    let registry_arg = builder.obj(registry)?;
    let account_builder = builder.programmable_move_call(
        ObjectId::FRAMEWORK,
        ident(SMART_ACCOUNT_MODULE),
        ident("claim_builder_v1"),
        vec![],
        vec![registry_arg, pk],
    );
    builder.programmable_move_call(
        ObjectId::FRAMEWORK,
        ident(SMART_ACCOUNT_MODULE),
        ident(if immutable {
            "build_immutable_v1"
        } else {
            "build_v1"
        }),
        vec![],
        vec![account_builder],
    );
    Ok(builder.finish())
}

/// PTB rotating the built-in authenticator public key of `account` to
/// `new_prefixed_pk`. The returned previous `PublicKey` is copy+drop, so it is
/// safe to leave unconsumed.
fn rotate_pk_ptb(
    account: CallArg,
    new_prefixed_pk: Vec<u8>,
) -> anyhow::Result<ProgrammableTransaction> {
    let mut builder = ProgrammableTransactionBuilder::new();
    let pk_bytes_arg = builder.pure(new_prefixed_pk)?;
    let pk = builder.programmable_move_call(
        ObjectId::FRAMEWORK,
        ident(PUBLIC_KEY_MODULE),
        ident("from_prefixed_bytes"),
        vec![],
        vec![pk_bytes_arg],
    );
    let account_arg = builder.obj(account)?;
    builder.programmable_move_call(
        ObjectId::FRAMEWORK,
        ident(SMART_ACCOUNT_MODULE),
        ident("rotate_builtin_auth_public_key"),
        vec![],
        vec![account_arg, pk],
    );
    Ok(builder.finish())
}

/// PTB detaching the built-in authenticator public key from `account`.
fn detach_pk_ptb(account: CallArg) -> anyhow::Result<ProgrammableTransaction> {
    let mut builder = ProgrammableTransactionBuilder::new();
    let account_arg = builder.obj(account)?;
    builder.programmable_move_call(
        ObjectId::FRAMEWORK,
        ident(SMART_ACCOUNT_MODULE),
        ident("detach_builtin_auth_public_key"),
        vec![],
        vec![account_arg],
    );
    Ok(builder.finish())
}

/// Wraps `GenericSignature` wire bytes in a hand-crafted `MoveAuthenticator`
/// that authenticates against `account`.
fn builtin_move_authenticator(
    wire_bytes: &[u8],
    account: CallArg,
) -> anyhow::Result<GenericSignature> {
    Ok(GenericSignature::MoveAuthenticator(
        MoveAuthenticator::new_v1(
            vec![CallArg::Pure(bcs::to_bytes(&wire_bytes.to_vec())?)],
            vec![],
            account,
        ),
    ))
}

/// Authenticates `tx_data` against a claimed `account` via a hand-crafted
/// `MoveAuthenticator` wrapping `actor`'s signature. Required for any operation
/// on a claimed account, because a plain signature on a claimed account is
/// rejected (`PlainSignatureOnClaimedAccount`).
async fn move_auth_sign(
    actor: &mut Actor,
    tx_data: &TransactionData,
    account: CallArg,
) -> anyhow::Result<GenericSignature> {
    let wire_bytes = actor.sign(tx_data).await.to_bytes();
    builtin_move_authenticator(&wire_bytes, account)
}

/// Executes `tx` to finality and asserts the execution succeeded.
async fn execute_and_assert_success(
    test_cluster: &TestCluster,
    tx: Transaction,
) -> anyhow::Result<TransactionEffects> {
    let (effects, _) = test_cluster
        .execute_transaction_return_raw_effects(tx)
        .await?;
    assert!(
        effects.status().is_success(),
        "transaction failed: {:?}",
        effects.status()
    );
    Ok(effects)
}

/// Executes `tx` to finality and asserts it aborted in
/// `iota::claim_registry`.
async fn execute_and_assert_claim_registry_abort(
    test_cluster: &TestCluster,
    tx: Transaction,
) -> anyhow::Result<()> {
    let (effects, _) = test_cluster
        .execute_transaction_return_raw_effects(tx)
        .await?;
    let ExecutionStatus::Failure { error, .. } = effects.status() else {
        panic!("expected execution failure, got {:?}", effects.status());
    };
    assert!(
        matches!(
            error,
            ExecutionError::MoveAbort { location: MoveLocation { module, .. }, .. }
                if module.as_str() == "claim_registry"
        ),
        "expected a claim_registry abort, got {error:?}"
    );
    Ok(())
}

/// Submits `tx` directly to a single validator, surfacing signing-time errors
/// that `execute_transaction` would swallow.
async fn handle_tx(
    test_cluster: &TestCluster,
    tx: Transaction,
) -> Result<HandleTransactionResponse, IotaError> {
    test_cluster
        .authority_aggregator()
        .authority_clients
        .values()
        .next()
        .unwrap()
        .authority_client()
        .handle_transaction(tx, Some(SocketAddr::new([127, 0, 0, 1].into(), 0)))
        .await
}

/// Extracts the freshly created shared object (the `SmartAccount`) from
/// transaction effects.
fn created_shared_object(effects: &TransactionEffects) -> ObjectRef {
    effects
        .all_changed_objects()
        .iter()
        .find_map(|change| match change {
            (obj_ref, Owner::Shared(_), WriteKind::Create) => Some(*obj_ref),
            _ => None,
        })
        .expect("expected a created shared object")
}

// ---------------------------------------------------
// --- Shared test flows ------------------------------
// ---------------------------------------------------

/// IMPLICIT happy path: no account object exists for a fresh key; a
/// plain-signed transaction authenticates via the synthetic builtin
/// authenticator with scheme and public key derived from the signature.
async fn run_implicit_happy_path(mut actor: Actor) -> anyhow::Result<()> {
    let test_cluster = TestClusterBuilder::new().build().await;
    let sender = actor.address();

    assert!(
        account_object(&test_cluster, sender).await.is_none(),
        "a fresh key must not have an account object on chain"
    );

    let gas = fund(&test_cluster, sender).await;
    let tx_data = transfer_tx_data(&test_cluster, sender, gas).await;
    let sig = actor.sign(&tx_data).await;
    execute_and_assert_success(
        &test_cluster,
        Transaction::from_generic_sig_data(tx_data, vec![sig]),
    )
    .await?;

    // The implicit flow does not materialize an account object.
    assert!(
        account_object(&test_cluster, sender).await.is_none(),
        "implicit authentication must not create an account object"
    );
    Ok(())
}

/// Claims the sender's own address as an explicit `SmartAccount` with a
/// plain-signed claim PTB (the claim transaction itself authenticates via the
/// IMPLICIT branch) and asserts the account object now exists at the sender
/// address.
async fn claim_account(
    test_cluster: &TestCluster,
    actor: &mut Actor,
    immutable: bool,
) -> anyhow::Result<ObjectRef> {
    let sender = actor.address();
    assert!(
        account_object(test_cluster, sender).await.is_none(),
        "address must be unclaimed before the claim"
    );

    let gas = fund(test_cluster, sender).await;
    let registry = claim_registry_arg(test_cluster).await;
    let pt = claim_ptb(registry, actor.prefixed_pk_bytes(), immutable)?;
    let tx_data = ptb_tx_data(test_cluster, sender, gas, pt).await;
    let sig = actor.sign(&tx_data).await;
    execute_and_assert_success(
        test_cluster,
        Transaction::from_generic_sig_data(tx_data, vec![sig]),
    )
    .await?;

    let account = account_object(test_cluster, sender)
        .await
        .expect("SmartAccount must exist at the sender address after the claim");
    let account_ref = account.object_ref();
    assert_eq!(account_ref.object_id, ObjectId::from(sender));
    if immutable {
        assert!(
            matches!(account.owner, Owner::Immutable),
            "expected an immutable account, found {:?}",
            account.owner
        );
    } else {
        assert!(
            matches!(account.owner, Owner::Shared(_)),
            "expected a shared account, found {:?}",
            account.owner
        );
    }
    Ok(account_ref)
}

/// Full implicit -> explicit transition: claim (implicit branch), after which
/// the account has an on-chain object. A subsequent plain signature is now
/// rejected (claimed accounts require a `MoveAuthenticator`); the same transfer
/// succeeds when wrapped in a `MoveAuthenticator` targeting the account.
async fn run_claim_transition(mut actor: Actor, immutable: bool) -> anyhow::Result<()> {
    let test_cluster = TestClusterBuilder::new().build().await;
    let sender = actor.address();

    claim_account(&test_cluster, &mut actor, immutable).await?;

    let gas = fund(&test_cluster, sender).await;
    let tx_data = transfer_tx_data(&test_cluster, sender, gas).await;

    // A plain signature on the now-claimed account is rejected.
    let plain_sig = actor.sign(&tx_data).await;
    let err = handle_tx(
        &test_cluster,
        Transaction::from_generic_sig_data(tx_data.clone(), vec![plain_sig]),
    )
    .await
    .expect_err("a plain signature on a claimed account must be rejected");
    assert!(
        format!("{err:?}").contains("PlainSignatureOnClaimedAccount"),
        "expected PlainSignatureOnClaimedAccount, got {err:?}"
    );

    // The same transfer succeeds via a `MoveAuthenticator` targeting the account.
    let account = account_auth_arg(&test_cluster, sender, immutable).await;
    let auth_sig = move_auth_sign(&mut actor, &tx_data, account).await?;
    execute_and_assert_success(
        &test_cluster,
        Transaction::from_generic_sig_data(tx_data, vec![auth_sig]),
    )
    .await?;
    Ok(())
}

/// Rotates the built-in public key of the claimed account at
/// `account_address` to `new_prefixed_pk`, authorized by `current` (the
/// current key holder).
async fn rotate_account_pk(
    test_cluster: &TestCluster,
    actor: &mut Actor,
    new_prefixed_pk: Vec<u8>,
) -> anyhow::Result<()> {
    let sender = actor.address();
    let account = shared_account_arg(test_cluster, sender, true).await;
    let pt = rotate_pk_ptb(account, new_prefixed_pk)?;
    let gas = fund(test_cluster, sender).await;
    let tx_data = ptb_tx_data(test_cluster, sender, gas, pt).await;
    // The account is claimed, so it must be authenticated via a `MoveAuthenticator`
    // (read-only auth object; the PTB above takes it as a mutable input and the two
    // are merged). A plain signature would be rejected.
    let auth_account = shared_account_arg(test_cluster, sender, false).await;
    let auth_sig = move_auth_sign(actor, &tx_data, auth_account).await?;
    execute_and_assert_success(
        test_cluster,
        Transaction::from_generic_sig_data(tx_data, vec![auth_sig]),
    )
    .await?;
    Ok(())
}

// ---------------------------------------------------
// --- Passkey helpers --------------------------------
// ---------------------------------------------------

/// Minimal `UserValidationMethod` that always approves — used by the mock
/// WebAuthn client.
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

/// The passkey macros live in a module so they can be imported by path at the
/// top of the file (path-based macro imports are not bound by the textual
/// ordering rules of `macro_rules!`), keeping the tests above the helpers.
mod passkey_util {
    /// Registers a fresh passkey credential with a mock WebAuthn client and
    /// expands to `(client, prefixed_pk_bytes)`.
    ///
    /// This is a macro rather than a function because the client's concrete
    /// type names a TLD-provider type from a crate this test crate does not
    /// depend on; at the expansion site the type is simply inferred.
    macro_rules! register_passkey {
        ($origin:expr) => {{
            let store: Option<Passkey> = None;
            let authenticator = PasskeyClient::new(Aaguid::new_empty(), store, AlwaysApprove);
            let mut client = WebAuthnClient::new(authenticator);

            let creation_opts = CredentialCreationOptions {
                public_key: PublicKeyCredentialCreationOptions {
                    rp: PublicKeyCredentialRpEntity {
                        id: None,
                        name: $origin.domain().unwrap().into(),
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
            let credential = client
                .register($origin, creation_opts, None)
                .await
                .expect("passkey registration failed");

            // Derive the compressed Secp256r1 public key from the DER-encoded
            // WebAuthn key and prefix it with the Passkey scheme flag.
            let verifying_key = p256::ecdsa::VerifyingKey::from_public_key_der(
                credential.response.public_key.unwrap().as_slice(),
            )
            .expect("invalid DER public key");
            let ep = verifying_key.to_encoded_point(false);
            let parity = if ep.y().unwrap()[31] % 2 == 0 {
                0x02
            } else {
                0x03
            };
            let mut prefixed_pk = vec![SignatureScheme::PasskeyAuthenticator.flag(), parity];
            prefixed_pk.extend_from_slice(ep.x().unwrap());
            (client, prefixed_pk)
        }};
    }

    /// Signs `tx_data` with the given mock WebAuthn client and expands to a
    /// `GenericSignature::PasskeyAuthenticator`. `$prefixed_pk` is the
    /// flag-prefixed public key returned by `register_passkey!`.
    macro_rules! passkey_sign {
        ($client:expr, $origin:expr, $prefixed_pk:expr, $tx_data:expr) => {{
            let intent_msg = IntentMessage::new(Intent::iota_transaction(), $tx_data.clone());
            let challenge: Bytes = intent_msg.signing_digest().as_bytes().to_vec().into();

            let request = CredentialRequestOptions {
                public_key: PublicKeyCredentialRequestOptions {
                    challenge,
                    timeout: None,
                    rp_id: Some($origin.domain().unwrap().into()),
                    allow_credentials: None,
                    user_verification: UserVerificationRequirement::default(),
                    attestation: Default::default(),
                    attestation_formats: None,
                    extensions: None,
                    hints: None,
                },
            };
            let auth_cred = $client
                .authenticate($origin, request, None)
                .await
                .expect("passkey authentication failed");

            // Build the Secp256r1 user signature in wire format (flag || sig || pk).
            let sig = p256::ecdsa::Signature::from_der(auth_cred.response.signature.as_slice())
                .expect("invalid DER signature");
            let sig_bytes = sig.normalize_s().unwrap_or(sig).to_bytes();
            let mut user_sig_bytes = vec![SignatureScheme::Secp256r1.flag()];
            user_sig_bytes.extend_from_slice(&sig_bytes);
            // Strip the Passkey scheme flag: the user signature carries the raw
            // compressed key under the Secp256r1 flag.
            user_sig_bytes.extend_from_slice(&$prefixed_pk[1..]);

            GenericSignature::PasskeyAuthenticator(
                PasskeyAuthenticator::new(
                    auth_cred.response.authenticator_data.as_slice().to_vec(),
                    String::from_utf8_lossy(auth_cred.response.client_data_json.as_slice()).into(),
                    SimpleSignature::from_bytes(&user_sig_bytes).expect("invalid wire signature"),
                )
                .expect("invalid passkey authenticator"),
            )
        }};
    }

    /// Builds an `Actor::Passkey`: registers a fresh credential and wraps the
    /// mock WebAuthn client into the type-erased `PasskeySigner` closure, so
    /// passkey tests can reuse the same shared flows as the other schemes.
    macro_rules! passkey_actor {
        () => {{
            let origin = Url::parse("https://www.iota.org").unwrap();
            let (client, prefixed_pk) = register_passkey!(&origin);

            let pk =
                PublicKey::try_from_bytes(SignatureScheme::PasskeyAuthenticator, &prefixed_pk[1..])
                    .expect("valid passkey public key");
            let address = Address::from(&pk);

            // An async-aware mutex: the client is mutably borrowed across the
            // WebAuthn `authenticate` await inside `passkey_sign!`.
            let client = std::rc::Rc::new(tokio::sync::Mutex::new(client));
            let signer_pk = prefixed_pk.clone();
            let signer: PasskeySigner = Box::new(move |tx_data: TransactionData| {
                let client = client.clone();
                let origin = origin.clone();
                let prefixed_pk = signer_pk.clone();
                Box::pin(async move {
                    let mut client = client.lock().await;
                    passkey_sign!(client, &origin, prefixed_pk, tx_data)
                })
            });

            Actor::Passkey {
                address,
                prefixed_pk,
                signer,
            }
        }};
    }

    pub(crate) use passkey_actor;
    pub(crate) use passkey_sign;
    pub(crate) use register_passkey;
}
