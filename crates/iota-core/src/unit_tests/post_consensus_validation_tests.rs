// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for post-consensus transaction validation and owned-object
//! conflict resolution.

use std::sync::Arc;

use iota_macros::sim_test;
use iota_protocol_config::{OverrideGuard, ProtocolConfig};
use iota_sdk_types::{
    Address, Command, Identifier, ObjectId, ObjectReference, OwnedObjectReference, Owner,
    Transaction, TransactionDigest, TransactionEffects, Version,
};
use iota_types::{
    base_types::CommitRound,
    crypto::{AccountPrivateKey, get_key_pair},
    effects::TransactionEffectsAPI,
    error::{IotaError, UserInputError},
    executable_transaction::VerifiedExecutableTransaction,
    messages_consensus::{ConsensusTransaction, ConsensusTransactionKind},
    object::Object,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    storage::ObjectKey,
    transaction::{
        CallArg, TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS, TransactionAPI, TransactionKey,
        VerifiedTransaction,
    },
    utils::to_sender_signed_transaction,
};

use crate::{
    authority::{
        ExecutionEnv,
        authority_per_epoch_store::{
            LockDetails,
            consensus_quarantine::ConsensusCommitOutput,
            handler_object_state::{HandlerLatestObjectKind, SyncAheadRecord},
        },
        authority_tests::{TestCallArg, call_move_, init_state_with_objects_and_object_basics},
        move_integration_tests::build_and_publish_test_package_with_upgrade_cap,
        test_authority_builder::TestAuthorityBuilder,
    },
    checkpoints::CheckpointServiceNoop,
    consensus_handler::{SequencedConsensusTransaction, VerifiedSequencedConsensusTransaction},
    post_consensus_validation,
    test_utils::make_transfer_object_transaction,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Wraps a `TransactionEnvelope` in a `UserTransactionV1` consensus
/// transaction.
fn make_user_tx_v1(
    tx: iota_types::transaction::TransactionEnvelope,
) -> VerifiedSequencedConsensusTransaction {
    let consensus_tx = ConsensusTransaction {
        kind: ConsensusTransactionKind::UserTransactionV1(Box::new(tx)),
        tracking_id: Default::default(),
    };
    VerifiedSequencedConsensusTransaction::new_test(consensus_tx)
}

/// Wraps a `VerifiedTransaction` in a `UserTransactionV1` consensus
/// transaction.
fn make_user_tx_v1_verified(tx: VerifiedTransaction) -> VerifiedSequencedConsensusTransaction {
    let consensus_tx = ConsensusTransaction {
        kind: ConsensusTransactionKind::UserTransactionV1(Box::new(tx.into())),
        tracking_id: Default::default(),
    };
    VerifiedSequencedConsensusTransaction::new_test(consensus_tx)
}

/// Wraps an `EndOfPublish` message as a consensus transaction.
fn make_end_of_publish() -> VerifiedSequencedConsensusTransaction {
    use iota_types::base_types::AuthorityName;
    let consensus_tx = ConsensusTransaction {
        kind: ConsensusTransactionKind::EndOfPublish(AuthorityName::ZERO),
        tracking_id: Default::default(),
    };
    VerifiedSequencedConsensusTransaction::new_test(consensus_tx)
}

// ---------------------------------------------------------------------------
// Validation tests
// ---------------------------------------------------------------------------

/// Test that a valid UserTransactionV1 passes through validation unchanged.
#[sim_test]
async fn test_valid_user_transaction_passes() {
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let recipient = Address::random();

    let object_id = ObjectId::random();
    let gas_id = ObjectId::random();

    let (authority, _) = init_state_with_objects_and_object_basics(vec![
        Object::with_id_owner_for_testing(object_id, sender),
        Object::with_id_owner_for_testing(gas_id, sender),
    ])
    .await;

    let epoch_store = authority.epoch_store_for_testing();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let object_ref = authority.get_object(&object_id).unwrap().object_ref();
    let gas_ref = authority.get_object(&gas_id).unwrap().object_ref();

    let tx =
        make_transfer_object_transaction(object_ref, gas_ref, sender, &sender_key, recipient, rgp);
    let mut transactions = vec![make_user_tx_v1(tx)];

    let (dropped, locks, user_tx_digests) =
        post_consensus_validation::validate_and_resolve_conflicts(
            &authority,
            &epoch_store,
            &mut transactions,
        )
        .await
        .unwrap();

    assert_eq!(
        transactions.len(),
        1,
        "Valid transaction should pass through"
    );
    assert!(dropped.is_empty(), "No transactions should be dropped");
    assert_eq!(
        locks.len(),
        2,
        "Locks for object and gas should be acquired"
    );
    assert_eq!(
        user_tx_digests.len(),
        1,
        "One user transaction digest should be collected"
    );
}

/// Test that non-UserTransactionV1 transactions (e.g. EndOfPublish) pass
/// through validation unchanged.
#[sim_test]
async fn test_non_user_transaction_passes_through() {
    let (authority, _) = init_state_with_objects_and_object_basics(vec![]).await;
    let epoch_store = authority.epoch_store_for_testing();

    let mut transactions = vec![make_end_of_publish()];

    let (dropped, locks, user_tx_digests) =
        post_consensus_validation::validate_and_resolve_conflicts(
            &authority,
            &epoch_store,
            &mut transactions,
        )
        .await
        .unwrap();

    assert_eq!(
        transactions.len(),
        1,
        "EndOfPublish should pass through unchanged"
    );
    assert!(dropped.is_empty());
    assert!(locks.is_empty());
    assert!(
        user_tx_digests.is_empty(),
        "No user transaction digests for non-user transactions"
    );
}

/// Test that duplicate transactions (same ConsensusTransactionKey) are
/// deduplicated: only the first occurrence is kept.
#[sim_test]
async fn test_duplicate_transaction_deduplicated() {
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let recipient = Address::random();

    let object_id = ObjectId::random();
    let gas_id = ObjectId::random();

    let (authority, _) = init_state_with_objects_and_object_basics(vec![
        Object::with_id_owner_for_testing(object_id, sender),
        Object::with_id_owner_for_testing(gas_id, sender),
    ])
    .await;

    let epoch_store = authority.epoch_store_for_testing();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let object_ref = authority.get_object(&object_id).unwrap().object_ref();
    let gas_ref = authority.get_object(&gas_id).unwrap().object_ref();

    let tx =
        make_transfer_object_transaction(object_ref, gas_ref, sender, &sender_key, recipient, rgp);

    // Same transaction wrapped twice — simulates it appearing in two validator DAG
    // blocks.
    let mut transactions = vec![make_user_tx_v1(tx.clone()), make_user_tx_v1(tx)];

    let (dropped, _locks, user_tx_digests) =
        post_consensus_validation::validate_and_resolve_conflicts(
            &authority,
            &epoch_store,
            &mut transactions,
        )
        .await
        .unwrap();

    assert_eq!(
        transactions.len(),
        1,
        "Duplicate should be removed; only first kept"
    );
    // Duplicates are silently dropped — not returned as errors.
    assert!(
        dropped.is_empty(),
        "Duplicate is a silent dedup, not an error"
    );
    assert_eq!(
        user_tx_digests.len(),
        1,
        "Dedup'd copy should not appear in user_tx_digests"
    );
}

/// Test that a mixed batch of valid, non-user, and duplicate transactions is
/// correctly filtered: only duplicates are removed, valid and non-user pass.
#[sim_test]
async fn test_mixed_batch_filtering() {
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let recipient = Address::random();

    let obj1_id = ObjectId::random();
    let gas1_id = ObjectId::random();
    let obj2_id = ObjectId::random();
    let gas2_id = ObjectId::random();

    let (authority, _) = init_state_with_objects_and_object_basics(vec![
        Object::with_id_owner_for_testing(obj1_id, sender),
        Object::with_id_owner_for_testing(gas1_id, sender),
        Object::with_id_owner_for_testing(obj2_id, sender),
        Object::with_id_owner_for_testing(gas2_id, sender),
    ])
    .await;

    let epoch_store = authority.epoch_store_for_testing();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let obj1_ref = authority.get_object(&obj1_id).unwrap().object_ref();
    let gas1_ref = authority.get_object(&gas1_id).unwrap().object_ref();
    let obj2_ref = authority.get_object(&obj2_id).unwrap().object_ref();
    let gas2_ref = authority.get_object(&gas2_id).unwrap().object_ref();

    let tx1 =
        make_transfer_object_transaction(obj1_ref, gas1_ref, sender, &sender_key, recipient, rgp);
    let tx2 =
        make_transfer_object_transaction(obj2_ref, gas2_ref, sender, &sender_key, recipient, rgp);

    // Order: tx1, tx1 duplicate, tx2, EndOfPublish
    let mut transactions = vec![
        make_user_tx_v1(tx1.clone()),
        make_user_tx_v1(tx1), // duplicate — should be removed
        make_user_tx_v1(tx2),
        make_end_of_publish(),
    ];

    let (dropped, _locks, user_tx_digests) =
        post_consensus_validation::validate_and_resolve_conflicts(
            &authority,
            &epoch_store,
            &mut transactions,
        )
        .await
        .unwrap();

    // tx1 first occurrence kept, tx1 duplicate removed, tx2 kept, eop kept.
    assert_eq!(
        transactions.len(),
        3,
        "tx1, tx2, and EndOfPublish should remain"
    );
    assert!(
        dropped.is_empty(),
        "Only duplicates removed; no semantic errors"
    );
    assert_eq!(
        user_tx_digests.len(),
        2,
        "tx1 + tx2 digests (duplicate and EndOfPublish excluded)"
    );
}

// ---------------------------------------------------------------------------
// Conflict resolution tests
// ---------------------------------------------------------------------------

/// Two transactions touching the same owned object: first wins, second is
/// dropped with a lock conflict error.
#[sim_test]
async fn test_simple_conflict() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let recipient1 = Address::random();
    let recipient2 = Address::random();

    let object_id = ObjectId::random();
    let gas1_id = ObjectId::random();
    let gas2_id = ObjectId::random();

    let (authority, _) = init_state_with_objects_and_object_basics(vec![
        Object::with_id_owner_for_testing(object_id, sender),
        Object::with_id_owner_for_testing(gas1_id, sender),
        Object::with_id_owner_for_testing(gas2_id, sender),
    ])
    .await;

    let epoch_store = authority.epoch_store_for_testing();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let object = authority.get_object(&object_id).unwrap();
    let gas1 = authority.get_object(&gas1_id).unwrap();
    let gas2 = authority.get_object(&gas2_id).unwrap();

    let tx1 = make_transfer_object_transaction(
        object.object_ref(),
        gas1.object_ref(),
        sender,
        &sender_key,
        recipient1,
        rgp,
    );
    let tx2 = make_transfer_object_transaction(
        object.object_ref(),
        gas2.object_ref(),
        sender,
        &sender_key,
        recipient2,
        rgp,
    );

    let verified_tx1 = epoch_store.verify_transaction(tx1).unwrap();
    let verified_tx2 = epoch_store.verify_transaction(tx2).unwrap();

    let mut transactions = vec![
        make_user_tx_v1_verified(verified_tx1.clone()),
        make_user_tx_v1_verified(verified_tx2.clone()),
    ];

    let (dropped, locks, user_tx_digests) =
        post_consensus_validation::validate_and_resolve_conflicts(
            &authority,
            &epoch_store,
            &mut transactions,
        )
        .await
        .unwrap();

    let (dropped_digests, _): (Vec<TransactionDigest>, Vec<IotaError>) =
        dropped.into_iter().unzip();

    assert_eq!(transactions.len(), 1, "Only one transaction should remain");
    assert_eq!(dropped_digests.len(), 1, "Exactly one should be dropped");
    assert_eq!(dropped_digests[0], *verified_tx2.digest());

    assert!(
        locks.contains_key(&object.object_ref()),
        "Lock should be acquired for the contested object"
    );
    assert_eq!(locks.get(&object.object_ref()), Some(verified_tx1.digest()));

    assert_eq!(
        user_tx_digests.len(),
        2,
        "Both kept and dropped user txs should be collected"
    );
    assert!(user_tx_digests.contains(verified_tx1.digest()));
    assert!(user_tx_digests.contains(verified_tx2.digest()));
}

