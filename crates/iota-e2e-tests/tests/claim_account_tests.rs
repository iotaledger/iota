// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests for the `ClaimAccount` transaction kind.

#[cfg(msim)]
use iota_macros::sim_test;
#[cfg(msim)]
use iota_sdk_types::{ObjectId, Owner};
#[cfg(msim)]
use test_cluster::TestClusterBuilder;

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

    let (public_key_scheme, public_key_raw_bytes) = claim_public_key(&keypair);
    let claim = SmartAccountClaim {
        public_key_scheme,
        public_key_raw_bytes,
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

    let (public_key_scheme, public_key_raw_bytes) = claim_public_key(&keypair);
    let claim = SmartAccountClaim {
        public_key_scheme,
        public_key_raw_bytes,
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

/// Pins the current, incorrect behaviour of claiming an address twice: the
/// second `ClaimAccount` succeeds and re-creates the account object under the
/// same id with a bumped version, even though that object was never a
/// transaction input.
///
/// `claim::claim_address` leaves double-claim prevention to its caller
/// and `smart_account::claim_builder` does not implement it, so nothing rejects
/// the second claim. Once prevention lands, both pins below have to flip: the
/// second claim must fail, and the account object must keep the version the
/// first claim gave it.
#[cfg(msim)]
#[sim_test]
async fn test_claim_account_twice_is_not_yet_prevented() {
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

    let rgp = test_cluster.get_reference_gas_price().await;
    let mut claimed = Vec::new();

    for attempt in 1..=2 {
        let (public_key_scheme, public_key_raw_bytes) = claim_public_key(&keypair);
        let claim = SmartAccountClaim {
            public_key_scheme,
            public_key_raw_bytes,
            build_kind: SmartAccountBuildKind::Immutable,
        };
        let tx_data = TransactionData::new(
            TransactionKind::new_claim_account(ClaimAccountTransaction::new_smart_account(claim)),
            owner,
            first_gas_coin(&test_cluster.wallet, owner).await,
            rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
            rgp,
        );
        let response = test_cluster.sign_and_execute_transaction(&tx_data).await;
        let effects = response.effects.expect("response must include effects");

        assert!(
            effects.status().is_ok(),
            "claim attempt {attempt} was expected to be accepted today; got {:?}",
            effects.status(),
        );

        let object_changes = response
            .object_changes
            .expect("response must include object changes");
        let (account_id, _) = created_smart_accounts(&object_changes)
            .into_iter()
            .next()
            .expect("the claim must create a SmartAccount");
        let version = effects
            .created()
            .iter()
            .find(|o| o.reference.object_id == account_id)
            .map(|o| o.reference.version)
            .expect("the SmartAccount must be reported as created");
        claimed.push((account_id, version));
    }

    let (first_id, first_version) = claimed[0];
    let (second_id, second_version) = claimed[1];

    assert_eq!(
        first_id, second_id,
        "both claims derive the account id from the sender address",
    );
    assert!(
        second_version > first_version,
        "the immutable account object was expected to be overwritten today; got \
         {second_version:?} after {first_version:?}",
    );
}

/// Verify that a `ClaimAccountTransaction` is rejected at validity-check time
/// when `enable_claim_account_transaction` is disabled.
#[cfg(msim)]
#[sim_test]
async fn test_claim_account_rejected_when_disabled() {
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
        config.set_enable_claim_account_transaction_for_testing(false);
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

    let (public_key_scheme, public_key_raw_bytes) = claim_public_key(&keypair);
    let claim = SmartAccountClaim {
        public_key_scheme,
        public_key_raw_bytes,
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
                msg.contains("claim account transactions are not enabled")
                    || msg.contains("unsupported"),
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
                "ClaimAccount must be rejected when the feature flag is disabled; got success",
            );
        }
    }
}

/// Verify that a `SmartAccount` claim is rejected at validity-check time when
/// `enable_builtin_move_authenticators` is disabled, even though
/// `enable_claim_account_transaction` is enabled.
///
/// The claimed account is authenticated by the built-in authenticator for its
/// key's scheme, so claiming without them would create an account that can
/// never authenticate a transaction, at an address that can only be claimed
/// once.
#[cfg(msim)]
#[sim_test]
async fn test_claim_account_rejected_without_builtin_authenticators() {
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

    let (public_key_scheme, public_key_raw_bytes) = claim_public_key(&keypair);
    let claim = SmartAccountClaim {
        public_key_scheme,
        public_key_raw_bytes,
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
                msg.contains("built-in move authenticators") || msg.contains("unsupported"),
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
                "SmartAccount claim must be rejected without built-in authenticators; got success",
            );
        }
    }
}

/// Verify that a `ClaimAccountTransaction` whose public key is not a valid
/// point on its curve is rejected at validity-check time.
///
/// Execution builds the Move `PublicKey` from these bytes directly instead of
/// calling `public_key::create`, so the validity check is the only thing that
/// rejects a malformed key.
#[cfg(msim)]
#[sim_test]
async fn test_claim_account_rejected_with_invalid_public_key() {
    use iota_json_rpc_types::IotaTransactionBlockEffectsAPI;
    use iota_keys::keystore::AccountKeystore;
    use iota_sdk_types::{
        Address, ClaimAccountTransaction, SmartAccountBuildKind, SmartAccountClaim, TransactionKind,
    };
    use iota_types::transaction::{
        TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionData, TransactionDataAPI,
    };

    telemetry_subscribers::init_for_testing();

    let test_cluster = TestClusterBuilder::new().build().await;

    let owner: Address = test_cluster
        .wallet
        .config()
        .keystore()
        .addresses()
        .into_iter()
        .next()
        .expect("wallet must have at least one account");

    // A compressed secp256k1 key must start with 0x02 or 0x03, so these bytes
    // are the right length but off the curve.
    let claim = SmartAccountClaim {
        public_key_scheme: 0x01,
        public_key_raw_bytes: vec![0xff; 33],
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
                msg.contains("invalid claim account public key"),
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
                "ClaimAccount with an invalid public key must be rejected; got success",
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

/// Build the scheme flag and raw key bytes of a `SmartAccountClaim` from an
/// iota-types `IotaKeyPair`.
#[cfg(msim)]
fn claim_public_key(kp: &iota_types::crypto::IotaKeyPair) -> (u8, Vec<u8>) {
    let public_key = kp.public();
    (public_key.flag(), public_key.as_ref().to_vec())
}
