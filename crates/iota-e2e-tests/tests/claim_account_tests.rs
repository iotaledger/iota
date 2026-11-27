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

    let _guard = enable_claim_account();

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

    let _guard = enable_claim_account();

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
/// with the default config of a local network, where
/// `enable_claim_account_transaction` is disabled.
#[cfg(msim)]
#[sim_test]
async fn test_claim_account_rejected_when_disabled() {
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
    // (UserInputError::Unsupported), not executed and failed.
    let err = test_cluster
        .wallet
        .execute_transaction_may_fail(test_cluster.wallet.sign_transaction(&tx_data))
        .await
        .expect_err("a ClaimAccount must be rejected while the kind is disabled");
    let msg = err.to_string();
    assert!(
        msg.contains("claim account transactions are not enabled"),
        "unexpected error message: {msg}",
    );
}

/// Verify that a `ClaimAccountTransaction` whose public key is not a valid
/// point on its curve is rejected at validity-check time.
///
/// Execution validates the key again in `public_key::create`, but the
/// transaction must be turned away before it is executed.
#[cfg(msim)]
#[sim_test]
async fn test_claim_account_rejected_with_invalid_public_key() {
    use iota_keys::keystore::AccountKeystore;
    use iota_sdk_types::{
        Address, ClaimAccountTransaction, SmartAccountClaim, Transaction, TransactionKind,
    };
    use iota_types::transaction::{
        TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionAPI,
    };

    telemetry_subscribers::init_for_testing();

    let _guard = enable_claim_account();

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
    // (UserInputError::Unsupported), not executed and aborted in
    // `public_key::create`.
    let err = test_cluster
        .wallet
        .execute_transaction_may_fail(test_cluster.wallet.sign_transaction(&tx_data))
        .await
        .expect_err("a ClaimAccount with an invalid public key must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("invalid claim account public key: "),
        "unexpected error message: {msg}",
    );
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
        move_public_key::MovePublicKey,
        transaction::{TransactionAPI, TransactionEnvelope, claim_account_min_gas_budget},
    };
    use rand::{SeedableRng, rngs::StdRng};

    telemetry_subscribers::init_for_testing();

    let _guard = enable_claim_account();

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

    let _guard = enable_claim_account();

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

