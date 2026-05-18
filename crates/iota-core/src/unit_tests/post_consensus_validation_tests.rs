// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for post-consensus transaction validation and owned-object
//! conflict resolution.

use iota_macros::sim_test;
use iota_protocol_config::ProtocolConfig;
use iota_types::{
    attestation::{Attestation, AttestationData, AttestedTransaction},
    base_types::{IotaAddress, ObjectID},
    crypto::{AccountKeyPair, get_key_pair},
    digests::TransactionDigest,
    error::IotaError,
    messages_consensus::{ConsensusTransaction, ConsensusTransactionKind},
    object::Object,
    transaction::VerifiedTransaction,
};

use crate::{
    authority::authority_tests::init_state_with_objects_and_object_basics,
    consensus_handler::VerifiedSequencedConsensusTransaction, post_consensus_validation,
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

/// Wraps a `Transaction` in a `UserTransactionV2` consensus transaction with
/// the given `attestor_index`. The block's `certificate_author_index` is always
/// `0` (set by `new_test`), so passing `0` produces a matching attestation and
/// any other value produces a mismatch.
fn make_user_tx_v2(
    tx: iota_types::transaction::Transaction,
    attestor_index: starfish_config::AuthorityIndex,
) -> VerifiedSequencedConsensusTransaction {
    let attestation = Attestation::Validator {
        payload: AttestationData::V1 {
            estimated_computation_cost: 0,
            object_versions: vec![],
        },
        attestor_index,
    };
    let attested = AttestedTransaction::new(tx, attestation);
    let consensus_tx = ConsensusTransaction {
        kind: ConsensusTransactionKind::UserTransactionV2(Box::new(attested)),
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
        config.set_enable_white_flag_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (IotaAddress, AccountKeyPair) = get_key_pair();
    let recipient = get_key_pair::<AccountKeyPair>().0;

    let object_id = ObjectID::random();
    let gas_id = ObjectID::random();

    let (authority, _) = init_state_with_objects_and_object_basics(vec![
        Object::with_id_owner_for_testing(object_id, sender),
        Object::with_id_owner_for_testing(gas_id, sender),
    ])
    .await;

    let epoch_store = authority.epoch_store_for_testing();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let object_ref = authority
        .get_object(&object_id)
        .await
        .unwrap()
        .compute_object_reference();
    let gas_ref = authority
        .get_object(&gas_id)
        .await
        .unwrap()
        .compute_object_reference();

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
        config.set_enable_white_flag_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (IotaAddress, AccountKeyPair) = get_key_pair();
    let recipient = get_key_pair::<AccountKeyPair>().0;

    let object_id = ObjectID::random();
    let gas_id = ObjectID::random();

    let (authority, _) = init_state_with_objects_and_object_basics(vec![
        Object::with_id_owner_for_testing(object_id, sender),
        Object::with_id_owner_for_testing(gas_id, sender),
    ])
    .await;

    let epoch_store = authority.epoch_store_for_testing();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let object_ref = authority
        .get_object(&object_id)
        .await
        .unwrap()
        .compute_object_reference();
    let gas_ref = authority
        .get_object(&gas_id)
        .await
        .unwrap()
        .compute_object_reference();

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
        config.set_enable_white_flag_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (IotaAddress, AccountKeyPair) = get_key_pair();
    let recipient = get_key_pair::<AccountKeyPair>().0;

    let obj1_id = ObjectID::random();
    let gas1_id = ObjectID::random();
    let obj2_id = ObjectID::random();
    let gas2_id = ObjectID::random();

    let (authority, _) = init_state_with_objects_and_object_basics(vec![
        Object::with_id_owner_for_testing(obj1_id, sender),
        Object::with_id_owner_for_testing(gas1_id, sender),
        Object::with_id_owner_for_testing(obj2_id, sender),
        Object::with_id_owner_for_testing(gas2_id, sender),
    ])
    .await;

    let epoch_store = authority.epoch_store_for_testing();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let obj1_ref = authority
        .get_object(&obj1_id)
        .await
        .unwrap()
        .compute_object_reference();
    let gas1_ref = authority
        .get_object(&gas1_id)
        .await
        .unwrap()
        .compute_object_reference();
    let obj2_ref = authority
        .get_object(&obj2_id)
        .await
        .unwrap()
        .compute_object_reference();
    let gas2_ref = authority
        .get_object(&gas2_id)
        .await
        .unwrap()
        .compute_object_reference();

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
        config.set_enable_white_flag_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let recipient1 = get_key_pair::<AccountKeyPair>().0;
    let recipient2 = get_key_pair::<AccountKeyPair>().0;

    let object_id = ObjectID::random();
    let gas1_id = ObjectID::random();
    let gas2_id = ObjectID::random();

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
        object.compute_object_reference(),
        gas1.compute_object_reference(),
        sender,
        &sender_key,
        recipient1,
        rgp,
    );
    let tx2 = make_transfer_object_transaction(
        object.compute_object_reference(),
        gas2.compute_object_reference(),
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
        locks.contains_key(&object.compute_object_reference()),
        "Lock should be acquired for the contested object"
    );
    assert_eq!(
        locks.get(&object.compute_object_reference()),
        Some(verified_tx1.digest())
    );

    assert_eq!(
        user_tx_digests.len(),
        2,
        "Both kept and dropped user txs should be collected"
    );
    assert!(user_tx_digests.contains(verified_tx1.digest()));
    assert!(user_tx_digests.contains(verified_tx2.digest()));
}

/// Two transactions on different objects: both pass with no conflicts.
#[sim_test]
async fn test_no_conflict() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_white_flag_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let recipient1 = get_key_pair::<AccountKeyPair>().0;
    let recipient2 = get_key_pair::<AccountKeyPair>().0;

    let object1_id = ObjectID::random();
    let object2_id = ObjectID::random();
    let gas1_id = ObjectID::random();
    let gas2_id = ObjectID::random();

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
        object1.compute_object_reference(),
        gas1.compute_object_reference(),
        sender,
        &sender_key,
        recipient1,
        rgp,
    );
    let tx2 = make_transfer_object_transaction(
        object2.compute_object_reference(),
        gas2.compute_object_reference(),
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
        locks.get(&object1.compute_object_reference()),
        Some(verified_tx1.digest())
    );
    assert_eq!(
        locks.get(&object2.compute_object_reference()),
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
        config.set_enable_white_flag_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let recipient1 = get_key_pair::<AccountKeyPair>().0;
    let recipient2 = get_key_pair::<AccountKeyPair>().0;
    let recipient3 = get_key_pair::<AccountKeyPair>().0;

    let object_a_id = ObjectID::random();
    let object_b_id = ObjectID::random();
    let object_c_id = ObjectID::random();
    let gas1_id = ObjectID::random();
    let shared_gas_id = ObjectID::random();

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
        object_a.compute_object_reference(),
        gas1.compute_object_reference(),
        sender,
        &sender_key,
        recipient1,
        rgp,
    );
    let tx2 = make_transfer_object_transaction(
        object_b.compute_object_reference(),
        shared_gas.compute_object_reference(),
        sender,
        &sender_key,
        recipient2,
        rgp,
    );
    let tx3 = make_transfer_object_transaction(
        object_c.compute_object_reference(),
        shared_gas.compute_object_reference(),
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
        locks.get(&object_a.compute_object_reference()),
        Some(verified_tx1.digest())
    );
    assert_eq!(
        locks.get(&object_b.compute_object_reference()),
        Some(verified_tx2.digest())
    );
    assert_eq!(
        locks.get(&shared_gas.compute_object_reference()),
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
        config.set_enable_white_flag_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let recipient = get_key_pair::<AccountKeyPair>().0;

    let object_a_id = ObjectID::random();
    let object_b_id = ObjectID::random();
    let gas1_id = ObjectID::random();
    let gas2_id = ObjectID::random();
    let gas3_id = ObjectID::random();
    let gas4_id = ObjectID::random();

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
        object_a.compute_object_reference(),
        gas1.compute_object_reference(),
        sender,
        &sender_key,
        recipient,
        rgp,
    );
    let tx2 = make_transfer_object_transaction(
        object_a.compute_object_reference(),
        gas2.compute_object_reference(),
        sender,
        &sender_key,
        recipient,
        rgp,
    );
    let tx3 = make_transfer_object_transaction(
        object_b.compute_object_reference(),
        gas3.compute_object_reference(),
        sender,
        &sender_key,
        recipient,
        rgp,
    );
    let tx4 = make_transfer_object_transaction(
        object_b.compute_object_reference(),
        gas4.compute_object_reference(),
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
        locks.get(&object_a.compute_object_reference()),
        Some(verified_tx1.digest())
    );
    assert_eq!(
        locks.get(&object_b.compute_object_reference()),
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
        config.set_enable_white_flag_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let recipient1 = get_key_pair::<AccountKeyPair>().0;
    let recipient2 = get_key_pair::<AccountKeyPair>().0;

    let object1_id = ObjectID::random();
    let object2_id = ObjectID::random();
    let shared_gas_id = ObjectID::random();

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
        object1.compute_object_reference(),
        shared_gas.compute_object_reference(),
        sender,
        &sender_key,
        recipient1,
        rgp,
    );
    let tx2 = make_transfer_object_transaction(
        object2.compute_object_reference(),
        shared_gas.compute_object_reference(),
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
        locks.get(&shared_gas.compute_object_reference()),
        Some(verified_tx1.digest())
    );
    assert_eq!(
        locks.get(&object1.compute_object_reference()),
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
        config.set_enable_white_flag_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let recipient = get_key_pair::<AccountKeyPair>().0;

    let object_a_id = ObjectID::random();
    let object_b_id = ObjectID::random();
    let gas1_id = ObjectID::random();
    let gas2_id = ObjectID::random();
    let gas3_id = ObjectID::random();

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

    use iota_types::{
        base_types::Identifier,
        transaction::{CallArg, TransactionData},
    };

    let tx1_data = TransactionData::new_move_call(
        sender,
        package_ref.object_id,
        Identifier::from_static("object_basics"),
        Identifier::from_static("update"),
        vec![],
        gas1.compute_object_reference(),
        vec![
            CallArg::ImmutableOrOwned(object_a.compute_object_reference()),
            CallArg::ImmutableOrOwned(object_b.compute_object_reference()),
        ],
        rgp * 1000,
        rgp,
    )
    .unwrap();
    let tx1 = iota_types::utils::to_sender_signed_transaction(tx1_data, &sender_key);

    let tx2 = make_transfer_object_transaction(
        object_a.compute_object_reference(),
        gas2.compute_object_reference(),
        sender,
        &sender_key,
        recipient,
        rgp,
    );
    let tx3 = make_transfer_object_transaction(
        object_b.compute_object_reference(),
        gas3.compute_object_reference(),
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
        locks.get(&object_a.compute_object_reference()),
        Some(verified_tx1.digest())
    );
    assert_eq!(
        locks.get(&object_b.compute_object_reference()),
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
        config.set_enable_white_flag_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let recipient = get_key_pair::<AccountKeyPair>().0;

    let object_a_id = ObjectID::random();
    let object_b_id = ObjectID::random();
    let gas1_id = ObjectID::random();
    let gas2_id = ObjectID::random();
    let shared_gas_id = ObjectID::random();

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
        object_a.compute_object_reference(),
        shared_gas.compute_object_reference(),
        sender,
        &sender_key,
        recipient,
        rgp,
    );
    let tx2 = make_transfer_object_transaction(
        object_a.compute_object_reference(),
        gas1.compute_object_reference(),
        sender,
        &sender_key,
        recipient,
        rgp,
    );
    let tx3 = make_transfer_object_transaction(
        object_b.compute_object_reference(),
        shared_gas.compute_object_reference(),
        sender,
        &sender_key,
        recipient,
        rgp,
    );
    let tx4 = make_transfer_object_transaction(
        object_b.compute_object_reference(),
        gas2.compute_object_reference(),
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
        locks.get(&object_a.compute_object_reference()),
        Some(verified_tx1.digest()),
        "tx1 should lock object_a"
    );
    assert_eq!(
        locks.get(&shared_gas.compute_object_reference()),
        Some(verified_tx1.digest()),
        "tx1 should lock shared_gas"
    );
    assert_eq!(
        locks.get(&object_b.compute_object_reference()),
        Some(verified_tx4.digest()),
        "tx4 should lock object_b (tx3 was dropped before locking)"
    );
    assert_eq!(
        locks.get(&gas2.compute_object_reference()),
        Some(verified_tx4.digest()),
        "tx4 should lock gas2"
    );
    assert!(
        !locks.contains_key(&gas1.compute_object_reference()),
        "gas1 should not be locked since tx2 was dropped"
    );

    assert_eq!(user_tx_digests.len(), 4);
    assert!(user_tx_digests.contains(verified_tx1.digest()));
    assert!(user_tx_digests.contains(verified_tx2.digest()));
    assert!(user_tx_digests.contains(verified_tx3.digest()));
    assert!(user_tx_digests.contains(verified_tx4.digest()));
}