/// Two transactions in the same commit reference the same owned object at
/// different versions, with the stale one ordered first (the scenario from
/// issue #10922). Because owned-object locks are keyed by the full
/// `ObjectReference`, the two never falsely conflict; the stale transaction is
/// dropped by the version check in `handle_transaction_validation_checks`
/// (Check #5) and the fresh transaction is kept and acquires the lock.
#[sim_test]
async fn test_stale_version_dropped_fresh_kept() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let recipient = Address::random();

    let object_id = ObjectId::random();
    let gas_stale_id = ObjectId::random();
    let gas_fresh_id = ObjectId::random();

    // The contested object is live at version 2, so a reference to version 1 is
    // stale and absent from the store.
    let object = Object::with_id_owner_version_for_testing(
        object_id,
        Version::from(2),
        Owner::Address(sender),
    );

    let (authority, _) = init_state_with_objects_and_object_basics(vec![
        object.clone(),
        Object::with_id_owner_for_testing(gas_stale_id, sender),
        Object::with_id_owner_for_testing(gas_fresh_id, sender),
    ])
    .await;

    let epoch_store = authority.epoch_store_for_testing();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let gas_stale = authority.get_object(&gas_stale_id).unwrap();
    let gas_fresh = authority.get_object(&gas_fresh_id).unwrap();

    let fresh_ref = object.object_ref();
    // Stale reference: same object id and digest, but the previous version.
    let stale_ref = ObjectReference::new(object_id, Version::from(1), fresh_ref.digest);

    let tx_stale = make_transfer_object_transaction(
        stale_ref,
        gas_stale.object_ref(),
        sender,
        &sender_key,
        recipient,
        rgp,
    );
    let tx_fresh = make_transfer_object_transaction(
        fresh_ref,
        gas_fresh.object_ref(),
        sender,
        &sender_key,
        recipient,
        rgp,
    );

    let verified_stale = epoch_store.verify_transaction(tx_stale).unwrap();
    let verified_fresh = epoch_store.verify_transaction(tx_fresh).unwrap();

    // Stale transaction ordered first, as described in the issue.
    let mut transactions = vec![
        make_user_tx_v1_verified(verified_stale.clone()),
        make_user_tx_v1_verified(verified_fresh.clone()),
    ];

    let (dropped, locks, user_tx_digests) =
        post_consensus_validation::validate_and_resolve_conflicts(
            &authority,
            &epoch_store,
            &mut transactions,
        )
        .await
        .unwrap();

    let (dropped_digests, dropped_errors): (Vec<TransactionDigest>, Vec<IotaError>) =
        dropped.into_iter().unzip();

    assert_eq!(transactions.len(), 1, "Only the fresh tx should remain");
    assert_eq!(
        dropped_digests,
        vec![*verified_stale.digest()],
        "Only the stale tx should be dropped"
    );
    assert!(
        matches!(
            dropped_errors[0],
            IotaError::UserInput {
                error: UserInputError::ObjectVersionUnavailableForConsumption { .. }
            }
        ),
        "Stale tx should be dropped because its version is unavailable, got {:?}",
        dropped_errors[0]
    );

    // The fresh tx acquired the lock on the live ref; the stale tx never locked
    // anything.
    assert_eq!(locks.get(&fresh_ref), Some(verified_fresh.digest()));
    assert!(
        !locks.contains_key(&stale_ref),
        "Stale tx must not acquire a lock"
    );

    // Both transactions passed dedup, so both digests are reported for soft-lock
    // release — the dropped stale tx as well as the kept fresh tx.
    assert_eq!(user_tx_digests.len(), 2);
    assert!(user_tx_digests.contains(verified_stale.digest()));
    assert!(user_tx_digests.contains(verified_fresh.digest()));
}

/// Two transactions on different objects: both pass with no conflicts.
#[sim_test]
async fn test_no_conflict() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let recipient1 = Address::random();
    let recipient2 = Address::random();

    let object1_id = ObjectId::random();
    let object2_id = ObjectId::random();
    let gas1_id = ObjectId::random();
    let gas2_id = ObjectId::random();

    let (authority, _) = init_state_with_objects_and_object_basics(vec![
        Object::with_id_owner_for_testing(object1_id, sender),
        Object::with_id_owner_for_testing(object2_id, sender),
        Object::with_id_owner_for_testing(gas1_id, sender),
        Object::with_id_owner_for_testing(gas2_id, sender),
    ])
    .await;

    let epoch_store = authority.epoch_store_for_testing();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let object1 = authority.get_object(&object1_id).unwrap();
    let object2 = authority.get_object(&object2_id).unwrap();
    let gas1 = authority.get_object(&gas1_id).unwrap();
    let gas2 = authority.get_object(&gas2_id).unwrap();

    let tx1 = make_transfer_object_transaction(
        object1.object_ref(),
        gas1.object_ref(),
        sender,
        &sender_key,
        recipient1,
        rgp,
    );
    let tx2 = make_transfer_object_transaction(
        object2.object_ref(),
        gas2.object_ref(),
        sender,
        &sender_key,
        recipient2,
        rgp,
    );

    let verified_tx1 = epoch_store.verify_transaction(tx1).unwrap();
    let verified_tx2 = epoch_store.verify_transaction(tx2).unwrap();

    let mut transactions = vec![
        make_user_tx_v1_verified(verified_tx1.clone()),
        make_user_tx_v1_verified(verified_tx2.clone()),
    ];

    let (dropped, locks, user_tx_digests) =
        post_consensus_validation::validate_and_resolve_conflicts(
            &authority,
            &epoch_store,
            &mut transactions,
        )
        .await
        .unwrap();

    assert_eq!(transactions.len(), 2, "Both transactions should remain");
    assert!(dropped.is_empty(), "No transactions should be dropped");
    assert_eq!(locks.len(), 4, "Four locks acquired (2 objects + 2 gas)");
    assert_eq!(
        locks.get(&object1.object_ref()),
        Some(verified_tx1.digest())
    );
    assert_eq!(
        locks.get(&object2.object_ref()),
        Some(verified_tx2.digest())
    );

    assert_eq!(user_tx_digests.len(), 2);
    assert!(user_tx_digests.contains(verified_tx1.digest()));
    assert!(user_tx_digests.contains(verified_tx2.digest()));
}

/// Three transactions with a chain conflict via shared gas: tx1 and tx2 win,
/// tx3 is dropped because tx2 already locked shared_gas.
#[sim_test]
async fn test_chain_conflict() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let recipient1 = Address::random();
    let recipient2 = Address::random();
    let recipient3 = Address::random();

    let object_a_id = ObjectId::random();
    let object_b_id = ObjectId::random();
    let object_c_id = ObjectId::random();
    let gas1_id = ObjectId::random();
    let shared_gas_id = ObjectId::random();

    let (authority, _) = init_state_with_objects_and_object_basics(vec![
        Object::with_id_owner_for_testing(object_a_id, sender),
        Object::with_id_owner_for_testing(object_b_id, sender),
        Object::with_id_owner_for_testing(object_c_id, sender),
        Object::with_id_owner_for_testing(gas1_id, sender),
        Object::with_id_owner_for_testing(shared_gas_id, sender),
    ])
    .await;

    let epoch_store = authority.epoch_store_for_testing();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let object_a = authority.get_object(&object_a_id).unwrap();
    let object_b = authority.get_object(&object_b_id).unwrap();
    let object_c = authority.get_object(&object_c_id).unwrap();
    let gas1 = authority.get_object(&gas1_id).unwrap();
    let shared_gas = authority.get_object(&shared_gas_id).unwrap();

    let tx1 = make_transfer_object_transaction(
        object_a.object_ref(),
        gas1.object_ref(),
        sender,
        &sender_key,
        recipient1,
        rgp,
    );
    let tx2 = make_transfer_object_transaction(
        object_b.object_ref(),
        shared_gas.object_ref(),
        sender,
        &sender_key,
        recipient2,
        rgp,
    );
    let tx3 = make_transfer_object_transaction(
        object_c.object_ref(),
        shared_gas.object_ref(),
        sender,
        &sender_key,
        recipient3,
        rgp,
    );

    let verified_tx1 = epoch_store.verify_transaction(tx1).unwrap();
    let verified_tx2 = epoch_store.verify_transaction(tx2).unwrap();
    let verified_tx3 = epoch_store.verify_transaction(tx3).unwrap();

    let mut transactions = vec![
        make_user_tx_v1_verified(verified_tx1.clone()),
        make_user_tx_v1_verified(verified_tx2.clone()),
        make_user_tx_v1_verified(verified_tx3.clone()),
    ];

    let (dropped, locks, user_tx_digests) =
        post_consensus_validation::validate_and_resolve_conflicts(
            &authority,
            &epoch_store,
            &mut transactions,
        )
        .await
        .unwrap();

    let (dropped_digests, _): (Vec<TransactionDigest>, Vec<IotaError>) =
        dropped.into_iter().unzip();

    assert_eq!(transactions.len(), 2, "Two transactions should remain");
    assert_eq!(dropped_digests.len(), 1, "Exactly one dropped");
    assert_eq!(dropped_digests[0], *verified_tx3.digest());

    assert_eq!(
        locks.get(&object_a.object_ref()),
        Some(verified_tx1.digest())
    );
    assert_eq!(
        locks.get(&object_b.object_ref()),
        Some(verified_tx2.digest())
    );
    assert_eq!(
        locks.get(&shared_gas.object_ref()),
        Some(verified_tx2.digest()),
        "tx2 should hold the shared gas lock"
    );

    assert_eq!(user_tx_digests.len(), 3);
    assert!(user_tx_digests.contains(verified_tx1.digest()));
    assert!(user_tx_digests.contains(verified_tx2.digest()));
    assert!(user_tx_digests.contains(verified_tx3.digest()));
}

/// Multiple independent conflict sets in one batch: tx1 beats tx2 on object A,
/// tx3 beats tx4 on object B.
#[sim_test]
async fn test_multiple_conflicts_in_batch() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let recipient = Address::random();

    let object_a_id = ObjectId::random();
    let object_b_id = ObjectId::random();
    let gas1_id = ObjectId::random();
    let gas2_id = ObjectId::random();
    let gas3_id = ObjectId::random();
    let gas4_id = ObjectId::random();

    let (authority, _) = init_state_with_objects_and_object_basics(vec![
        Object::with_id_owner_for_testing(object_a_id, sender),
        Object::with_id_owner_for_testing(object_b_id, sender),
        Object::with_id_owner_for_testing(gas1_id, sender),
        Object::with_id_owner_for_testing(gas2_id, sender),
        Object::with_id_owner_for_testing(gas3_id, sender),
        Object::with_id_owner_for_testing(gas4_id, sender),
    ])
    .await;

    let epoch_store = authority.epoch_store_for_testing();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let object_a = authority.get_object(&object_a_id).unwrap();
    let object_b = authority.get_object(&object_b_id).unwrap();
    let gas1 = authority.get_object(&gas1_id).unwrap();
    let gas2 = authority.get_object(&gas2_id).unwrap();
    let gas3 = authority.get_object(&gas3_id).unwrap();
    let gas4 = authority.get_object(&gas4_id).unwrap();

    let tx1 = make_transfer_object_transaction(
        object_a.object_ref(),
        gas1.object_ref(),
        sender,
        &sender_key,
        recipient,
        rgp,
    );
    let tx2 = make_transfer_object_transaction(
        object_a.object_ref(),
        gas2.object_ref(),
        sender,
        &sender_key,
        recipient,
        rgp,
    );
    let tx3 = make_transfer_object_transaction(
        object_b.object_ref(),
        gas3.object_ref(),
        sender,
        &sender_key,
        recipient,
        rgp,
    );
    let tx4 = make_transfer_object_transaction(
        object_b.object_ref(),
        gas4.object_ref(),
        sender,
        &sender_key,
        recipient,
        rgp,
    );

    let verified_tx1 = epoch_store.verify_transaction(tx1).unwrap();
    let verified_tx2 = epoch_store.verify_transaction(tx2).unwrap();
    let verified_tx3 = epoch_store.verify_transaction(tx3).unwrap();
    let verified_tx4 = epoch_store.verify_transaction(tx4).unwrap();

    let mut transactions = vec![
        make_user_tx_v1_verified(verified_tx1.clone()),
        make_user_tx_v1_verified(verified_tx2.clone()),
        make_user_tx_v1_verified(verified_tx3.clone()),
        make_user_tx_v1_verified(verified_tx4.clone()),
    ];

    let (dropped, locks, user_tx_digests) =
        post_consensus_validation::validate_and_resolve_conflicts(
            &authority,
            &epoch_store,
            &mut transactions,
        )
        .await
        .unwrap();

    let (dropped_digests, _): (Vec<TransactionDigest>, Vec<IotaError>) =
        dropped.into_iter().unzip();

    assert_eq!(transactions.len(), 2, "Two transactions should remain");
    assert_eq!(dropped_digests.len(), 2, "Two should be dropped");
    assert!(dropped_digests.contains(verified_tx2.digest()));
    assert!(dropped_digests.contains(verified_tx4.digest()));

    assert_eq!(
        locks.get(&object_a.object_ref()),
        Some(verified_tx1.digest())
    );
    assert_eq!(
        locks.get(&object_b.object_ref()),
        Some(verified_tx3.digest())
    );

    assert_eq!(user_tx_digests.len(), 4);
    assert!(user_tx_digests.contains(verified_tx1.digest()));
    assert!(user_tx_digests.contains(verified_tx2.digest()));
    assert!(user_tx_digests.contains(verified_tx3.digest()));
    assert!(user_tx_digests.contains(verified_tx4.digest()));
}

/// Two transactions sharing the same gas object: first wins, second dropped.
#[sim_test]
async fn test_gas_object_conflict() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let recipient1 = Address::random();
    let recipient2 = Address::random();

    let object1_id = ObjectId::random();
    let object2_id = ObjectId::random();
    let shared_gas_id = ObjectId::random();

    let (authority, _) = init_state_with_objects_and_object_basics(vec![
        Object::with_id_owner_for_testing(object1_id, sender),
        Object::with_id_owner_for_testing(object2_id, sender),
        Object::with_id_owner_for_testing(shared_gas_id, sender),
    ])
    .await;

    let epoch_store = authority.epoch_store_for_testing();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let object1 = authority.get_object(&object1_id).unwrap();
    let object2 = authority.get_object(&object2_id).unwrap();
    let shared_gas = authority.get_object(&shared_gas_id).unwrap();

    let tx1 = make_transfer_object_transaction(
        object1.object_ref(),
        shared_gas.object_ref(),
        sender,
        &sender_key,
        recipient1,
        rgp,
    );
    let tx2 = make_transfer_object_transaction(
        object2.object_ref(),
        shared_gas.object_ref(),
        sender,
        &sender_key,
        recipient2,
        rgp,
    );

    let verified_tx1 = epoch_store.verify_transaction(tx1).unwrap();
    let verified_tx2 = epoch_store.verify_transaction(tx2).unwrap();

    let mut transactions = vec![
        make_user_tx_v1_verified(verified_tx1.clone()),
        make_user_tx_v1_verified(verified_tx2.clone()),
    ];

    let (dropped, locks, user_tx_digests) =
        post_consensus_validation::validate_and_resolve_conflicts(
            &authority,
            &epoch_store,
            &mut transactions,
        )
        .await
        .unwrap();

    let (dropped_digests, _): (Vec<TransactionDigest>, Vec<IotaError>) =
        dropped.into_iter().unzip();

    assert_eq!(transactions.len(), 1, "Only one should remain");
    assert_eq!(dropped_digests.len(), 1, "One should be dropped");
    assert_eq!(dropped_digests[0], *verified_tx2.digest());

    assert_eq!(
        locks.get(&shared_gas.object_ref()),
        Some(verified_tx1.digest())
    );
    assert_eq!(
        locks.get(&object1.object_ref()),
        Some(verified_tx1.digest())
    );

    assert_eq!(user_tx_digests.len(), 2);
    assert!(user_tx_digests.contains(verified_tx1.digest()));
    assert!(user_tx_digests.contains(verified_tx2.digest()));
}

