// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests for the `claim_registry` module.
//!
//! Scenarios covered:
//! - Happy-path: claim a default IOTA account from an Ed25519 key and then
//!   authenticate a `MoveAuthenticator` transaction using that account.
//! - Duplicate-claim rejection (`EAlreadyClaimed`, error code 1).
//! - Address-mismatch rejection (`EAddressMismatch`, error code 0).

use fastcrypto::encoding::{Encoding, Hex};
use iota_keys::keystore::AccountKeystore;
use iota_macros::sim_test;
use iota_types::{
    IOTA_CLAIM_REGISTRY_OBJECT_ID, IOTA_FRAMEWORK_ADDRESS, IOTA_FRAMEWORK_PACKAGE_ID,
    base_types::{IotaAddress, ObjectID},
    crypto::{IotaSignature, SignatureScheme},
    effects::TransactionEffectsAPI,
    execution_status::ExecutionFailureStatus,
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
use move_command_line_common::error_bitset::ErrorBitset;
use move_core_types::ident_str;
use test_cluster::{TestCluster, TestClusterBuilder};

// ---------------------------------------------------------------------------
// Protocol-upgrade test (msim only)
// ---------------------------------------------------------------------------

/// Verify that `ClaimRegistry` is created via `EndOfEpochTransaction` when a
/// network that was started at protocol v22 (no `enable_claim_registry`) upgrades
/// to v23 (first version where `enable_claim_registry = true`).
///
/// Flow:
///   1. Build a cluster at protocol v22 — registry must NOT be present at genesis.
///   2. All validators support up to v23, so they vote to upgrade on the first epoch change.
///   3. After epoch 1 the protocol version must be 23 and the registry must exist as
///      a shared object with `ObjectID == IOTA_CLAIM_REGISTRY_OBJECT_ID`.
#[cfg(msim)]
#[sim_test]
async fn test_claim_registry_created_on_protocol_upgrade() {
    use iota_protocol_config::ProtocolVersion;
    use iota_types::supported_protocol_versions::SupportedProtocolVersions;

    telemetry_subscribers::init_for_testing();

    // v22 → enable_claim_registry = false → no registry at genesis
    // v23 → enable_claim_registry = true  → registry created at epoch-change
    const PRE: u64 = 22;
    const POST: u64 = 23;

    let test_cluster = TestClusterBuilder::new()
        .with_protocol_version(ProtocolVersion::new(PRE))
        .with_epoch_duration_ms(20000)
        .with_supported_protocol_versions(SupportedProtocolVersions::new_for_testing(PRE, POST))
        .build()
        .await;

    // Genesis is at v22 → ClaimRegistry must NOT exist yet.
    assert!(
        test_cluster
            .get_object_from_fullnode_store(&IOTA_CLAIM_REGISTRY_OBJECT_ID)
            .await
            .is_none(),
        "ClaimRegistry must NOT exist at genesis (protocol v{PRE})"
    );

    // Wait for the epoch change that upgrades the protocol to v23.
    let system_state = test_cluster.wait_for_epoch(Some(1)).await;
    assert_eq!(
        system_state.protocol_version(),
        POST,
        "Expected protocol version {POST} after epoch 1"
    );

    // After the upgrade: ClaimRegistry must now exist as a shared object.
    let reg = test_cluster
        .get_object_from_fullnode_store(&IOTA_CLAIM_REGISTRY_OBJECT_ID)
        .await
        .expect("ClaimRegistry must exist after upgrade to protocol v{POST}");
    assert!(
        matches!(reg.owner(), Owner::Shared { .. }),
        "ClaimRegistry must be a shared object; got {:?}",
        reg.owner()
    );
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

/// Build a `CallArg` for the `ClaimRegistry` shared object at genesis.
async fn registry_call_arg(cluster: &TestCluster, mutable: bool) -> CallArg {
    let obj = cluster
        .get_object_from_fullnode_store(&IOTA_CLAIM_REGISTRY_OBJECT_ID)
        .await
        .expect("ClaimRegistry must exist at genesis");
    let Owner::Shared {
        initial_shared_version,
    } = obj.owner()
    else {
        panic!("ClaimRegistry must be a shared object");
    };
    CallArg::Object(ObjectArg::SharedObject {
        id: IOTA_CLAIM_REGISTRY_OBJECT_ID,
        initial_shared_version: *initial_shared_version,
        mutable,
    })
}

/// Build a PTB that calls `iota::claim_registry::claim_ed25519`.
fn build_claim_ed25519_pt(
    registry_arg: CallArg,
    pubkey_bytes: Vec<u8>,
) -> anyhow::Result<iota_types::transaction::ProgrammableTransaction> {
    let mut b = ProgrammableTransactionBuilder::new();
    let reg = b.input(registry_arg)?;
    let pk = b.pure(pubkey_bytes)?;
    b.programmable_move_call(
        IOTA_FRAMEWORK_PACKAGE_ID,
        ident_str!("claim_registry").to_owned(),
        ident_str!("claim_ed25519").to_owned(),
        vec![],
        vec![reg, pk],
    );
    Ok(b.finish())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Claim a default IOTA account for a fresh Ed25519 key, then send a
/// `MoveAuthenticator` transaction authenticated by that account.
#[sim_test]
async fn test_claim_registry_claim_and_authenticate_ed25519() -> anyhow::Result<()> {
    telemetry_subscribers::init_for_testing();

    let mut cluster = TestClusterBuilder::new().build().await;

    // ── 1. Generate a fresh Ed25519 keypair and add it to the wallet keystore ──
    let (derived_address, _mnemonic, _scheme) = cluster
        .wallet
        .config_mut()
        .keystore_mut()
        .generate_and_add_new_key(SignatureScheme::ED25519, None, None, None)?;

    // Raw 32-byte Ed25519 public key (no scheme flag prefix).
    let pubkey_bytes: Vec<u8> = cluster
        .wallet
        .config()
        .keystore()
        .get_key(&derived_address)?
        .public()
        .as_ref()
        .to_vec();

    // ── 2. Fund the derived address so it can pay gas ──────────────────────────
    let rgp = cluster.get_reference_gas_price().await;
    cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), derived_address)
        .await;

    // ── 3. Build and execute the claim transaction ─────────────────────────────
    let registry_arg = registry_call_arg(&cluster, true).await;
    let pt = build_claim_ed25519_pt(registry_arg, pubkey_bytes.clone())?;

    let tx_data = cluster
        .test_transaction_builder_with_sender(derived_address)
        .await
        .programmable(pt)
        .build();
    let claim_tx = cluster.wallet.sign_transaction(&tx_data);
    let (effects, _) = cluster
        .execute_transaction_return_raw_effects(claim_tx)
        .await?;

    assert!(
        effects.status().is_ok(),
        "Claim transaction must succeed; got: {:?}",
        effects.status()
    );

    // ── 4. Verify IotaDefaultAccount was created as a shared object ───────────
    //   ObjectID == derived_address (new_uid_from_hash(ctx.sender())).
    let account_id = ObjectID::from(derived_address);

    let (_account_ref, account_owner) = effects
        .all_changed_objects()
        .into_iter()
        .find(|(obj_ref, _, kind)| obj_ref.0 == account_id && *kind == WriteKind::Create)
        .map(|(obj_ref, owner, _)| (obj_ref, owner))
        .expect("IotaDefaultAccount must appear as a created object in effects");

    let Owner::Shared {
        initial_shared_version: account_shared_version,
    } = account_owner
    else {
        panic!("IotaDefaultAccount must be shared; got {:?}", account_owner);
    };

    // ── 5. Build a simple PTB for the MoveAuthenticator transaction ────────────
    // Use the freshest gas coin for derived_address (balance reduced after claim).
    let fresh_gas = cluster
        .wallet
        .get_one_gas_object_owned_by_address(derived_address)
        .await?
        .expect("derived_address must still own a gas coin after claim");

    let simple_pt = {
        let mut b = ProgrammableTransactionBuilder::new();
        // A trivial side effect: transfer 1 MIST to address 0.
        b.transfer_iota(IotaAddress::ZERO, Some(1));
        b.finish()
    };

    let tx_data = TransactionData::new_programmable_allow_sponsor(
        derived_address,
        vec![fresh_gas],
        simple_pt,
        rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        rgp,
        derived_address,
    );

    // ── 6. Sign the transaction digest with the Ed25519 private key ───────────
    let tx_digest: [u8; 32] = tx_data.digest().into_inner();
    let raw_sig: Vec<u8> = cluster
        .wallet
        .config()
        .keystore()
        .sign_hashed(&derived_address, &tx_digest)?
        .signature_bytes()
        .to_vec();

    // ── 7. Wrap in a MoveAuthenticator signature ───────────────────────────────
    // The authenticate function signature is:
    //   authenticate(account: &IotaDefaultAccount, signature: vector<u8>, ...)
    // so call_args = [signature], object_to_authenticate = account.
    let sig_call_arg = CallArg::Pure(bcs::to_bytes(&raw_sig)?);
    let account_call_arg = CallArg::Object(ObjectArg::SharedObject {
        id: account_id,
        initial_shared_version: account_shared_version,
        mutable: false,
    });
    let move_auth = GenericSignature::MoveAuthenticator(MoveAuthenticator::new_v1(
        vec![sig_call_arg],
        vec![],
        account_call_arg,
    ));

    // ── 8. Execute the MoveAuthenticator transaction ───────────────────────────
    let auth_tx = Transaction::from_generic_sig_data(tx_data, vec![move_auth]);
    let response = cluster.execute_transaction(auth_tx).await;

    assert!(
        response.confirmed_local_execution.unwrap_or(false),
        "MoveAuthenticator transaction must be confirmed locally"
    );
    assert!(
        response.errors.is_empty(),
        "MoveAuthenticator transaction must have no errors: {:?}",
        response.errors
    );

    Ok(())
}

/// Claiming the same address twice must abort with `EAlreadyClaimed` (code 1).
#[sim_test]
async fn test_claim_registry_duplicate_claim_fails() -> anyhow::Result<()> {
    telemetry_subscribers::init_for_testing();

    let mut cluster = TestClusterBuilder::new().build().await;

    let (derived_address, _mnemonic, _scheme) = cluster
        .wallet
        .config_mut()
        .keystore_mut()
        .generate_and_add_new_key(SignatureScheme::ED25519, None, None, None)?;

    let pubkey_bytes: Vec<u8> = cluster
        .wallet
        .config()
        .keystore()
        .get_key(&derived_address)?
        .public()
        .as_ref()
        .to_vec();

    let rgp = cluster.get_reference_gas_price().await;
    cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), derived_address)
        .await;

    // First claim — must succeed.
    let reg1 = registry_call_arg(&cluster, true).await;
    let pt1 = build_claim_ed25519_pt(reg1, pubkey_bytes.clone())?;
    let tx1 = cluster
        .test_transaction_builder_with_sender(derived_address)
        .await
        .programmable(pt1)
        .build();
    let (eff1, _) = cluster
        .execute_transaction_return_raw_effects(cluster.wallet.sign_transaction(&tx1))
        .await?;
    assert!(
        eff1.status().is_ok(),
        "First claim must succeed; got {:?}",
        eff1.status()
    );

    // Fund a second gas coin so we can pay for the second transaction.
    cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), derived_address)
        .await;

    // Second claim — must fail.
    let reg2 = registry_call_arg(&cluster, true).await;
    let pt2 = build_claim_ed25519_pt(reg2, pubkey_bytes)?;
    let tx2 = cluster
        .test_transaction_builder_with_sender(derived_address)
        .await
        .programmable(pt2)
        .build();
    let (eff2, _) = cluster
        .execute_transaction_return_raw_effects(cluster.wallet.sign_transaction(&tx2))
        .await?;

    assert!(
        eff2.status().is_err(),
        "Second claim must fail; got {:?}",
        eff2.status()
    );

    // Verify: abort in claim_registry with error code 1 (EAlreadyClaimed).
    let (failure, _) = eff2.status().clone().unwrap_err();
    assert!(
        matches!(
            failure,
            ExecutionFailureStatus::MoveAbort(ref loc, code)
            if loc.module.name().as_str() == "claim_registry"
                && loc.module.address() == &IOTA_FRAMEWORK_ADDRESS
                && ErrorBitset::from_u64(code).unwrap().error_code() == Some(1)
        ),
        "Expected EAlreadyClaimed (code 1) from claim_registry; got {failure:?}"
    );

    Ok(())
}