/// A `UserTransactionV2` whose `attestor_index` matches the block's
/// `certificate_author_index` (both `0`) passes attestor verification (Check
/// #3) and is treated the same as a valid V1 transaction.
#[sim_test]
async fn test_v2_passes() {
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_white_flag_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (IotaAddress, AccountKeyPair) = get_key_pair();
    let recipient = get_key_pair::<AccountKeyPair>().0;

    let object_id = ObjectID::random();
    let gas_id = ObjectID::random();

    let (authority, _) = init_state_with_objects_and_object_basics(vec![
        Object::with_id_owner_for_testing(object_id, sender),
        Object::with_id_owner_for_testing(gas_id, sender),
    ])
    .await;

    let epoch_store = authority.epoch_store_for_testing();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let object_ref = authority
        .get_object(&object_id)
        .await
        .unwrap()
        .compute_object_reference();
    let gas_ref = authority
        .get_object(&gas_id)
        .await
        .unwrap()
        .compute_object_reference();
    let tx =
        make_transfer_object_transaction(object_ref, gas_ref, sender, &sender_key, recipient, rgp);
    let digest = *tx.digest();

    // attestor_index 0 == certificate_author_index 0 set by new_test → match.
    let mut transactions = vec![make_user_tx_v2(
        tx,
        starfish_config::AuthorityIndex::new_for_test(0),
    )];

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
        "V2 with matching attestor should be kept"
    );
    assert!(dropped.is_empty(), "No errors expected");
    assert_eq!(
        locks.len(),
        2,
        "Locks for object and gas should be acquired"
    );
    assert_eq!(user_tx_digests, vec![digest]);
}