/// tx1 locks both A and B; tx2 (object A) and tx3 (object B) are both dropped.
#[sim_test]
async fn test_winner_blocks_multiple_losers() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let recipient = Address::random();

    let object_a_id = ObjectId::random();
    let object_b_id = ObjectId::random();
    let gas1_id = ObjectId::random();
    let gas2_id = ObjectId::random();
    let gas3_id = ObjectId::random();

    let (authority, package_ref) = init_state_with_objects_and_object_basics(vec![
        Object::with_id_owner_for_testing(object_a_id, sender),
        Object::with_id_owner_for_testing(object_b_id, sender),
        Object::with_id_owner_for_testing(gas1_id, sender),
        Object::with_id_owner_for_testing(gas2_id, sender),
        Object::with_id_owner_for_testing(gas3_id, sender),
    ])
    .await;

    let epoch_store = authority.epoch_store_for_testing();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let object_a = authority.get_object(&object_a_id).unwrap();
    let object_b = authority.get_object(&object_b_id).unwrap();
    let gas1 = authority.get_object(&gas1_id).unwrap();
    let gas2 = authority.get_object(&gas2_id).unwrap();
    let gas3 = authority.get_object(&gas3_id).unwrap();

    use iota_sdk_types::{Identifier, Transaction};
    use iota_types::transaction::{CallArg, TransactionAPI};

    let tx1 = Transaction::new_move_call(
        sender,
        package_ref.object_id,
        Identifier::from_static("object_basics"),
        Identifier::from_static("update"),
        vec![],
        gas1.object_ref(),
        vec![
            CallArg::ImmutableOrOwned(object_a.object_ref()),
            CallArg::ImmutableOrOwned(object_b.object_ref()),
        ],
        rgp * 1000,
        rgp,
    )
    .unwrap();
    let tx1 = iota_types::utils::to_sender_signed_transaction(tx1, &sender_key);

    let tx2 = make_transfer_object_transaction(
        object_a.object_ref(),
        gas2.object_ref(),
        sender,
        &sender_key,
        recipient,
        rgp,
    );
    let tx3 = make_transfer_object_transaction(
        object_b.object_ref(),
        gas3.object_ref(),
        sender,
        &sender_key,
        recipient,
        rgp,
    );

    let verified_tx1 = epoch_store.verify_transaction(tx1).unwrap();
    let verified_tx2 = epoch_store.verify_transaction(tx2).unwrap();
    let verified_tx3 = epoch_store.verify_transaction(tx3).unwrap();

    let mut transactions = vec![
        make_user_tx_v1_verified(verified_tx1.clone()),
        make_user_tx_v1_verified(verified_tx2.clone()),
        make_user_tx_v1_verified(verified_tx3.clone()),
    ];

    let (dropped, locks, user_tx_digests) =
        post_consensus_validation::validate_and_resolve_conflicts(
            &authority,
            &epoch_store,
            &mut transactions,
        )
        .await
        .unwrap();

    let (dropped_digests, _): (Vec<TransactionDigest>, Vec<IotaError>) =
        dropped.into_iter().unzip();

    assert_eq!(transactions.len(), 1, "Only tx1 should remain");
    assert_eq!(dropped_digests.len(), 2, "tx2 and tx3 should be dropped");
    assert!(dropped_digests.contains(verified_tx2.digest()));
    assert!(dropped_digests.contains(verified_tx3.digest()));

    assert_eq!(
        locks.get(&object_a.object_ref()),
        Some(verified_tx1.digest())
    );
    assert_eq!(
        locks.get(&object_b.object_ref()),
        Some(verified_tx1.digest())
    );

    assert_eq!(user_tx_digests.len(), 3);
    assert!(user_tx_digests.contains(verified_tx1.digest()));
    assert!(user_tx_digests.contains(verified_tx2.digest()));
    assert!(user_tx_digests.contains(verified_tx3.digest()));
}

/// Verifies that dropped transactions don't acquire locks, allowing later
/// transactions to use those objects.
///
/// tx1 (object_a, shared_gas) wins.
/// tx2 (object_a, gas1) drops — object_a conflict with tx1.
/// tx3 (object_b, shared_gas) drops — shared_gas conflict with tx1.
/// tx4 (object_b, gas2) wins — object_b is free since tx3 was dropped.
#[sim_test]
async fn test_dropped_tx_does_not_acquire_locks() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let recipient = Address::random();

    let object_a_id = ObjectId::random();
    let object_b_id = ObjectId::random();
    let gas1_id = ObjectId::random();
    let gas2_id = ObjectId::random();
    let shared_gas_id = ObjectId::random();

    let (authority, _) = init_state_with_objects_and_object_basics(vec![
        Object::with_id_owner_for_testing(object_a_id, sender),
        Object::with_id_owner_for_testing(object_b_id, sender),
        Object::with_id_owner_for_testing(gas1_id, sender),
        Object::with_id_owner_for_testing(gas2_id, sender),
        Object::with_id_owner_for_testing(shared_gas_id, sender),
    ])
    .await;

    let epoch_store = authority.epoch_store_for_testing();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let object_a = authority.get_object(&object_a_id).unwrap();
    let object_b = authority.get_object(&object_b_id).unwrap();
    let gas1 = authority.get_object(&gas1_id).unwrap();
    let gas2 = authority.get_object(&gas2_id).unwrap();
    let shared_gas = authority.get_object(&shared_gas_id).unwrap();

    let tx1 = make_transfer_object_transaction(
        object_a.object_ref(),
        shared_gas.object_ref(),
        sender,
        &sender_key,
        recipient,
        rgp,
    );
    let tx2 = make_transfer_object_transaction(
        object_a.object_ref(),
        gas1.object_ref(),
        sender,
        &sender_key,
        recipient,
        rgp,
    );
    let tx3 = make_transfer_object_transaction(
        object_b.object_ref(),
        shared_gas.object_ref(),
        sender,
        &sender_key,
        recipient,
        rgp,
    );
    let tx4 = make_transfer_object_transaction(
        object_b.object_ref(),
        gas2.object_ref(),
        sender,
        &sender_key,
        recipient,
        rgp,
    );

    let verified_tx1 = epoch_store.verify_transaction(tx1).unwrap();
    let verified_tx2 = epoch_store.verify_transaction(tx2).unwrap();
    let verified_tx3 = epoch_store.verify_transaction(tx3).unwrap();
    let verified_tx4 = epoch_store.verify_transaction(tx4).unwrap();

    let mut transactions = vec![
        make_user_tx_v1_verified(verified_tx1.clone()),
        make_user_tx_v1_verified(verified_tx2.clone()),
        make_user_tx_v1_verified(verified_tx3.clone()),
        make_user_tx_v1_verified(verified_tx4.clone()),
    ];

    let (dropped, locks, user_tx_digests) =
        post_consensus_validation::validate_and_resolve_conflicts(
            &authority,
            &epoch_store,
            &mut transactions,
        )
        .await
        .unwrap();

    let (dropped_digests, _): (Vec<TransactionDigest>, Vec<IotaError>) =
        dropped.into_iter().unzip();

    assert_eq!(transactions.len(), 2, "tx1 and tx4 should remain");
    assert_eq!(dropped_digests.len(), 2, "tx2 and tx3 should be dropped");
    assert!(dropped_digests.contains(verified_tx2.digest()));
    assert!(dropped_digests.contains(verified_tx3.digest()));

    assert_eq!(
        locks.get(&object_a.object_ref()),
        Some(verified_tx1.digest()),
        "tx1 should lock object_a"
    );
    assert_eq!(
        locks.get(&shared_gas.object_ref()),
        Some(verified_tx1.digest()),
        "tx1 should lock shared_gas"
    );
    assert_eq!(
        locks.get(&object_b.object_ref()),
        Some(verified_tx4.digest()),
        "tx4 should lock object_b (tx3 was dropped before locking)"
    );
    assert_eq!(
        locks.get(&gas2.object_ref()),
        Some(verified_tx4.digest()),
        "tx4 should lock gas2"
    );
    assert!(
        !locks.contains_key(&gas1.object_ref()),
        "gas1 should not be locked since tx2 was dropped"
    );

    assert_eq!(user_tx_digests.len(), 4);
    assert!(user_tx_digests.contains(verified_tx1.digest()));
    assert!(user_tx_digests.contains(verified_tx2.digest()));
    assert!(user_tx_digests.contains(verified_tx3.digest()));
    assert!(user_tx_digests.contains(verified_tx4.digest()));
}

// ---------------------------------------------------------------------------
// Checkpoint-root regression test (issue #11649)
// ---------------------------------------------------------------------------

/// Regression test for the P-COOL checkpoint fork observed in the
/// double-spend stress test (#11649).
///
/// A validator that lags through an epoch boundary executes a transaction via
/// state-sync (from an already-certified checkpoint) *before* its own consensus
/// handler processes the commit that sequenced that transaction. When the
/// commit is finally processed, the post-consensus "already-executed" check
/// (Check #1) silently drops the transaction, so it is excluded from the
/// locally-built checkpoint `roots`. The rest of the committee included it, so
/// the local checkpoint forks and the node panics with
/// "Local checkpoint fork detected".
///
/// Invariant under test: a committee-sequenced transaction that this node has
/// executed must still appear in the pending checkpoint roots, regardless of
/// whether it was executed via its own consensus or via state-sync.
///
/// This test FAILS on the buggy code (the tx is missing from roots) and passes
/// once the already-executed transaction is kept in the checkpoint roots.
///
/// Single-process and fully deterministic, so it uses `#[tokio::test]` rather
/// than `#[sim_test]` — it does not need the deterministic simulator.
#[tokio::test]
async fn already_executed_tx_must_remain_in_checkpoint_roots() {
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let recipient = Address::random();

    let object_id = ObjectId::random();
    let gas_id = ObjectId::random();

    let (authority, _) = init_state_with_objects_and_object_basics(vec![
        Object::with_id_owner_for_testing(object_id, sender),
        Object::with_id_owner_for_testing(gas_id, sender),
    ])
    .await;

    let epoch_store = authority.epoch_store_for_testing();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let object_ref = authority.get_object(&object_id).unwrap().object_ref();
    let gas_ref = authority.get_object(&gas_id).unwrap().object_ref();

    let tx =
        make_transfer_object_transaction(object_ref, gas_ref, sender, &sender_key, recipient, rgp);
    let verified_tx = epoch_store.verify_transaction(tx).unwrap();
    let tx_digest = *verified_tx.digest();

    // Simulate state-sync winning the race: execute the transaction locally via
    // the checkpoint execution path (as a lagging node catching up would),
    // *before* the consensus handler processes the commit that sequenced it.
    let executable = VerifiedExecutableTransaction::new_from_checkpoint(
        verified_tx.clone(),
        epoch_store.epoch(),
        // checkpoint
        1,
    );
    authority
        .try_execute_immediately(&executable, ExecutionEnv::new(), &epoch_store)
        .unwrap();
    assert!(
        authority
            .get_transaction_cache_reader()
            .try_is_tx_already_executed(&tx_digest)
            .unwrap(),
        "precondition: transaction should be marked executed after state-sync execution"
    );

    // Now the committee's consensus delivers the same transaction. Process the
    // commit exactly as the consensus handler would, which builds the pending
    // checkpoint for this commit.
    let seq_tx = SequencedConsensusTransaction::new_test(ConsensusTransaction {
        kind: ConsensusTransactionKind::UserTransactionV1(Box::new(verified_tx.into())),
        tracking_id: Default::default(),
    });
    authority
        .epoch_store_for_testing()
        .process_consensus_transactions_for_tests(
            vec![seq_tx],
            &Arc::new(CheckpointServiceNoop {}),
            authority.get_object_cache_reader().as_ref(),
            &authority.metrics,
            // skip_consensus_commit_prologue_in_test
            true,
            &authority,
        )
        .await
        .unwrap();

    // The transaction must still be a checkpoint root on this node, otherwise
    // its locally-built checkpoint diverges from the committee's certified one.
    let all_roots: Vec<TransactionKey> = authority
        .epoch_store_for_testing()
        .get_pending_checkpoints(None)
        .unwrap()
        .into_iter()
        .flat_map(|(_, cp)| cp.roots().clone())
        .collect();

    assert!(
        all_roots.contains(&TransactionKey::Digest(tx_digest)),
        "already-executed transaction {tx_digest:?} was dropped from checkpoint roots \
         (fork bug #11649); roots = {all_roots:?}"
    );
}

