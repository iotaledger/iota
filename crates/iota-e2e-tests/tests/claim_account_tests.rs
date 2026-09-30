// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests for the `ClaimAccount` transaction kind.

#[cfg(msim)]
use iota_macros::sim_test;
#[cfg(msim)]
use iota_sdk_types::Owner;
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
    use iota_sdk_crypto::simple::SimpleKeypair;
    use iota_sdk_types::{
        Address, ClaimAccountTransaction, SmartAccountBuildKind, SmartAccountClaim, Transaction,
        TransactionKind,
    };
    use iota_types::transaction::{
        TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionAPI,
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

    let keypair: SimpleKeypair = test_cluster
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
    let tx_data = Transaction::new(
        kind,
        owner,
        first_gas_coin(&test_cluster.wallet, owner).await,
        rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        rgp,
    );
    let response = test_cluster
        .wallet
        .execute_transaction_may_fail(test_cluster.wallet.sign_transaction(&tx_data))
        .await
        .expect("ClaimAccount transaction must execute");

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

/// A `ClaimAccount` at exactly the budget floor the validity check demands
/// must not run out of gas: the sequencer stages the claim entry before the
/// claim executes, so an aborting claim would leave the address explicit with
/// no account object behind it. The floor scales with the gas price and the
/// key size, so the largest MultiSig committee at the highest admissible gas
/// price is the most expensive claim the floor has to cover.
#[cfg(msim)]
#[sim_test]
async fn test_claim_account_at_the_gas_floor_succeeds() {
    use iota_json_rpc_types::IotaTransactionBlockEffectsAPI;
    use iota_protocol_config::{Chain, ProtocolConfig, ProtocolVersion};
    use iota_sdk_crypto::{Signer, ed25519::Ed25519PrivateKey, simple::SimpleKeypair};
    use iota_sdk_types::{
        Address, ClaimAccountTransaction, SmartAccountBuildKind, SmartAccountClaim, Transaction,
        TransactionKind, UserSignature,
        crypto::{
            MULTISIG_COMMITTEE_SIZE_MAX, MultisigAggregatedSignature, MultisigCommittee,
            MultisigMember, SimpleSignature,
        },
    };
    use iota_types::transaction::{TransactionAPI, TransactionEnvelope};

    telemetry_subscribers::init_for_testing();

    let test_cluster = TestClusterBuilder::new().build().await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let protocol_config = ProtocolConfig::get_for_version(ProtocolVersion::MAX, Chain::Unknown);
    let gas_price = protocol_config.max_gas_price();

    // The largest committee the chain accepts, with a threshold one member
    // meets.
    let keys: Vec<Ed25519PrivateKey> = (0..MULTISIG_COMMITTEE_SIZE_MAX)
        .map(|_| Ed25519PrivateKey::random())
        .collect();
    let committee = MultisigCommittee::new(
        keys.iter()
            .map(|key| MultisigMember::new(key.public_key(), 1))
            .collect(),
        1,
    )
    .expect("a valid committee");
    let sender = Address::from(&committee);
    let claim = SmartAccountClaim::new_multisig(&committee, SmartAccountBuildKind::Mutable);
    let budget = protocol_config
        .claim_account_min_gas_budget(gas_price, claim.public_key_raw_bytes.len() as u64);

    let gas = test_cluster
        .fund_address_and_return_gas(rgp, Some(2 * budget), sender)
        .await;
    let tx_data = Transaction::new(
        TransactionKind::new_claim_account(ClaimAccountTransaction::new_smart_account(claim)),
        sender,
        gas,
        budget,
        gas_price,
    );
    let signer: SimpleKeypair = keys[0].clone().into();
    let signature: SimpleSignature = signer.sign(&tx_data.signing_digest());
    let multisig = UserSignature::Multisig(MultisigAggregatedSignature::new_unchecked(
        vec![signature.into()],
        0b1,
        committee,
    ));
    let tx = TransactionEnvelope::from_user_sig_data(tx_data, vec![multisig]);

    let response = test_cluster
        .wallet
        .execute_transaction_may_fail(tx)
        .await
        .expect("a claim at the floor must execute");
    let effects = response.effects.expect("response must include effects");
    assert!(
        effects.status().is_ok(),
        "a claim at the floor must not run out of gas; got {:?}",
        effects.status(),
    );
    let object_changes = response
        .object_changes
        .expect("response must include object changes");
    assert_eq!(
        created_smart_accounts(&object_changes).len(),
        1,
        "the claim must create the account object"
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
    use iota_sdk_crypto::simple::SimpleKeypair;
    use iota_sdk_types::{
        Address, ClaimAccountTransaction, SmartAccountBuildKind, SmartAccountClaim, Transaction,
        TransactionKind,
    };
    use iota_types::transaction::{
        TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionAPI,
    };

    telemetry_subscribers::init_for_testing();

    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(20000)
        .build()
        .await;

    // Use a different wallet account from the mutable test to avoid double-claim.
    let addresses = test_cluster.wallet.config().keystore().addresses();
    let owner: Address = addresses.get(1).copied().unwrap_or(addresses[0]);

    let keypair: SimpleKeypair = test_cluster
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
    let tx_data = Transaction::new(
        kind,
        owner,
        first_gas_coin(&test_cluster.wallet, owner).await,
        rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        rgp,
    );
    let response = test_cluster
        .wallet
        .execute_transaction_may_fail(test_cluster.wallet.sign_transaction(&tx_data))
        .await
        .expect("ClaimAccount transaction must execute");

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

/// Claiming an address twice: the second `ClaimAccount` is dropped by the
/// sequencer's duplicate-claim guard before it executes, and the account
/// object keeps the version the first claim gave it. Nothing on the Move side
/// prevents a double claim, so the guard is the only thing standing between a
/// second claim and a second object under the same id.
#[cfg(msim)]
#[sim_test]
async fn test_claim_account_twice_is_rejected() {
    use iota_json_rpc_types::IotaTransactionBlockEffectsAPI;
    use iota_keys::keystore::AccountKeystore;
    use iota_sdk_crypto::simple::SimpleKeypair;
    use iota_sdk_types::{
        Address, ClaimAccountTransaction, SmartAccountBuildKind, SmartAccountClaim, Transaction,
        TransactionKind,
    };
    use iota_types::transaction::{
        TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionAPI,
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

    let keypair: SimpleKeypair = test_cluster
        .wallet
        .config()
        .keystore()
        .get_key(&owner)
        .expect("keypair must exist for owner")
        .as_keypair()
        .expect("stored key must be a keypair")
        .clone();

    let rgp = test_cluster.get_reference_gas_price().await;
    let claim_tx = |gas| {
        let (public_key_scheme, public_key_raw_bytes) = claim_public_key(&keypair);
        let claim = SmartAccountClaim {
            public_key_scheme,
            public_key_raw_bytes,
            build_kind: SmartAccountBuildKind::Immutable,
        };
        let tx_data = Transaction::new(
            TransactionKind::new_claim_account(ClaimAccountTransaction::new_smart_account(claim)),
            owner,
            gas,
            rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
            rgp,
        );
        test_cluster.wallet.sign_transaction(&tx_data)
    };

    let response = test_cluster
        .wallet
        .execute_transaction_may_fail(claim_tx(first_gas_coin(&test_cluster.wallet, owner).await))
        .await
        .expect("the first claim must execute");
    let effects = response.effects.expect("response must include effects");
    assert!(
        effects.status().is_ok(),
        "the first claim must succeed; got {:?}",
        effects.status(),
    );
    let object_changes = response
        .object_changes
        .expect("response must include object changes");
    let (account_id, _) = created_smart_accounts(&object_changes)
        .into_iter()
        .next()
        .expect("the claim must create a SmartAccount");
    let first_version = effects
        .created()
        .iter()
        .find(|o| o.reference.object_id == account_id)
        .map(|o| o.reference.version)
        .expect("the SmartAccount must be reported as created");

    // The second claim is dropped by the sequencer, so the client sees an
    // error instead of effects.
    let error = test_cluster
        .wallet
        .execute_transaction_may_fail(claim_tx(first_gas_coin(&test_cluster.wallet, owner).await))
        .await
        .expect_err("a second claim for an explicit address must be rejected");
    assert!(
        format!("{error:#}").contains("already explicit"),
        "unexpected error for the second claim: {error:#}",
    );

    let account = test_cluster
        .get_object_from_fullnode_store(&account_id)
        .await
        .expect("the account object must exist");
    assert_eq!(
        account.version(),
        first_version,
        "the account object must keep the version the first claim created it at",
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
    use iota_sdk_crypto::simple::SimpleKeypair;
    use iota_sdk_types::{
        Address, ClaimAccountTransaction, SmartAccountBuildKind, SmartAccountClaim, Transaction,
        TransactionKind,
    };
    use iota_types::transaction::{
        TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionAPI,
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

    let keypair: SimpleKeypair = test_cluster
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
    let tx_data = Transaction::new(
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
    use iota_sdk_crypto::simple::SimpleKeypair;
    use iota_sdk_types::{
        Address, ClaimAccountTransaction, SmartAccountBuildKind, SmartAccountClaim, Transaction,
        TransactionKind,
    };
    use iota_types::transaction::{
        TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionAPI,
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

    let keypair: SimpleKeypair = test_cluster
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
    let tx_data = Transaction::new(
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
        Address, ClaimAccountTransaction, SmartAccountBuildKind, SmartAccountClaim, Transaction,
        TransactionKind,
    };
    use iota_types::transaction::{
        TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionAPI,
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
    let tx_data = Transaction::new(
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

/// Build the scheme flag and raw key bytes of a `SmartAccountClaim` from a
/// `SimpleKeypair`.
#[cfg(msim)]
fn claim_public_key(kp: &iota_sdk_crypto::simple::SimpleKeypair) -> (u8, Vec<u8>) {
    let public_key = kp.public_key();
    (public_key.scheme().to_u8(), public_key.as_ref().to_vec())
}
