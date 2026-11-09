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

/// A second `ClaimAccount` of an address is rejected, and the account object
/// keeps the version the first claim gave it.
#[cfg(msim)]
#[sim_test]
async fn test_claim_account_twice_is_rejected() {
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
    let claim_tx = |gas| {
        let (public_key_scheme, public_key_raw_bytes) = claim_public_key(&keypair);
        Transaction::new(
            TransactionKind::new_claim_account(ClaimAccountTransaction::new_smart_account(
                SmartAccountClaim {
                    public_key_scheme,
                    public_key_raw_bytes,
                },
            )),
            owner,
            gas,
            rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
            rgp,
        )
    };

    let first = claim_tx(first_gas_coin(&test_cluster.wallet, owner).await);
    let response = test_cluster
        .wallet
        .execute_transaction_may_fail(test_cluster.wallet.sign_transaction(&first))
        .await
        .expect("the first claim must execute");
    let effects = response.effects.expect("response must include effects");
    assert!(effects.status().is_ok(), "{:?}", effects.status());
    let (account_id, _) = created_smart_accounts(
        &response
            .object_changes
            .expect("response must include object changes"),
    )
    .into_iter()
    .next()
    .expect("the claim must create a SmartAccount");
    let first_version = effects
        .created()
        .iter()
        .find(|o| o.reference.object_id == account_id)
        .map(|o| o.reference.version)
        .expect("the SmartAccount must be reported as created");

    let second = claim_tx(first_gas_coin(&test_cluster.wallet, owner).await);
    let error = test_cluster
        .wallet
        .execute_transaction_may_fail(test_cluster.wallet.sign_transaction(&second))
        .await
        .expect_err("the second claim must be rejected");
    assert!(
        error.to_string().contains("already explicit"),
        "unexpected error: {error}"
    );
    let account = test_cluster
        .get_object_from_fullnode_store(&account_id)
        .await
        .expect("the account must exist");
    assert_eq!(account.version(), first_version);
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
/// Execution validates the key again in `public_key::create`, but the
/// transaction must be turned away before it is executed.
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
        transaction::{TransactionAPI, TransactionEnvelope, claim_account_min_gas_budget},
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
    let budget = claim_account_min_gas_budget(
        &protocol_config,
        gas_price,
        claim.public_key_raw_bytes.len() as u64,
    );

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

/// A second claim paid with a gas coin older than the account. Without the
/// check it would re-create the account object below the version the account
/// has reached, which stops the nodes that execute it.
#[cfg(msim)]
#[sim_test]
async fn test_second_claim_with_an_older_gas_coin_leaves_the_account_unchanged() {
    run_second_claim_after_account_mutations(false).await;
}

/// A second claim paid with a gas coin newer than the account. Without the
/// check it would re-create the account object with a new initial shared
/// version, so transactions naming the old one would be rejected.
#[cfg(msim)]
#[sim_test]
async fn test_second_claim_with_a_newer_gas_coin_leaves_the_account_unchanged() {
    run_second_claim_after_account_mutations(true).await;
}

/// Claims the wallet's first address, raises the account's version with
/// `smart_account::add_field` calls the account signs, claims again with a
/// gas coin older or newer than the account, and checks that the second claim
/// is rejected, every node still holds the account as it was, and the
/// account still takes transactions.
#[cfg(msim)]
async fn run_second_claim_after_account_mutations(second_claim_with_newer_gas: bool) {
    use iota_json_rpc_types::IotaTransactionBlockEffectsAPI;
    use iota_keys::keystore::AccountKeystore;
    use iota_sdk_crypto::simple::SimpleKeypair;
    use iota_sdk_types::{
        Address, ClaimAccountTransaction, Identifier, ObjectId, ObjectReference,
        SharedObjectReference, SmartAccountClaim, Transaction, TransactionKind, TypeTag,
    };
    use iota_types::{
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        transaction::{CallArg, TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionAPI},
    };

    telemetry_subscribers::init_for_testing();

    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(600_000)
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
    let budget = rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE;
    let coins = test_cluster
        .wallet
        .get_gas_objects_owned_by_address(owner, None)
        .await
        .expect("gas lookup must succeed");
    assert!(coins.len() >= 4, "the owner needs four gas coins");
    let (claim_coin, mut mutation_coin, old_coin) = (coins[0], coins[1], coins[2]);

    let claim_tx = |gas: ObjectReference| {
        let (public_key_scheme, public_key_raw_bytes) = claim_public_key(&keypair);
        Transaction::new(
            TransactionKind::new_claim_account(ClaimAccountTransaction::new_smart_account(
                SmartAccountClaim {
                    public_key_scheme,
                    public_key_raw_bytes,
                },
            )),
            owner,
            gas,
            budget,
            rgp,
        )
    };
    let account_id = ObjectId::from(owner);

    let first = test_cluster
        .wallet
        .execute_transaction_may_fail(test_cluster.wallet.sign_transaction(&claim_tx(claim_coin)))
        .await
        .expect("the first claim must execute");
    let first_effects = first.effects.expect("response must include effects");
    assert!(
        first_effects.status().is_ok(),
        "{:?}",
        first_effects.status()
    );
    let first_created = first_effects
        .created()
        .iter()
        .find(|o| o.reference.object_id == account_id)
        .expect("the first claim must create the account")
        .clone();
    let Owner::Shared(initial_shared_version) = first_created.owner else {
        panic!("the account must be shared, got {:?}", first_created.owner);
    };

    for i in 0..5u64 {
        let mut builder = ProgrammableTransactionBuilder::new();
        let account = builder
            .obj(CallArg::Shared(SharedObjectReference::new(
                account_id,
                initial_shared_version,
                true,
            )))
            .unwrap();
        let name = builder.pure(i).unwrap();
        let value = builder.pure(i).unwrap();
        builder.programmable_move_call(
            ObjectId::FRAMEWORK,
            Identifier::from_static("smart_account"),
            Identifier::from_static("add_field"),
            vec![TypeTag::U64, TypeTag::U64],
            vec![account, name, value],
        );
        let tx = Transaction::new_programmable(
            owner,
            vec![mutation_coin],
            builder.finish(),
            budget,
            rgp,
        );
        let response =
            execute_as_claimed_account(&test_cluster, &keypair, tx, initial_shared_version).await;
        let effects = response.effects.expect("response must include effects");
        assert!(effects.status().is_ok(), "{:?}", effects.status());
        mutation_coin = effects.gas_object().reference;
    }
    let account_before = test_cluster
        .get_object_from_fullnode_store(&account_id)
        .await
        .expect("the account must exist");

    let second_gas = if second_claim_with_newer_gas {
        mutation_coin
    } else {
        old_coin
    };
    let second = test_cluster
        .wallet
        .execute_transaction_may_fail(test_cluster.wallet.sign_transaction(&claim_tx(second_gas)))
        .await;
    let error = second.expect_err("the second claim must be rejected");
    assert!(
        error.to_string().contains("already explicit"),
        "unexpected error: {error}"
    );

    let expected = (
        account_before.version(),
        account_before.digest(),
        Owner::Shared(initial_shared_version),
    );
    for handle in test_cluster.swarm.validator_node_handles() {
        let latest = handle.with(|node| {
            node.state()
                .get_object(&account_id)
                .map(|o| (o.version(), o.digest(), o.owner().clone()))
        });
        assert_eq!(latest, Some(expected.clone()));
    }
    let fullnode_latest = test_cluster
        .get_object_from_fullnode_store(&account_id)
        .await
        .map(|o| (o.version(), o.digest(), o.owner().clone()));
    assert_eq!(fullnode_latest, Some(expected));

    // Whether the network still runs transactions on the account.
    let mut builder = ProgrammableTransactionBuilder::new();
    let account = builder
        .obj(CallArg::Shared(SharedObjectReference::new(
            account_id,
            initial_shared_version,
            true,
        )))
        .unwrap();
    let name = builder.pure(100u64).unwrap();
    let value = builder.pure(100u64).unwrap();
    builder.programmable_move_call(
        ObjectId::FRAMEWORK,
        Identifier::from_static("smart_account"),
        Identifier::from_static("add_field"),
        vec![TypeTag::U64, TypeTag::U64],
        vec![account, name, value],
    );
    let gas = test_cluster
        .wallet
        .get_gas_objects_owned_by_address(owner, None)
        .await
        .unwrap()
        .into_iter()
        .find(|c| c.object_id == coins[3].object_id)
        .unwrap_or(coins[3]);
    let tx = Transaction::new_programmable(owner, vec![gas], builder.finish(), budget, rgp);
    let response =
        execute_as_claimed_account(&test_cluster, &keypair, tx, initial_shared_version).await;
    let effects = response.effects.expect("response must include effects");
    assert!(effects.status().is_ok(), "{:?}", effects.status());
}

/// Two claims of one address sent at the same time: exactly one is kept.
#[cfg(msim)]
#[sim_test]
async fn test_concurrent_claims_of_one_address_keep_one() {
    use iota_json_rpc_types::IotaTransactionBlockEffectsAPI;
    use iota_keys::keystore::AccountKeystore;
    use iota_sdk_crypto::simple::SimpleKeypair;
    use iota_sdk_types::{
        Address, ClaimAccountTransaction, ObjectId, ObjectReference, SmartAccountClaim,
        Transaction, TransactionKind,
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
    let coins = test_cluster
        .wallet
        .get_gas_objects_owned_by_address(owner, None)
        .await
        .expect("gas lookup must succeed");
    let claim_tx = |gas: ObjectReference| {
        let (public_key_scheme, public_key_raw_bytes) = claim_public_key(&keypair);
        test_cluster.wallet.sign_transaction(&Transaction::new(
            TransactionKind::new_claim_account(ClaimAccountTransaction::new_smart_account(
                SmartAccountClaim {
                    public_key_scheme,
                    public_key_raw_bytes,
                },
            )),
            owner,
            gas,
            rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
            rgp,
        ))
    };
    let (first, second) = (claim_tx(coins[0]), claim_tx(coins[1]));

    let (first, second) = tokio::join!(
        test_cluster.wallet.execute_transaction_may_fail(first),
        test_cluster.wallet.execute_transaction_may_fail(second),
    );
    let succeeded = [&first, &second]
        .into_iter()
        .filter(|result| {
            result.as_ref().is_ok_and(|response| {
                response
                    .effects
                    .as_ref()
                    .is_some_and(|effects| effects.status().is_ok())
            })
        })
        .count();
    assert_eq!(
        succeeded, 1,
        "exactly one claim must succeed; got {first:?} and {second:?}"
    );
    assert!(
        test_cluster
            .get_object_from_fullnode_store(&ObjectId::from(owner))
            .await
            .is_some()
    );
}

/// An address claimed in an earlier epoch cannot be claimed again.
#[cfg(msim)]
#[sim_test]
async fn test_claim_in_a_later_epoch_is_rejected() {
    use iota_json_rpc_types::IotaTransactionBlockEffectsAPI;
    use iota_keys::keystore::AccountKeystore;
    use iota_sdk_crypto::simple::SimpleKeypair;
    use iota_sdk_types::{
        Address, ClaimAccountTransaction, ObjectReference, SmartAccountClaim, Transaction,
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
    let claim_tx = |gas: ObjectReference| {
        let (public_key_scheme, public_key_raw_bytes) = claim_public_key(&keypair);
        test_cluster.wallet.sign_transaction(&Transaction::new(
            TransactionKind::new_claim_account(ClaimAccountTransaction::new_smart_account(
                SmartAccountClaim {
                    public_key_scheme,
                    public_key_raw_bytes,
                },
            )),
            owner,
            gas,
            rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
            rgp,
        ))
    };

    let response = test_cluster
        .wallet
        .execute_transaction_may_fail(claim_tx(first_gas_coin(&test_cluster.wallet, owner).await))
        .await
        .expect("the first claim must execute");
    let effects = response.effects.expect("response must include effects");
    assert!(effects.status().is_ok(), "{:?}", effects.status());

    test_cluster.force_new_epoch().await;

    let error = test_cluster
        .wallet
        .execute_transaction_may_fail(claim_tx(first_gas_coin(&test_cluster.wallet, owner).await))
        .await
        .expect_err("a claim in a later epoch must be rejected");
    assert!(
        error.to_string().contains("already explicit"),
        "unexpected error: {error}"
    );
}

/// Once an address is claimed, a transaction where it signs with a plain
/// signature is rejected; the same transaction signed by the account's
/// built-in authenticator executes.
#[cfg(msim)]
#[sim_test]
async fn test_plain_signature_for_a_claimed_address_is_rejected() {
    use iota_json_rpc_types::IotaTransactionBlockEffectsAPI;
    use iota_keys::keystore::AccountKeystore;
    use iota_sdk_crypto::simple::SimpleKeypair;
    use iota_sdk_types::{
        Address, ClaimAccountTransaction, Identifier, ObjectId, SharedObjectReference,
        SmartAccountClaim, Transaction, TransactionKind, TypeTag,
    };
    use iota_types::{
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        transaction::{CallArg, TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionAPI},
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
    let budget = rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE;

    let (public_key_scheme, public_key_raw_bytes) = claim_public_key(&keypair);
    let claim = Transaction::new(
        TransactionKind::new_claim_account(ClaimAccountTransaction::new_smart_account(
            SmartAccountClaim {
                public_key_scheme,
                public_key_raw_bytes,
            },
        )),
        owner,
        first_gas_coin(&test_cluster.wallet, owner).await,
        budget,
        rgp,
    );
    let response = test_cluster
        .wallet
        .execute_transaction_may_fail(test_cluster.wallet.sign_transaction(&claim))
        .await
        .expect("the claim must execute");
    let effects = response.effects.expect("response must include effects");
    assert!(effects.status().is_ok(), "{:?}", effects.status());
    let account_id = ObjectId::from(owner);
    let Owner::Shared(initial_shared_version) = effects
        .created()
        .iter()
        .find(|o| o.reference.object_id == account_id)
        .expect("the claim must create the account")
        .owner
        .clone()
    else {
        panic!("the account must be shared");
    };

    let mut builder = ProgrammableTransactionBuilder::new();
    let account = builder
        .obj(CallArg::Shared(SharedObjectReference::new(
            account_id,
            initial_shared_version,
            true,
        )))
        .unwrap();
    let name = builder.pure(1u64).unwrap();
    let value = builder.pure(1u64).unwrap();
    builder.programmable_move_call(
        ObjectId::FRAMEWORK,
        Identifier::from_static("smart_account"),
        Identifier::from_static("add_field"),
        vec![TypeTag::U64, TypeTag::U64],
        vec![account, name, value],
    );
    let tx = Transaction::new_programmable(
        owner,
        vec![first_gas_coin(&test_cluster.wallet, owner).await],
        builder.finish(),
        budget,
        rgp,
    );

    let error = test_cluster
        .wallet
        .execute_transaction_may_fail(test_cluster.wallet.sign_transaction(&tx))
        .await
        .expect_err("a plain signature for the claimed address must be rejected");
    assert!(
        error
            .to_string()
            .contains("must sign with a MoveAuthenticator"),
        "unexpected error: {error}"
    );

    let response =
        execute_as_claimed_account(&test_cluster, &keypair, tx, initial_shared_version).await;
    let effects = response.effects.expect("response must include effects");
    assert!(effects.status().is_ok(), "{:?}", effects.status());
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Executes `tx`, sent by a claimed account, signed by the account's built-in
/// authenticator with `keypair`, as an explicit account must sign. A
/// just-claimed account is not visible at the commit until its claim is a
/// few commits old, so a rejection is retried for a while.
#[cfg(msim)]
async fn execute_as_claimed_account(
    test_cluster: &test_cluster::TestCluster,
    keypair: &iota_sdk_crypto::simple::SimpleKeypair,
    tx: iota_sdk_types::Transaction,
    initial_shared_version: iota_sdk_types::Version,
) -> iota_json_rpc_types::IotaTransactionBlockResponse {
    use iota_sdk_crypto::Signer;
    use iota_sdk_types::{
        MoveAuthenticatorV1, ObjectId, SenderSignedTransaction, SharedObjectReference,
        UserSignature,
        crypto::{Intent, IntentMessage, SimpleSignature},
    };
    use iota_types::transaction::{CallArg, TransactionAPI, TransactionEnvelope};

    let intent_msg = IntentMessage::new(Intent::iota_transaction(), tx.clone());
    let signature: SimpleSignature = keypair.sign(&intent_msg.signing_digest());
    let signature_bytes = UserSignature::Simple(signature).to_bytes();
    let authenticator = MoveAuthenticatorV1::new(
        vec![CallArg::Pure(
            bcs::to_bytes(&signature_bytes).expect("signature bytes encode"),
        )],
        vec![],
        SharedObjectReference::new(ObjectId::from(tx.sender()), initial_shared_version, false),
    );
    let mut last_error = None;
    for _ in 0..30 {
        let signed = TransactionEnvelope::new(SenderSignedTransaction::new(
            tx.clone(),
            vec![UserSignature::MoveAuthenticator(
                authenticator.clone().into(),
            )],
        ));
        match test_cluster
            .wallet
            .execute_transaction_may_fail(signed)
            .await
        {
            Ok(response) => return response,
            Err(error) => {
                last_error = Some(error);
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    }
    panic!("the account's transaction must execute: {last_error:?}");
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

/// Build the scheme flag and raw key bytes of a `SmartAccountClaim` from a
/// `SimpleKeypair`.
#[cfg(msim)]
fn claim_public_key(kp: &iota_sdk_crypto::simple::SimpleKeypair) -> (u8, Vec<u8>) {
    let public_key = kp.public_key();
    (public_key.scheme().to_u8(), public_key.as_ref().to_vec())
}