/// Supplying a public key whose derived address does not match the sender must
/// abort with `EAddressMismatch` (error code 0).
#[sim_test]
async fn test_claim_registry_wrong_pubkey_fails() -> anyhow::Result<()> {
    telemetry_subscribers::init_for_testing();

    let cluster = TestClusterBuilder::new().build().await;

    // Use the first wallet address as sender (key IS in keystore, but the
    // wrong_pubkey below derives to a completely different address).
    let sender = cluster
        .wallet
        .config()
        .keystore()
        .addresses()
        .first()
        .cloned()
        .expect("test cluster must have at least one account");

    // A hard-coded 32-byte Ed25519 public key whose address is not `sender`.
    // This is the same key used in the transactional test fixtures.
    let wrong_pubkey: Vec<u8> =
        Hex::decode("cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88")
            .expect("valid hex");

    let registry_arg = registry_call_arg(&cluster, true).await;
    let pt = build_claim_ed25519_pt(registry_arg, wrong_pubkey)?;
    let tx = cluster
        .test_transaction_builder_with_sender(sender)
        .await
        .programmable(pt)
        .build();
    let (effects, _) = cluster
        .execute_transaction_return_raw_effects(cluster.wallet.sign_transaction(&tx))
        .await?;

    assert!(
        effects.status().is_err(),
        "Wrong-pubkey claim must fail; got {:?}",
        effects.status()
    );

    // Verify: abort in claim_registry with error code 0 (EAddressMismatch).
    let (failure, _) = effects.status().clone().unwrap_err();
    assert!(
        matches!(
            failure,
            ExecutionFailureStatus::MoveAbort(ref loc, code)
            if loc.module.name().as_str() == "claim_registry"
                && loc.module.address() == &IOTA_FRAMEWORK_ADDRESS
                && ErrorBitset::from_u64(code).unwrap().error_code() == Some(0)
        ),
        "Expected EAddressMismatch (code 0) from claim_registry; got {failure:?}"
    );

    Ok(())
}