/// With `enable_claim_account_transaction` off, a `ClaimAccount` never reaches
/// a commit. An honest validator rejects it at submission, so one validator
/// skips its validity and input checks and puts it in a block, as a faulty one
/// can. The other validators refuse that block, so nobody sequences or
/// executes the claim. They keep building the certified checkpoints, and an
/// ordinary transaction submitted through one of them still executes.
///
/// Simulator only, because the skipped checks are fail points.
#[cfg(msim)]
#[sim_test]
async fn test_claim_account_from_a_faulty_validator_is_refused_when_disabled() {
    use std::{net::SocketAddr, time::Duration};

    use iota_core::authority_client::validator_v2::ValidatorV2API;
    use iota_keys::keystore::AccountKeystore;
    use iota_macros::{clear_fail_point, register_fail_point_if};
    use iota_sdk_crypto::simple::SimpleKeypair;
    use iota_sdk_types::{
        Address, ClaimAccountTransaction, SmartAccountClaim, Transaction, TransactionKind,
    };
    use iota_types::{
        effects::TransactionEffectsAPI,
        error::{IotaError, UserInputError},
        messages_grpc::TxStatusUpdate,
        transaction::{TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionAPI},
    };

    telemetry_subscribers::init_for_testing();

    let test_cluster = TestClusterBuilder::new().build().await;
    let mut addresses = test_cluster
        .wallet
        .config()
        .keystore()
        .addresses()
        .into_iter();
    let owner: Address = addresses.next().expect("wallet must have an account");
    let other: Address = addresses.next().expect("wallet must have a second account");
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
    let kind = TransactionKind::new_claim_account(ClaimAccountTransaction::new_smart_account(
        SmartAccountClaim {
            public_key_scheme,
            public_key_raw_bytes,
        },
    ));
    let rgp = test_cluster.get_reference_gas_price().await;
    let gas = first_gas_coin(&test_cluster.wallet, owner).await;
    let claim = test_cluster.wallet.sign_transaction(&Transaction::new(
        kind,
        owner,
        gas,
        rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        rgp,
    ));
    let claim_digest = *claim.digest();

    // An honest validator rejects the claim at submission.
    let client_ip = SocketAddr::new([127, 0, 0, 1].into(), 0);
    let validators = test_cluster.get_validator_pubkeys();
    let client_of = |name| {
        test_cluster.authority_aggregator().authority_clients[name]
            .authority_client()
            .clone()
    };
    let rejected = client_of(&validators[1])
        .submit_tx(vec![claim.clone()], Some(client_ip))
        .await
        .unwrap();
    assert!(
        matches!(
            rejected.as_slice(),
            [(
                digest,
                TxStatusUpdate::Rejected {
                    error: IotaError::UserInput {
                        error: UserInputError::Unsupported(_)
                    }
                }
            )] if *digest == claim_digest
        ),
        "an honest validator must reject the claim as unsupported, got {rejected:?}"
    );

    // One validator skips its validity and input checks and puts the claim in
    // a block.
    let faulty = validators[0];
    let faulty_sim_id = test_cluster
        .swarm
        .node(&faulty)
        .unwrap()
        .get_node_handle()
        .unwrap()
        .with(|_| iota_simulator::current_simnode_id());
    for fail_point in ["admit-without-validity-check", "admit-without-input-checks"] {
        register_fail_point_if(fail_point, move || {
            iota_simulator::current_simnode_id() == faulty_sim_id
        });
    }
    let submitted = tokio::time::timeout(
        Duration::from_secs(60),
        client_of(&faulty).submit_tx(vec![claim], Some(client_ip)),
    )
    .await
    .expect("the submission must be answered within 60s")
    .unwrap();
    clear_fail_point("admit-without-validity-check");
    clear_fail_point("admit-without-input-checks");
    assert!(
        matches!(
            submitted.as_slice(),
            [(digest, TxStatusUpdate::Submitted)] if *digest == claim_digest
        ),
        "the validator skipping its checks must submit the claim, got {submitted:?}"
    );

    // The other validators keep executing checkpoints, and the ones they build
    // themselves are the certified ones. The faulty validator is left out: the
    // others refuse its blocks from the one with the claim on.
    let highest_executed = |handle: &iota_node::IotaNodeHandle| {
        handle.with(|node| {
            node.state()
                .get_checkpoint_store()
                .get_highest_executed_checkpoint_seq_number()
                .unwrap()
                .unwrap_or_default()
        })
    };
    let last_built = |handle: &iota_node::IotaNodeHandle| {
        handle.with(|node| {
            node.state()
                .epoch_store_for_testing()
                .last_built_checkpoint_builder_summary()
                .unwrap()
                .map(|summary| summary.summary.sequence_number)
                .unwrap_or_default()
        })
    };
    let honest: Vec<_> = validators[1..]
        .iter()
        .map(|name| {
            test_cluster
                .swarm
                .node(name)
                .unwrap()
                .get_node_handle()
                .unwrap()
        })
        .collect();
    let first_compared = honest.iter().map(highest_executed).min().unwrap();
    let target = honest.iter().map(highest_executed).max().unwrap() + 20;
    for handle in &honest {
        tokio::time::timeout(Duration::from_secs(180), async {
            while highest_executed(handle) < target || last_built(handle) < target {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        })
        .await
        .expect("the other validators must execute and build checkpoints past the refusal");
        handle.with(|node| {
            let store = node.state().get_checkpoint_store().clone();
            for sequence_number in first_compared..=target {
                let local = store
                    .get_locally_computed_checkpoint(sequence_number)
                    .unwrap();
                let certified = store
                    .get_checkpoint_by_sequence_number(sequence_number)
                    .unwrap();
                if let (Some(local), Some(certified)) = (local, certified) {
                    assert_eq!(
                        local.digest(),
                        *certified.digest(),
                        "a validator's checkpoint {sequence_number} must match the certified one"
                    );
                }
            }
        });
    }

    // The claim reached no commit: twenty checkpoints later no other validator
    // has dropped it at a commit, which it does with a claim it sequences.
    for handle in &honest {
        let sequenced = tokio::time::timeout(
            Duration::from_secs(1),
            handle.with_async(|node| async move {
                node.state()
                    .epoch_store_for_testing()
                    .notify_read_dropped_digests(claim_digest)
                    .await
            }),
        )
        .await;
        assert!(
            sequenced.is_err(),
            "a validator sequenced the claim and dropped it: {sequenced:?}"
        );
    }

    // Nobody executed the claim, and its gas coin keeps its version.
    for handle in test_cluster.all_node_handles() {
        let executed = handle.with(|node| {
            node.state()
                .get_transaction_cache_reader()
                .try_get_executed_effects(&claim_digest)
                .unwrap()
        });
        assert!(executed.is_none(), "no node may execute the claim");
    }
    assert_eq!(
        test_cluster.get_latest_object_ref(&gas.object_id).await,
        gas,
        "the dropped claim's gas coin must keep its version"
    );

    // An ordinary transaction submitted through another validator still
    // executes.
    let transfer = test_cluster.wallet.sign_transaction(
        &test_cluster
            .test_transaction_builder_with_sender(other)
            .await
            .transfer_iota(Some(1), owner)
            .build(),
    );
    let transfer_digest = *transfer.digest();
    let submitted = client_of(&validators[1])
        .submit_tx(vec![transfer], Some(client_ip))
        .await
        .unwrap();
    assert!(
        matches!(
            submitted.as_slice(),
            [(digest, TxStatusUpdate::Submitted)] if *digest == transfer_digest
        ),
        "another validator must submit the transfer, got {submitted:?}"
    );
    let effects = tokio::time::timeout(
        Duration::from_secs(60),
        test_cluster
            .fullnode_handle
            .iota_node
            .with_async(|node| async move {
                node.state()
                    .get_transaction_cache_reader()
                    .try_notify_read_executed_effects("", &[transfer_digest])
                    .await
            }),
    )
    .await
    .expect("the transfer must execute within 60s")
    .unwrap()
    .remove(0);
    assert!(effects.status().is_success(), "{:?}", effects.status());
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

    let _guard = enable_claim_account();

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

    let _guard = enable_claim_account();

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

    let _guard = enable_claim_account();

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

    let _guard = enable_claim_account();

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

/// Turn on `enable_claim_account_transaction`, which no protocol version
/// enables yet, with the bounds on a claim's cost that it needs, until the
/// returned guard is dropped.
#[cfg(msim)]
fn enable_claim_account() -> iota_protocol_config::OverrideGuard {
    iota_protocol_config::ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_claim_account_max_computation_units_for_testing(5_000);
        config.set_claim_account_storage_bytes_bound_for_testing(2_000);
        config.set_enable_claim_account_transaction_for_testing(true);
        config
    })
}

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

