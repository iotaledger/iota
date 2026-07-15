// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{
    Address, Identifier, ObjectId, ObjectReference, SharedObjectReference, StructTag, TypeTag,
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    base_types::dbg_addr,
    crypto::{AccountKeyPair, get_account_key_pair},
    deny_list_v1::{
        DenyCapV1, RegulatedCoinMetadata, check_address_denied_by_config, check_global_pause,
        get_per_type_coin_deny_list_v1,
    },
    effects::{TransactionEffects, TransactionEffectsAPI},
    error::{IotaError, UserInputError},
    messages_consensus::{ConsensusTransaction, ConsensusTransactionKind},
    object::Object,
    transaction::{CallArg, TEST_ONLY_GAS_UNIT_FOR_PUBLISH, VerifiedTransaction},
};
use prometheus_filtered::Registry;
use typed_store::DBMetrics;

use crate::{
    authority::{
        AuthorityState, authority_tests::send_and_confirm_transaction_,
        move_integration_tests::build_and_try_publish_test_package,
        test_authority_builder::TestAuthorityBuilder,
    },
    consensus_handler::VerifiedSequencedConsensusTransaction,
    post_consensus_validation,
    test_utils::make_transfer_object_transaction,
};

// Test that a v1 regulated coin can be created and all the necessary objects
// are created with the right types. Also test that we could create the deny
// list config for the coin and all types can be loaded in Rust.
#[tokio::test]
async fn test_regulated_coin_v1_types() {
    let env = new_authority_and_publish("coin_deny_list_v1").await;

    // Step 1: Publish the regulated coin and check basic types.
    let mut deny_cap_object = None;
    let mut metadata_object = None;
    let mut regulated_metadata_object = None;
    let mut package_id = None;
    for (oref, _owner) in env.publish_effects.created() {
        let object = env.authority.get_object(&oref.object_id).unwrap();
        if object.is_package() {
            package_id = Some(object.id());
            continue;
        }
        let t = object.type_().unwrap();
        if t.is_deny_cap_v1() {
            assert!(deny_cap_object.is_none());
            deny_cap_object = Some(object);
        } else if t.is_regulated_coin_metadata() {
            assert!(regulated_metadata_object.is_none());
            regulated_metadata_object = Some(object);
        } else if t.is_coin_metadata() {
            assert!(metadata_object.is_none());
            metadata_object = Some(object);
        }
    }
    let package_id = package_id.unwrap();
    // Check that publishing the package created
    // the metadata, deny cap, and regulated metadata.
    // Check that all their fields are consistent.
    let metadata_object = metadata_object.unwrap();
    let deny_cap_object = deny_cap_object.unwrap();
    let deny_cap: DenyCapV1 = deny_cap_object.to_rust().unwrap();
    assert_eq!(deny_cap.id.id.bytes, deny_cap_object.id());
    assert!(deny_cap.allow_global_pause);

    let regulated_metadata_object = regulated_metadata_object.unwrap();
    let regulated_metadata: RegulatedCoinMetadata = regulated_metadata_object.to_rust().unwrap();
    assert_eq!(
        regulated_metadata.id.id.bytes,
        regulated_metadata_object.id()
    );
    assert_eq!(
        regulated_metadata.deny_cap_object.bytes,
        deny_cap_object.id()
    );
    assert_eq!(
        regulated_metadata.coin_metadata_object.bytes,
        metadata_object.id()
    );

    // Step 2: Deny an address and check the denylist types.
    let deny_list_object_init_version = env
        .get_latest_object_ref(&ObjectId::DENY_LIST)
        .await
        .version;
    let regulated_coin_type = TypeTag::Struct(Box::new(StructTag::new(
        package_id,
        Identifier::from_static("regulated_coin"),
        Identifier::from_static("REGULATED_COIN"),
        vec![],
    )));
    let deny_address = dbg_addr(2);
    let tx = TestTransactionBuilder::new(
        env.sender,
        env.get_latest_object_ref(&env.gas_object_id).await,
        env.authority.reference_gas_price_for_testing().unwrap(),
    )
    .move_call(
        ObjectId::FRAMEWORK,
        "coin",
        "deny_list_v1_add",
        vec![
            CallArg::Shared(SharedObjectReference::new(
                ObjectId::DENY_LIST,
                deny_list_object_init_version,
                true,
            )),
            CallArg::ImmutableOrOwned(deny_cap_object.object_ref()),
            CallArg::pure(&deny_address),
        ],
    )
    .with_type_args(vec![regulated_coin_type.clone()])
    .build_and_sign(&env.keypair);
    let (_, effects) = send_and_confirm_transaction_(&env.authority, None, tx, true)
        .await
        .unwrap();
    if effects.status().is_failure() {
        panic!("Failed to add address to deny list: {:?}", effects.status());
    }
    let coin_deny_config = get_per_type_coin_deny_list_v1(
        &regulated_coin_type.to_canonical_string(false),
        &env.authority.get_object_store(),
    )
    .unwrap();
    // Updates from the current epoch will not be read.
    assert!(!check_address_denied_by_config(
        &coin_deny_config,
        deny_address,
        &env.authority.get_object_store(),
        Some(0),
    ));
    // If no epoch is specified, we always read the latest value, and it should be
    // denied.
    assert!(check_address_denied_by_config(
        &coin_deny_config,
        deny_address,
        &env.authority.get_object_store(),
        None,
    ));
    // If no epoch is specified, we always read the latest value, and it should be
    // denied.
    assert!(check_address_denied_by_config(
        &coin_deny_config,
        deny_address,
        &env.authority.get_object_store(),
        None,
    ));

    // If we change the current epoch to be 1, the change from epoch 0
    // would be considered as from previous epoch, and hence will be
    // used.
    assert!(check_address_denied_by_config(
        &coin_deny_config,
        deny_address,
        &env.authority.get_object_store(),
        Some(1),
    ));
    // Check a different address, and it should not be denied.
    assert!(!check_address_denied_by_config(
        &coin_deny_config,
        dbg_addr(3),
        &env.authority.get_object_store(),
        Some(1),
    ));

    // Step 3: Enable global pause and check the global pause types.
    let tx = TestTransactionBuilder::new(
        env.sender,
        env.get_latest_object_ref(&env.gas_object_id).await,
        env.authority.reference_gas_price_for_testing().unwrap(),
    )
    .move_call(
        ObjectId::FRAMEWORK,
        "coin",
        "deny_list_v1_enable_global_pause",
        vec![
            CallArg::Shared(SharedObjectReference::new(
                ObjectId::DENY_LIST,
                deny_list_object_init_version,
                true,
            )),
            CallArg::ImmutableOrOwned(env.get_latest_object_ref(&deny_cap_object.id()).await),
        ],
    )
    .with_type_args(vec![regulated_coin_type.clone()])
    .build_and_sign(&env.keypair);
    let (_, effects) = send_and_confirm_transaction_(&env.authority, None, tx, true)
        .await
        .unwrap();
    if effects.status().is_failure() {
        panic!("Failed to enable global pause: {:?}", effects.status());
    }
    println!("Effects: {effects:?}");
    assert!(check_global_pause(
        &coin_deny_config,
        &env.authority.get_object_store(),
        None,
    ));
    assert!(!check_global_pause(
        &coin_deny_config,
        &env.authority.get_object_store(),
        Some(0),
    ));
    assert!(check_global_pause(
        &coin_deny_config,
        &env.authority.get_object_store(),
        Some(1),
    ));
}

