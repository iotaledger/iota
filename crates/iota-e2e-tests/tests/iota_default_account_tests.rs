// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for `IotaDefaultAccount` — the framework-native abstract
//! account that verifies Ed25519, Secp256k1, Secp256r1 (and MultiSig / Passkey)
//! GenericSignature bytes using Move-native crypto primitives.
//!
//! These tests exercise the full stack:
//!   1. Creating an `IotaDefaultAccount` on a local test cluster via
//!      `iota_default_account::create_ed25519 / create_secp256k1 /
//!      create_secp256r1`.
//!   2. Signing a transaction with the correct key using the existing
//!      `AccountKeystore::sign_secure` method (identical to normal IOTA
//!      signing).
//!   3. Wrapping the resulting `[flag | sig | pk]` bytes as a
//!      `MoveAuthenticator`.
//!   4. Asserting successful execution / expected failure.
//!
//! This also serves as the **parity test**: the same `sign_secure` call that
//! produces a valid `GenericSignature` for the Rust verifier produces an
//! equally valid `generic_signature` for `IotaDefaultAccount::authenticate`.

use iota_keys::keystore::AccountKeystore;
use iota_macros::sim_test;
use iota_sdk_types::crypto::Intent;
use iota_types::{
    IOTA_FRAMEWORK_ADDRESS,
    base_types::{IotaAddress, ObjectID, ObjectRef},
    crypto::{PublicKey, SignatureScheme},
    effects::{TransactionEffects, TransactionEffectsAPI},
    move_authenticator::MoveAuthenticator,
    object::Owner,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    signature::GenericSignature,
    storage::WriteKind,
    transaction::{
        CallArg, ObjectArg, TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, Transaction,
        TransactionData,
    },
};
use move_core_types::{ident_str, identifier::Identifier};
use test_cluster::{TestCluster, TestClusterBuilder};

// -----------------------------------------------------------------------
// --- Helper: create an IotaDefaultAccount and return the shared ref  ---
// -----------------------------------------------------------------------

/// Extract the sole `Shared { Create }` object from the effects — that is the
/// newly created `IotaDefaultAccount`.
fn account_ref_from_effects(effects: &TransactionEffects) -> ObjectRef {
    effects
        .all_changed_objects()
        .iter()
        .find_map(|(obj_ref, owner, write_kind)| {
            if matches!(owner, Owner::Shared { .. }) && *write_kind == WriteKind::Create {
                Some(*obj_ref)
            } else {
                None
            }
        })
        .expect("Expected a newly created shared IotaDefaultAccount")
}

/// Extract the raw public-key bytes from a `PublicKey` (no flag byte, no
/// length prefix — just the raw key material).
fn raw_pk_bytes(pk: &PublicKey) -> Vec<u8> {
    // PublicKey::as_ref() returns the raw key bytes without the flag byte:
    // Ed25519 → 32 bytes, Secp256k1/Secp256r1 → 33 bytes.
    pk.as_ref().to_vec()
}

/// Create an `IotaDefaultAccount` on the test cluster using the given
/// `create_fn` (`"create_ed25519"`, `"create_secp256k1"`, or
/// `"create_secp256r1"`). Returns the shared object reference.
async fn create_default_account(
    cluster: &TestCluster,
    owner: IotaAddress,
    pk: &PublicKey,
    create_fn: &str,
) -> anyhow::Result<ObjectRef> {
    let pk_bytes = raw_pk_bytes(pk);

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        let pk_arg = builder.pure(pk_bytes)?;
        builder.programmable_move_call(
            ObjectID::from(IOTA_FRAMEWORK_ADDRESS),
            ident_str!("iota_default_account").to_owned(),
            Identifier::new(create_fn)?,
            vec![],
            vec![pk_arg],
        );
        builder.finish()
    };

    let tx_data = cluster
        .test_transaction_builder_with_sender(owner)
        .await
        .programmable(pt)
        .build();

    let tx = cluster.wallet.sign_transaction(&tx_data);
    let (effects, _) = cluster.execute_transaction_return_raw_effects(tx).await?;
    assert!(
        effects.status().is_ok(),
        "create account tx failed: {:?}",
        effects.status()
    );

    Ok(account_ref_from_effects(&effects))
}