// ---------------------------------------------------------------------------
// Claims that reach a commit: sponsors, races through two validators, and
// transactions a faulty validator admits
// ---------------------------------------------------------------------------

/// The Ed25519 keypair the wallet holds for `address`.
#[cfg(msim)]
fn wallet_keypair(
    test_cluster: &test_cluster::TestCluster,
    address: iota_sdk_types::Address,
) -> iota_sdk_crypto::simple::SimpleKeypair {
    use iota_keys::keystore::AccountKeystore;

    test_cluster
        .wallet
        .config()
        .keystore()
        .get_key(&address)
        .expect("the wallet holds the address's key")
        .as_keypair()
        .expect("stored key must be a keypair")
        .clone()
}

/// A `ClaimAccount` of `owner` with `keypair`'s key, paid with `gas`,
/// unsigned.
#[cfg(msim)]
fn claim_transaction(
    keypair: &iota_sdk_crypto::simple::SimpleKeypair,
    owner: iota_sdk_types::Address,
    gas: iota_sdk_types::ObjectReference,
    gas_price: u64,
) -> iota_sdk_types::Transaction {
    use iota_sdk_types::{
        ClaimAccountTransaction, SmartAccountClaim, Transaction, TransactionKind,
    };
    use iota_types::transaction::TransactionAPI;

    let (public_key_scheme, public_key_raw_bytes) = claim_public_key(keypair);
    Transaction::new(
        TransactionKind::new_claim_account(ClaimAccountTransaction::new_smart_account(
            SmartAccountClaim {
                public_key_scheme,
                public_key_raw_bytes,
            },
        )),
        owner,
        gas,
        gas_price * iota_types::transaction::TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        gas_price,
    )
}