// The coin deny-list read mode of `handle_transaction_validation_checks`
// decides whether its verdict depends on the validator's execution progress.
//
// With the latest-value read (`epoch_gated_coin_deny_list = false`), the
// verdict for one and the same transaction flips within an epoch once the
// `deny_list_v1_add` transaction executes locally. That is intended for
// admission at signing (denials apply immediately), but post-consensus it
// would make two validators - one that has already executed the deny-add and
// one that has not - reach opposite keep/drop decisions for the same
// sequenced transaction, diverging the checkpoint. The epoch-gated read
// (`epoch_gated_coin_deny_list = true`) returns the value settled before the
// current epoch and is identical at both frontiers, which is why the
// post-consensus caller uses it.
//
// The two calls below (before/after executing the deny-add on a single
// authority) model those two execution frontiers.
#[tokio::test]
async fn test_coin_deny_list_read_modes_across_execution_progress() {
    let env = new_authority_and_publish("coin_deny_list_v1_mintable").await;
    let epoch_store = env.authority.epoch_store_for_testing();
    assert_eq!(epoch_store.epoch(), 0);
    let rgp = env.authority.reference_gas_price_for_testing().unwrap();

    let (package_id, deny_cap_ref, coin_id) = find_published_objects(&env);
    let regulated_coin_type = TypeTag::Struct(Box::new(StructTag::new(
        package_id,
        Identifier::from_static("regulated_coin"),
        Identifier::from_static("REGULATED_COIN"),
        vec![],
    )));

    // Give the transfer its own gas object so the deny-add below (which
    // consumes the publisher's gas) does not invalidate the transfer's input
    // references.
    let transfer_gas_object = Object::with_owner_for_testing(env.sender);
    env.authority
        .insert_genesis_object(transfer_gas_object.clone());

    let transfer_tx = VerifiedTransaction::new_unchecked(
        TestTransactionBuilder::new(env.sender, transfer_gas_object.object_ref(), rgp)
            .transfer(env.get_latest_object_ref(&coin_id).await, dbg_addr(2))
            .build_and_sign(&env.keypair),
    );

    // Frontier 1 - the deny-add has not executed: no deny list config exists
    // for the coin type, both read modes pass the transaction.
    for epoch_gated in [false, true] {
        env.authority
            .handle_transaction_validation_checks(&transfer_tx, &epoch_store, epoch_gated)
            .await
            .expect("no deny list entry executed yet");
    }
    assert!(
        get_per_type_coin_deny_list_v1(
            &regulated_coin_type.to_canonical_string(false),
            &env.authority.get_object_store(),
        )
        .is_none()
    );

    // Execute `deny_list_v1_add(sender)` in the same epoch.
    let deny_list_object_init_version = env
        .get_latest_object_ref(&ObjectId::DENY_LIST)
        .await
        .version;
    let deny_tx = TestTransactionBuilder::new(
        env.sender,
        env.get_latest_object_ref(&env.gas_object_id).await,
        rgp,
    )
    .move_call(
        ObjectId::FRAMEWORK,
        "coin",
        "deny_list_v1_add",
        vec![
            CallArg::Shared(SharedObjectReference::new(
                ObjectId::DENY_LIST,
                deny_list_object_init_version,
                true,
            )),
            CallArg::ImmutableOrOwned(deny_cap_ref),
            CallArg::pure(&env.sender),
        ],
    )
    .with_type_args(vec![regulated_coin_type.clone()])
    .build_and_sign(&env.keypair);
    let (_, effects) = send_and_confirm_transaction_(&env.authority, None, deny_tx, true)
        .await
        .unwrap();
    assert!(!effects.status().is_failure(), "{:?}", effects.status());

    // Frontier 2, latest-value read - the very same transaction is now
    // rejected within the same epoch: the verdict followed the local
    // execution frontier.
    let err = env
        .authority
        .handle_transaction_validation_checks(&transfer_tx, &epoch_store, false)
        .await
        .unwrap_err();
    assert!(
        matches!(
            &err,
            IotaError::UserInput {
                error: UserInputError::AddressDeniedForCoin { address, .. }
            } if *address == env.sender
        ),
        "unexpected error: {err:?}"
    );

    // Frontier 2, epoch-gated read - same verdict as frontier 1: the entry
    // written this epoch is not active yet, execution progress is irrelevant.
    env.authority
        .handle_transaction_validation_checks(&transfer_tx, &epoch_store, true)
        .await
        .expect("entry written this epoch must not be active for the epoch-gated read");
}