/// Companion to the test above: a double-spend *loser* (dropped by the lock
/// conflict check) must be **excluded** from checkpoint roots.
///
/// This guards against an over-broad fix that simply seeds roots from the full
/// sequenced set: such a fix would include the never-executed loser as a root,
/// and the checkpoint builder would then block forever waiting for its effects.
/// Only the winner of the owned-object conflict may appear in the roots.
#[tokio::test]
async fn double_spend_loser_excluded_from_checkpoint_roots() {
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let recipient = Address::random();

    // One owned object spent by both transactions, plus a distinct gas object each
    // so the only conflict is on `object_id`.
    let object_id = ObjectId::random();
    let gas_a = ObjectId::random();
    let gas_b = ObjectId::random();

    let (authority, _) = init_state_with_objects_and_object_basics(vec![
        Object::with_id_owner_for_testing(object_id, sender),
        Object::with_id_owner_for_testing(gas_a, sender),
        Object::with_id_owner_for_testing(gas_b, sender),
    ])
    .await;

    let epoch_store = authority.epoch_store_for_testing();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let object_ref = authority.get_object(&object_id).unwrap().object_ref();
    let gas_a_ref = authority.get_object(&gas_a).unwrap().object_ref();
    let gas_b_ref = authority.get_object(&gas_b).unwrap().object_ref();

    // Two transactions spending the same owned object — a double spend.
    let tx_winner = make_transfer_object_transaction(
        object_ref,
        gas_a_ref,
        sender,
        &sender_key,
        recipient,
        rgp,
    );
    let tx_loser = make_transfer_object_transaction(
        object_ref,
        gas_b_ref,
        sender,
        &sender_key,
        recipient,
        rgp,
    );
    let winner_digest = *epoch_store
        .verify_transaction(tx_winner.clone())
        .unwrap()
        .digest();
    let loser_digest = *epoch_store
        .verify_transaction(tx_loser.clone())
        .unwrap()
        .digest();
    assert_ne!(winner_digest, loser_digest);

    // The first occurrence in the commit wins the lock; the second conflicts.
    let seq = |tx: iota_types::transaction::TransactionEnvelope| {
        SequencedConsensusTransaction::new_test(ConsensusTransaction {
            kind: ConsensusTransactionKind::UserTransactionV1(Box::new(
                epoch_store.verify_transaction(tx).unwrap().into(),
            )),
            tracking_id: Default::default(),
        })
    };
    authority
        .epoch_store_for_testing()
        .process_consensus_transactions_for_tests(
            vec![seq(tx_winner), seq(tx_loser)],
            &Arc::new(CheckpointServiceNoop {}),
            authority.get_object_cache_reader().as_ref(),
            &authority.metrics,
            // skip_consensus_commit_prologue_in_test
            true,
            &authority,
        )
        .await
        .unwrap();

    let all_roots: Vec<TransactionKey> = authority
        .epoch_store_for_testing()
        .get_pending_checkpoints(None)
        .unwrap()
        .into_iter()
        .flat_map(|(_, cp)| cp.roots().clone())
        .collect();

    assert!(
        all_roots.contains(&TransactionKey::Digest(winner_digest)),
        "conflict winner {winner_digest:?} should be a checkpoint root; roots = {all_roots:?}"
    );
    assert!(
        !all_roots.contains(&TransactionKey::Digest(loser_digest)),
        "double-spend loser {loser_digest:?} must NOT be a checkpoint root; roots = {all_roots:?}"
    );
}

// ---------------------------------------------------------------------------
// Tier 2 (quarantine) and Tier 3 (persistent DB) lock-tier coverage
// ---------------------------------------------------------------------------
//
// `validate_and_resolve_conflicts` performs the same 3-tier lock lookup in
// two places (via `find_existing_lock`):
//   * Check #1 (already-executed branch): same digest = OK; different digest =
//     `fatal!`.
//   * Check #4 (conflict drop): a hit from a different digest = drop with
//     `ObjectLockConflict`; a hit from the same digest (a deferred tx's own
//     prior-round lock) is exempt and the tx is retained.
//
// The earlier tests cover Tier 1 (`current_commit_locks` HashMap within the
// same commit). The tests below close the matrix by seeding Tier 2 (consensus
// quarantine) and Tier 3 (persistent DB).

/// Which of the two non-local tiers to seed an existing lock into.
#[derive(Clone, Copy)]
enum LockTier {
    Quarantine,
    Persistent,
}

/// Shared setup: one owned object + one gas object, both owned by `sender`.
/// `_config_guard` keeps the P-COOL protocol-config override active for
/// the duration of the test; on drop it clears the thread-local override so a
/// later test on the same OS thread can install its own.
struct LockTierSetup {
    authority: Arc<crate::authority::AuthorityState>,
    epoch_store: Arc<crate::authority::authority_per_epoch_store::AuthorityPerEpochStore>,
    sender: Address,
    sender_key: AccountPrivateKey,
    recipient: Address,
    object_ref: ObjectReference,
    gas_ref: ObjectReference,
    rgp: u64,
    _config_guard: OverrideGuard,
}

async fn setup_lock_tier() -> LockTierSetup {
    let _config_guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let recipient = Address::random();
    let object_id = ObjectId::random();
    let gas_id = ObjectId::random();

    let (authority, _) = init_state_with_objects_and_object_basics(vec![
        Object::with_id_owner_for_testing(object_id, sender),
        Object::with_id_owner_for_testing(gas_id, sender),
    ])
    .await;

    let epoch_store = (*authority.epoch_store_for_testing()).clone();
    let rgp = authority.reference_gas_price_for_testing().unwrap();
    let object_ref = authority.get_object(&object_id).unwrap().object_ref();
    let gas_ref = authority.get_object(&gas_id).unwrap().object_ref();

    LockTierSetup {
        authority,
        epoch_store,
        sender,
        sender_key,
        recipient,
        object_ref,
        gas_ref,
        rgp,
        _config_guard,
    }
}

impl LockTierSetup {
    /// Builds and verifies a transfer-object transaction spending
    /// `self.object_ref` paid by `self.gas_ref`.
    fn make_tx(&self) -> VerifiedTransaction {
        let tx = make_transfer_object_transaction(
            self.object_ref,
            self.gas_ref,
            self.sender,
            &self.sender_key,
            self.recipient,
            self.rgp,
        );
        self.epoch_store.verify_transaction(tx).unwrap()
    }

    /// Marks `verified_tx` as executed via the checkpoint-executor path
    /// (simulates state-sync winning the race against this node's consensus
    /// handler).
    fn execute_via_state_sync(&self, verified_tx: &VerifiedTransaction) {
        let executable = VerifiedExecutableTransaction::new_from_checkpoint(
            verified_tx.clone(),
            self.epoch_store.epoch(),
            1,
        );
        self.authority
            .try_execute_immediately(&executable, ExecutionEnv::new(), &self.epoch_store)
            .unwrap();
    }

    /// Seeds `(self.object_ref, self.gas_ref) -> locker` into the requested
    /// tier. For `Persistent`, a signed-transaction row backing the lock is
    /// also written.
    fn seed_lock(&self, tier: LockTier, locker_tx: &VerifiedTransaction) {
        let digest = *locker_tx.digest();
        match tier {
            LockTier::Quarantine => {
                seed_quarantined_lock(&self.epoch_store, self.object_ref, digest);
                seed_quarantined_lock(&self.epoch_store, self.gas_ref, digest);
            }
            LockTier::Persistent => {
                seed_persistent_lock(
                    &self.authority,
                    &self.epoch_store,
                    locker_tx.clone(),
                    &[self.object_ref, self.gas_ref],
                );
            }
        }
    }
}

/// Seeds a single lock into the consensus quarantine.
fn seed_quarantined_lock(
    epoch_store: &crate::authority::authority_per_epoch_store::AuthorityPerEpochStore,
    obj_ref: ObjectReference,
    locker: LockDetails,
) {
    let mut output = ConsensusCommitOutput::default();
    output.set_owned_object_locks(std::collections::HashMap::from([(obj_ref, locker)]));
    output.set_default_commit_stats_for_testing();
    epoch_store.push_consensus_output_for_tests(output);
}

/// Seeds locks directly into the persistent DB via the cache writer's
/// `try_acquire_transaction_locks`.
fn seed_persistent_lock(
    authority: &crate::authority::AuthorityState,
    epoch_store: &crate::authority::authority_per_epoch_store::AuthorityPerEpochStore,
    verified_tx: VerifiedTransaction,
    owned_inputs: &[ObjectReference],
) {
    use iota_types::transaction::VerifiedSignedTransaction;
    let signed = VerifiedSignedTransaction::new(
        epoch_store.epoch(),
        verified_tx,
        authority.name,
        &*authority.secret,
    );
    authority
        .get_cache_writer()
        .try_acquire_transaction_locks(epoch_store, owned_inputs, signed)
        .expect("seed_persistent_lock: try_acquire_transaction_locks failed");
}

/// Body for the Check #4 drop case: a lock held by a DIFFERENT tx in the given
/// tier causes the new contender to be dropped with `ObjectLockConflict`.
async fn run_different_digest_lock_drops_contender(tier: LockTier) {
    let s = setup_lock_tier().await;

    // A first tx owns the lock in `tier`.
    let other = s.make_tx();
    s.seed_lock(tier, &other);

    // A different tx contending for the same owned input arrives via consensus.
    // For Quarantine we can build a contender with the same inputs because
    // make_tx is hashed by recipient/sender_key which are stable; produce a
    // different digest by swapping recipient.
    let alt_recipient = Address::random();
    let new_tx_raw = make_transfer_object_transaction(
        s.object_ref,
        s.gas_ref,
        s.sender,
        &s.sender_key,
        alt_recipient,
        s.rgp,
    );
    let new_verified = s.epoch_store.verify_transaction(new_tx_raw).unwrap();
    let new_digest = *new_verified.digest();
    assert_ne!(new_digest, *other.digest());

    let mut transactions = vec![make_user_tx_v1_verified(new_verified)];
    let (dropped, _, all_digests) = post_consensus_validation::validate_and_resolve_conflicts(
        &s.authority,
        &s.epoch_store,
        &mut transactions,
    )
    .await
    .unwrap();

    assert_eq!(transactions.len(), 0, "contender must be removed");
    assert_eq!(all_digests.len(), 1);
    assert_eq!(all_digests[0], new_digest);
    assert_eq!(dropped.len(), 1);
    assert_eq!(dropped[0].0, new_digest);
    assert!(
        matches!(dropped[0].1, IotaError::ObjectLockConflict { .. }),
        "expected ObjectLockConflict, got {:?}",
        dropped[0].1
    );
}

/// Body for the Check #1 retain case: an already-executed tx finding its OWN
/// digest as the lock holder in the given tier must NOT be dropped and must
/// NOT trigger `fatal!`.
async fn run_same_digest_lock_retains_already_executed(tier: LockTier) {
    let s = setup_lock_tier().await;

    let tx = s.make_tx();
    let tx_digest = *tx.digest();

    // Order matters for the Persistent tier: the lock must be acquired *before*
    // we mark the tx as executed (otherwise the perpetual
    // `live_owned_object_markers` for its inputs are already consumed).
    s.seed_lock(tier, &tx);
    s.execute_via_state_sync(&tx);

    let mut transactions = vec![make_user_tx_v1_verified(tx)];
    let (dropped, _, all_digests) = post_consensus_validation::validate_and_resolve_conflicts(
        &s.authority,
        &s.epoch_store,
        &mut transactions,
    )
    .await
    .unwrap();

    assert!(
        dropped.is_empty(),
        "already-executed tx must not be dropped"
    );
    assert_eq!(
        transactions.len(),
        1,
        "already-executed tx must be retained"
    );

    assert_eq!(all_digests.len(), 1,);
    assert!(
        s.authority
            .get_transaction_cache_reader()
            .try_is_tx_already_executed(&tx_digest)
            .unwrap()
    );
}

/// Body for the deferred-transaction self-lock case: a transaction that already
/// holds its OWN lock in the given tier (from a prior consensus round in which
/// it acquired owned-object locks and was then deferred for shared-object
/// congestion) must NOT be dropped when it is reloaded and re-validated.
/// Without the self-exemption in Check #4 it would conflict with its own lock,
/// drop as `ObjectLockConflict`, and never execute.
///
/// Unlike the already-executed case (Check #1), the transaction is NOT executed
/// here, so it flows through the full validation pass (Check #2-#5) and must
/// survive end-to-end and re-acquire its locks.
///
/// Dedup by digest already runs upstream in Check #0, so a same-digest lock
/// seen in Check #4 can only be this transaction's own prior-round lock (a
/// deferred tx), never a same-commit duplicate.
async fn run_self_lock_retains_deferred_tx(tier: LockTier) {
    let setup = setup_lock_tier().await;

    let tx = setup.make_tx();
    let tx_digest = *tx.digest();

    // Seed the tx's own lock into the tier, as round r would before deferral.
    setup.seed_lock(tier, &tx);

    let mut transactions = vec![make_user_tx_v1_verified(tx)];
    let (dropped, locks, all_digests) = post_consensus_validation::validate_and_resolve_conflicts(
        &setup.authority,
        &setup.epoch_store,
        &mut transactions,
    )
    .await
    .unwrap();

    assert!(
        dropped.is_empty(),
        "deferred tx must not self-conflict on its own prior-round lock"
    );
    assert_eq!(transactions.len(), 1, "deferred tx must be retained");
    assert_eq!(
        all_digests,
        vec![tx_digest],
        "deferred tx digest is reported"
    );
    assert_eq!(
        locks.get(&setup.object_ref),
        Some(&tx_digest),
        "deferred tx must re-acquire its own object lock"
    );
    assert_eq!(
        locks.get(&setup.gas_ref),
        Some(&tx_digest),
        "deferred tx must re-acquire its own gas lock"
    );
}

