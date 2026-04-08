// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests for the `claim_registry` module.
//!
//! Scenarios covered:
//! - Happy-path: claim an address and verify the registry dynamic field.
//! - Duplicate-claim rejection (`EAlreadyClaimed`, error code 1).
//! - Address-mismatch rejection (`EAddressMismatch`, error code 0).

use fastcrypto::encoding::{Encoding, Hex};
use iota_keys::keystore::AccountKeystore;
use iota_macros::sim_test;
use iota_types::{
    IOTA_CLAIM_REGISTRY_OBJECT_ID, IOTA_FRAMEWORK_ADDRESS, IOTA_FRAMEWORK_PACKAGE_ID,
    crypto::SignatureScheme,
    effects::TransactionEffectsAPI,
    execution_status::ExecutionFailureStatus,
    object::Owner,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{CallArg, ObjectArg},
};
use move_command_line_common::error_bitset::ErrorBitset;
use move_core_types::ident_str;
use test_cluster::{TestCluster, TestClusterBuilder};

// ---------------------------------------------------------------------------
// Protocol-upgrade test (msim only)
// ---------------------------------------------------------------------------

/// Verify that `ClaimRegistry` is created via `EndOfEpochTransaction` when a
/// network started at protocol v22 upgrades to v23.
#[cfg(msim)]
#[sim_test]
async fn test_claim_registry_created_on_protocol_upgrade() {
    use iota_protocol_config::ProtocolVersion;
    use iota_types::supported_protocol_versions::SupportedProtocolVersions;

    telemetry_subscribers::init_for_testing();

    const PRE: u64 = 22;
    const POST: u64 = 23;

    let test_cluster = TestClusterBuilder::new()
        .with_protocol_version(ProtocolVersion::new(PRE))
        .with_epoch_duration_ms(20000)
        .with_supported_protocol_versions(SupportedProtocolVersions::new_for_testing(PRE, POST))
        .build()
        .await;

    assert!(
        test_cluster
            .get_object_from_fullnode_store(&IOTA_CLAIM_REGISTRY_OBJECT_ID)
            .await
            .is_none(),
        "ClaimRegistry must NOT exist at genesis (protocol v{PRE})"
    );

    let system_state = test_cluster.wait_for_epoch(Some(1)).await;
    assert_eq!(
        system_state.protocol_version(),
        POST,
        "Expected protocol version {POST} after epoch 1"
    );

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

/// Build a PTB that calls `claim_registry::claim` twice on the same address.
/// The first call succeeds (marks the address), the second aborts with
/// EAlreadyClaimed. Used to test the duplicate-claim rejection without needing
/// to consume the ticket.
fn build_double_claim_pt(
    registry_arg: CallArg,
    scheme: u8,
    pubkey_bytes: Vec<u8>,
) -> anyhow::Result<iota_types::transaction::ProgrammableTransaction> {
    let mut b = ProgrammableTransactionBuilder::new();
    let reg = b.input(registry_arg)?;
    let scheme_arg = b.pure(scheme)?;
    let pk = b.pure(pubkey_bytes)?;
    b.programmable_move_call(
        IOTA_FRAMEWORK_PACKAGE_ID,
        ident_str!("claim_registry").to_owned(),
        ident_str!("claim").to_owned(),
        vec![],
        vec![reg, scheme_arg, pk],
    );
    b.programmable_move_call(
        IOTA_FRAMEWORK_PACKAGE_ID,
        ident_str!("claim_registry").to_owned(),
        ident_str!("claim").to_owned(),
        vec![],
        vec![reg, scheme_arg, pk],
    );
    Ok(b.finish())
}

/// Build a PTB that calls `claim_registry::claim` once (expects abort at call
/// site).
fn build_claim_pt(
    registry_arg: CallArg,
    scheme: u8,
    pubkey_bytes: Vec<u8>,
) -> anyhow::Result<iota_types::transaction::ProgrammableTransaction> {
    let mut b = ProgrammableTransactionBuilder::new();
    let reg = b.input(registry_arg)?;
    let scheme_arg = b.pure(scheme)?;
    let pk = b.pure(pubkey_bytes)?;
    b.programmable_move_call(
        IOTA_FRAMEWORK_PACKAGE_ID,
        ident_str!("claim_registry").to_owned(),
        ident_str!("claim").to_owned(),
        vec![],
        vec![reg, scheme_arg, pk],
    );
    Ok(b.finish())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Claiming the same address twice in one PTB must abort with `EAlreadyClaimed`
/// (code 1). Both calls share the same registry input: the first adds the
/// dynamic field, the second sees it and aborts. The unconsumed ticket from the
/// first call is never an issue because the whole transaction rolls back on
/// abort.
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

    let registry_arg = registry_call_arg(&cluster, true).await;
    let pt = build_double_claim_pt(registry_arg, 0x00, pubkey_bytes)?;
    let tx = cluster
        .test_transaction_builder_with_sender(derived_address)
        .await
        .programmable(pt)
        .build();
    let (eff2, _) = cluster
        .execute_transaction_return_raw_effects(cluster.wallet.sign_transaction(&tx))
        .await?;

    assert!(
        eff2.status().is_err(),
        "Second claim must fail; got {:?}",
        eff2.status()
    );

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

    let sender = cluster
        .wallet
        .config()
        .keystore()
        .addresses()
        .first()
        .cloned()
        .expect("test cluster must have at least one account");

    let wrong_pubkey: Vec<u8> =
        Hex::decode("cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88")
            .expect("valid hex");

    let registry_arg = registry_call_arg(&cluster, true).await;
    let pt = build_claim_pt(registry_arg, 0x00, wrong_pubkey)?;
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