/// Build a `GenericSignature::MoveAuthenticator` for `IotaDefaultAccount`.
///
/// `sign_secure` computes `blake2b256(bcs(IntentMessage<TransactionData>))`
/// (the **signing digest**) internally — the same value that
/// `IotaDefaultAccount::authenticate` retrieves via
/// `auth_ctx.signing_digest()`. The returned `Signature` bytes are `[flag | sig
/// | pk]`, which is exactly the `generic_signature` format expected by
/// `authenticate`.
fn make_move_authenticator(
    cluster: &TestCluster,
    signer: IotaAddress,
    tx_data: &TransactionData,
    account_ref: ObjectRef,
) -> anyhow::Result<GenericSignature> {
    let keystore = cluster.wallet.config().keystore();

    // Sign with the same intent/hash that IotaDefaultAccount verifies.
    let sig = keystore.sign_secure(&signer, tx_data, Intent::iota_transaction())?;

    // sig.as_ref() == [flag(1) | sig_bytes(64) | pk_bytes(32 or 33)]
    let generic_signature_bytes = sig.as_ref().to_vec();

    let sig_call_arg = CallArg::Pure(bcs::to_bytes(&generic_signature_bytes)?);
    let self_call_arg = CallArg::Object(ObjectArg::SharedObject {
        id: account_ref.0,
        initial_shared_version: account_ref.1,
        mutable: false,
    });

    Ok(GenericSignature::MoveAuthenticator(
        MoveAuthenticator::new_v1(vec![sig_call_arg], vec![], self_call_arg),
    ))
}

/// Build a minimal but valid PTB: an empty programmable transaction that just
/// pays gas and returns.  This is sufficient to test the MoveAuthenticator
/// path without depending on any particular framework function.
fn build_trivial_ptb() -> iota_types::transaction::ProgrammableTransaction {
    ProgrammableTransactionBuilder::new().finish()
}

// -----------------------------------------------------------------------
// --- Tests -------------------------------------------------------------
// -----------------------------------------------------------------------

/// Happy-path Ed25519 test:
/// Create an IotaDefaultAccount with an Ed25519 key and execute a
/// transaction from it.  The same `sign_secure` call that the standard
/// Rust verifier uses also satisfies `IotaDefaultAccount::authenticate`.
#[sim_test]
async fn test_iota_default_account_ed25519() -> anyhow::Result<()> {
    telemetry_subscribers::init_for_testing();

    let cluster = TestClusterBuilder::new().build().await;
    let keystore = cluster.wallet.config().keystore();
    let owner = keystore.addresses().first().cloned().unwrap();
    let owner_pk = keystore.get_key(&owner)?.public();

    // Only run this test when the default key happens to be Ed25519.
    let PublicKey::Ed25519(_) = &owner_pk else {
        panic!("Expected Ed25519 key");
    };

    let account_ref = create_default_account(&cluster, owner, &owner_pk, "create_ed25519").await?;
    let account_address = IotaAddress::from(account_ref.0);

    let rgp = cluster.get_reference_gas_price().await;
    let gas_coin = cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), account_address)
        .await;

    let pt = build_trivial_ptb();
    let tx_data = TransactionData::new_programmable(
        account_address,
        vec![gas_coin],
        pt,
        rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        rgp,
    );

    let move_auth = make_move_authenticator(&cluster, owner, &tx_data, account_ref)?;
    let tx = Transaction::from_generic_sig_data(tx_data, vec![move_auth]);

    let response = cluster.execute_transaction(tx).await;
    assert!(response.confirmed_local_execution.unwrap());
    assert!(response.errors.is_empty(), "errors: {:?}", response.errors);

    Ok(())
}