#[tokio::test]
async fn tier2_quarantine_self_lock_retains_deferred_tx() {
    run_self_lock_retains_deferred_tx(LockTier::Quarantine).await;
}

#[tokio::test]
async fn tier3_persistent_self_lock_retains_deferred_tx() {
    run_self_lock_retains_deferred_tx(LockTier::Persistent).await;
}

#[tokio::test]
async fn tier2_quarantine_different_digest_lock_drops_contender() {
    run_different_digest_lock_drops_contender(LockTier::Quarantine).await;
}

#[tokio::test]
async fn tier2_quarantine_same_digest_lock_retains_already_executed() {
    run_same_digest_lock_retains_already_executed(LockTier::Quarantine).await;
}

#[tokio::test]
async fn tier3_persistent_different_digest_lock_drops_contender() {
    run_different_digest_lock_drops_contender(LockTier::Persistent).await;
}

#[tokio::test]
async fn tier3_persistent_same_digest_lock_retains_already_executed() {
    run_same_digest_lock_retains_already_executed(LockTier::Persistent).await;
}

/// Check #1 / `fatal!` invariant: when an already-executed transaction's owned
/// input is locked by a DIFFERENT transaction digest,
/// `validate_and_resolve_conflicts` must panic — the executed-but-out-locked
/// state is a real consistency violation, not a recoverable conflict.
///
/// Uses the Tier 2 (quarantine) seeding path; the helper is tier-agnostic, so a
/// single test covers both quarantine and persistent-DB code paths.
#[tokio::test]
#[should_panic(expected = "locked by a different transaction")]
async fn already_executed_tx_locked_by_different_digest_is_fatal() {
    let s = setup_lock_tier().await;

    // The tx the committee actually executed (via state-sync on this node).
    let tx = s.make_tx();
    s.execute_via_state_sync(&tx);

    // Seed the quarantine with a lock on `tx`'s inputs held by a DIFFERENT
    // digest — simulates the consistency violation the `fatal!` guards against.
    let other_digest = TransactionDigest::random();
    assert_ne!(other_digest, *tx.digest());
    seed_quarantined_lock(&s.epoch_store, s.object_ref, other_digest);

    let mut transactions = vec![make_user_tx_v1_verified(tx)];
    // Expected to panic via `fatal!` before returning.
    let _ = post_consensus_validation::validate_and_resolve_conflicts(
        &s.authority,
        &s.epoch_store,
        &mut transactions,
    )
    .await;
}

// ---------------------------------------------------------------------------
// Governance deny rule tests
// ---------------------------------------------------------------------------

/// Activates `rules` on the epoch store by recording a proposal from this
/// (sole, full-stake) validator through the quarantine push path. A strictly
/// higher `generation` supersedes a previously activated set.
fn activate_deny_rules(
    epoch_store: &Arc<crate::authority::authority_per_epoch_store::AuthorityPerEpochStore>,
    rules: iota_sdk_types::DenyRuleSet,
    generation: u64,
) {
    let mut output = ConsensusCommitOutput::new(0);
    output.record_deny_rule_proposal(
        iota_types::messages_consensus::TransactionDenyRuleProposal {
            authority: epoch_store.name,
            generation,
            proposed_rules: rules,
        },
    );
    output.set_default_commit_stats_for_testing();
    epoch_store.push_consensus_output_for_tests(output);
}

/// With `deny_rule_governance` enabled, post-consensus validation drops a
/// transaction whose sender is denied by the consensus-governed active set.
#[sim_test]
async fn post_consensus_validation_uses_governance_rules_when_enabled() {
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config.set_deny_rule_governance_for_testing(true);
        config
    });

    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let recipient = Address::random();
    let object_id = ObjectId::random();
    let gas_id = ObjectId::random();
    let (authority, _) = init_state_with_objects_and_object_basics(vec![
        Object::with_id_owner_for_testing(object_id, sender),
        Object::with_id_owner_for_testing(gas_id, sender),
    ])
    .await;
    let epoch_store = authority.epoch_store_for_testing();

    activate_deny_rules(
        &epoch_store,
        iota_sdk_types::DenyRuleSet {
            denied_addresses: [sender].into(),
            ..Default::default()
        },
        1,
    );

    let object_ref = authority.get_object(&object_id).unwrap().object_ref();
    let gas_ref = authority.get_object(&gas_id).unwrap().object_ref();
    let rgp = authority.reference_gas_price_for_testing().unwrap();
    let tx =
        make_transfer_object_transaction(object_ref, gas_ref, sender, &sender_key, recipient, rgp);
    let digest = *tx.digest();
    let mut transactions = vec![make_user_tx_v1(tx)];

    let (dropped, locks, _) = post_consensus_validation::validate_and_resolve_conflicts(
        &authority,
        &epoch_store,
        &mut transactions,
    )
    .await
    .unwrap();

    assert_eq!(dropped.len(), 1);
    assert_eq!(dropped[0].0, digest);
    assert!(matches!(
        &dropped[0].1,
        IotaError::UserInput {
            error: UserInputError::TransactionDenied { .. }
        }
    ));
    assert!(transactions.is_empty());
    assert!(locks.is_empty(), "dropped transaction must not take locks");
}

/// With `deny_rule_governance` enabled and active rules denying an unrelated
/// address, a non-denied sender's transaction is kept and takes its locks.
#[sim_test]
async fn post_consensus_validation_keeps_non_denied_transactions() {
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config.set_deny_rule_governance_for_testing(true);
        config
    });

    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let recipient = Address::random();
    let object_id = ObjectId::random();
    let gas_id = ObjectId::random();
    let (authority, _) = init_state_with_objects_and_object_basics(vec![
        Object::with_id_owner_for_testing(object_id, sender),
        Object::with_id_owner_for_testing(gas_id, sender),
    ])
    .await;
    let epoch_store = authority.epoch_store_for_testing();

    activate_deny_rules(
        &epoch_store,
        iota_sdk_types::DenyRuleSet {
            denied_addresses: [Address::random()].into(),
            ..Default::default()
        },
        1,
    );

    let object_ref = authority.get_object(&object_id).unwrap().object_ref();
    let gas_ref = authority.get_object(&gas_id).unwrap().object_ref();
    let rgp = authority.reference_gas_price_for_testing().unwrap();
    let tx =
        make_transfer_object_transaction(object_ref, gas_ref, sender, &sender_key, recipient, rgp);
    let mut transactions = vec![make_user_tx_v1(tx)];

    let (dropped, locks, _) = post_consensus_validation::validate_and_resolve_conflicts(
        &authority,
        &epoch_store,
        &mut transactions,
    )
    .await
    .unwrap();

    assert!(dropped.is_empty(), "{dropped:?}");
    assert_eq!(transactions.len(), 1, "non-denied transaction must be kept");
    assert_eq!(
        locks.len(),
        2,
        "kept transaction must lock its owned inputs"
    );
}

/// With `deny_rule_governance` disabled, the same active set is ignored and
/// validation falls back to the (empty) local config.
#[sim_test]
async fn post_consensus_validation_uses_local_config_when_disabled() {
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let recipient = Address::random();
    let object_id = ObjectId::random();
    let gas_id = ObjectId::random();
    let (authority, _) = init_state_with_objects_and_object_basics(vec![
        Object::with_id_owner_for_testing(object_id, sender),
        Object::with_id_owner_for_testing(gas_id, sender),
    ])
    .await;
    let epoch_store = authority.epoch_store_for_testing();

    activate_deny_rules(
        &epoch_store,
        iota_sdk_types::DenyRuleSet {
            denied_addresses: [sender].into(),
            ..Default::default()
        },
        1,
    );

    let object_ref = authority.get_object(&object_id).unwrap().object_ref();
    let gas_ref = authority.get_object(&gas_id).unwrap().object_ref();
    let rgp = authority.reference_gas_price_for_testing().unwrap();
    let tx =
        make_transfer_object_transaction(object_ref, gas_ref, sender, &sender_key, recipient, rgp);
    let mut transactions = vec![make_user_tx_v1(tx)];

    let (dropped, locks, _) = post_consensus_validation::validate_and_resolve_conflicts(
        &authority,
        &epoch_store,
        &mut transactions,
    )
    .await
    .unwrap();

    assert!(dropped.is_empty());
    assert_eq!(transactions.len(), 1);
    assert_eq!(locks.len(), 2);
}

/// A sender denied by the active set is dropped, and after a newer-generation
/// proposal withdraws the rules the same sender's transaction is kept.
#[sim_test]
async fn post_consensus_validation_applies_relaxed_rules() {
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config.set_deny_rule_governance_for_testing(true);
        config
    });

    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let recipient = Address::random();
    let object_id = ObjectId::random();
    let gas_id = ObjectId::random();
    let (authority, _) = init_state_with_objects_and_object_basics(vec![
        Object::with_id_owner_for_testing(object_id, sender),
        Object::with_id_owner_for_testing(gas_id, sender),
    ])
    .await;
    let epoch_store = authority.epoch_store_for_testing();

    activate_deny_rules(
        &epoch_store,
        iota_sdk_types::DenyRuleSet {
            denied_addresses: [sender].into(),
            ..Default::default()
        },
        1,
    );

    let object_ref = authority.get_object(&object_id).unwrap().object_ref();
    let gas_ref = authority.get_object(&gas_id).unwrap().object_ref();
    let rgp = authority.reference_gas_price_for_testing().unwrap();
    let tx =
        make_transfer_object_transaction(object_ref, gas_ref, sender, &sender_key, recipient, rgp);
    let mut transactions = vec![make_user_tx_v1(tx)];

    let (dropped, locks, _) = post_consensus_validation::validate_and_resolve_conflicts(
        &authority,
        &epoch_store,
        &mut transactions,
    )
    .await
    .unwrap();
    assert_eq!(dropped.len(), 1, "denied sender must be dropped");
    assert!(locks.is_empty());

    // Withdraw the rules with a newer-generation empty proposal.
    activate_deny_rules(&epoch_store, Default::default(), 2);

    // The dropped transaction did not execute, so the same object refs are
    // still current for a fresh transaction (distinct digest via a new
    // recipient) from the no-longer-denied sender.
    let recipient = Address::random();
    let tx =
        make_transfer_object_transaction(object_ref, gas_ref, sender, &sender_key, recipient, rgp);
    let mut transactions = vec![make_user_tx_v1(tx)];

    let (dropped, locks, _) = post_consensus_validation::validate_and_resolve_conflicts(
        &authority,
        &epoch_store,
        &mut transactions,
    )
    .await
    .unwrap();
    assert!(dropped.is_empty(), "{dropped:?}");
    assert_eq!(
        transactions.len(),
        1,
        "previously denied sender must be kept after withdrawal"
    );
    assert_eq!(locks.len(), 2);
}

// ---------------------------------------------------------------------------
// P-COOL deterministic-validation bookkeeping (execution hook)
// ---------------------------------------------------------------------------

/// Shared setup for the execution-hook bookkeeping tests: the P-COOL flags
/// (held by `_config_guard` for the test's duration when enabled), plus
/// helpers to execute transactions and assert the bookkeeping rows they
/// leave. Execution provenance never matters to the hook - only whether the
/// digest is registered in the digest -> commit-round map (handler-known)
/// or not (sync-ahead).
struct BookkeepingSetup {
    authority: Arc<crate::authority::AuthorityState>,
    epoch_store: Arc<crate::authority::authority_per_epoch_store::AuthorityPerEpochStore>,
    package_id: ObjectId,
    rgp: u64,
    _config_guard: Option<OverrideGuard>,
}

async fn setup_bookkeeping(
    genesis_objects: Vec<Object>,
    validation_enabled: bool,
) -> BookkeepingSetup {
    let _config_guard = validation_enabled.then(|| {
        ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_enable_pcool_flow_for_testing(true);
            config.set_pcool_deterministic_validation_for_testing(true);
            config
        })
    });

    let (authority, package) = init_state_with_objects_and_object_basics(genesis_objects).await;
    let epoch_store = (*authority.epoch_store_for_testing()).clone();
    let rgp = authority.reference_gas_price_for_testing().unwrap();
    BookkeepingSetup {
        authority,
        epoch_store,
        package_id: package.object_id,
        rgp,
        _config_guard,
    }
}

/// Like [`setup_bookkeeping`] with the flags on, but publishing the given
/// test package instead of `object_basics`; `gas_id` is created at genesis
/// for `sender` and funds the publish and the test's transactions.
async fn setup_bookkeeping_with_package(
    package_name: &str,
    sender: Address,
    sender_key: &AccountPrivateKey,
    gas_id: ObjectId,
) -> BookkeepingSetup {
    let _config_guard = Some(ProtocolConfig::apply_overrides_for_testing(
        |_, mut config| {
            config.set_enable_pcool_flow_for_testing(true);
            config.set_pcool_deterministic_validation_for_testing(true);
            config
        },
    ));

    let authority = TestAuthorityBuilder::new().build().await;
    authority.insert_genesis_object(Object::with_id_owner_for_testing(gas_id, sender));
    let (package, _) = build_and_publish_test_package_with_upgrade_cap(
        &authority,
        &sender,
        sender_key,
        &gas_id,
        package_name,
        false,
    )
    .await;
    let epoch_store = (*authority.epoch_store_for_testing()).clone();
    let rgp = authority.reference_gas_price_for_testing().unwrap();
    BookkeepingSetup {
        authority,
        epoch_store,
        package_id: package.object_id,
        rgp,
        _config_guard,
    }
}