/// Submits `transaction` to `validator` alone and returns its answer.
#[cfg(msim)]
async fn submit_to(
    test_cluster: &test_cluster::TestCluster,
    validator: &iota_types::base_types::AuthorityName,
    transaction: iota_types::transaction::TransactionEnvelope,
) -> iota_types::messages_grpc::TxStatusUpdate {
    use std::net::SocketAddr;

    use iota_core::authority_client::validator_v2::ValidatorV2API;

    let mut answers = test_cluster.authority_aggregator().authority_clients[validator]
        .authority_client()
        .clone()
        .submit_tx(
            vec![transaction],
            Some(SocketAddr::new([127, 0, 0, 1].into(), 0)),
        )
        .await
        .unwrap();
    assert_eq!(answers.len(), 1);
    answers.remove(0).1
}

/// Submits `transaction` to `validator`, which skips its validity and input
/// checks, as a faulty validator can, so that it reaches a commit.
#[cfg(msim)]
async fn submit_through_a_faulty_validator(
    test_cluster: &test_cluster::TestCluster,
    validator: &iota_types::base_types::AuthorityName,
    transaction: iota_types::transaction::TransactionEnvelope,
) {
    use iota_macros::{clear_fail_point, register_fail_point_if};
    use iota_types::messages_grpc::TxStatusUpdate;

    let faulty_sim_id = test_cluster
        .swarm
        .node(validator)
        .unwrap()
        .get_node_handle()
        .unwrap()
        .with(|_| iota_simulator::current_simnode_id());
    for fail_point in ["admit-without-validity-check", "admit-without-input-checks"] {
        register_fail_point_if(fail_point, move || {
            iota_simulator::current_simnode_id() == faulty_sim_id
        });
    }
    let answer = submit_to(test_cluster, validator, transaction).await;
    clear_fail_point("admit-without-validity-check");
    clear_fail_point("admit-without-input-checks");
    assert!(
        matches!(answer, TxStatusUpdate::Submitted),
        "the faulty validator must submit the transaction, got {answer:?}"
    );
}

/// What each validator records as the final drop of `digest`, waiting up to
/// a minute for it; `None` where it records none.
#[cfg(msim)]
async fn drops_on_every_validator(
    test_cluster: &test_cluster::TestCluster,
    digest: iota_sdk_types::TransactionDigest,
) -> Vec<Option<iota_types::error::IotaError>> {
    let mut drops = vec![];
    for handle in test_cluster.all_validator_handles() {
        drops.push(
            tokio::time::timeout(
                std::time::Duration::from_secs(60),
                handle.with_async(|node| async move {
                    node.state()
                        .epoch_store_for_testing()
                        .notify_read_dropped_digests(digest)
                        .await
                }),
            )
            .await
            .ok(),
        );
    }
    drops
}

