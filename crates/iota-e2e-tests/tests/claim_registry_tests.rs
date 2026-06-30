// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests for the `claim_registry` module.

#[cfg(msim)]
use iota_macros::sim_test;
#[cfg(msim)]
use iota_sdk_types::{ObjectId, Owner};
#[cfg(msim)]
use test_cluster::TestClusterBuilder;

// ---------------------------------------------------------------------------
// Feature-flag test (msim only)
// ---------------------------------------------------------------------------

/// Verify that `ClaimRegistry` creation is gated by the `enable_claim_registry`
/// feature flag, driving the flag at runtime rather than through a protocol
/// version upgrade.
///
/// While the flag is disabled the registry must not exist. Once enabled, the
/// `ClaimRegistry` is created by the `EndOfEpochTransaction` of the first epoch
/// that runs with the flag on, becoming visible at the start of the following
/// epoch.
#[cfg(msim)]
#[sim_test]
async fn test_claim_registry_created_when_flag_enabled() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use iota_protocol_config::ProtocolConfig;

    telemetry_subscribers::init_for_testing();

    // The override is re-applied whenever an epoch store is (re)created, so
    // flipping this flag at runtime takes effect from the next epoch onwards.
    let enable_claim_registry = Arc::new(AtomicBool::new(false));
    let _guard = {
        let enable_claim_registry = enable_claim_registry.clone();
        ProtocolConfig::apply_overrides_for_testing(move |_, mut config| {
            config.set_enable_claim_registry_for_testing(
                enable_claim_registry.load(Ordering::SeqCst),
            );
            config
        })
    };

    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(20000)
        .build()
        .await;

    // Disabled: the registry must not exist at genesis...
    assert!(
        test_cluster
            .get_object_from_fullnode_store(&ObjectId::CLAIM_REGISTRY)
            .await
            .is_none(),
        "ClaimRegistry must NOT exist at genesis while the flag is disabled"
    );

    // ...nor after a full epoch has run with the flag still disabled.
    test_cluster.wait_for_epoch(Some(1)).await;
    assert!(
        test_cluster
            .get_object_from_fullnode_store(&ObjectId::CLAIM_REGISTRY)
            .await
            .is_none(),
        "ClaimRegistry must NOT exist while the flag is disabled"
    );

    // Enable the flag. The next epoch store picks up the new config, and the
    // registry is created by that epoch's end-of-epoch transaction, becoming
    // visible at the start of the following epoch.
    enable_claim_registry.store(true, Ordering::SeqCst);

    let mut registry = None;
    for target_epoch in 2..=5 {
        test_cluster.wait_for_epoch(Some(target_epoch)).await;
        if let Some(object) = test_cluster
            .get_object_from_fullnode_store(&ObjectId::CLAIM_REGISTRY)
            .await
        {
            registry = Some(object);
            break;
        }
    }

    let registry = registry.expect("ClaimRegistry must be created once the flag is enabled");
    assert!(
        matches!(registry.owner(), Owner::Shared { .. }),
        "ClaimRegistry must be a shared object; got {:?}",
        registry.owner()
    );
}

// ---------------------------------------------------------------------------
// ClaimAccount transaction kind tests (msim only)
// ---------------------------------------------------------------------------

/// Verify that a `TransactionKind::ClaimAccount` with
/// `SmartAccountBuildKind::Mutable` succeeds when both protocol flags are
/// enabled, and that the transaction is accepted.
#[cfg(msim)]
#[sim_test]
async fn test_claim_account_mutable_succeeds() {
    use iota_json_rpc_types::IotaTransactionBlockEffectsAPI;
    use iota_keys::keystore::AccountKeystore;
    use iota_sdk_types::{
        Address, ClaimAccountTransaction, SmartAccountBuildKind, SmartAccountClaim, TransactionKind,
    };
    use iota_types::{
        crypto::IotaKeyPair,
        transaction::{
            TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionData, TransactionDataAPI,
        },
    };

    telemetry_subscribers::init_for_testing();

    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(20000)
        .build()
        .await;

    let registry_initial_shared_version = wait_for_claim_registry(&test_cluster).await;

    let owner: Address = test_cluster
        .wallet
        .config()
        .keystore()
        .addresses()
        .into_iter()
        .next()
        .expect("wallet must have at least one account");

    let keypair: IotaKeyPair = test_cluster
        .wallet
        .config()
        .keystore()
        .get_key(&owner)
        .expect("keypair must exist for owner")
        .as_keypair()
        .expect("stored key must be a keypair")
        .clone();

    let claim = SmartAccountClaim {
        public_key: sdk_ed25519_public_key(&keypair),
        claim_registry_initial_shared_version: registry_initial_shared_version,
        fields: vec![],
        build_kind: SmartAccountBuildKind::Mutable,
    };
    let kind =
        TransactionKind::new_claim_account(ClaimAccountTransaction::new_smart_account(claim));

    let rgp = test_cluster.get_reference_gas_price().await;
    let tx_data = TransactionData::new(
        kind,
        owner,
        first_gas_coin(&test_cluster.wallet, owner).await,
        rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        rgp,
    );
    let response = test_cluster.sign_and_execute_transaction(&tx_data).await;

    let effects = response.effects.expect("response must include effects");
    assert!(
        effects.status().is_ok(),
        "ClaimAccount (Mutable) transaction must succeed; got {:?}",
        effects.status(),
    );

    let object_changes = response
        .object_changes
        .expect("response must include object changes");
    let smart_accounts = created_smart_accounts(&object_changes);

    assert_eq!(
        smart_accounts.len(),
        1,
        "Expected exactly one SmartAccount created; got {smart_accounts:?}",
    );
    let (_, sa_owner) = &smart_accounts[0];
    assert!(
        matches!(sa_owner, Owner::Shared(_)),
        "Mutable SmartAccount must be a shared object; got {sa_owner:?}",
    );
}