/// From the `tto::M1::start` effects, the `(parent, child)` pair where the
/// child is address-owned by the parent object's id - the receivable shape.
fn parent_and_child(
    created: Vec<OwnedObjectReference>,
) -> (OwnedObjectReference, OwnedObjectReference) {
    created
        .iter()
        .find_map(|child| match child.owner {
            Owner::Address(addr) => created
                .iter()
                .find(|parent| parent.reference.object_id == ObjectId::from(addr))
                .map(|parent| (*parent, *child)),
            _ => None,
        })
        .expect("start() creates a child owned by another created object's id")
}

impl BookkeepingSetup {
    /// The latest reference of `id` in the authority's view.
    fn latest_ref(&self, id: &ObjectId) -> ObjectReference {
        self.authority.get_object(id).unwrap().object_ref()
    }

    /// A keyed read of `(id, version)` from the object store, the way
    /// validation's content read would issue it; `None` for tombstones.
    fn store_object(&self, id: &ObjectId, version: Version) -> Option<Object> {
        self.authority
            .get_object_cache_reader()
            .get_object_by_key(id, version)
    }

    /// Executes `tx` directly (`new_from_checkpoint` merely avoids needing a
    /// certificate). Unless the digest was registered in the digest ->
    /// commit-round map beforehand, the hook classifies it sync-ahead.
    fn execute(&self, tx: VerifiedTransaction) -> TransactionEffects {
        let effects = self.execute_unchecked(tx);
        assert!(effects.status().is_success(), "{:?}", effects.status());
        effects
    }

    /// [`Self::execute`] without asserting execution success, for scenarios
    /// exercising aborted transactions.
    fn execute_unchecked(&self, tx: VerifiedTransaction) -> TransactionEffects {
        let executable =
            VerifiedExecutableTransaction::new_from_checkpoint(tx, self.epoch_store.epoch(), 1);
        let (effects, _) = self
            .authority
            .try_execute_immediately(&executable, ExecutionEnv::new(), &self.epoch_store)
            .unwrap();
        effects
    }

    /// Registers `tx`'s digest in the digest -> commit-round map for `round`
    /// before executing - the handler-known classification, under which the
    /// hook writes handler-latest rows instead of sync-ahead records.
    fn execute_as_handler_known(
        &self,
        tx: VerifiedTransaction,
        round: CommitRound,
    ) -> TransactionEffects {
        self.epoch_store
            .assign_commit_to_transactions(round, vec![*tx.digest()]);
        self.execute(tx)
    }

    /// A verified call into `module::function` of the published test package
    /// at the gas coin's latest version.
    fn build_move_call(
        &self,
        module: &'static str,
        function: &'static str,
        args: Vec<CallArg>,
        gas_id: &ObjectId,
        sender: Address,
        sender_key: &AccountPrivateKey,
    ) -> VerifiedTransaction {
        let tx = Transaction::new_move_call(
            sender,
            self.package_id,
            Identifier::from_static(module),
            Identifier::from_static(function),
            vec![],
            self.latest_ref(gas_id),
            args,
            TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS * self.rgp,
            self.rgp,
        )
        .unwrap();
        let tx = to_sender_signed_transaction(tx, sender_key);
        self.epoch_store.verify_transaction(tx).unwrap()
    }

    /// Builds a call into `module::function` of the published test package at
    /// the gas coin's latest version and executes it.
    fn move_call(
        &self,
        module: &'static str,
        function: &'static str,
        args: Vec<CallArg>,
        gas_id: &ObjectId,
        sender: Address,
        sender_key: &AccountPrivateKey,
    ) -> TransactionEffects {
        self.execute(self.build_move_call(module, function, args, gas_id, sender, sender_key))
    }

    /// Builds a call into the `object_basics` module and executes it through
    /// the certificate + consensus path.
    async fn shared_object_basics_call(
        &self,
        function: &'static str,
        args: Vec<TestCallArg>,
        gas_id: &ObjectId,
        sender: Address,
        sender_key: &AccountPrivateKey,
    ) -> TransactionEffects {
        let effects = call_move_(
            &self.authority,
            None,
            gas_id,
            &sender,
            sender_key,
            &self.package_id,
            "object_basics",
            function,
            vec![],
            args,
            true, // the call takes shared-object inputs
        )
        .await
        .unwrap();
        assert!(effects.status().is_success(), "{:?}", effects.status());

        effects
    }

    /// [`Self::move_call`] into the `object_basics` module.
    fn object_basics_call(
        &self,
        function: &'static str,
        args: Vec<CallArg>,
        gas_id: &ObjectId,
        sender: Address,
        sender_key: &AccountPrivateKey,
    ) -> TransactionEffects {
        self.move_call("object_basics", function, args, gas_id, sender, sender_key)
    }

    /// Creates a fresh `object_basics::Object` owned by `sender`;
    /// returns the created reference and the effects.
    fn create_object(
        &self,
        gas_id: &ObjectId,
        sender: Address,
        sender_key: &AccountPrivateKey,
    ) -> (ObjectReference, TransactionEffects) {
        let effects = self.object_basics_call(
            "create",
            vec![
                CallArg::Pure(bcs::to_bytes(&16u64).unwrap()),
                CallArg::Pure(bcs::to_bytes(&sender).unwrap()),
            ],
            gas_id,
            sender,
            sender_key,
        );
        (effects.created()[0].reference, effects)
    }

    /// Builds a transfer of `object_id` to `recipient` at the objects' latest
    /// versions, paid by `gas_id`, and executes it.
    fn transfer(
        &self,
        object_id: &ObjectId,
        gas_id: &ObjectId,
        sender: Address,
        sender_key: &AccountPrivateKey,
        recipient: Address,
    ) -> TransactionEffects {
        let tx = make_transfer_object_transaction(
            self.latest_ref(object_id),
            self.latest_ref(gas_id),
            sender,
            sender_key,
            recipient,
            self.rgp,
        );
        self.execute(self.epoch_store.verify_transaction(tx).unwrap())
    }

    /// Asserts `id`'s sync-ahead record is exactly `{ base_version,
    /// latest_created }`. `base_version` is the version the chain grew from
    /// (`None` when the chain itself created the id) and never changes once
    /// the record exists; `latest_created` is the chain's head.
    #[track_caller]
    fn assert_record(&self, id: &ObjectId, base_version: Option<Version>, latest_created: Version) {
        assert_eq!(
            self.epoch_store.sync_record(id).unwrap(),
            Some(SyncAheadRecord {
                base_version,
                latest_created,
            }),
            "sync-ahead record mismatch for {id:?}"
        );
    }

    /// Asserts the version consumed as `consumed_ref` is sheltered with the
    /// consumed bytes.
    #[track_caller]
    fn assert_sheltered(&self, consumed_ref: ObjectReference) {
        let digest = consumed_ref.digest;
        let sheltered = self
            .epoch_store
            .sheltered_object(&ObjectKey::from(consumed_ref))
            .unwrap()
            .expect("a version consumed by sync execution must be sheltered");
        assert_eq!(
            sheltered.digest(),
            digest,
            "sheltered bytes must match the consumed version"
        );
    }

    #[track_caller]
    fn assert_not_sheltered(&self, reference: ObjectReference) {
        assert!(
            self.epoch_store
                .sheltered_object(&ObjectKey::from(reference))
                .unwrap()
                .is_none(),
            "a version no sync execution consumed must not be sheltered"
        );
    }
}

#[tokio::test]
async fn executed_transaction_updates_sync_ahead_bookkeeping() {
    let (address_1, address_1_key): (Address, AccountPrivateKey) = get_key_pair();
    let (address_2, address_2_key): (Address, AccountPrivateKey) = get_key_pair();
    let obj_id = ObjectId::random();
    let gas1_id = ObjectId::random();
    let gas2_id = ObjectId::random();

    let s = setup_bookkeeping(
        vec![
            Object::with_id_owner_for_testing(obj_id, address_1),
            Object::with_id_owner_for_testing(gas1_id, address_1),
            Object::with_id_owner_for_testing(gas2_id, address_2),
        ],
        true,
    )
    .await;

    // The first sync-executed transfer consumes the genesis versions: every
    // written object gets a sync-ahead record based at the version it
    // consumed, and no handler-latest row.
    let obj_genesis_ref = s.latest_ref(&obj_id);
    let gas1_ref = s.latest_ref(&gas1_id);
    let first = s.transfer(&obj_id, &gas1_id, address_1, &address_1_key, address_2);

    s.assert_record(&gas1_id, Some(gas1_ref.version), first.lamport_version());
    s.assert_record(
        &obj_id,
        Some(obj_genesis_ref.version),
        first.lamport_version(),
    );
    assert_eq!(s.epoch_store.handler_latest(&obj_id).unwrap(), None);

    // A second transfer extends the object's chain: the base stays the
    // version the chain originally grew from while the chain head advances.
    let obj_v1_ref = s.latest_ref(&obj_id);
    let gas2_ref = s.latest_ref(&gas2_id);
    let second = s.transfer(&obj_id, &gas2_id, address_2, &address_2_key, address_1);

    s.assert_record(&gas2_id, Some(gas2_ref.version), second.lamport_version());
    s.assert_record(
        &obj_id,
        Some(obj_genesis_ref.version),
        second.lamport_version(),
    );

    // Both consumed versions are sheltered with their bytes; the live latest
    // version is not.
    s.assert_sheltered(obj_genesis_ref);
    s.assert_sheltered(obj_v1_ref);
    s.assert_not_sheltered(s.latest_ref(&obj_id));
}

#[tokio::test]
async fn handler_known_transaction_writes_handler_latest_only() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let obj_id = ObjectId::random();
    let gas_id = ObjectId::random();

    let s = setup_bookkeeping(
        vec![
            Object::with_id_owner_for_testing(obj_id, sender),
            Object::with_id_owner_for_testing(gas_id, sender),
        ],
        true,
    )
    .await;

    let obj_genesis_ref = s.latest_ref(&obj_id);
    let tx = make_transfer_object_transaction(
        obj_genesis_ref,
        s.latest_ref(&gas_id),
        sender,
        &sender_key,
        Address::random(),
        s.rgp,
    );
    let tx = s.epoch_store.verify_transaction(tx).unwrap();

    // The handler registered the digest before execution: the hook writes
    // handler-latest rows and neither sync records nor shelter bytes.
    let effects = s.execute_as_handler_known(tx, 7);

    let row = s
        .epoch_store
        .handler_latest(&obj_id)
        .unwrap()
        .expect("a handler-known execution must write a handler-latest row");
    assert_eq!(row.version, effects.lamport_version());
    assert_eq!(row.produced_at, 7);
    assert_eq!(row.kind, HandlerLatestObjectKind::Live);

    assert_eq!(s.epoch_store.sync_record(&obj_id).unwrap(), None);
    s.assert_not_sheltered(obj_genesis_ref);
}

#[tokio::test]
async fn bookkeeping_disabled_writes_nothing() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let obj_id = ObjectId::random();
    let gas_id = ObjectId::random();

    let s = setup_bookkeeping(
        vec![
            Object::with_id_owner_for_testing(obj_id, sender),
            Object::with_id_owner_for_testing(gas_id, sender),
        ],
        false,
    )
    .await;

    let obj_genesis_ref = s.latest_ref(&obj_id);
    s.transfer(&obj_id, &gas_id, sender, &sender_key, Address::random());

    assert_eq!(s.epoch_store.handler_latest(&obj_id).unwrap(), None);
    assert_eq!(s.epoch_store.sync_record(&obj_id).unwrap(), None);
    s.assert_not_sheltered(obj_genesis_ref);
}

#[tokio::test]
async fn sync_ahead_created_object_has_no_base_version() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();

    let s = setup_bookkeeping(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let (created_ref, effects) = s.create_object(&gas_id, sender, &sender_key);

    // `None` marks an id the sync-ahead chain itself created: no named
    // version of it may answer keep at validation.
    s.assert_record(created_ref.object_id(), None, effects.lamport_version());
    s.assert_not_sheltered(created_ref);
}

#[tokio::test]
async fn sync_ahead_delete_shelters_the_consumed_version() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();

    let s = setup_bookkeeping(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let (created_ref, _) = s.create_object(&gas_id, sender, &sender_key);
    let delete_effects = s.object_basics_call(
        "delete",
        vec![CallArg::ImmutableOrOwned(created_ref)],
        &gas_id,
        sender,
        &sender_key,
    );

    // The deletion consumes the created version: its bytes are sheltered and
    // the record's head advances to the tombstone version. The base never
    // changes once the record exists - it stays `None` because this chain
    // created the id instead of consuming a pre-existing version.
    s.assert_sheltered(created_ref);
    s.assert_record(
        created_ref.object_id(),
        None,
        delete_effects.lamport_version(),
    );

    // A keyed store read at the delete-tombstone version answers `None`; the
    // consumed pre-delete bytes are reachable only through the shelter.
    assert!(
        s.store_object(created_ref.object_id(), delete_effects.lamport_version())
            .is_none()
    );
}