// Runs the real post-consensus validation pipeline over a `UserTransactionV1`
// spending a regulated coin whose type was globally paused earlier in the
// same epoch. The pause has already executed locally, so the latest-value
// read would report "paused" and drop the transaction - on this validator,
// but not on one whose execution lags behind the pause. The epoch-gated read
// used post-consensus keeps it on every validator: the pause activates next
// epoch. (Recipient-side enforcement at execution,
// `check_coin_deny_list_v1_during_execution`, has always been epoch-gated
// the same way.)
#[tokio::test]
async fn test_post_consensus_keeps_tx_spending_coin_paused_this_epoch() {
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let env = new_authority_and_publish("coin_deny_list_v1_mintable").await;
    let rgp = env.authority.reference_gas_price_for_testing().unwrap();

    let (package_id, deny_cap_ref, coin_id) = find_published_objects(&env);
    let regulated_coin_type = TypeTag::Struct(Box::new(StructTag::new(
        package_id,
        Identifier::from_static("regulated_coin"),
        Identifier::from_static("REGULATED_COIN"),
        vec![],
    )));

    // Globally pause the regulated coin; executes in the current epoch.
    let deny_list_object_init_version = env
        .get_latest_object_ref(&ObjectId::DENY_LIST)
        .await
        .version;
    let pause_tx = TestTransactionBuilder::new(
        env.sender,
        env.get_latest_object_ref(&env.gas_object_id).await,
        rgp,
    )
    .move_call(
        ObjectId::FRAMEWORK,
        "coin",
        "deny_list_v1_enable_global_pause",
        vec![
            CallArg::Shared(SharedObjectReference::new(
                ObjectId::DENY_LIST,
                deny_list_object_init_version,
                true,
            )),
            CallArg::ImmutableOrOwned(deny_cap_ref),
        ],
    )
    .with_type_args(vec![regulated_coin_type.clone()])
    .build_and_sign(&env.keypair);
    let (_, effects) = send_and_confirm_transaction_(&env.authority, None, pause_tx, true)
        .await
        .unwrap();
    assert!(!effects.status().is_failure(), "{:?}", effects.status());

    // A transaction spending the paused coin, sequenced as `UserTransactionV1`.
    let transfer_tx = make_transfer_object_transaction(
        env.get_latest_object_ref(&coin_id).await,
        env.get_latest_object_ref(&env.gas_object_id).await,
        env.sender,
        &env.keypair,
        dbg_addr(9),
        rgp,
    );
    let digest = *transfer_tx.digest();
    let consensus_tx = ConsensusTransaction {
        kind: ConsensusTransactionKind::UserTransactionV1(Box::new(transfer_tx)),
        tracking_id: Default::default(),
    };
    let mut transactions = vec![VerifiedSequencedConsensusTransaction::new_test(
        consensus_tx,
    )];

    let epoch_store = env.authority.epoch_store_for_testing();
    let (dropped, _locks, user_tx_digests) =
        post_consensus_validation::validate_and_resolve_conflicts(
            &env.authority,
            &epoch_store,
            &mut transactions,
        )
        .await
        .unwrap();

    assert!(dropped.is_empty(), "unexpected drops: {dropped:?}");
    assert_eq!(transactions.len(), 1);
    assert_eq!(user_tx_digests, vec![digest]);
}