/// Verify that a `TransactionKind::ClaimAccount` with
/// `SmartAccountBuildKind::Immutable` succeeds and creates an immutable
/// `SmartAccount` object.
#[cfg(msim)]
#[sim_test]
async fn test_claim_account_immutable_succeeds() {
    use iota_json_rpc_types::IotaTransactionBlockEffectsAPI;
    use iota_keys::keystore::AccountKeystore;
    use iota_sdk_types::{
        Address, ClaimAccountTransaction, SmartAccountBuildKind, SmartAccountClaim, TransactionKind,
    };
    use iota_types::{
        crypto::IotaKeyPair,
        transaction::{
            TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionData, TransactionDataAPI,
        },
    };

    telemetry_subscribers::init_for_testing();

    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(20000)
        .build()
        .await;

    let registry_initial_shared_version = wait_for_claim_registry(&test_cluster).await;

    // Use a different wallet account from the mutable test to avoid double-claim.
    let addresses = test_cluster.wallet.config().keystore().addresses();
    let owner: Address = addresses.get(1).copied().unwrap_or(addresses[0]);

    let keypair: IotaKeyPair = test_cluster
        .wallet
        .config()
        .keystore()
        .get_key(&owner)
        .expect("keypair must exist for owner")
        .as_keypair()
        .expect("stored key must be a keypair")
        .clone();

    let claim = SmartAccountClaim {
        public_key: sdk_ed25519_public_key(&keypair),
        claim_registry_initial_shared_version: registry_initial_shared_version,
        fields: vec![],
        build_kind: SmartAccountBuildKind::Immutable,
    };
    let kind =
        TransactionKind::new_claim_account(ClaimAccountTransaction::new_smart_account(claim));

    let rgp = test_cluster.get_reference_gas_price().await;
    let tx_data = TransactionData::new(
        kind,
        owner,
        first_gas_coin(&test_cluster.wallet, owner).await,
        rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        rgp,
    );
    let response = test_cluster.sign_and_execute_transaction(&tx_data).await;

    let effects = response.effects.expect("response must include effects");
    assert!(
        effects.status().is_ok(),
        "ClaimAccount (Immutable) transaction must succeed; got {:?}",
        effects.status(),
    );

    let object_changes = response
        .object_changes
        .expect("response must include object changes");
    let smart_accounts = created_smart_accounts(&object_changes);

    assert_eq!(
        smart_accounts.len(),
        1,
        "Expected exactly one SmartAccount created; got {smart_accounts:?}",
    );
    let (_, sa_owner) = &smart_accounts[0];
    assert!(
        matches!(sa_owner, Owner::Immutable),
        "Immutable SmartAccount must be an immutable object; got {sa_owner:?}",
    );
}