#[tokio::test]
async fn sync_ahead_wrapped_object_is_sheltered_and_reappears_on_unwrap() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();

    let s = setup_bookkeeping(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let (created_ref, _) = s.create_object(&gas_id, sender, &sender_key);
    let wrap_effects = s.object_basics_call(
        "wrap",
        vec![CallArg::ImmutableOrOwned(created_ref)],
        &gas_id,
        sender,
        &sender_key,
    );

    // Wrapping consumes the object's version into the new wrapper: the bytes
    // are sheltered and the wrapped id's record head advances to the
    // tombstone version. The wrapper is a new id created by this chain, so
    // its record starts with `base_version: None`.
    s.assert_sheltered(created_ref);
    s.assert_record(
        created_ref.object_id(),
        None,
        wrap_effects.lamport_version(),
    );

    let wrapper_ref = wrap_effects.created()[0].reference;
    s.assert_record(
        wrapper_ref.object_id(),
        None,
        wrap_effects.lamport_version(),
    );

    // A keyed store read at the wrap-tombstone version answers `None`: the
    // bytes live only inside the wrapper and, for validation, in the shelter.
    assert!(
        s.store_object(created_ref.object_id(), wrap_effects.lamport_version())
            .is_none()
    );

    // Unwrapping deletes the wrapper and resurfaces the SAME object id at a
    // higher version: the id's record extends across the wrap tombstone
    // (base untouched, head at the unwrap version), the consumed wrapper
    // version is sheltered, and the live unwrapped version is not.
    let unwrap_effects = s.object_basics_call(
        "unwrap",
        vec![CallArg::ImmutableOrOwned(wrapper_ref)],
        &gas_id,
        sender,
        &sender_key,
    );

    let unwrapped_ref = unwrap_effects.unwrapped()[0].reference;
    assert_eq!(unwrapped_ref.object_id(), created_ref.object_id());
    assert!(unwrapped_ref.version > created_ref.version);

    s.assert_record(
        created_ref.object_id(),
        None,
        unwrap_effects.lamport_version(),
    );

    // The wrapper's own chain now ends in its deletion: consumed by the
    // unwrap, its record head advances to the unwrap version.
    s.assert_sheltered(wrapper_ref);
    s.assert_record(
        wrapper_ref.object_id(),
        None,
        unwrap_effects.lamport_version(),
    );

    s.assert_not_sheltered(unwrapped_ref);

    // The unwrapped live version answers a keyed store read again.
    assert!(
        s.store_object(unwrapped_ref.object_id(), unwrapped_ref.version)
            .is_some()
    );
}

#[tokio::test]
async fn sync_ahead_creations_of_every_owner_kind_get_records() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_bookkeeping_with_package("tto", sender, &sender_key, gas_id).await;

    let gas_ref_before = s.latest_ref(&gas_id);
    let start_effects = s.move_call("M1", "start", vec![], &gas_id, sender, &sender_key);

    // `start` creates an address-owned object, a receivable child, a frozen
    // object, a shared object, and a dynamic object field child with its `Field`
    // wrapper. Every creation gets a sync-ahead record with no base,
    // whatever its owner kind: a sync-created object must answer missing at
    // validation, never read as pre-epoch state. None is sheltered - nothing
    // was consumed at these ids.
    for created in start_effects.created() {
        assert_eq!(
            s.epoch_store
                .sync_record(created.reference.object_id())
                .unwrap(),
            Some(SyncAheadRecord {
                base_version: None,
                latest_created: start_effects.lamport_version(),
            }),
            "sync-ahead record mismatch for {:?} (owner {:?})",
            created.reference.object_id(),
            created.owner
        );
        s.assert_not_sheltered(created.reference);
    }

    // The fixture really spans the owner kinds.
    let owners: Vec<_> = start_effects.created().iter().map(|o| o.owner).collect();
    assert!(owners.iter().any(|o| matches!(o, Owner::Shared(_))));
    assert!(owners.iter().any(|o| matches!(o, Owner::Immutable)));
    assert!(owners.iter().any(|o| matches!(o, Owner::Object(_))));
    assert!(owners.iter().any(|o| matches!(o, Owner::Address(_))));

    // The gas coin is the only consumed input: its version is sheltered.
    s.assert_sheltered(gas_ref_before);
}

#[tokio::test]
async fn sync_ahead_removed_dynamic_field_shelters_the_runtime_loaded_child() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let (parent_ref, _) = s.create_object(&gas_id, sender, &sender_key);
    let (value_ref, _) = s.create_object(&gas_id, sender, &sender_key);

    // `add_field` wraps the value into a `Field` child object hanging off the
    // parent's id; the child is the only creation and is object-owned.
    let add_effects = s.object_basics_call(
        "add_field",
        vec![
            CallArg::ImmutableOrOwned(parent_ref),
            CallArg::ImmutableOrOwned(value_ref),
        ],
        &gas_id,
        sender,
        &sender_key,
    );
    let field_ref = add_effects.created()[0].reference;
    assert!(matches!(add_effects.created()[0].owner, Owner::Object(_)));

    // The field was created, not consumed: nothing to shelter yet.
    s.assert_not_sheltered(field_ref);
    let parent_before_remove = s.latest_ref(parent_ref.object_id());

    // `remove_field` declares only the parent (and gas) as inputs: the
    // `Field` child is loaded at runtime, deleted, and its value unwrapped.
    // The consumed child version must be sheltered through the store
    // fallback, and its tombstone version answers no store read.
    let remove_effects = s.object_basics_call(
        "remove_field",
        vec![CallArg::ImmutableOrOwned(parent_before_remove)],
        &gas_id,
        sender,
        &sender_key,
    );

    s.assert_sheltered(field_ref);
    s.assert_record(
        field_ref.object_id(),
        None,
        remove_effects.lamport_version(),
    );
    assert!(
        s.store_object(field_ref.object_id(), remove_effects.lamport_version())
            .is_none()
    );

    // The parent is a declared input of both steps: its consumed versions are
    // sheltered through the primary (declared-inputs) leg.
    s.assert_sheltered(parent_ref);
    s.assert_sheltered(parent_before_remove);
    // The value object resurfaces from the field via unwrap: same id, record
    // head at the remove version.
    let unwrapped_value = remove_effects.unwrapped()[0].reference;
    assert_eq!(unwrapped_value.object_id(), value_ref.object_id());
    s.assert_record(
        value_ref.object_id(),
        None,
        remove_effects.lamport_version(),
    );
}

#[tokio::test]
async fn sync_ahead_removed_dynamic_object_field_shelters_child_and_wrapper() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let (parent_ref, _) = s.create_object(&gas_id, sender, &sender_key);
    let (value_ref, _) = s.create_object(&gas_id, sender, &sender_key);

    // `add_ofield` keeps the value a live object: it creates a `Field`
    // wrapper holding the value's id and re-owns the value object-to-object.
    let add_effects = s.object_basics_call(
        "add_ofield",
        vec![
            CallArg::ImmutableOrOwned(parent_ref),
            CallArg::ImmutableOrOwned(value_ref),
        ],
        &gas_id,
        sender,
        &sender_key,
    );
    let field_wrapper_ref = add_effects.created()[0].reference;
    let value_after_add = add_effects
        .mutated()
        .into_iter()
        .find(|m| m.reference.object_id == *value_ref.object_id())
        .expect("adding the object field mutates the value object");
    assert!(matches!(value_after_add.owner, Owner::Object(_)));

    // `remove_ofield` declares only the parent (and gas): BOTH the `Field`
    // wrapper (deleted) and the value object (mutated back to address-owned)
    // are loaded at runtime - two consumed versions sheltered through the
    // store fallback.
    let remove_effects = s.object_basics_call(
        "remove_ofield",
        vec![CallArg::ImmutableOrOwned(
            s.latest_ref(parent_ref.object_id()),
        )],
        &gas_id,
        sender,
        &sender_key,
    );

    s.assert_sheltered(field_wrapper_ref);
    s.assert_sheltered(value_after_add.reference);
    s.assert_record(
        value_ref.object_id(),
        None,
        remove_effects.lamport_version(),
    );

    // The value is live and address-owned again at its new version.
    let value_now = s.latest_ref(value_ref.object_id());
    assert_eq!(value_now.version, remove_effects.lamport_version());
    assert!(
        s.store_object(value_ref.object_id(), value_now.version)
            .is_some()
    );
}

#[tokio::test]
async fn sync_ahead_received_object_is_sheltered_from_the_runtime_load() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_bookkeeping_with_package("tto", sender, &sender_key, gas_id).await;

    let start_effects = s.move_call("M1", "start", vec![], &gas_id, sender, &sender_key);
    let (parent, child) = parent_and_child(start_effects.created());

    // The receive names the child only as a `Receiving` argument: it is not a
    // declared input, so execution loads it from the store at runtime and the
    // hook must fetch the consumed bytes the same way (the store fallback at
    // the call site). `M1::receiver` transfers the received child onward.
    let receive_effects = s.move_call(
        "M1",
        "receiver",
        vec![
            CallArg::ImmutableOrOwned(parent.reference),
            CallArg::Receiving(child.reference),
        ],
        &gas_id,
        sender,
        &sender_key,
    );

    // The base stays `None`: the child was created by this same sync-ahead
    // chain (the `start` call), and the receive only extends the chain.
    s.assert_sheltered(child.reference);
    s.assert_record(
        child.reference.object_id(),
        None,
        receive_effects.lamport_version(),
    );

    // The parent was mutated through its `&mut` argument: a declared input,
    // consumed and sheltered like any other.
    s.assert_sheltered(parent.reference);
    s.assert_record(
        parent.reference.object_id(),
        None,
        receive_effects.lamport_version(),
    );
}

#[tokio::test]
async fn sync_ahead_shared_mutation_writes_nothing_and_deletion_is_recorded() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    // Creating the shared object is recorded like any creation.
    let share_effects = s.object_basics_call("share", vec![], &gas_id, sender, &sender_key);
    let shared_ref = share_effects.created()[0].reference;
    assert!(matches!(share_effects.created()[0].owner, Owner::Shared(_)));
    s.assert_record(
        shared_ref.object_id(),
        None,
        share_effects.lamport_version(),
    );

    // Mutating the shared input writes nothing: the record stays where the
    // creation left it (a busy shared object like the Clock would otherwise
    // rewrite its record every commit), and shared inputs are never
    // sheltered. No check consults shared state beyond existence, creation,
    // and deletion.
    s.shared_object_basics_call(
        "set_value",
        vec![
            TestCallArg::Object(*shared_ref.object_id()),
            TestCallArg::Pure(bcs::to_bytes(&42u64).unwrap()),
        ],
        &gas_id,
        sender,
        &sender_key,
    )
    .await;
    s.assert_record(
        shared_ref.object_id(),
        None,
        share_effects.lamport_version(),
    );
    s.assert_not_sheltered(shared_ref);

    // Deleting the shared object IS recorded - deletion is one of the shared
    // facts validation consults - but the consumed shared version is still
    // not sheltered.
    let shared_before_delete = s.latest_ref(shared_ref.object_id());
    let delete_effects = s
        .shared_object_basics_call(
            "delete",
            vec![TestCallArg::Object(*shared_ref.object_id())],
            &gas_id,
            sender,
            &sender_key,
        )
        .await;
    s.assert_record(
        shared_ref.object_id(),
        None,
        delete_effects.lamport_version(),
    );
    s.assert_not_sheltered(shared_before_delete);
    assert!(
        s.store_object(shared_ref.object_id(), delete_effects.lamport_version())
            .is_none()
    );
}

#[tokio::test]
async fn aborted_transaction_still_records_and_shelters_its_gas() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let gas_genesis_version = s.latest_ref(&gas_id).version;
    let (parent_ref, _) = s.create_object(&gas_id, sender, &sender_key);
    let gas_before = s.latest_ref(&gas_id);

    // `remove_field` aborts: no field was ever added to the parent.
    let effects = s.execute_unchecked(s.build_move_call(
        "object_basics",
        "remove_field",
        vec![CallArg::ImmutableOrOwned(
            s.latest_ref(parent_ref.object_id()),
        )],
        &gas_id,
        sender,
        &sender_key,
    ));
    assert!(!effects.status().is_success());

    // The aborted execution still consumed and rewrote the gas coin: its
    // consumed version is recorded and sheltered like any other write.
    s.assert_sheltered(gas_before);
    s.assert_record(
        &gas_id,
        Some(gas_genesis_version),
        effects.lamport_version(),
    );
}

#[tokio::test]
async fn smashed_gas_coin_is_recorded_and_sheltered() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas1_id = ObjectId::random();
    let gas2_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![
            Object::with_id_owner_for_testing(gas1_id, sender),
            Object::with_id_owner_for_testing(gas2_id, sender),
        ],
        true,
    )
    .await;

    let gas1_ref = s.latest_ref(&gas1_id);
    let gas2_ref = s.latest_ref(&gas2_id);

    // Paying with two coins smashes them: the second is merged into the
    // first and deleted, without ever being named by a command.
    let mut builder = ProgrammableTransactionBuilder::new();
    builder.command(Command::new_move_call(
        s.package_id,
        Identifier::from_static("object_basics"),
        Identifier::from_static("share"),
        vec![],
        vec![],
    ));
    let tx = Transaction::new_programmable(
        sender,
        vec![gas1_ref, gas2_ref],
        builder.finish(),
        TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS * s.rgp,
        s.rgp,
    );
    let effects = s.execute(
        s.epoch_store
            .verify_transaction(to_sender_signed_transaction(tx, &sender_key))
            .unwrap(),
    );
    assert_eq!(effects.deleted().len(), 1);
    assert_eq!(effects.deleted()[0].object_id, *gas2_ref.object_id());

    // Both coins were consumed - the survivor mutated, the smashed one
    // deleted - so both versions are recorded and sheltered.
    s.assert_sheltered(gas1_ref);
    s.assert_sheltered(gas2_ref);
    s.assert_record(&gas2_id, Some(gas2_ref.version), effects.lamport_version());
}