/// Returns the package id, deny cap reference, and minted regulated coin id
/// created by publishing `coin_deny_list_v1_mintable`.
fn find_published_objects(env: &TestEnv) -> (ObjectId, ObjectReference, ObjectId) {
    let mut package_id = None;
    let mut deny_cap_ref = None;
    let mut coin_id = None;
    for (oref, _owner) in env.publish_effects.created() {
        let object = env.authority.get_object(&oref.object_id).unwrap();
        if object.is_package() {
            package_id = Some(object.id());
            continue;
        }
        if object.type_().unwrap().is_deny_cap_v1() {
            deny_cap_ref = Some(object.object_ref());
        } else if !object.is_gas_coin() && object.coin_type_opt().is_some() {
            coin_id = Some(object.id());
        }
    }
    (package_id.unwrap(), deny_cap_ref.unwrap(), coin_id.unwrap())
}

struct TestEnv {
    authority: Arc<AuthorityState>,
    sender: Address,
    keypair: AccountKeyPair,
    gas_object_id: ObjectId,
    publish_effects: TransactionEffects,
}

impl TestEnv {
    async fn get_latest_object_ref(&self, id: &ObjectId) -> ObjectReference {
        self.authority.get_object(id).unwrap().object_ref()
    }
}

async fn new_authority_and_publish(path: &str) -> TestEnv {
    // typed-store's `DBMetrics` registers rocksdb metrics into the global
    // `default_registry()` on first initialization, and concurrent first-time
    // initializers race with `AlreadyReg`. Pre-initialize with a throwaway
    // registry up front: distinct registries can't collide, and once the init
    // has run the authority build reuses the cached metrics. Without this,
    // running several authority-building tests from this file concurrently
    // flakes.
    let _ = DBMetrics::init(&Registry::new());

    let (sender, keypair) = get_account_key_pair();
    let gas_object = Object::with_owner_for_testing(sender);
    let gas_object_id = gas_object.id();
    let authority = TestAuthorityBuilder::new()
        .with_starting_objects(&[gas_object])
        .build()
        .await;
    let rgp = authority.reference_gas_price_for_testing().unwrap();
    let (_, effects) = build_and_try_publish_test_package(
        &authority,
        &sender,
        &keypair,
        &gas_object_id,
        path,
        TEST_ONLY_GAS_UNIT_FOR_PUBLISH * rgp,
        rgp,
        false,
    )
    .await;
    TestEnv {
        authority,
        sender,
        keypair,
        gas_object_id,
        publish_effects: effects.into_data(),
    }
}