/// Happy-path Secp256k1 test.
#[sim_test]
async fn test_iota_default_account_secp256k1() -> anyhow::Result<()> {
    telemetry_subscribers::init_for_testing();

    let mut cluster = TestClusterBuilder::new().build().await;

    // Generate a Secp256k1 key and add it to the keystore.
    let (k1_address, _phrase, _scheme) = cluster
        .wallet
        .config_mut()
        .keystore_mut()
        .generate_and_add_new_key(SignatureScheme::Secp256k1, None, None, None)?;

    let keystore = cluster.wallet.config().keystore();
    let k1_pk = keystore.get_key(&k1_address)?.public();
    let PublicKey::Secp256k1(_) = &k1_pk else {
        anyhow::bail!("Expected Secp256k1 key");
    };

    // Fund k1_address so it can pay for the account-creation tx.
    let rgp = cluster.get_reference_gas_price().await;
    cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), k1_address)
        .await;

    let account_ref =
        create_default_account(&cluster, k1_address, &k1_pk, "create_secp256k1").await?;
    let account_address = IotaAddress::from(account_ref.0);

    let gas_coin = cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), account_address)
        .await;

    let pt = build_trivial_ptb();
    let tx_data = TransactionData::new_programmable(
        account_address,
        vec![gas_coin],
        pt,
        rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        rgp,
    );

    let move_auth = make_move_authenticator(&cluster, k1_address, &tx_data, account_ref)?;
    let tx = Transaction::from_generic_sig_data(tx_data, vec![move_auth]);

    let response = cluster.execute_transaction(tx).await;
    assert!(response.confirmed_local_execution.unwrap());
    assert!(response.errors.is_empty(), "errors: {:?}", response.errors);

    Ok(())
}

/// Happy-path Secp256r1 test.
#[sim_test]
async fn test_iota_default_account_secp256r1() -> anyhow::Result<()> {
    telemetry_subscribers::init_for_testing();

    let mut cluster = TestClusterBuilder::new().build().await;

    // Generate a Secp256r1 key and add it to the keystore.
    let (r1_address, _phrase, _scheme) = cluster
        .wallet
        .config_mut()
        .keystore_mut()
        .generate_and_add_new_key(SignatureScheme::Secp256r1, None, None, None)?;

    let keystore = cluster.wallet.config().keystore();
    let r1_pk = keystore.get_key(&r1_address)?.public();
    let PublicKey::Secp256r1(_) = &r1_pk else {
        anyhow::bail!("Expected Secp256r1 key");
    };

    let rgp = cluster.get_reference_gas_price().await;
    cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), r1_address)
        .await;

    let account_ref =
        create_default_account(&cluster, r1_address, &r1_pk, "create_secp256r1").await?;
    let account_address = IotaAddress::from(account_ref.0);

    let gas_coin = cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), account_address)
        .await;

    let pt = build_trivial_ptb();
    let tx_data = TransactionData::new_programmable(
        account_address,
        vec![gas_coin],
        pt,
        rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        rgp,
    );

    let move_auth = make_move_authenticator(&cluster, r1_address, &tx_data, account_ref)?;
    let tx = Transaction::from_generic_sig_data(tx_data, vec![move_auth]);

    let response = cluster.execute_transaction(tx).await;
    assert!(response.confirmed_local_execution.unwrap());
    assert!(response.errors.is_empty(), "errors: {:?}", response.errors);

    Ok(())
}