#[tokio::test]
async fn sync_published_package_is_recorded_but_never_sheltered() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    // Unlike `setup_bookkeeping` (whose `object_basics` package is inserted
    // at genesis and thus correctly carries no record), this setup publishes
    // the package through a real transaction, with the flags on and the
    // digest unknown to the round map.
    let s = setup_bookkeeping_with_package("tto", sender, &sender_key, gas_id).await;

    // A sync-published package must answer missing at validation - never
    // read as pre-epoch state - so its creation is recorded. Like any
    // creation, nothing is sheltered at its id.
    let package_ref = s.latest_ref(&s.package_id);
    s.assert_record(&s.package_id, None, package_ref.version);
    s.assert_not_sheltered(package_ref);
}

#[tokio::test]
async fn sync_ahead_received_wrapper_is_sheltered_and_unwraps_its_content() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_bookkeeping_with_package("tto", sender, &sender_key, gas_id).await;

    // `M2::start` creates the parent and a receivable wrapper `C { wrapped: B }`.
    // `B` is created and wrapped within the same transaction, so it never gets
    // a store row or an effects entry - only the two outer objects appear.
    let start_effects = s.move_call("M2", "start", vec![], &gas_id, sender, &sender_key);
    assert_eq!(start_effects.created().len(), 2);
    let (parent, wrapper) = parent_and_child(start_effects.created());

    // `unwrap_receiver` receives the wrapper (a runtime load - not a declared
    // input), destructures it, transfers the inner object onward, and deletes
    // the wrapper's id.
    let unwrap_effects = s.move_call(
        "M2",
        "unwrap_receiver",
        vec![
            CallArg::ImmutableOrOwned(parent.reference),
            CallArg::Receiving(wrapper.reference),
        ],
        &gas_id,
        sender,
        &sender_key,
    );

    // The consumed wrapper version is sheltered through the store fallback;
    // since the inner object's bytes live inside the wrapper's, they are
    // sheltered transitively - the only copy of them on this path.
    s.assert_sheltered(wrapper.reference);
    s.assert_record(
        wrapper.reference.object_id(),
        None,
        unwrap_effects.lamport_version(),
    );
    assert!(
        s.store_object(
            wrapper.reference.object_id(),
            unwrap_effects.lamport_version()
        )
        .is_none()
    );

    // The inner object surfaces for the first time as `unwrapped`: its very
    // first store row is this version, so its record starts here - no base,
    // nothing sheltered at its id.
    let inner = unwrap_effects.unwrapped()[0].reference;
    s.assert_record(inner.object_id(), None, unwrap_effects.lamport_version());
    s.assert_not_sheltered(inner);
    assert!(s.store_object(inner.object_id(), inner.version).is_some());
}

#[tokio::test]
async fn sync_ahead_received_then_deleted_object_is_sheltered() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_bookkeeping_with_package("tto", sender, &sender_key, gas_id).await;

    let start_effects = s.move_call("M1", "start", vec![], &gas_id, sender, &sender_key);
    let (parent, child) = parent_and_child(start_effects.created());

    // `M1::deleter` receives the child and destroys it: the consumed version
    // is sheltered from the runtime load, the record head advances to the
    // delete tombstone, and the tombstone version answers no store read.
    let delete_effects = s.move_call(
        "M1",
        "deleter",
        vec![
            CallArg::ImmutableOrOwned(parent.reference),
            CallArg::Receiving(child.reference),
        ],
        &gas_id,
        sender,
        &sender_key,
    );

    s.assert_sheltered(child.reference);
    s.assert_record(
        child.reference.object_id(),
        None,
        delete_effects.lamport_version(),
    );
    assert!(
        s.store_object(
            child.reference.object_id(),
            delete_effects.lamport_version()
        )
        .is_none()
    );
}

#[tokio::test]
async fn immutable_input_read_does_not_extend_record_or_shelter() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();

    let s = setup_bookkeeping(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let (mutated_ref, _) = s.create_object(&gas_id, sender, &sender_key);
    let (frozen_id_ref, _) = s.create_object(&gas_id, sender, &sender_key);
    let freeze_effects = s.object_basics_call(
        "freeze_object",
        vec![CallArg::ImmutableOrOwned(frozen_id_ref)],
        &gas_id,
        sender,
        &sender_key,
    );
    let frozen_ref = s.latest_ref(frozen_id_ref.object_id());

    let update_effects = s.object_basics_call(
        "update",
        vec![
            CallArg::ImmutableOrOwned(mutated_ref),
            CallArg::ImmutableOrOwned(frozen_ref),
        ],
        &gas_id,
        sender,
        &sender_key,
    );

    // The read-only immutable input is not consumed: no shelter row, and its
    // record head stays where the freeze left it. Only the mutated object's
    // chain advances, sheltering the version the update consumed.
    s.assert_not_sheltered(frozen_ref);
    s.assert_record(
        frozen_id_ref.object_id(),
        None,
        freeze_effects.lamport_version(),
    );

    s.assert_sheltered(mutated_ref);
    s.assert_record(
        mutated_ref.object_id(),
        None,
        update_effects.lamport_version(),
    );
}

// ---------------------------------------------------------------------------
// Immutable object inputs (issue #12602)
// ---------------------------------------------------------------------------

/// A state with one immutable object, two gas coins and one spare owned object,
/// plus builders for transactions that read the immutable object.
struct ImmutableInputSetup {
    authority: Arc<crate::authority::AuthorityState>,
    epoch_store: Arc<crate::authority::authority_per_epoch_store::AuthorityPerEpochStore>,
    sender: Address,
    sender_key: AccountPrivateKey,
    package_ref: ObjectReference,
    immutable_ref: ObjectReference,
    gas1_ref: ObjectReference,
    gas2_ref: ObjectReference,
    owned_ref: ObjectReference,
    rgp: u64,
    _config_guard: OverrideGuard,
}

/// Builds the state above with the P-COOL flow on and the immutable-lock skip
/// set to `skip_immutable_locks`.
async fn setup_immutable_input(skip_immutable_locks: bool) -> ImmutableInputSetup {
    let _config_guard = ProtocolConfig::apply_overrides_for_testing(move |_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config.set_pcool_skip_immutable_object_locks_for_testing(skip_immutable_locks);
        config
    });

    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();

    let immutable_id = ObjectId::random();
    let gas1_id = ObjectId::random();
    let gas2_id = ObjectId::random();
    let owned_id = ObjectId::random();

    let (authority, package_ref) = init_state_with_objects_and_object_basics(vec![
        Object::immutable_with_id_for_testing(immutable_id),
        Object::with_id_owner_for_testing(gas1_id, sender),
        Object::with_id_owner_for_testing(gas2_id, sender),
        Object::with_id_owner_for_testing(owned_id, sender),
    ])
    .await;

    let epoch_store = authority.epoch_store_for_testing().clone();
    let rgp = authority.reference_gas_price_for_testing().unwrap();
    let object_ref = |id| authority.get_object(&id).unwrap().object_ref();

    ImmutableInputSetup {
        immutable_ref: object_ref(immutable_id),
        gas1_ref: object_ref(gas1_id),
        gas2_ref: object_ref(gas2_id),
        owned_ref: object_ref(owned_id),
        authority,
        epoch_store,
        sender,
        sender_key,
        package_ref,
        rgp,
        _config_guard,
    }
}

impl ImmutableInputSetup {
    /// A transaction reading the immutable object, paid by `gas_ref`. Passing a
    /// distinct gas coin yields a distinct digest with no shared owned input.
    fn read_immutable(&self, gas_ref: ObjectReference) -> VerifiedTransaction {
        self.build(gas_ref, &[self.immutable_ref])
    }

    /// A transaction reading the immutable object and taking `owned_ref` as a
    /// second input, paid by `gas_ref`.
    fn read_immutable_with_owned(&self, gas_ref: ObjectReference) -> VerifiedTransaction {
        self.build(gas_ref, &[self.immutable_ref, self.owned_ref])
    }

    /// Builds an `object_basics::update` call over `inputs`. The argument
    /// types deliberately mismatch, so execution fails — a failed execution
    /// must still report every owned input it consumed.
    fn build(&self, gas_ref: ObjectReference, inputs: &[ObjectReference]) -> VerifiedTransaction {
        let mut builder = ProgrammableTransactionBuilder::new();
        let args: Vec<_> = inputs
            .iter()
            .map(|obj_ref| builder.obj(CallArg::ImmutableOrOwned(*obj_ref)).unwrap())
            .collect();
        let first = args[0];
        builder.command(Command::new_move_call(
            self.package_ref.object_id,
            Identifier::new("object_basics").unwrap(),
            Identifier::new("update").unwrap(),
            vec![],
            vec![first, first],
        ));
        let tx = Transaction::new_programmable(
            self.sender,
            vec![gas_ref],
            builder.finish(),
            self.rgp * TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS * 5,
            self.rgp,
        );
        self.epoch_store
            .verify_transaction(to_sender_signed_transaction(tx, &self.sender_key))
            .unwrap()
    }

    /// Marks `tx` as executed via the checkpoint-executor path, as state-sync
    /// would when it wins the race against this node's consensus handler.
    fn execute_via_state_sync(&self, tx: &VerifiedTransaction) {
        let executable = VerifiedExecutableTransaction::new_from_checkpoint(
            tx.clone(),
            self.epoch_store.epoch(),
            1,
        );
        self.authority
            .try_execute_immediately(&executable, ExecutionEnv::new(), &self.epoch_store)
            .unwrap();
    }

    async fn resolve(
        &self,
        transactions: &mut Vec<VerifiedSequencedConsensusTransaction>,
    ) -> (
        Vec<(TransactionDigest, IotaError)>,
        std::collections::HashMap<ObjectReference, LockDetails>,
    ) {
        let (dropped, locks, _) = post_consensus_validation::validate_and_resolve_conflicts(
            &self.authority,
            &self.epoch_store,
            transactions,
        )
        .await
        .unwrap();
        (dropped, locks)
    }
}

/// An immutable object is read-only and cannot be double-spent, so referencing
/// it must not acquire an owned-object lock: any number of transactions in the
/// same epoch may take it as an input (issue #12602).
#[tokio::test]
async fn test_immutable_object_input_not_locked() {
    telemetry_subscribers::init_for_testing();
    let s = setup_immutable_input(true).await;

    let tx1 = s.read_immutable(s.gas1_ref);
    let tx2 = s.read_immutable(s.gas2_ref);
    let mut transactions = vec![make_user_tx_v1_verified(tx1), make_user_tx_v1_verified(tx2)];

    let (dropped, locks) = s.resolve(&mut transactions).await;

    assert!(
        dropped.is_empty(),
        "no transaction may be dropped over an immutable input: {dropped:?}"
    );
    assert_eq!(transactions.len(), 2, "both transactions must be kept");
    assert!(
        !locks.contains_key(&s.immutable_ref),
        "immutable object must not be locked"
    );
    assert_eq!(
        locks.len(),
        2,
        "only the two gas coins may be locked, got {locks:?}"
    );
}

/// The gate itself: with the flag off, the first reader locks the immutable
/// object and the second is dropped against it.
#[tokio::test]
async fn test_immutable_object_input_locked_when_flag_disabled() {
    telemetry_subscribers::init_for_testing();
    let s = setup_immutable_input(false).await;

    let tx1 = s.read_immutable(s.gas1_ref);
    let tx2 = s.read_immutable(s.gas2_ref);
    let tx2_digest = *tx2.digest();
    let mut transactions = vec![
        make_user_tx_v1_verified(tx1.clone()),
        make_user_tx_v1_verified(tx2),
    ];

    let (dropped, locks) = s.resolve(&mut transactions).await;

    assert_eq!(dropped.len(), 1, "the second reader must be dropped");
    assert_eq!(dropped[0].0, tx2_digest);
    assert!(matches!(dropped[0].1, IotaError::ObjectLockConflict { .. }));
    assert_eq!(
        locks.get(&s.immutable_ref),
        Some(tx1.digest()),
        "the immutable object is locked while the flag is off"
    );
}

/// The already-executed branch registers locks from the transaction's own
/// effects, so it locks the owned inputs it consumed and leaves the immutable
/// input free. A later reader of the same immutable object is then kept
/// instead of hitting the winner-out-locked `fatal!`.
#[tokio::test]
async fn test_already_executed_tx_does_not_lock_immutable_input() {
    telemetry_subscribers::init_for_testing();
    let s = setup_immutable_input(true).await;

    let executed = s.read_immutable_with_owned(s.gas1_ref);
    s.execute_via_state_sync(&executed);

    let later_reader = s.read_immutable(s.gas2_ref);
    let mut transactions = vec![
        make_user_tx_v1_verified(executed.clone()),
        make_user_tx_v1_verified(later_reader),
    ];

    let (dropped, locks) = s.resolve(&mut transactions).await;

    assert!(dropped.is_empty(), "{dropped:?}");
    assert_eq!(
        transactions.len(),
        2,
        "the executed winner and the later reader must both be kept"
    );
    assert!(
        !locks.contains_key(&s.immutable_ref),
        "an executed transaction must not lock its immutable input"
    );
    assert_eq!(
        locks.get(&s.gas1_ref),
        Some(executed.digest()),
        "the executed transaction still locks the gas coin it consumed"
    );
    assert_eq!(
        locks.get(&s.owned_ref),
        Some(executed.digest()),
        "an owned input the Move call never mutated is still consumed, so it \
         is still locked"
    );
}
