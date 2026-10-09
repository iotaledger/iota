// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests for the `ClaimAccount` transaction kind.

#[cfg(msim)]
use iota_macros::sim_test;
#[cfg(msim)]
use iota_sdk_types::Owner;
#[cfg(msim)]
use test_cluster::TestClusterBuilder;

/// Verify that a `TransactionKind::ClaimAccount` succeeds when both
/// protocol flags are enabled, and that the transaction is accepted.
#[cfg(msim)]
#[sim_test]
async fn test_claim_account_succeeds() {
    use iota_json_rpc_types::IotaTransactionBlockEffectsAPI;
    use iota_keys::keystore::AccountKeystore;
    use iota_sdk_crypto::simple::SimpleKeypair;
    use iota_sdk_types::{
        Address, ClaimAccountTransaction, SmartAccountClaim, Transaction, TransactionKind,
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
        "ClaimAccount transaction must succeed; got {:?}",
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
        "A claimed SmartAccount must be a shared object; got {sa_owner:?}",
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
    use iota_sdk_crypto::simple::SimpleKeypair;
    use iota_sdk_types::{
        Address, ClaimAccountTransaction, SmartAccountClaim, Transaction, TransactionKind,
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
    let mut claimed = Vec::new();

    for attempt in 1..=2 {
        let (public_key_scheme, public_key_raw_bytes) = claim_public_key(&keypair);
        let claim = SmartAccountClaim {
            public_key_scheme,
            public_key_raw_bytes,
        };
        let tx_data = Transaction::new(
            TransactionKind::new_claim_account(ClaimAccountTransaction::new_smart_account(claim)),
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
        "the account object was expected to be overwritten today; got \
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
    use iota_sdk_crypto::simple::SimpleKeypair;
    use iota_sdk_types::{
        Address, ClaimAccountTransaction, SmartAccountClaim, Transaction, TransactionKind,
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
        Address, ClaimAccountTransaction, SmartAccountClaim, Transaction, TransactionKind,
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
        Address, ClaimAccountTransaction, SmartAccountClaim, Transaction, TransactionKind,
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

/// A `ClaimAccount` at exactly the budget floor the validity check demands
/// must not run out of gas. The floor scales with the gas price and the key
/// size, so the largest MultiSig committee at the highest admissible gas price
/// is the most expensive claim the floor has to cover.
#[cfg(msim)]
#[sim_test]
async fn test_claim_account_at_the_gas_floor_succeeds() {
    use iota_json_rpc_types::IotaTransactionBlockEffectsAPI;
    use iota_protocol_config::{Chain, ProtocolConfig, ProtocolVersion};
    use iota_sdk_crypto::{Signer, ed25519::Ed25519PrivateKey, simple::SimpleKeypair};
    use iota_sdk_types::{
        ClaimAccountTransaction, SignatureScheme, SmartAccountClaim, Transaction, TransactionKind,
        UserSignature,
        crypto::{
            Intent, IntentMessage, MULTISIG_COMMITTEE_SIZE_MAX, MultisigAggregatedSignature,
            MultisigCommittee, MultisigMember, SimpleSignature,
        },
    };
    use iota_types::{
        account_abstraction::public_key::MovePublicKey,
        transaction::{TransactionAPI, TransactionEnvelope},
    };
    use rand::{SeedableRng, rngs::StdRng};

    telemetry_subscribers::init_for_testing();

    let test_cluster = TestClusterBuilder::new().build().await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let protocol_config = ProtocolConfig::get_for_version(ProtocolVersion::MAX, Chain::Unknown);
    let gas_price = protocol_config.max_gas_price();

    // The largest committee the chain accepts, with a threshold one member
    // meets.
    let mut rng = StdRng::from_seed([7; 32]);
    let keys: Vec<Ed25519PrivateKey> = (0..MULTISIG_COMMITTEE_SIZE_MAX)
        .map(|_| Ed25519PrivateKey::random_with(&mut rng))
        .collect();
    let committee = MultisigCommittee::new(
        keys.iter()
            .map(|key| MultisigMember::new(key.public_key(), 1))
            .collect(),
        1,
    )
    .expect("a valid committee");
    let claim = SmartAccountClaim::new_multisig(&committee);
    let sender = MovePublicKey::new(
        SignatureScheme::Multisig,
        claim.public_key_raw_bytes.clone(),
    )
    .expect("a valid multisig key")
    .address()
    .expect("a multisig key derives an address");
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
    let signer = SimpleKeypair::from(keys[0].clone());
    let digest = IntentMessage::new(Intent::iota_transaction(), tx_data.clone()).signing_digest();
    let signature: SimpleSignature = signer.sign(&digest);
    let multisig = MultisigAggregatedSignature::new(vec![signature.into()], committee)
        .expect("a valid multisig signature");
    let tx =
        TransactionEnvelope::from_user_sig_data(tx_data, vec![UserSignature::Multisig(multisig)]);

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

/// Verify that a `TransactionKind::ClaimAccount` whose public key derives an
/// address other than the sender is rejected at validation, before it is
/// executed.
#[cfg(msim)]
#[sim_test]
async fn test_claim_account_rejected_when_key_does_not_derive_sender() {
    use iota_keys::keystore::AccountKeystore;
    use iota_sdk_crypto::simple::SimpleKeypair;
    use iota_sdk_types::{
        Address, ClaimAccountTransaction, SmartAccountClaim, Transaction, TransactionKind,
    };
    use iota_types::transaction::{
        TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionAPI,
    };

    telemetry_subscribers::init_for_testing();

    let test_cluster = TestClusterBuilder::new().build().await;

    let mut addresses = test_cluster
        .wallet
        .config()
        .keystore()
        .addresses()
        .into_iter();
    let sender: Address = addresses
        .next()
        .expect("wallet must have at least one account");
    let other: Address = addresses
        .next()
        .expect("wallet must have at least two accounts");

    // The claim carries the key of another account, which derives `other`.
    let keypair: SimpleKeypair = test_cluster
        .wallet
        .config()
        .keystore()
        .get_key(&other)
        .expect("keypair must exist for the other account")
        .as_keypair()
        .expect("stored key must be a keypair")
        .clone();
    let (public_key_scheme, public_key_raw_bytes) = claim_public_key(&keypair);
    let claim = SmartAccountClaim {
        public_key_scheme,
        public_key_raw_bytes,
    };
    let kind =
        TransactionKind::new_claim_account(ClaimAccountTransaction::new_smart_account(claim));

    let rgp = test_cluster.get_reference_gas_price().await;
    let tx_data = Transaction::new(
        kind,
        sender,
        first_gas_coin(&test_cluster.wallet, sender).await,
        rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        rgp,
    );

    // The transaction must be rejected before execution
    // (UserInputError::IncorrectUserSignature), not abort in Move.
    let err = test_cluster
        .wallet
        .execute_transaction_may_fail(test_cluster.wallet.sign_transaction(&tx_data))
        .await
        .expect_err("a claim whose key does not derive the sender must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("claimed public key derives"),
        "unexpected error message: {msg}",
    );
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