/// Verify that dynamic fields passed via `SmartAccountField` are actually
/// stored on the created `SmartAccount`.
#[cfg(msim)]
#[sim_test]
async fn test_claim_account_with_dynamic_field() {
    use iota_json_rpc_types::IotaTransactionBlockEffectsAPI;
    use iota_keys::keystore::AccountKeystore;
    use iota_sdk_types::{
        Address, ClaimAccountTransaction, SmartAccountBuildKind, SmartAccountClaim,
        SmartAccountField, TransactionKind, TypeTag,
    };
    use iota_types::{
        crypto::IotaKeyPair,
        transaction::{
            TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionData, TransactionDataAPI,
        },
    };

    telemetry_subscribers::init_for_testing();

    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(20000)
        .build()
        .await;

    let registry_initial_shared_version = wait_for_claim_registry(&test_cluster).await;

    let owner: Address = test_cluster
        .wallet
        .config()
        .keystore()
        .addresses()
        .into_iter()
        .next()
        .expect("wallet must have at least one account");

    let keypair: IotaKeyPair = test_cluster
        .wallet
        .config()
        .keystore()
        .get_key(&owner)
        .expect("keypair must exist for owner")
        .as_keypair()
        .expect("stored key must be a keypair")
        .clone();

    // Add a single u64 dynamic field: name = 42u64, value = 100u64.
    let field = SmartAccountField {
        name_type: TypeTag::U64,
        name_bcs: bcs::to_bytes(&42u64).unwrap(),
        value_type: TypeTag::U64,
        value_bcs: bcs::to_bytes(&100u64).unwrap(),
    };

    let claim = SmartAccountClaim {
        public_key: sdk_ed25519_public_key(&keypair),
        claim_registry_initial_shared_version: registry_initial_shared_version,
        fields: vec![field],
        build_kind: SmartAccountBuildKind::Mutable,
    };
    let kind =
        TransactionKind::new_claim_account(ClaimAccountTransaction::new_smart_account(claim));

    let rgp = test_cluster.get_reference_gas_price().await;
    let tx_data = TransactionData::new(
        kind,
        owner,
        first_gas_coin(&test_cluster.wallet, owner).await,
        rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        rgp,
    );
    let response = test_cluster.sign_and_execute_transaction(&tx_data).await;

    let effects = response.effects.expect("response must include effects");
    assert!(
        effects.status().is_ok(),
        "ClaimAccount (with dynamic field) must succeed; got {:?}",
        effects.status(),
    );

    // Locate the SmartAccount in the object changes (internal DFs are also
    // created).
    let object_changes = response
        .object_changes
        .expect("response must include object changes");
    let smart_accounts = created_smart_accounts(&object_changes);
    assert_eq!(
        smart_accounts.len(),
        1,
        "Expected exactly one SmartAccount created; got {smart_accounts:?}",
    );
    let (smart_account_id, _) = smart_accounts.into_iter().next().unwrap();

    // Verify the user-added dynamic field is present on the SmartAccount.
    // Internal fields (authenticator ref, public key) use struct-type keys;
    // only the user field uses a u64 key.
    let client = test_cluster.wallet.get_client().await.unwrap();
    let df_page = client
        .read_api()
        .get_dynamic_fields(smart_account_id, None, None)
        .await
        .expect("dynamic field query must succeed");

    let u64_field = df_page
        .data
        .iter()
        .find(|f| f.name.type_ == TypeTag::U64)
        .expect("Expected a u64-keyed dynamic field on the SmartAccount");

    assert_eq!(
        u64_field.name.value,
        serde_json::json!("42"),
        "Expected dynamic field name to be 42u64; got {:?}",
        u64_field.name.value,
    );

    // Fetch the field object to verify the stored value.
    use iota_json_rpc_types::{IotaMoveValue, IotaObjectDataOptions, IotaParsedData};
    let field_object = client
        .read_api()
        .get_object_with_options(
            u64_field.object_id,
            IotaObjectDataOptions::new().with_content(),
        )
        .await
        .expect("field object fetch must succeed");

    let move_obj = match field_object
        .data
        .expect("field object must have data")
        .content
        .expect("field object must have content")
    {
        IotaParsedData::MoveObject(obj) => *obj,
        _ => panic!("dynamic field content must be a Move object"),
    };

    let stored_value = move_obj
        .read_dynamic_field_value("value")
        .expect("Field<u64, u64> must have a value field");
    assert_eq!(
        stored_value,
        IotaMoveValue::String("100".to_string()),
        "Expected dynamic field value to be 100u64",
    );
}