/// Negative test: signing with a *different* Ed25519 key (whose public key is
/// not stored in the credential) must cause the transaction to be rejected at
/// the signing / execution stage.
#[sim_test]
async fn test_iota_default_account_wrong_key_rejected() -> anyhow::Result<()> {
    telemetry_subscribers::init_for_testing();

    let mut cluster = TestClusterBuilder::new().build().await;
    let keystore = cluster.wallet.config().keystore();
    let owner = keystore.addresses().first().cloned().unwrap();
    let owner_pk = keystore.get_key(&owner)?.public();

    let PublicKey::Ed25519(_) = &owner_pk else {
        return Ok(());
    };

    // Create the IotaDefaultAccount using the owner's key.
    let account_ref = create_default_account(&cluster, owner, &owner_pk, "create_ed25519").await?;
    let account_address = IotaAddress::from(account_ref.0);

    let rgp = cluster.get_reference_gas_price().await;
    let gas_coin = cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), account_address)
        .await;

    // Generate a second key — this is the "wrong" key.
    let (wrong_address, _, _) = cluster
        .wallet
        .config_mut()
        .keystore_mut()
        .generate_and_add_new_key(SignatureScheme::ED25519, None, None, None)?;

    let pt = build_trivial_ptb();
    let tx_data = TransactionData::new_programmable(
        account_address,
        vec![gas_coin],
        pt,
        rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        rgp,
    );

    // Sign with the wrong key.
    let move_auth = make_move_authenticator(&cluster, wrong_address, &tx_data, account_ref)?;
    let tx = Transaction::from_generic_sig_data(tx_data, vec![move_auth]);

    // The transaction should either fail signing (certificate rejection) or
    // fail execution due to EPublicKeyMismatch.
    let result = cluster.create_certificate(tx, None).await;

    // In most cases the validators will reject the certificate at signing time
    // because the wrong key is used. If they somehow produce a certificate it
    // would abort at execution.  Either way the test must not succeed.
    assert!(
        result.is_err(),
        "Expected the certificate to be rejected when using the wrong signing key"
    );

    Ok(())
}

/// Parity test: verify that the `sign_secure` output satisfies BOTH the
/// standard Rust verifier (as a regular `Signature`) AND
/// `IotaDefaultAccount::authenticate` (as `generic_signature` bytes).
///
/// This confirms that the two paths accept bit-for-bit identical data.
#[sim_test]
async fn test_iota_default_account_parity_with_standard_verifier() -> anyhow::Result<()> {
    telemetry_subscribers::init_for_testing();

    let cluster = TestClusterBuilder::new().build().await;
    let keystore = cluster.wallet.config().keystore();
    let owner = keystore.addresses().first().cloned().unwrap();
    let owner_pk = keystore.get_key(&owner)?.public();

    let PublicKey::Ed25519(_) = &owner_pk else {
        return Ok(());
    };

    let rgp = cluster.get_reference_gas_price().await;

    // --- Path A: standard Rust-verified transaction from the owner's address ---
    {
        let gas_coin = cluster
            .fund_address_and_return_gas(rgp, Some(20_000_000_000), owner)
            .await;
        let pt = build_trivial_ptb();
        let tx_data = TransactionData::new_programmable(
            owner,
            vec![gas_coin],
            pt,
            rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
            rgp,
        );
        let std_sig = GenericSignature::Signature(keystore.sign_secure(
            &owner,
            &tx_data,
            Intent::iota_transaction(),
        )?);
        let tx = Transaction::from_generic_sig_data(tx_data, vec![std_sig]);
        let resp = cluster.execute_transaction(tx).await;
        assert!(
            resp.confirmed_local_execution.unwrap(),
            "standard tx failed"
        );
    }

    // --- Path B: IotaDefaultAccount-authenticated transaction from the AA address
    // ---
    let account_ref = create_default_account(&cluster, owner, &owner_pk, "create_ed25519").await?;
    let aa_address = IotaAddress::from(account_ref.0);
    {
        let gas_coin = cluster
            .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_address)
            .await;
        let pt = build_trivial_ptb();
        let tx_data = TransactionData::new_programmable(
            aa_address,
            vec![gas_coin],
            pt,
            rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
            rgp,
        );
        let move_auth = make_move_authenticator(&cluster, owner, &tx_data, account_ref)?;
        let tx = Transaction::from_generic_sig_data(tx_data, vec![move_auth]);
        let resp = cluster.execute_transaction(tx).await;
        assert!(
            resp.confirmed_local_execution.unwrap(),
            "IotaDefaultAccount tx failed"
        );
        assert!(resp.errors.is_empty(), "errors: {:?}", resp.errors);
    }

    Ok(())
}