/// A `UserTransactionV2` whose `attestor_index` does not match the block's
/// `certificate_author_index` is dropped via Check #3. Critically, its digest
/// must still appear in `all_user_tx_digests` so the caller can release the
/// pre-consensus soft lock — this is the invariant the loop restructure
/// protects.
#[sim_test]
async fn test_v2_attestor_mismatch() {
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_white_flag_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (IotaAddress, AccountKeyPair) = get_key_pair();
    let recipient = get_key_pair::<AccountKeyPair>().0;

    let object_id = ObjectID::random();
    let gas_id = ObjectID::random();

    let (authority, _) = init_state_with_objects_and_object_basics(vec![
        Object::with_id_owner_for_testing(object_id, sender),
        Object::with_id_owner_for_testing(gas_id, sender),
    ])
    .await;

    let epoch_store = authority.epoch_store_for_testing();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let object_ref = authority
        .get_object(&object_id)
        .await
        .unwrap()
        .compute_object_reference();
    let gas_ref = authority
        .get_object(&gas_id)
        .await
        .unwrap()
        .compute_object_reference();
    let tx =
        make_transfer_object_transaction(object_ref, gas_ref, sender, &sender_key, recipient, rgp);
    let digest = *tx.digest();

    // attestor_index 1 != certificate_author_index 0 → mismatch.
    let mut transactions = vec![make_user_tx_v2(
        tx,
        starfish_config::AuthorityIndex::new_for_test(1),
    )];

    let (dropped, locks, user_tx_digests) =
        post_consensus_validation::validate_and_resolve_conflicts(
            &authority,
            &epoch_store,
            &mut transactions,
        )
        .await
        .unwrap();

    assert!(
        transactions.is_empty(),
        "Mismatched V2 should be removed from the batch"
    );
    assert_eq!(
        dropped.len(),
        1,
        "Mismatch should produce one dropped entry"
    );
    assert!(
        matches!(dropped[0].1, IotaError::AttestationAuthorMismatch { .. }),
        "expected AttestationAuthorMismatch, got {:?}",
        dropped[0].1,
    );
    assert!(
        locks.is_empty(),
        "No locks should be acquired for a dropped transaction"
    );
    // The digest must be in user_tx_digests even though the transaction was
    // dropped — the caller needs it to release the pre-consensus soft lock.
    assert_eq!(
        user_tx_digests,
        vec![digest],
        "digest must be collected before Check #3 for soft-lock release",
    );
}