/// Verify that a `ClaimAccountTransaction` is rejected at validity-check time
/// when `enable_claim_registry` is disabled.
#[cfg(msim)]
#[sim_test]
async fn test_claim_account_rejected_when_registry_disabled() {
    use iota_json_rpc_types::IotaTransactionBlockEffectsAPI;
    use iota_keys::keystore::AccountKeystore;
    use iota_protocol_config::ProtocolConfig;
    use iota_sdk_types::{
        Address, ClaimAccountTransaction, SmartAccountBuildKind, SmartAccountClaim, TransactionKind,
    };
    use iota_types::{
        crypto::IotaKeyPair,
        transaction::{
            TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionData, TransactionDataAPI,
        },
    };

    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_claim_registry_for_testing(false);
        config.set_enable_builtin_move_authenticators_for_testing(false);
        config
    });

    let test_cluster = TestClusterBuilder::new().build().await;

    let owner: Address = test_cluster
        .wallet
        .config()
        .keystore()
        .addresses()
        .into_iter()
        .next()
        .expect("wallet must have at least one account");

    let keypair: IotaKeyPair = test_cluster
        .wallet
        .config()
        .keystore()
        .get_key(&owner)
        .expect("keypair must exist for owner")
        .as_keypair()
        .expect("stored key must be a keypair")
        .clone();

    let claim = SmartAccountClaim {
        public_key: sdk_ed25519_public_key(&keypair),
        // version is irrelevant — validity check aborts before execution
        claim_registry_initial_shared_version: 1,
        fields: vec![],
        build_kind: SmartAccountBuildKind::Mutable,
    };
    let kind =
        TransactionKind::new_claim_account(ClaimAccountTransaction::new_smart_account(claim));

    let rgp = test_cluster.get_reference_gas_price().await;
    let tx_data = TransactionData::new(
        kind,
        owner,
        first_gas_coin(&test_cluster.wallet, owner).await,
        rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        rgp,
    );

    // The transaction must be rejected before execution
    // (UserInputError::Unsupported).
    let result = test_cluster
        .wallet
        .execute_transaction_may_fail(test_cluster.wallet.sign_transaction(&tx_data))
        .await;

    match result {
        Err(e) => {
            let msg = e.to_string().to_lowercase();
            assert!(
                msg.contains("claim registry") || msg.contains("unsupported"),
                "unexpected error message: {msg}",
            );
        }
        Ok(resp) => {
            let status = resp
                .effects
                .as_ref()
                .expect("response must include effects")
                .status();
            assert!(
                status.is_err(),
                "ClaimAccount must be rejected when claim_registry is disabled; got success",
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Wait for the `ClaimRegistry` to appear and return its
/// `initial_shared_version`.
#[cfg(msim)]
async fn wait_for_claim_registry(test_cluster: &test_cluster::TestCluster) -> u64 {
    let mut registry_obj = None;
    for target_epoch in 1..=5 {
        test_cluster.wait_for_epoch(Some(target_epoch)).await;
        if let Some(obj) = test_cluster
            .get_object_from_fullnode_store(&ObjectId::CLAIM_REGISTRY)
            .await
        {
            registry_obj = Some(obj);
            break;
        }
    }
    let registry_obj =
        registry_obj.expect("ClaimRegistry must be created when the flag is enabled");
    let Owner::Shared(initial_version) = registry_obj.owner() else {
        panic!("ClaimRegistry must be a shared object");
    };
    initial_version.as_u64()
}

/// Fetch the first gas coin `ObjectRef` owned by a wallet address.
#[cfg(msim)]
async fn first_gas_coin(
    wallet: &iota_sdk::wallet_context::WalletContext,
    owner: iota_sdk_types::Address,
) -> iota_sdk_types::ObjectReference {
    wallet
        .get_gas_objects_owned_by_address(owner, None)
        .await
        .expect("gas lookup must succeed")
        .into_iter()
        .next()
        .expect("owner must have at least one gas coin")
}

/// Return the `(ObjectId, Owner)` pairs for every `SmartAccount` object in the
/// provided object-change list.  The `ClaimAccount` transaction also creates
/// internal dynamic-field objects (authenticator ref, public key), so callers
/// should not assume the SmartAccount is the only created object.
#[cfg(msim)]
fn created_smart_accounts(
    object_changes: &[iota_json_rpc_types::ObjectChange],
) -> Vec<(iota_sdk_types::ObjectId, iota_sdk_types::Owner)> {
    use iota_json_rpc_types::ObjectChange;
    object_changes
        .iter()
        .filter_map(|c| match c {
            ObjectChange::Created {
                object_type,
                object_id,
                owner,
                ..
            } if object_type.module().as_str() == "smart_account"
                && object_type.name().as_str() == "SmartAccount" =>
            {
                Some((*object_id, owner.clone()))
            }
            _ => None,
        })
        .collect()
}

/// Build the SDK `PublicKey` from an iota-types `IotaKeyPair` (Ed25519 only).
#[cfg(msim)]
fn sdk_ed25519_public_key(
    kp: &iota_types::crypto::IotaKeyPair,
) -> iota_sdk_types::crypto::PublicKey {
    use iota_sdk_types::crypto::{Ed25519PublicKey, PublicKey as SdkPublicKey, PublicKeyExt};
    match kp {
        iota_types::crypto::IotaKeyPair::Ed25519(_) => SdkPublicKey::Ed25519(
            Ed25519PublicKey::from_bytes(kp.public().as_ref())
                .expect("wallet public key must be valid"),
        ),
        other => panic!(
            "expected Ed25519 wallet key; got {:?}",
            other.public().scheme()
        ),
    }
}