/// Whether each node of the cluster, the fullnode included, executed
/// `digest`.
#[cfg(msim)]
fn executed_on_every_node(
    test_cluster: &test_cluster::TestCluster,
    digest: iota_sdk_types::TransactionDigest,
) -> Vec<bool> {
    test_cluster
        .all_node_handles()
        .iter()
        .map(|handle| {
            handle.with(|node| {
                node.state()
                    .get_transaction_cache_reader()
                    .try_get_executed_effects(&digest)
                    .unwrap()
                    .is_some()
            })
        })
        .collect()
}

/// Waits until the fullnode has executed `count` more checkpoints.
#[cfg(msim)]
async fn wait_for_checkpoints(test_cluster: &test_cluster::TestCluster, count: u64) {
    let highest = || {
        test_cluster.fullnode_handle.iota_node.with(|node| {
            node.state()
                .get_checkpoint_store()
                .get_highest_executed_checkpoint_seq_number()
                .unwrap()
                .unwrap_or_default()
        })
    };
    let target = highest() + count;
    tokio::time::timeout(std::time::Duration::from_secs(120), async {
        while highest() < target {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    })
    .await
    .expect("the fullnode must execute more checkpoints");
}

/// Another address pays for a claim: the claim creates the account at the
/// sender's address, the sponsor's coin pays, and the sponsor still sends with
/// its plain signature.
#[cfg(msim)]
#[sim_test]
async fn test_sponsored_claim_account_succeeds() {
    use iota_keys::keystore::AccountKeystore;
    use iota_sdk_types::{ObjectId, UserSignature, crypto::Intent};
    use iota_types::{
        effects::TransactionEffectsAPI,
        transaction::{TransactionAPI, TransactionEnvelope},
    };

    telemetry_subscribers::init_for_testing();
    let _guard = enable_claim_account();
    let test_cluster = TestClusterBuilder::new().build().await;
    let sender = test_cluster.get_address_0();
    let sponsor = test_cluster.get_address_1();
    let rgp = test_cluster.get_reference_gas_price().await;
    let sponsor_gas = first_gas_coin(&test_cluster.wallet, sponsor).await;
    let mut tx = claim_transaction(
        &wallet_keypair(&test_cluster, sender),
        sender,
        sponsor_gas,
        rgp,
    );
    tx.gas_data_mut().owner = sponsor;
    let keystore = test_cluster.wallet.config().keystore();
    let signatures = [sender, sponsor]
        .iter()
        .map(|signer| {
            UserSignature::Simple(
                keystore
                    .sign_secure(signer, &tx, Intent::iota_transaction())
                    .unwrap(),
            )
        })
        .collect();

    let effects = test_cluster
        .execute_transaction(TransactionEnvelope::from_user_sig_data(tx, signatures))
        .await;

    assert!(effects.status().is_success(), "{:?}", effects.status());
    assert_eq!(
        effects.gas_object().reference().object_id,
        sponsor_gas.object_id
    );
    let account = effects
        .created()
        .into_iter()
        .find(|created| matches!(created.owner(), Owner::Shared(_)))
        .expect("the claim creates the account");
    assert_eq!(account.reference().object_id, ObjectId::from(sender));
    let transfer = test_cluster
        .test_transaction_builder_with_sender(sponsor)
        .await
        .transfer_iota(Some(1), sender)
        .build();
    let effects = test_cluster.sign_and_execute_transaction(&transfer).await;
    assert!(effects.status().is_success(), "{:?}", effects.status());
}

/// Two claims of one address submitted to two validators at once, so that no
/// validator's soft lock sees both: both reach consensus, and post-consensus
/// validation keeps one. Every node executes the same one; every validator
/// drops the other with `AccountAlreadyExplicit` and tells its submitter, and
/// its gas coin keeps its version.
#[cfg(msim)]
#[sim_test]
async fn test_claims_sent_to_two_validators_keep_one() {
    use iota_sdk_types::ObjectId;
    use iota_types::{
        error::{IotaError, UserInputError},
        messages_grpc::TxStatusUpdate,
    };

    telemetry_subscribers::init_for_testing();
    let _guard = enable_claim_account();
    let test_cluster = TestClusterBuilder::new().build().await;
    let owner = test_cluster.get_address_0();
    let keypair = wallet_keypair(&test_cluster, owner);
    let rgp = test_cluster.get_reference_gas_price().await;
    let coins = test_cluster
        .wallet
        .get_gas_objects_owned_by_address(owner, None)
        .await
        .unwrap();
    let claims = [coins[0], coins[1]].map(|gas| {
        test_cluster
            .wallet
            .sign_transaction(&claim_transaction(&keypair, owner, gas, rgp))
    });
    let validators = test_cluster.get_validator_pubkeys();

    let answers = tokio::join!(
        submit_to(&test_cluster, &validators[0], claims[0].clone()),
        submit_to(&test_cluster, &validators[1], claims[1].clone()),
    );

    assert!(
        matches!(
            answers,
            (TxStatusUpdate::Submitted, TxStatusUpdate::Submitted)
        ),
        "{answers:?}"
    );
    tokio::time::timeout(std::time::Duration::from_secs(60), async {
        while test_cluster
            .get_object_from_fullnode_store(&ObjectId::from(owner))
            .await
            .is_none()
        {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    })
    .await
    .expect("one claim must create the account");
    let executed: Vec<_> = claims
        .iter()
        .map(|claim| executed_on_every_node(&test_cluster, *claim.digest()))
        .collect();
    let winner = executed
        .iter()
        .position(|nodes| nodes.iter().any(|executed| *executed))
        .expect("a node executed one of the claims");
    let loser = 1 - winner;
    wait_for_checkpoints(&test_cluster, 5).await;
    assert!(
        executed_on_every_node(&test_cluster, *claims[winner].digest())
            .iter()
            .all(|executed| *executed)
    );
    assert!(
        executed_on_every_node(&test_cluster, *claims[loser].digest())
            .iter()
            .all(|executed| !*executed)
    );
    let already_explicit: IotaError =
        UserInputError::AccountAlreadyExplicit { address: owner }.into();
    assert!(
        drops_on_every_validator(&test_cluster, *claims[loser].digest())
            .await
            .iter()
            .all(|drop| drop.as_ref() == Some(&already_explicit))
    );
    let loser_gas = [coins[0], coins[1]][loser];
    assert_eq!(
        test_cluster
            .get_latest_object_ref(&loser_gas.object_id)
            .await,
        loser_gas
    );
}

/// A claim of each wallet address submitted to one validator while a
/// plain-signed transfer of the address is submitted to another: the claim
/// always executes, and the transfer either executes on every node or on none,
/// as post-consensus validation kept it before the claim or dropped it after.
#[cfg(msim)]
#[sim_test]
async fn test_claim_racing_a_plain_signed_transfer_has_one_outcome_on_every_node() {
    use futures::future::join_all;
    use iota_types::effects::TransactionEffectsAPI;

    telemetry_subscribers::init_for_testing();
    let _guard = enable_claim_account();
    let test_cluster = TestClusterBuilder::new().build().await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let validators = test_cluster.get_validator_pubkeys();
    let mut pairs = vec![];
    for owner in test_cluster.get_addresses() {
        let keypair = wallet_keypair(&test_cluster, owner);
        let coins = test_cluster
            .wallet
            .get_gas_objects_owned_by_address(owner, None)
            .await
            .unwrap();
        let claim = test_cluster
            .wallet
            .sign_transaction(&claim_transaction(&keypair, owner, coins[0], rgp));
        let transfer = test_cluster.wallet.sign_transaction(
            &iota_test_transaction_builder::TestTransactionBuilder::new(owner, coins[1], rgp)
                .transfer_iota(Some(1), iota_sdk_types::Address::ZERO)
                .build(),
        );
        pairs.push((claim, transfer));
    }

    join_all(pairs.iter().flat_map(|(claim, transfer)| {
        [
            submit_to(&test_cluster, &validators[0], claim.clone()),
            submit_to(&test_cluster, &validators[1], transfer.clone()),
        ]
    }))
    .await;

    let claims: Vec<_> = pairs.iter().map(|(claim, _)| *claim.digest()).collect();
    let effects = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        test_cluster
            .fullnode_handle
            .iota_node
            .with_async(|node| async move {
                node.state()
                    .get_transaction_cache_reader()
                    .try_notify_read_executed_effects("", &claims)
                    .await
            }),
    )
    .await
    .expect("every claim must execute")
    .unwrap();
    assert!(effects.iter().all(|effects| effects.status().is_success()));
    wait_for_checkpoints(&test_cluster, 10).await;
    for (_, transfer) in &pairs {
        let executed = executed_on_every_node(&test_cluster, *transfer.digest());
        assert!(
            executed.iter().all(|on_node| *on_node == executed[0]),
            "transfer {}: {executed:?}",
            transfer.digest()
        );
    }
}

/// After a claim executed, a validator that skips its admission checks puts a
/// second claim of the address in a block. Every validator drops it at the
/// commit with `AccountAlreadyExplicit`, nobody executes it, and the account
/// and the claim's gas coin are left as they were.
#[cfg(msim)]
#[sim_test]
async fn test_second_claim_admitted_by_a_faulty_validator_is_dropped_at_the_commit() {
    use iota_sdk_types::ObjectId;
    use iota_types::error::{IotaError, UserInputError};

    telemetry_subscribers::init_for_testing();
    let _guard = enable_claim_account();
    let test_cluster = TestClusterBuilder::new().build().await;
    let owner = test_cluster.get_address_0();
    let keypair = wallet_keypair(&test_cluster, owner);
    let rgp = test_cluster.get_reference_gas_price().await;
    let first = claim_transaction(
        &keypair,
        owner,
        first_gas_coin(&test_cluster.wallet, owner).await,
        rgp,
    );
    test_cluster
        .wallet
        .execute_transaction_may_fail(test_cluster.wallet.sign_transaction(&first))
        .await
        .expect("the first claim must execute");
    let account = test_cluster
        .get_latest_object_ref(&ObjectId::from(owner))
        .await;
    let gas = first_gas_coin(&test_cluster.wallet, owner).await;
    let second = test_cluster
        .wallet
        .sign_transaction(&claim_transaction(&keypair, owner, gas, rgp));
    let digest = *second.digest();

    submit_through_a_faulty_validator(
        &test_cluster,
        &test_cluster.get_validator_pubkeys()[0],
        second,
    )
    .await;

    let already_explicit: IotaError =
        UserInputError::AccountAlreadyExplicit { address: owner }.into();
    assert_eq!(
        drops_on_every_validator(&test_cluster, digest).await,
        vec![Some(already_explicit); test_cluster.all_validator_handles().len()]
    );
    wait_for_checkpoints(&test_cluster, 5).await;
    assert!(
        executed_on_every_node(&test_cluster, digest)
            .iter()
            .all(|executed| !*executed)
    );
    assert_eq!(
        test_cluster
            .get_latest_object_ref(&ObjectId::from(owner))
            .await,
        account
    );
    assert_eq!(
        test_cluster.get_latest_object_ref(&gas.object_id).await,
        gas
    );
}

/// After a claim executed, a validator that skips its admission checks puts a
/// transfer of the address signed with its plain signature in a block. Every
/// validator drops it at the commit, as a copy: nobody executes it and its gas
/// coin keeps its version. The same transfer signed through the account's
/// built-in authenticator, with the same digest, then executes.
#[cfg(msim)]
#[sim_test]
async fn test_plain_signature_admitted_by_a_faulty_validator_is_dropped_at_the_commit() {
    use iota_json_rpc_types::IotaTransactionBlockEffectsAPI;
    use iota_sdk_types::ObjectId;

    telemetry_subscribers::init_for_testing();
    let _guard = enable_claim_account();
    let test_cluster = TestClusterBuilder::new().build().await;
    let owner = test_cluster.get_address_0();
    let keypair = wallet_keypair(&test_cluster, owner);
    let rgp = test_cluster.get_reference_gas_price().await;
    let claim = claim_transaction(
        &keypair,
        owner,
        first_gas_coin(&test_cluster.wallet, owner).await,
        rgp,
    );
    let response = test_cluster
        .wallet
        .execute_transaction_may_fail(test_cluster.wallet.sign_transaction(&claim))
        .await
        .expect("the claim must execute");
    let effects = response.effects.expect("response must include effects");
    let Owner::Shared(initial_shared_version) = effects
        .created()
        .iter()
        .find(|created| created.reference.object_id == ObjectId::from(owner))
        .expect("the claim creates the account")
        .owner
        .clone()
    else {
        panic!("the account must be shared");
    };
    let gas = first_gas_coin(&test_cluster.wallet, owner).await;
    let transfer = iota_test_transaction_builder::TestTransactionBuilder::new(owner, gas, rgp)
        .transfer_iota(Some(1), iota_sdk_types::Address::ZERO)
        .with_gas_budget(
            rgp * iota_types::transaction::TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        )
        .build();
    let plain = test_cluster.wallet.sign_transaction(&transfer);
    let digest = *plain.digest();

    submit_through_a_faulty_validator(
        &test_cluster,
        &test_cluster.get_validator_pubkeys()[0],
        plain,
    )
    .await;

    wait_for_checkpoints(&test_cluster, 10).await;
    assert!(
        executed_on_every_node(&test_cluster, digest)
            .iter()
            .all(|executed| !*executed)
    );
    assert_eq!(
        test_cluster.get_latest_object_ref(&gas.object_id).await,
        gas
    );
    let response =
        execute_as_claimed_account(&test_cluster, &keypair, transfer, initial_shared_version).await;
    assert_eq!(response.digest, digest);
    let effects = response.effects.expect("response must include effects");
    assert!(effects.status().is_ok(), "{:?}", effects.status());
}

/// A validator that skips its admission checks puts in a block a claim whose
/// key does not derive its sender, signed by the sender. Block verification
/// does not run the validity check, so the block is accepted, and every
/// validator drops the claim at the commit with the validity error. The
/// rightful claim of the address then executes.
#[cfg(msim)]
#[sim_test]
async fn test_claim_whose_key_does_not_derive_the_sender_from_a_faulty_validator_is_dropped_at_the_commit()
 {
    use iota_json_rpc_types::IotaTransactionBlockEffectsAPI;
    use iota_types::error::{IotaError, UserInputError};

    telemetry_subscribers::init_for_testing();
    let _guard = enable_claim_account();
    let test_cluster = TestClusterBuilder::new().build().await;
    let owner = test_cluster.get_address_0();
    let other = test_cluster.get_address_1();
    let rgp = test_cluster.get_reference_gas_price().await;
    let coins = test_cluster
        .wallet
        .get_gas_objects_owned_by_address(owner, None)
        .await
        .unwrap();
    let mismatched = test_cluster.wallet.sign_transaction(&claim_transaction(
        &wallet_keypair(&test_cluster, other),
        owner,
        coins[0],
        rgp,
    ));
    let digest = *mismatched.digest();

    submit_through_a_faulty_validator(
        &test_cluster,
        &test_cluster.get_validator_pubkeys()[0],
        mismatched,
    )
    .await;

    let drops = drops_on_every_validator(&test_cluster, digest).await;
    assert!(
        drops.iter().all(|drop| matches!(
            drop,
            Some(IotaError::UserInput {
                error: UserInputError::IncorrectUserSignature { .. }
            })
        )),
        "{drops:?}"
    );
    let response = test_cluster
        .wallet
        .execute_transaction_may_fail(test_cluster.wallet.sign_transaction(&claim_transaction(
            &wallet_keypair(&test_cluster, owner),
            owner,
            coins[1],
            rgp,
        )))
        .await
        .expect("the rightful claim must execute");
    let effects = response.effects.expect("response must include effects");
    assert!(effects.status().is_ok(), "{:?}", effects.status());
}
