// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for post-consensus transaction validation and owned-object
//! conflict resolution.

use std::sync::Arc;

use iota_macros::sim_test;
use iota_protocol_config::{OverrideGuard, ProtocolConfig};
use iota_sdk_types::{ObjectId, Owner};
use iota_types::{
    base_types::{IotaAddress, ObjectRef, SequenceNumber},
    crypto::{AccountKeyPair, get_key_pair},
    digests::TransactionDigest,
    error::{IotaError, UserInputError},
    executable_transaction::VerifiedExecutableTransaction,
    messages_consensus::{ConsensusTransaction, ConsensusTransactionKind},
    object::Object,
    transaction::{TransactionKey, VerifiedTransaction},
};

use crate::{
    authority::{
        authority_per_epoch_store::{LockDetails, consensus_quarantine::ConsensusCommitOutput},
        authority_tests::init_state_with_objects_and_object_basics,
    },
    checkpoints::CheckpointServiceNoop,
    consensus_handler::{SequencedConsensusTransaction, VerifiedSequencedConsensusTransaction},
    post_consensus_validation,
    test_utils::make_transfer_object_transaction,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Wraps a `Transaction` in a `UserTransactionV1` consensus transaction.
fn make_user_tx_v1(
    tx: iota_types::transaction::Transaction,
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

    let (sender, sender_key): (IotaAddress, AccountKeyPair) = get_key_pair();
    let recipient = get_key_pair::<AccountKeyPair>().0;

    let object_id = ObjectId::random();
    let gas_id = ObjectId::random();

    let (authority, _) = init_state_with_objects_and_object_basics(vec![
        Object::with_id_owner_for_testing(object_id, sender),
        Object::with_id_owner_for_testing(gas_id, sender),
    ])
    .await;

    let epoch_store = authority.epoch_store_for_testing();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let object_ref = authority.get_object(&object_id).await.unwrap().object_ref();
    let gas_ref = authority.get_object(&gas_id).await.unwrap().object_ref();

    let tx =
        make_transfer_object_transaction(object_ref, gas_ref, sender, &sender_key, recipient, rgp);
    let mut transactions = vec![make_user_tx_v1(tx)];

    let (dropped, locks) = post_consensus_validation::validate_and_resolve_conflicts(
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
}

/// Test that non-UserTransactionV1 transactions (e.g. EndOfPublish) pass
/// through validation unchanged.
#[sim_test]
async fn test_non_user_transaction_passes_through() {
    let (authority, _) = init_state_with_objects_and_object_basics(vec![]).await;
    let epoch_store = authority.epoch_store_for_testing();

    let mut transactions = vec![make_end_of_publish()];

    let (dropped, locks) = post_consensus_validation::validate_and_resolve_conflicts(
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
}

/// Test that duplicate transactions (same ConsensusTransactionKey) are
/// deduplicated: only the first occurrence is kept.
#[sim_test]
async fn test_duplicate_transaction_deduplicated() {
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (IotaAddress, AccountKeyPair) = get_key_pair();
    let recipient = get_key_pair::<AccountKeyPair>().0;

    let object_id = ObjectId::random();
    let gas_id = ObjectId::random();

    let (authority, _) = init_state_with_objects_and_object_basics(vec![
        Object::with_id_owner_for_testing(object_id, sender),
        Object::with_id_owner_for_testing(gas_id, sender),
    ])
    .await;

    let epoch_store = authority.epoch_store_for_testing();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let object_ref = authority.get_object(&object_id).await.unwrap().object_ref();
    let gas_ref = authority.get_object(&gas_id).await.unwrap().object_ref();

    let tx =
        make_transfer_object_transaction(object_ref, gas_ref, sender, &sender_key, recipient, rgp);

    // Same transaction wrapped twice — simulates it appearing in two validator DAG
    // blocks.
    let mut transactions = vec![make_user_tx_v1(tx.clone()), make_user_tx_v1(tx)];

    let (dropped, _locks) = post_consensus_validation::validate_and_resolve_conflicts(
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
}

/// Test that a mixed batch of valid, non-user, and duplicate transactions is
/// correctly filtered: only duplicates are removed, valid and non-user pass.
#[sim_test]
async fn test_mixed_batch_filtering() {
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (IotaAddress, AccountKeyPair) = get_key_pair();
    let recipient = get_key_pair::<AccountKeyPair>().0;

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

    let obj1_ref = authority.get_object(&obj1_id).await.unwrap().object_ref();
    let gas1_ref = authority.get_object(&gas1_id).await.unwrap().object_ref();
    let obj2_ref = authority.get_object(&obj2_id).await.unwrap().object_ref();
    let gas2_ref = authority.get_object(&gas2_id).await.unwrap().object_ref();

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

    let (dropped, _locks) = post_consensus_validation::validate_and_resolve_conflicts(
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

    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let recipient1 = get_key_pair::<AccountKeyPair>().0;
    let recipient2 = get_key_pair::<AccountKeyPair>().0;

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

    let object = authority.get_object(&object_id).await.unwrap();
    let gas1 = authority.get_object(&gas1_id).await.unwrap();
    let gas2 = authority.get_object(&gas2_id).await.unwrap();

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

    let (dropped, locks) = post_consensus_validation::validate_and_resolve_conflicts(
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
}

/// Two transactions in the same commit reference the same owned object at
/// different versions, with the stale one ordered first (the scenario from
/// issue #10922). Because owned-object locks are keyed by the full `ObjectRef`,
/// the two never falsely conflict; the stale transaction is dropped by the
/// version check in `handle_transaction_validation_checks` (Check #5) and the
/// fresh transaction is kept and acquires the lock.
#[sim_test]
async fn test_stale_version_dropped_fresh_kept() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let recipient = get_key_pair::<AccountKeyPair>().0;

    let object_id = ObjectId::random();
    let gas_stale_id = ObjectId::random();
    let gas_fresh_id = ObjectId::random();

    // The contested object is live at version 2, so a reference to version 1 is
    // stale and absent from the store.
    let object = Object::with_id_owner_version_for_testing(
        object_id,
        SequenceNumber::from(2),
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

    let gas_stale = authority.get_object(&gas_stale_id).await.unwrap();
    let gas_fresh = authority.get_object(&gas_fresh_id).await.unwrap();

    let fresh_ref = object.object_ref();
    // Stale reference: same object id and digest, but the previous version.
    let stale_ref = ObjectRef::new(object_id, SequenceNumber::from(1), fresh_ref.digest);

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

    let (dropped, locks) = post_consensus_validation::validate_and_resolve_conflicts(
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
}

/// Two transactions on different objects: both pass with no conflicts.
#[sim_test]
async fn test_no_conflict() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let recipient1 = get_key_pair::<AccountKeyPair>().0;
    let recipient2 = get_key_pair::<AccountKeyPair>().0;

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

    let object1 = authority.get_object(&object1_id).await.unwrap();
    let object2 = authority.get_object(&object2_id).await.unwrap();
    let gas1 = authority.get_object(&gas1_id).await.unwrap();
    let gas2 = authority.get_object(&gas2_id).await.unwrap();

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

    let (dropped, locks) = post_consensus_validation::validate_and_resolve_conflicts(
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

    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let recipient1 = get_key_pair::<AccountKeyPair>().0;
    let recipient2 = get_key_pair::<AccountKeyPair>().0;
    let recipient3 = get_key_pair::<AccountKeyPair>().0;

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

    let object_a = authority.get_object(&object_a_id).await.unwrap();
    let object_b = authority.get_object(&object_b_id).await.unwrap();
    let object_c = authority.get_object(&object_c_id).await.unwrap();
    let gas1 = authority.get_object(&gas1_id).await.unwrap();
    let shared_gas = authority.get_object(&shared_gas_id).await.unwrap();

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

    let (dropped, locks) = post_consensus_validation::validate_and_resolve_conflicts(
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

    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let recipient = get_key_pair::<AccountKeyPair>().0;

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

    let object_a = authority.get_object(&object_a_id).await.unwrap();
    let object_b = authority.get_object(&object_b_id).await.unwrap();
    let gas1 = authority.get_object(&gas1_id).await.unwrap();
    let gas2 = authority.get_object(&gas2_id).await.unwrap();
    let gas3 = authority.get_object(&gas3_id).await.unwrap();
    let gas4 = authority.get_object(&gas4_id).await.unwrap();

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

    let (dropped, locks) = post_consensus_validation::validate_and_resolve_conflicts(
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
}

/// Two transactions sharing the same gas object: first wins, second dropped.
#[sim_test]
async fn test_gas_object_conflict() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let recipient1 = get_key_pair::<AccountKeyPair>().0;
    let recipient2 = get_key_pair::<AccountKeyPair>().0;

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

    let object1 = authority.get_object(&object1_id).await.unwrap();
    let object2 = authority.get_object(&object2_id).await.unwrap();
    let shared_gas = authority.get_object(&shared_gas_id).await.unwrap();

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

    let (dropped, locks) = post_consensus_validation::validate_and_resolve_conflicts(
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
}

/// tx1 locks both A and B; tx2 (object A) and tx3 (object B) are both dropped.
#[sim_test]
async fn test_winner_blocks_multiple_losers() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let recipient = get_key_pair::<AccountKeyPair>().0;

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

    let object_a = authority.get_object(&object_a_id).await.unwrap();
    let object_b = authority.get_object(&object_b_id).await.unwrap();
    let gas1 = authority.get_object(&gas1_id).await.unwrap();
    let gas2 = authority.get_object(&gas2_id).await.unwrap();
    let gas3 = authority.get_object(&gas3_id).await.unwrap();

    use iota_sdk_types::Identifier;
    use iota_types::transaction::{CallArg, TransactionData, TransactionDataAPI};

    let tx1_data = TransactionData::new_move_call(
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
    let tx1 = iota_types::utils::to_sender_signed_transaction(tx1_data, &sender_key);

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

    let (dropped, locks) = post_consensus_validation::validate_and_resolve_conflicts(
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

    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let recipient = get_key_pair::<AccountKeyPair>().0;

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

    let object_a = authority.get_object(&object_a_id).await.unwrap();
    let object_b = authority.get_object(&object_b_id).await.unwrap();
    let gas1 = authority.get_object(&gas1_id).await.unwrap();
    let gas2 = authority.get_object(&gas2_id).await.unwrap();
    let shared_gas = authority.get_object(&shared_gas_id).await.unwrap();

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

    let (dropped, locks) = post_consensus_validation::validate_and_resolve_conflicts(
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

    let (sender, sender_key): (IotaAddress, AccountKeyPair) = get_key_pair();
    let recipient = get_key_pair::<AccountKeyPair>().0;

    let object_id = ObjectId::random();
    let gas_id = ObjectId::random();

    let (authority, _) = init_state_with_objects_and_object_basics(vec![
        Object::with_id_owner_for_testing(object_id, sender),
        Object::with_id_owner_for_testing(gas_id, sender),
    ])
    .await;

    let epoch_store = authority.epoch_store_for_testing();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let object_ref = authority.get_object(&object_id).await.unwrap().object_ref();
    let gas_ref = authority.get_object(&gas_id).await.unwrap().object_ref();

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
        .try_execute_immediately(&executable, None, &epoch_store)
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
            authority.get_transaction_cache_reader().as_ref(),
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

    let (sender, sender_key): (IotaAddress, AccountKeyPair) = get_key_pair();
    let recipient = get_key_pair::<AccountKeyPair>().0;

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

    let object_ref = authority.get_object(&object_id).await.unwrap().object_ref();
    let gas_a_ref = authority.get_object(&gas_a).await.unwrap().object_ref();
    let gas_b_ref = authority.get_object(&gas_b).await.unwrap().object_ref();

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
    let seq = |tx: iota_types::transaction::Transaction| {
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
            authority.get_transaction_cache_reader().as_ref(),
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
//   * Check #4 (conflict drop): any hit = drop with `ObjectLockConflict`.
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
    sender: IotaAddress,
    sender_key: AccountKeyPair,
    recipient: IotaAddress,
    object_ref: iota_types::base_types::ObjectRef,
    gas_ref: iota_types::base_types::ObjectRef,
    rgp: u64,
    _config_guard: OverrideGuard,
}

async fn setup_lock_tier() -> LockTierSetup {
    let _config_guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (IotaAddress, AccountKeyPair) = get_key_pair();
    let recipient = get_key_pair::<AccountKeyPair>().0;
    let object_id = ObjectId::random();
    let gas_id = ObjectId::random();

    let (authority, _) = init_state_with_objects_and_object_basics(vec![
        Object::with_id_owner_for_testing(object_id, sender),
        Object::with_id_owner_for_testing(gas_id, sender),
    ])
    .await;

    let epoch_store = (*authority.epoch_store_for_testing()).clone();
    let rgp = authority.reference_gas_price_for_testing().unwrap();
    let object_ref = authority.get_object(&object_id).await.unwrap().object_ref();
    let gas_ref = authority.get_object(&gas_id).await.unwrap().object_ref();

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
            .try_execute_immediately(&executable, None, &self.epoch_store)
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
    obj_ref: iota_types::base_types::ObjectRef,
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
    owned_inputs: &[iota_types::base_types::ObjectRef],
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
    let alt_recipient = get_key_pair::<AccountKeyPair>().0;
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
    let (dropped, _) = post_consensus_validation::validate_and_resolve_conflicts(
        &s.authority,
        &s.epoch_store,
        &mut transactions,
    )
    .await
    .unwrap();

    assert_eq!(transactions.len(), 0, "contender must be removed");
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
    let (dropped, _) = post_consensus_validation::validate_and_resolve_conflicts(
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
    assert!(
        s.authority
            .get_transaction_cache_reader()
            .try_is_tx_already_executed(&tx_digest)
            .unwrap()
    );
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
