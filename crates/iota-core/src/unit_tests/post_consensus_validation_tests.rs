// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for post-consensus transaction validation, owned-object
//! conflict resolution, and the P-COOL bookkeeping written by executed
//! transactions.

use std::{path::PathBuf, sync::Arc, time::Duration};

use iota_config::verifier_signing_config::VerifierSigningConfig;
use iota_macros::sim_test;
use iota_protocol_config::{OverrideGuard, ProtocolConfig};
use iota_sdk_types::{
    Address, Command, GasCostSummary, Identifier, ObjectDigest, ObjectId, ObjectReference,
    OwnedObjectReference, Owner, SenderSignedTransaction, SharedObjectReference, Transaction,
    TransactionDigest, TransactionEffects, Version,
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_transaction_checks::VerifierLimitsSource;
use iota_types::{
    IOTA_FRAMEWORK_PACKAGE_ID, IOTA_SYSTEM_STATE_OBJECT_ID,
    crypto::{AccountPrivateKey, get_key_pair},
    effects::{TestEffectsBuilder, TransactionEffectsAPI},
    error::{IotaError, IotaResult, UserInputError},
    executable_transaction::VerifiedExecutableTransaction,
    messages_consensus::{ConsensusTransaction, ConsensusTransactionKind},
    object::Object,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    storage::{BackingPackageStore, ObjectKey},
    transaction::{
        CallArg, InputObjectKind, ObjectReadResult, ObjectReadResultKind,
        SenderSignedTransactionAPI, TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS, TransactionAPI,
        TransactionEnvelope, TransactionKey, VerifiedTransaction,
    },
    utils::to_sender_signed_transaction,
};

use crate::{
    authority::{
        AuthorityState, ExecutionEnv,
        authority_per_epoch_store::{
            ExecutionIndices, ExecutionIndicesWithStats, LockDetails,
            authority_per_epoch_store_tests::reopen,
            consensus_quarantine::ConsensusCommitOutput,
            handler_object_state::{
                CommitIndex, HandlerProcessedObject, HandlerProcessedObjectKind, SyncAheadRecord,
                handler_processed_upserts,
            },
        },
        authority_tests::{init_state_with_objects_and_object_basics, publish_object_basics},
        move_integration_tests::build_and_publish_test_package_with_upgrade_cap,
        shared_object_version_manager::Schedulable,
        test_authority_builder::TestAuthorityBuilder,
    },
    checkpoints::CheckpointServiceNoop,
    consensus_handler::{
        ConsensusCommitInfo, ExecutionWatcher, SequencedConsensusTransaction,
        VerifiedSequencedConsensusTransaction,
    },
    post_consensus_input_reader::{
        DropKind, InputResolution, MissingKind, OwnedVerdict, SharedVerdict, ValidationAtCommit,
        reader::CommitIndexedReader,
    },
    post_consensus_validation::{self, PostConsensusVerdict},
    test_utils::make_transfer_object_transaction,
    transaction_input_loader::TransactionInputLoader,
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
            1,
            false,
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

/// Runs post-consensus validation on a transfer whose gas object is named one
/// version below the range assigned to canceled transactions, checks that it is
/// dropped, and returns the error it was dropped with.
async fn drop_error_for_gas_version_below_canceled_range(
    validate_input_object_versions: bool,
) -> IotaError {
    let _guard = ProtocolConfig::apply_overrides_for_testing(move |_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config.set_validate_input_object_versions_for_testing(validate_input_object_versions);
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
    let stored_gas_ref = authority.get_object(&gas_id).unwrap().object_ref();
    let gas_ref = ObjectReference::new(
        stored_gas_ref.object_id,
        Version::MAX_VALID_EXCL - 1,
        stored_gas_ref.digest,
    );

    let tx =
        make_transfer_object_transaction(object_ref, gas_ref, sender, &sender_key, recipient, rgp);
    let digest = *tx.digest();
    let mut transactions = vec![make_user_tx_v1(tx)];

    let (mut dropped, locks, user_tx_digests) =
        post_consensus_validation::validate_and_resolve_conflicts(
            &authority,
            &epoch_store,
            1,
            false,
            &mut transactions,
        )
        .await
        .unwrap();

    assert!(transactions.is_empty(), "The transaction should be dropped");
    assert!(locks.is_empty(), "A dropped transaction acquires no locks");
    assert_eq!(
        user_tx_digests,
        vec![digest],
        "A dropped transaction still releases its pre-consensus lock"
    );
    assert_eq!(dropped.len(), 1);
    let (dropped_digest, error) = dropped.pop().unwrap();
    assert_eq!(dropped_digest, digest);

    error
}

/// With the version bound on, the drop comes from the structural check and
/// reads nothing but the transaction bytes, so it does not depend on what the
/// local store holds.
#[tokio::test]
async fn test_gas_version_below_canceled_range_dropped_before_load() {
    let error = drop_error_for_gas_version_below_canceled_range(true).await;
    assert!(
        matches!(
            error,
            IotaError::UserInput {
                error: UserInputError::InvalidSequenceNumber
            }
        ),
        "unexpected error: {error:?}"
    );
}

/// With the version bound off, the same transaction is dropped only once the
/// gas object fails to load at the named version.
#[tokio::test]
async fn test_gas_version_below_canceled_range_dropped_at_load_when_flag_disabled() {
    let error = drop_error_for_gas_version_below_canceled_range(false).await;
    assert!(
        matches!(
            error,
            IotaError::UserInput {
                error: UserInputError::ObjectNotFound { .. }
            }
        ),
        "unexpected error: {error:?}"
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
            1,
            false,
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
            1,
            false,
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
            1,
            false,
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
            1,
            false,
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
            1,
            false,
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
            1,
            false,
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
            1,
            false,
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
            1,
            false,
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
            1,
            false,
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
            1,
            false,
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
            1,
            false,
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
//   * Check #1 (already-executed branch): same digest = OK; different digest = `fatal!`.
//   * Check #4 (conflict drop): a hit from a different digest = drop with `ObjectLockConflict`; a
//     hit from the same digest (a deferred tx's own prior-round lock) is exempt and the tx is
//     retained.
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
        1,
        false,
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
        1,
        false,
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
        1,
        false,
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
        1,
        false,
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
    let mut output = ConsensusCommitOutput::new(0, 0);
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
        1,
        false,
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
        1,
        false,
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
        1,
        false,
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
        1,
        false,
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
        1,
        false,
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
/// transaction key is registered in the transaction-key -> commit-index map
/// (handler-known) or not (sync-ahead).
struct BookkeepingSetup {
    authority: Arc<crate::authority::AuthorityState>,
    epoch_store: Arc<crate::authority::authority_per_epoch_store::AuthorityPerEpochStore>,
    package_id: ObjectId,
    rgp: u64,
    _config_guard: Option<OverrideGuard>,
}

/// What one call of `validate_and_resolve_conflicts` decided, for comparing
/// two runs of the same commit.
#[derive(Debug, PartialEq, Eq)]
struct ValidationResult {
    kept: Vec<TransactionDigest>,
    dropped: Vec<(TransactionDigest, IotaError)>,
    locks: std::collections::HashMap<ObjectReference, LockDetails>,
}

async fn setup_bookkeeping(
    genesis_objects: Vec<Object>,
    validation_enabled: bool,
) -> BookkeepingSetup {
    let config_guard = ProtocolConfig::apply_overrides_for_testing(move |_, mut config| {
        if validation_enabled {
            config.enable_pcool_deterministic_validation_for_testing();
        } else {
            config.set_pcool_deterministic_validation_for_testing(false);
        }
        config
    });
    setup_bookkeeping_with_config_guard(genesis_objects, Some(config_guard)).await
}

/// Like [`setup_bookkeeping`] with the flags on, but for a node whose authority
/// key is not in the committee, as on a full node.
async fn setup_bookkeeping_outside_committee(genesis_objects: Vec<Object>) -> BookkeepingSetup {
    let config_guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.enable_pcool_deterministic_validation_for_testing();
        config
    });
    // The builder's genesis committee holds its own generated validator keys,
    // so a node signing with a fresh key is outside it.
    let (_, keypair): (_, iota_types::crypto::AuthorityKeyPair) = get_key_pair();
    build_bookkeeping_setup(genesis_objects, Some(config_guard), Some(&keypair)).await
}

/// Like [`setup_bookkeeping`], under the protocol config override the caller
/// installed. The guard lives as long as the setup.
async fn setup_bookkeeping_with_config_guard(
    genesis_objects: Vec<Object>,
    config_guard: Option<OverrideGuard>,
) -> BookkeepingSetup {
    build_bookkeeping_setup(genesis_objects, config_guard, None).await
}

async fn build_bookkeeping_setup(
    genesis_objects: Vec<Object>,
    _config_guard: Option<OverrideGuard>,
    keypair: Option<&iota_types::crypto::AuthorityKeyPair>,
) -> BookkeepingSetup {
    let builder = match keypair {
        Some(keypair) => TestAuthorityBuilder::new().with_keypair(keypair),
        None => TestAuthorityBuilder::new(),
    };
    let authority = builder.build().await;
    for object in genesis_objects {
        authority.insert_genesis_object(object);
    }
    let (authority, package) = publish_object_basics(authority).await;
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
            config.enable_pcool_deterministic_validation_for_testing();
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
        .find_map(|child| match *child.owner() {
            Owner::Address(addr) => created
                .iter()
                .find(|parent| parent.reference().object_id == ObjectId::from(addr))
                .map(|parent| (*parent, *child)),
            _ => None,
        })
        .expect("start() creates a child owned by another created object's id")
}

/// The arguments of `object_basics::create` for an object owned by `owner`.
fn create_object_args(owner: Address) -> Vec<CallArg> {
    vec![
        CallArg::Pure(bcs::to_bytes(&16u64).unwrap()),
        CallArg::Pure(bcs::to_bytes(&owner).unwrap()),
    ]
}

/// The initial shared version of a shared owner; panics for any other owner.
fn initial_shared_version(owner: &Owner) -> Version {
    match owner {
        Owner::Shared(initial_shared_version) => *initial_shared_version,
        other => panic!("expected a shared object, got owner {other:?}"),
    }
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
    /// certificate). Unless the key was registered in the transaction-key ->
    /// commit-index map beforehand, the hook classifies it sync-ahead.
    fn execute(&self, tx: VerifiedTransaction) -> TransactionEffects {
        let effects = self.execute_unchecked(tx);
        assert!(effects.status().is_success(), "{:?}", effects.status());
        effects
    }

    /// [`Self::execute`] without asserting execution success, for scenarios
    /// exercising aborted transactions.
    fn execute_unchecked(&self, tx: VerifiedTransaction) -> TransactionEffects {
        self.execute_executable(&self.executable(tx), ExecutionEnv::new())
    }

    fn executable(&self, tx: VerifiedTransaction) -> VerifiedExecutableTransaction {
        VerifiedExecutableTransaction::new_from_checkpoint(tx, self.epoch_store.epoch(), 1)
    }

    fn execute_executable(
        &self,
        executable: &VerifiedExecutableTransaction,
        execution_env: ExecutionEnv,
    ) -> TransactionEffects {
        let (effects, _) = self
            .authority
            .try_execute_immediately(executable, execution_env, &self.epoch_store)
            .unwrap();
        effects
    }

    /// The handler-processed row of `id` at `version`, which the handler must
    /// have written.
    #[track_caller]
    fn handler_processed_object(&self, id: &ObjectId, version: Version) -> HandlerProcessedObject {
        self.epoch_store
            .handler_processed_object(&ObjectKey(*id, version))
            .unwrap()
            .unwrap_or_else(|| {
                panic!(
                    "the handler must have written a handler-processed row for {id} at {version}"
                )
            })
    }

    /// Asserts that no handler-processed row exists for `id` at `version` -
    /// the version a sync-ahead execution produced.
    #[track_caller]
    fn assert_no_handler_row(&self, id: &ObjectId, version: Version) {
        assert_eq!(
            self.epoch_store
                .handler_processed_object(&ObjectKey(*id, version))
                .unwrap(),
            None
        );
    }

    /// Assigns `txs` to commit `index` as its roots, then executes them in
    /// order - the handler-known classification, under which the hook writes
    /// handler-processed rows instead of sync-ahead records. A commit is
    /// assigned once, so every transaction of a commit goes in one call.
    fn execute_as_handler_known(
        &self,
        txs: Vec<VerifiedTransaction>,
        index: CommitIndex,
    ) -> Vec<TransactionEffects> {
        self.epoch_store.assign_commit_to_transactions(
            index,
            txs.iter()
                .map(|tx| TransactionKey::Digest(*tx.digest()))
                .collect(),
        );
        txs.into_iter().map(|tx| self.execute(tx)).collect()
    }

    /// [`Self::execute_as_handler_known`] of a call into the `object_basics`
    /// module.
    fn handler_known_object_basics_call(
        &self,
        function: &'static str,
        args: Vec<CallArg>,
        gas_id: &ObjectId,
        sender: Address,
        sender_key: &AccountPrivateKey,
        index: CommitIndex,
    ) -> TransactionEffects {
        let tx = self.build_move_call("object_basics", function, args, gas_id, sender, sender_key);
        self.execute_as_handler_known(vec![tx], index).remove(0)
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

    /// Builds a call into the `object_basics` module taking shared-object
    /// inputs and executes it, with the shared versions assigned directly
    /// rather than through consensus: the digest never reaches the handler,
    /// so the hook classifies this execution sync-ahead like every other
    /// direct execution of the fixture.
    fn shared_object_basics_call(
        &self,
        function: &'static str,
        args: Vec<CallArg>,
        gas_id: &ObjectId,
        sender: Address,
        sender_key: &AccountPrivateKey,
    ) -> TransactionEffects {
        let tx = self.build_move_call("object_basics", function, args, gas_id, sender, sender_key);
        self.execute_with_assigned_shared_versions(tx)
    }

    /// [`Self::shared_object_basics_call`] with the digest registered for
    /// `index` first, so the hook classifies the execution handler-known.
    fn handler_known_shared_object_basics_call(
        &self,
        function: &'static str,
        args: Vec<CallArg>,
        gas_id: &ObjectId,
        sender: Address,
        sender_key: &AccountPrivateKey,
        index: CommitIndex,
    ) -> TransactionEffects {
        let tx = self.build_move_call("object_basics", function, args, gas_id, sender, sender_key);
        self.epoch_store
            .assign_commit_to_transactions(index, vec![TransactionKey::Digest(*tx.digest())]);
        self.execute_with_assigned_shared_versions(tx)
    }

    /// Executes `tx` with its shared input versions assigned directly rather
    /// than through consensus.
    fn execute_with_assigned_shared_versions(&self, tx: VerifiedTransaction) -> TransactionEffects {
        let effects = self.execute_with_assigned_shared_versions_unchecked(tx);
        assert!(effects.status().is_success(), "{:?}", effects.status());
        effects
    }

    /// [`Self::execute_with_assigned_shared_versions`] without the success
    /// check, for a transaction expected to fail, such as one naming a
    /// deleted shared object.
    fn execute_with_assigned_shared_versions_unchecked(
        &self,
        tx: VerifiedTransaction,
    ) -> TransactionEffects {
        let executable = self.executable(tx);
        let assigned_versions = self
            .epoch_store
            .assign_shared_object_versions_for_tests(
                self.authority.get_object_cache_reader().as_ref(),
                std::slice::from_ref(&executable),
            )
            .unwrap()
            .into_map()
            .remove(&executable.key())
            .expect("version assignment must cover the transaction it was given");
        self.execute_executable(
            &executable,
            ExecutionEnv::new().with_assigned_versions(assigned_versions),
        )
    }

    /// The commit-indexed reader as of `commit_index`.
    fn reader_at(&self, commit_index: CommitIndex) -> CommitIndexedReader {
        CommitIndexedReader::new(
            self.authority.get_object_cache_reader().clone(),
            self.epoch_store.clone(),
            commit_index,
        )
    }

    /// The commit-indexed reader's verdict for the owned input `reference`,
    /// as of `commit_index`.
    fn read_owned(&self, commit_index: CommitIndex, reference: ObjectReference) -> OwnedVerdict {
        self.reader_at(commit_index).read_owned(reference).unwrap()
    }

    /// The commit-indexed reader's verdict for shared input `id` declared at
    /// `initial_shared_version`, as of `commit_index`.
    fn read_shared(
        &self,
        commit_index: CommitIndex,
        id: &ObjectId,
        initial_shared_version: Version,
    ) -> SharedVerdict {
        self.reader_at(commit_index)
            .read_shared(*id, initial_shared_version)
            .unwrap()
    }

    /// The loader's resolution of `tx`'s inputs as of `commit_index`.
    fn read_inputs_at_commit(
        &self,
        commit_index: CommitIndex,
        tx: &VerifiedTransaction,
    ) -> InputResolution {
        let kinds = tx.collect_all_input_object_kind_for_reading().unwrap();
        TransactionInputLoader::new(self.authority.get_object_cache_reader().clone())
            .read_objects_at_commit(&self.reader_at(commit_index), &kinds)
            .unwrap()
    }

    /// Records a sync-ahead publish of the `object_basics` package, which the
    /// fixture inserts at genesis, so the reader treats it as published by a
    /// commit the handler has not reached.
    fn record_package_published_ahead(&self, gas_id: &ObjectId, sender: Address) {
        let transaction = SenderSignedTransaction::new(
            TestTransactionBuilder::new(sender, self.latest_ref(gas_id), self.rgp)
                .transfer_iota(None, sender)
                .build(),
            vec![],
        );
        let effects = TestEffectsBuilder::new(&transaction)
            .with_created_objects([(self.package_id, Owner::Immutable)])
            .build();
        self.epoch_store
            .record_executed_transaction(
                &TransactionKey::Digest(*effects.transaction_digest()),
                &effects,
                self.authority.get_object_store().as_ref(),
            )
            .unwrap();
    }

    /// The argument naming the shared object `id` as a mutable input.
    fn shared_arg(&self, id: &ObjectId) -> CallArg {
        let initial_shared_version =
            initial_shared_version(&self.authority.get_object(id).unwrap().owner);
        CallArg::Shared(SharedObjectReference::new(
            *id,
            initial_shared_version,
            true,
        ))
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
            create_object_args(sender),
            gas_id,
            sender,
            sender_key,
        );
        (*effects.created()[0].reference(), effects)
    }

    /// A verified transfer of `object_id` to `recipient` at the objects'
    /// latest versions, paid by `gas_id`.
    fn build_transfer(
        &self,
        object_id: &ObjectId,
        gas_id: &ObjectId,
        sender: Address,
        sender_key: &AccountPrivateKey,
        recipient: Address,
    ) -> VerifiedTransaction {
        let tx = make_transfer_object_transaction(
            self.latest_ref(object_id),
            self.latest_ref(gas_id),
            sender,
            sender_key,
            recipient,
            self.rgp,
        );
        self.epoch_store.verify_transaction(tx).unwrap()
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
        self.execute(self.build_transfer(object_id, gas_id, sender, sender_key, recipient))
    }

    /// Starts the execution watcher, as the consensus handler does when the
    /// feature is on. Keep the returned value alive: dropping it aborts the
    /// task.
    fn start_execution_watcher(&self) -> ExecutionWatcher {
        ExecutionWatcher::start(self.epoch_store.clone())
    }

    /// Makes the current sync-ahead records of `ids` durable, standing in for
    /// the checkpoint executor's batch, so a test can observe the commit
    /// flush deleting them from the table.
    fn flush_sync_ahead_records(&self, ids: &[ObjectId]) {
        let sync_rows = ids
            .iter()
            .map(|id| {
                (
                    *id,
                    self.epoch_store.sync_ahead_record(id).unwrap().unwrap(),
                )
            })
            .collect();
        self.epoch_store
            .flush_sync_ahead_rows_for_testing(sync_rows, vec![])
            .unwrap();
    }

    fn highest_fully_executed_commit(&self) -> CommitIndex {
        *self
            .epoch_store
            .subscribe_highest_fully_executed_commit()
            .borrow()
    }

    /// Waits for commit `index` to be fully executed, failing the test if it
    /// does not happen promptly.
    async fn wait_for_fully_executed_commit(&self, index: CommitIndex) {
        tokio::time::timeout(
            Duration::from_secs(10),
            self.epoch_store.wait_for_fully_executed_commit(index),
        )
        .await
        .unwrap_or_else(|_| panic!("commit {index} must become fully executed"));
    }

    /// Asserts commit `index` does not become fully executed. Meant for
    /// `start_paused` tests: the paused clock fires the timeout only once
    /// every task, the watcher included, is idle.
    async fn assert_commit_not_fully_executed(&self, index: CommitIndex) {
        assert!(
            tokio::time::timeout(
                Duration::from_millis(200),
                self.epoch_store.wait_for_fully_executed_commit(index),
            )
            .await
            .is_err(),
            "commit {index} must not be fully executed yet"
        );
    }

    /// Asserts `id`'s sync-ahead record is exactly `{ base_version,
    /// latest_created }`. `base_version` is the version the chain grew from
    /// (`None` when the chain itself created the id) and never changes once
    /// the record exists; `latest_created` is the chain's head.
    #[track_caller]
    fn assert_record(&self, id: &ObjectId, base_version: Option<Version>, latest_created: Version) {
        assert_eq!(
            self.epoch_store.sync_ahead_record(id).unwrap(),
            Some(SyncAheadRecord {
                base_version,
                latest_created,
                initial_shared_version: None,
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

    /// Assigns commit `index` with no roots and marks it fully executed.
    fn complete_empty_commit(&self, index: CommitIndex) {
        self.epoch_store
            .assign_commit_to_transactions(index, vec![]);
        self.epoch_store
            .record_commit_fully_executed(index, &[])
            .unwrap();
    }

    /// Installs a randomness manager on a reopened epoch store, which the
    /// handler's commit processing needs.
    async fn initialize_randomness(&self) {
        let randomness = crate::epoch::randomness::RandomnessManager::try_new(
            Arc::downgrade(&self.epoch_store),
            Box::new(crate::mock_consensus::MockConsensusClient::new(
                Arc::downgrade(&self.authority),
                crate::mock_consensus::ConsensusMode::Noop,
            )),
            iota_network::randomness::Handle::new_stub(),
            self.authority.config.authority_key_pair(),
        )
        .await
        .unwrap();
        self.epoch_store
            .set_randomness_manager(randomness)
            .await
            .unwrap();
    }

    /// Runs `transactions` through the consensus handler's processing of
    /// commit `index`, dropping copies already marked processed as the handler
    /// does, and returns the transactions it schedules.
    async fn process_at(
        &self,
        index: CommitIndex,
        transactions: &[VerifiedTransaction],
    ) -> Vec<VerifiedExecutableTransaction> {
        let verified = transactions
            .iter()
            .cloned()
            .filter_map(|tx| {
                self.epoch_store.verify_consensus_transaction(
                    make_user_tx_v1_verified(tx).0,
                    &self.authority.metrics.skipped_consensus_txns,
                )
            })
            .collect();
        let (scheduled, _) = self
            .epoch_store
            .process_consensus_transactions_and_commit_boundary(
                verified,
                &ExecutionIndicesWithStats::default(),
                &Arc::new(CheckpointServiceNoop {}),
                self.authority.get_object_cache_reader().as_ref(),
                &ConsensusCommitInfo::new_for_test(index, index, index * 1_000, true),
                &self.authority.metrics,
                &self.authority,
            )
            .await
            .unwrap();
        scheduled
            .into_iter()
            .map(|tx| tx.as_tx().unwrap().clone())
            .collect()
    }

    /// Validates `transactions` at commit `index` with deterministic
    /// validation on.
    async fn validate_at(
        &self,
        index: CommitIndex,
        transactions: &[VerifiedTransaction],
    ) -> ValidationResult {
        let mut sequenced: Vec<_> = transactions
            .iter()
            .cloned()
            .map(make_user_tx_v1_verified)
            .collect();
        let (dropped, locks, _) = post_consensus_validation::validate_and_resolve_conflicts(
            &self.authority,
            &self.epoch_store,
            index,
            true,
            &mut sequenced,
        )
        .await
        .unwrap();
        ValidationResult {
            kept: sequenced
                .iter()
                .map(|tx| tx.0.transaction.user_transaction_digest().unwrap())
                .collect(),
            dropped,
            locks,
        }
    }

    /// The roots of every pending checkpoint, by height.
    fn checkpoint_roots(&self) -> Vec<(u64, Vec<TransactionKey>)> {
        self.epoch_store
            .get_pending_checkpoints(None)
            .unwrap()
            .into_iter()
            .map(|(height, checkpoint)| (height, checkpoint.into_v1().roots))
            .collect()
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
    // consumed, and no handler-processed row.
    let obj_genesis_ref = s.latest_ref(&obj_id);
    let gas1_ref = s.latest_ref(&gas1_id);
    let first = s.transfer(&obj_id, &gas1_id, address_1, &address_1_key, address_2);

    s.assert_record(&gas1_id, Some(gas1_ref.version), first.lamport_version());
    s.assert_record(
        &obj_id,
        Some(obj_genesis_ref.version),
        first.lamport_version(),
    );
    s.assert_no_handler_row(&obj_id, first.lamport_version());

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
async fn handler_known_transaction_writes_handler_processed_only() {
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
    // handler-processed rows and neither sync records nor shelter bytes.
    let gas_genesis_ref = s.latest_ref(&gas_id);
    let effects = s.execute_as_handler_known(vec![tx], 7).remove(0);

    // The gas coin is a written object like any other.
    for consumed_ref in [obj_genesis_ref, gas_genesis_ref] {
        let id = consumed_ref.object_id();
        let row = s.handler_processed_object(id, effects.lamport_version());
        assert_eq!(row.produced_at, 7);
        assert_eq!(row.kind, HandlerProcessedObjectKind::Live);

        assert_eq!(s.epoch_store.sync_ahead_record(id).unwrap(), None);
        s.assert_not_sheltered(consumed_ref);
    }
}

#[tokio::test]
async fn handler_known_delete_writes_a_deleted_tombstone_row() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let create_effects = s.handler_known_object_basics_call(
        "create",
        create_object_args(sender),
        &gas_id,
        sender,
        &sender_key,
        3,
    );
    let created_ref = *create_effects.created()[0].reference();
    assert_eq!(
        s.handler_processed_object(created_ref.object_id(), created_ref.version)
            .kind,
        HandlerProcessedObjectKind::Live
    );

    // The deletion in a later commit replaces the live row with a tombstone
    // at the deletion version, carrying the store's deleted-object digest.
    let delete_effects = s.handler_known_object_basics_call(
        "delete",
        vec![CallArg::ImmutableOrOwned(created_ref)],
        &gas_id,
        sender,
        &sender_key,
        4,
    );
    assert_eq!(
        s.handler_processed_object(created_ref.object_id(), delete_effects.lamport_version()),
        HandlerProcessedObject {
            digest: ObjectDigest::OBJECT_DELETED,
            kind: HandlerProcessedObjectKind::Deleted,
            produced_at: 4,
            initial_shared_version: None,
        }
    );

    // Handler-known executions leave no sync-ahead trace, consumed inputs
    // included.
    assert_eq!(
        s.epoch_store
            .sync_ahead_record(created_ref.object_id())
            .unwrap(),
        None
    );
    s.assert_not_sheltered(created_ref);
    assert_eq!(s.epoch_store.sync_ahead_record(&gas_id).unwrap(), None);
}

#[tokio::test]
async fn handler_known_wrap_and_unwrap_move_the_row_through_a_wrapped_tombstone() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let (created_ref, _) = s.create_object(&gas_id, sender, &sender_key);
    let wrap_effects = s.handler_known_object_basics_call(
        "wrap",
        vec![CallArg::ImmutableOrOwned(created_ref)],
        &gas_id,
        sender,
        &sender_key,
        5,
    );
    assert_eq!(
        s.handler_processed_object(created_ref.object_id(), wrap_effects.lamport_version()),
        HandlerProcessedObject {
            digest: ObjectDigest::OBJECT_WRAPPED,
            kind: HandlerProcessedObjectKind::Wrapped,
            produced_at: 5,
            initial_shared_version: None,
        }
    );
    let wrapper_ref = *wrap_effects.created()[0].reference();
    assert_eq!(
        s.handler_processed_object(wrapper_ref.object_id(), wrapper_ref.version)
            .kind,
        HandlerProcessedObjectKind::Live
    );

    // Unwrapping resurfaces the id at a higher version with a live row; the
    // wrapped tombstone stays at its own version, and the wrapper gets a
    // deleted tombstone.
    let unwrap_effects = s.handler_known_object_basics_call(
        "unwrap",
        vec![CallArg::ImmutableOrOwned(wrapper_ref)],
        &gas_id,
        sender,
        &sender_key,
        6,
    );
    let unwrapped_ref = *unwrap_effects.unwrapped()[0].reference();
    assert_eq!(unwrapped_ref.object_id(), created_ref.object_id());
    assert_eq!(
        s.handler_processed_object(created_ref.object_id(), unwrapped_ref.version),
        HandlerProcessedObject {
            digest: unwrapped_ref.digest,
            kind: HandlerProcessedObjectKind::Live,
            produced_at: 6,
            initial_shared_version: None,
        }
    );
    assert_eq!(
        s.handler_processed_object(created_ref.object_id(), wrap_effects.lamport_version())
            .kind,
        HandlerProcessedObjectKind::Wrapped
    );
    assert_eq!(
        s.handler_processed_object(wrapper_ref.object_id(), unwrap_effects.lamport_version()),
        HandlerProcessedObject {
            digest: ObjectDigest::OBJECT_DELETED,
            kind: HandlerProcessedObjectKind::Deleted,
            produced_at: 6,
            initial_shared_version: None,
        }
    );
}

#[tokio::test]
async fn handler_known_share_records_the_initial_shared_version() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let share_effects =
        s.handler_known_object_basics_call("share", vec![], &gas_id, sender, &sender_key, 5);
    let shared = share_effects.created()[0];

    // The creation row doubles as the created-shared flag: it carries the
    // initial shared version, which the shared-input checks read.
    assert_eq!(
        s.handler_processed_object(shared.reference().object_id(), shared.reference().version),
        HandlerProcessedObject {
            digest: shared.reference().digest,
            kind: HandlerProcessedObjectKind::Live,
            produced_at: 5,
            initial_shared_version: Some(initial_shared_version(shared.owner())),
        }
    );
    assert_eq!(
        s.epoch_store
            .sync_ahead_record(shared.reference().object_id())
            .unwrap(),
        None
    );
    s.assert_not_sheltered(*shared.reference());
}

#[tokio::test]
async fn handler_known_shared_delete_keeps_the_initial_shared_version_on_the_tombstone_row() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let share_effects =
        s.handler_known_object_basics_call("share", vec![], &gas_id, sender, &sender_key, 5);
    let shared = share_effects.created()[0];
    let shared_id = shared.reference().object_id();
    let initial = initial_shared_version(shared.owner());

    // Once the object is gone, the tombstone row is the only place this
    // validator can still read the initial shared version a transaction
    // declares against.
    let delete_effects = s.handler_known_shared_object_basics_call(
        "delete",
        vec![s.shared_arg(shared_id)],
        &gas_id,
        sender,
        &sender_key,
        6,
    );
    assert_eq!(
        s.handler_processed_object(shared_id, delete_effects.lamport_version()),
        HandlerProcessedObject {
            digest: ObjectDigest::OBJECT_DELETED,
            kind: HandlerProcessedObjectKind::Deleted,
            produced_at: 6,
            initial_shared_version: Some(initial),
        }
    );
    assert_eq!(s.epoch_store.sync_ahead_record(shared_id).unwrap(), None);
}

#[tokio::test]
async fn handler_catching_up_past_sync_execution_replaces_records_with_handler_processed_rows() {
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

    // State sync executes the transfer before the handler processes the
    // commit that kept it.
    let obj_genesis_ref = s.latest_ref(&obj_id);
    let gas_genesis_ref = s.latest_ref(&gas_id);
    let effects = s.transfer(&obj_id, &gas_id, sender, &sender_key, Address::random());
    let digest = *effects.transaction_digest();
    s.assert_record(
        &obj_id,
        Some(obj_genesis_ref.version),
        effects.lamport_version(),
    );
    s.assert_no_handler_row(&obj_id, effects.lamport_version());

    // The handler reaches that commit: it registers the digest (the hook
    // already ran, so nothing consults the entry) and, with the commit
    // fully executed, applies the upserts derived from the durable effects.
    let state = s.epoch_store.handler_object_state_for_testing();
    let key = TransactionKey::Digest(digest);
    s.epoch_store.assign_commit_to_transactions(4, vec![key]);
    assert_eq!(state.commit_index_of(&key), Some(4));
    assert_eq!(state.overlay_sizes_for_testing().0, 0);
    s.epoch_store
        .record_commit_fully_executed(4, &handler_processed_upserts(&effects, 4))
        .unwrap();

    // Handler-processed rows, which only the completion wrote, now answer from
    // the overlay for every written object, the sync records whose whole
    // chain the handler passed are gone, and the commit's map entries are
    // dropped.
    assert_eq!(state.overlay_sizes_for_testing().0, 2);
    for id in [&obj_id, &gas_id] {
        let row = s.handler_processed_object(id, effects.lamport_version());
        assert_eq!(row.produced_at, 4);
        assert_eq!(row.kind, HandlerProcessedObjectKind::Live);
        assert_eq!(s.epoch_store.sync_ahead_record(id).unwrap(), None);
    }
    assert_eq!(state.commit_index_of(&key), None);

    // The sheltered bytes stay: a crash before this commit's output flushes
    // replays and re-validates it, so their eviction keys off the flushed
    // frontier, not off execution completion.
    s.assert_sheltered(obj_genesis_ref);
    s.assert_sheltered(gas_genesis_ref);
}

#[tokio::test]
async fn handler_catching_up_partway_through_a_chain_keeps_its_sync_record() {
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

    // Two sync-executed transfers form one chain on the object.
    let obj_genesis_version = s.latest_ref(&obj_id).version;
    let first = s.transfer(&obj_id, &gas1_id, address_1, &address_1_key, address_2);
    let second = s.transfer(&obj_id, &gas2_id, address_2, &address_2_key, address_1);
    s.assert_record(&obj_id, Some(obj_genesis_version), second.lamport_version());

    // The handler passes only the first commit. The object's record stays -
    // its chain head is above the handler-written version, so the handler
    // has not passed the whole chain - while the gas coin's chain, which
    // ended in that commit, is passed and its record goes.
    s.epoch_store.assign_commit_to_transactions(
        4,
        vec![TransactionKey::Digest(*first.transaction_digest())],
    );
    s.epoch_store
        .record_commit_fully_executed(4, &handler_processed_upserts(&first, 4))
        .unwrap();
    assert_eq!(
        s.handler_processed_object(&obj_id, first.lamport_version())
            .produced_at,
        4
    );
    s.assert_record(&obj_id, Some(obj_genesis_version), second.lamport_version());
    assert_eq!(s.epoch_store.sync_ahead_record(&gas1_id).unwrap(), None);

    // Passing the second commit completes the catch-up.
    s.epoch_store.assign_commit_to_transactions(
        5,
        vec![TransactionKey::Digest(*second.transaction_digest())],
    );
    s.epoch_store
        .record_commit_fully_executed(5, &handler_processed_upserts(&second, 5))
        .unwrap();
    let row = s.handler_processed_object(&obj_id, second.lamport_version());
    assert_eq!(row.produced_at, 5);
    assert_eq!(s.epoch_store.sync_ahead_record(&obj_id).unwrap(), None);
    assert_eq!(s.epoch_store.sync_ahead_record(&gas2_id).unwrap(), None);
}

#[tokio::test(start_paused = true)]
async fn watcher_completes_a_commit_once_its_roots_execute() {
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
    let _watcher = s.start_execution_watcher();
    let state = s.epoch_store.handler_object_state_for_testing();

    // The handler assigns the commit before its root executes; the watcher
    // must wait for the root's effects.
    let tx = s.build_transfer(&obj_id, &gas_id, sender, &sender_key, Address::random());
    let key = TransactionKey::Digest(*tx.digest());
    s.epoch_store.assign_commit_to_transactions(1, vec![key]);
    s.assert_commit_not_fully_executed(1).await;
    assert_eq!(state.commit_index_of(&key), Some(1));

    let effects = s.execute(tx);
    s.wait_for_fully_executed_commit(1).await;
    assert_eq!(s.highest_fully_executed_commit(), 1);
    assert_eq!(state.commit_index_of(&key), None);
    for id in [&obj_id, &gas_id] {
        assert_eq!(
            s.handler_processed_object(id, effects.lamport_version())
                .produced_at,
            1
        );
    }
}

#[tokio::test(start_paused = true)]
async fn watcher_completes_commits_in_assignment_order() {
    let (address_1, address_1_key): (Address, AccountPrivateKey) = get_key_pair();
    let (address_2, address_2_key): (Address, AccountPrivateKey) = get_key_pair();
    let obj1_id = ObjectId::random();
    let gas1_id = ObjectId::random();
    let obj2_id = ObjectId::random();
    let gas2_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![
            Object::with_id_owner_for_testing(obj1_id, address_1),
            Object::with_id_owner_for_testing(gas1_id, address_1),
            Object::with_id_owner_for_testing(obj2_id, address_2),
            Object::with_id_owner_for_testing(gas2_id, address_2),
        ],
        true,
    )
    .await;
    let _watcher = s.start_execution_watcher();
    let state = s.epoch_store.handler_object_state_for_testing();

    let first = s.build_transfer(
        &obj1_id,
        &gas1_id,
        address_1,
        &address_1_key,
        Address::random(),
    );
    let second = s.build_transfer(
        &obj2_id,
        &gas2_id,
        address_2,
        &address_2_key,
        Address::random(),
    );
    let first_key = TransactionKey::Digest(*first.digest());
    let second_key = TransactionKey::Digest(*second.digest());
    s.epoch_store
        .assign_commit_to_transactions(1, vec![first_key]);
    s.epoch_store
        .assign_commit_to_transactions(2, vec![second_key]);

    // The later commit's root executes first. Completing commit 2 now would
    // claim commit 1 executed too, so the watcher must hold it back.
    s.execute(second);
    s.assert_commit_not_fully_executed(1).await;
    assert_eq!(state.commit_index_of(&second_key), Some(2));

    s.execute(first);
    s.wait_for_fully_executed_commit(2).await;
    assert_eq!(state.commit_index_of(&first_key), None);
    assert_eq!(state.commit_index_of(&second_key), None);
}

/// The flush reaches a commit before the watcher does (here: its root
/// executed ahead of the handler, as when state sync runs ahead of the
/// checkpoint builder). The flush completes the commit from its own batch;
/// the watcher reaching it afterwards must change nothing.
#[tokio::test]
async fn flush_first_completion_leaves_the_watcher_a_no_op() {
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
    let state = s.epoch_store.handler_object_state_for_testing();

    // Sync-ahead execution, with its records made durable.
    let effects = s.transfer(&obj_id, &gas_id, sender, &sender_key, Address::random());
    let version = effects.lamport_version();
    s.flush_sync_ahead_records(&[obj_id, gas_id]);

    // The handler reaches the commit and its checkpoint executes before the
    // watcher runs.
    let key = TransactionKey::Digest(*effects.transaction_digest());
    s.epoch_store.assign_commit_to_transactions(1, vec![key]);
    let written = s
        .epoch_store
        .flush_commit_through_quarantine_for_testing(1, vec![*effects.transaction_digest()])
        .unwrap();

    assert_eq!(written, handler_processed_upserts(&effects, 1));
    assert_eq!(state.overlay_sizes_for_testing().0, 0);
    for id in [obj_id, gas_id] {
        let row = s
            .epoch_store
            .durable_handler_processed_object_for_testing(&ObjectKey(id, version))
            .unwrap()
            .expect("the flush must write the commit's rows");
        assert_eq!(row.produced_at, 1);
        assert_eq!(
            s.epoch_store
                .durable_sync_ahead_record_for_testing(&id)
                .unwrap(),
            None
        );
    }
    assert_eq!(state.commit_index_of(&key), None);
    assert_eq!(s.highest_fully_executed_commit(), 1);

    // The watcher now receives commit 1, then an empty commit 2; once commit
    // 2 is complete, commit 1 has been handled. Had the watcher completed
    // commit 1 again, its rows would sit in the overlay with no flush left
    // to evict them.
    let _watcher = s.start_execution_watcher();
    s.epoch_store.assign_commit_to_transactions(2, vec![]);
    s.wait_for_fully_executed_commit(2).await;
    assert_eq!(state.overlay_sizes_for_testing().0, 0);
}

/// A flush that derives a commit's rows from effects fails when a root has
/// none, so the batch is never written without that root's rows and the
/// commit stays waiting to be completed.
#[tokio::test]
async fn flush_fails_when_a_root_has_no_effects() {
    let s = setup_bookkeeping(vec![], true).await;
    let state = s.epoch_store.handler_object_state_for_testing();

    let missing = TransactionDigest::random();
    let key = TransactionKey::Digest(missing);
    s.epoch_store.assign_commit_to_transactions(1, vec![key]);
    let result = s
        .epoch_store
        .flush_commit_through_quarantine_for_testing(1, vec![missing]);

    assert!(
        matches!(result, Err(IotaError::TransactionEffectsNotFound { digest }) if digest == missing),
        "{result:?}"
    );
    assert_eq!(state.commit_index_of(&key), Some(1));
    assert_eq!(s.highest_fully_executed_commit(), 0);
}

#[tokio::test]
async fn sync_record_deletions_ride_their_own_commits_flush() {
    let (address_1, address_1_key): (Address, AccountPrivateKey) = get_key_pair();
    let (address_2, address_2_key): (Address, AccountPrivateKey) = get_key_pair();
    let obj1_id = ObjectId::random();
    let gas1_id = ObjectId::random();
    let obj2_id = ObjectId::random();
    let gas2_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![
            Object::with_id_owner_for_testing(obj1_id, address_1),
            Object::with_id_owner_for_testing(gas1_id, address_1),
            Object::with_id_owner_for_testing(obj2_id, address_2),
            Object::with_id_owner_for_testing(gas2_id, address_2),
        ],
        true,
    )
    .await;
    let durable_record = |id: &ObjectId| {
        s.epoch_store
            .durable_sync_ahead_record_for_testing(id)
            .unwrap()
    };

    // Two sync-ahead transfers on disjoint objects, with their records made
    // durable.
    let first = s.transfer(
        &obj1_id,
        &gas1_id,
        address_1,
        &address_1_key,
        Address::random(),
    );
    let second = s.transfer(
        &obj2_id,
        &gas2_id,
        address_2,
        &address_2_key,
        Address::random(),
    );
    s.flush_sync_ahead_records(&[obj1_id, gas1_id, obj2_id, gas2_id]);

    // The handler reaches both commits and the watcher completes them,
    // queuing each commit's record deletions under that commit.
    let first_key = TransactionKey::Digest(*first.transaction_digest());
    let second_key = TransactionKey::Digest(*second.transaction_digest());
    for (index, key, effects) in [(1, first_key, &first), (2, second_key, &second)] {
        s.epoch_store
            .assign_commit_to_transactions(index, vec![key]);
        s.epoch_store
            .record_commit_fully_executed(index, &handler_processed_upserts(effects, index))
            .unwrap();
    }

    // Commit 1's flush deletes only its own records: commit 2's replacing
    // rows are not durable yet, so its records must stay.
    s.epoch_store
        .flush_commit_through_quarantine_for_testing(1, vec![*first.transaction_digest()])
        .unwrap();
    assert_eq!(durable_record(&obj1_id), None);
    assert_eq!(durable_record(&gas1_id), None);
    assert!(durable_record(&obj2_id).is_some());
    assert!(durable_record(&gas2_id).is_some());

    s.epoch_store
        .flush_commit_through_quarantine_for_testing(2, vec![*second.transaction_digest()])
        .unwrap();
    assert_eq!(durable_record(&obj2_id), None);
    assert_eq!(durable_record(&gas2_id), None);
}

/// The checkpoint batch makes a completed commit's rows durable before the
/// commit flushes, so the flush has none of them left to write.
#[tokio::test]
async fn flush_skips_rows_the_checkpoint_batch_made_durable() {
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
    let state = s.epoch_store.handler_object_state_for_testing();

    let _watcher = s.start_execution_watcher();
    let effects = s
        .execute_as_handler_known(
            vec![s.build_transfer(&obj_id, &gas_id, sender, &sender_key, Address::random())],
            1,
        )
        .remove(0);
    s.wait_for_fully_executed_commit(1).await;
    s.epoch_store
        .persist_checkpoint_bookkeeping([&effects])
        .unwrap();
    assert_eq!(state.overlay_sizes_for_testing().0, 0);

    let written = s
        .epoch_store
        .flush_commit_through_quarantine_for_testing(1, vec![*effects.transaction_digest()])
        .unwrap();
    assert_eq!(written, vec![]);
    assert_eq!(state.overlay_sizes_for_testing().0, 0);
    for id in [obj_id, gas_id] {
        let row = s
            .epoch_store
            .durable_handler_processed_object_for_testing(&ObjectKey(id, effects.lamport_version()))
            .unwrap()
            .expect("the checkpoint batch must have written the commit's rows");
        assert_eq!(row.produced_at, 1);
    }
}

/// Without a checkpoint batch in between, the flush of a commit the watcher
/// completed writes all of the commit's rows and evicts them.
#[tokio::test]
async fn flush_writes_completed_commit_rows_still_in_the_overlay() {
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
    let state = s.epoch_store.handler_object_state_for_testing();

    let _watcher = s.start_execution_watcher();
    let effects = s
        .execute_as_handler_known(
            vec![s.build_transfer(&obj_id, &gas_id, sender, &sender_key, Address::random())],
            1,
        )
        .remove(0);
    s.wait_for_fully_executed_commit(1).await;
    assert_eq!(state.overlay_sizes_for_testing().0, 2);

    let written = s
        .epoch_store
        .flush_commit_through_quarantine_for_testing(1, vec![*effects.transaction_digest()])
        .unwrap();
    assert_eq!(written, handler_processed_upserts(&effects, 1));
    assert_eq!(state.overlay_sizes_for_testing().0, 0);
    for (key, row) in written {
        assert_eq!(
            s.epoch_store
                .durable_handler_processed_object_for_testing(&key)
                .unwrap(),
            Some(row)
        );
    }
}

/// The checkpoint batch makes a commit's rows durable before the watcher
/// completes the commit, so the watcher leaves them out of the overlay and
/// the commit's flush has none of them left to write.
#[tokio::test]
async fn watcher_skips_rows_the_checkpoint_batch_made_durable() {
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
    let state = s.epoch_store.handler_object_state_for_testing();

    let effects = s
        .execute_as_handler_known(
            vec![s.build_transfer(&obj_id, &gas_id, sender, &sender_key, Address::random())],
            1,
        )
        .remove(0);
    s.epoch_store
        .persist_checkpoint_bookkeeping([&effects])
        .unwrap();
    assert_eq!(state.overlay_sizes_for_testing().0, 0);

    let _watcher = s.start_execution_watcher();
    s.wait_for_fully_executed_commit(1).await;
    assert_eq!(state.overlay_sizes_for_testing().0, 0);

    let written = s
        .epoch_store
        .flush_commit_through_quarantine_for_testing(1, vec![*effects.transaction_digest()])
        .unwrap();
    assert_eq!(written, vec![]);
    for id in [obj_id, gas_id] {
        let row = s
            .epoch_store
            .durable_handler_processed_object_for_testing(&ObjectKey(id, effects.lamport_version()))
            .unwrap()
            .expect("the checkpoint batch must have written the commit's rows");
        assert_eq!(row.produced_at, 1);
    }
}

/// The watcher completed a commit, then the node crashed before the commit
/// flushed. After the restart the overlay is empty and the watcher has not
/// completed the replayed commit, so its flush derives every row from
/// effects and writes them.
#[tokio::test]
async fn replayed_flush_after_restart_writes_rows_from_effects() {
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

    let watcher = s.start_execution_watcher();
    let effects = s
        .execute_as_handler_known(
            vec![s.build_transfer(&obj_id, &gas_id, sender, &sender_key, Address::random())],
            1,
        )
        .remove(0);
    s.wait_for_fully_executed_commit(1).await;
    drop(watcher);

    let reopened = reopen(&s.authority, &s.epoch_store);
    reopened.set_effects_store(s.authority.get_transaction_cache_reader().clone());
    assert_eq!(
        reopened
            .handler_object_state_for_testing()
            .overlay_sizes_for_testing(),
        (0, 0, 0)
    );

    reopened.assign_commit_to_transactions(
        1,
        vec![TransactionKey::Digest(*effects.transaction_digest())],
    );
    let written = reopened
        .flush_commit_through_quarantine_for_testing(1, vec![*effects.transaction_digest()])
        .unwrap();
    assert_eq!(written, handler_processed_upserts(&effects, 1));
    for (key, row) in written {
        assert_eq!(
            reopened
                .durable_handler_processed_object_for_testing(&key)
                .unwrap(),
            Some(row)
        );
    }
}

/// A commit's queued sync-record deletions ride its flush batch even when
/// the checkpoint batch already made every one of the commit's rows durable
/// and the flush writes none of them.
#[tokio::test]
async fn sync_record_deletions_ride_a_flush_that_writes_no_rows() {
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
    let durable_record = |id: &ObjectId| {
        s.epoch_store
            .durable_sync_ahead_record_for_testing(id)
            .unwrap()
    };

    // A sync-ahead transfer with its records durable; the watcher then
    // completes its commit, queuing the records' deletions under commit 1.
    let effects = s.transfer(&obj_id, &gas_id, sender, &sender_key, Address::random());
    s.flush_sync_ahead_records(&[obj_id, gas_id]);
    let _watcher = s.start_execution_watcher();
    s.epoch_store.assign_commit_to_transactions(
        1,
        vec![TransactionKey::Digest(*effects.transaction_digest())],
    );
    s.wait_for_fully_executed_commit(1).await;

    // The checkpoint batch makes the commit's rows durable but leaves the
    // records for the flush to delete.
    s.epoch_store
        .persist_checkpoint_bookkeeping([&effects])
        .unwrap();
    assert!(durable_record(&obj_id).is_some());
    assert!(durable_record(&gas_id).is_some());

    let written = s
        .epoch_store
        .flush_commit_through_quarantine_for_testing(1, vec![*effects.transaction_digest()])
        .unwrap();
    assert_eq!(written, vec![]);
    assert_eq!(durable_record(&obj_id), None);
    assert_eq!(durable_record(&gas_id), None);
}

/// A commit executed before a crash but not yet flushed loses its rows with
/// the overlay, and the execution hook does not run again for its
/// transactions. When the handler replays the commit, the watcher must
/// restore the rows from the effects already on disk.
#[tokio::test]
async fn watcher_restores_rows_of_a_replayed_commit_after_restart() {
    let (address_1, address_1_key): (Address, AccountPrivateKey) = get_key_pair();
    let (address_2, address_2_key): (Address, AccountPrivateKey) = get_key_pair();
    let obj1_id = ObjectId::random();
    let gas1_id = ObjectId::random();
    let obj2_id = ObjectId::random();
    let gas2_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![
            Object::with_id_owner_for_testing(obj1_id, address_1),
            Object::with_id_owner_for_testing(gas1_id, address_1),
            Object::with_id_owner_for_testing(obj2_id, address_2),
            Object::with_id_owner_for_testing(gas2_id, address_2),
        ],
        true,
    )
    .await;

    // Commit 1 executes and flushes; commit 2 executes and completes, but its
    // output is still in the quarantine when the node crashes.
    let watcher = s.start_execution_watcher();
    let first = s
        .execute_as_handler_known(
            vec![s.build_transfer(
                &obj1_id,
                &gas1_id,
                address_1,
                &address_1_key,
                Address::random(),
            )],
            1,
        )
        .remove(0);
    s.wait_for_fully_executed_commit(1).await;
    s.epoch_store
        .flush_commit_through_quarantine_for_testing(1, vec![*first.transaction_digest()])
        .unwrap();
    let second = s
        .execute_as_handler_known(
            vec![s.build_transfer(
                &obj2_id,
                &gas2_id,
                address_2,
                &address_2_key,
                Address::random(),
            )],
            2,
        )
        .remove(0);
    s.wait_for_fully_executed_commit(2).await;
    drop(watcher);

    let reopened = reopen(&s.authority, &s.epoch_store);
    reopened.set_effects_store(s.authority.get_transaction_cache_reader().clone());
    let row = |effects: &TransactionEffects, id: ObjectId| {
        reopened
            .handler_processed_object(&ObjectKey(id, effects.lamport_version()))
            .unwrap()
    };

    // The value resumes from the flushed commit. Commit 1's rows are on disk;
    // commit 2's were only in the overlay and are gone.
    assert_eq!(
        *reopened.subscribe_highest_fully_executed_commit().borrow(),
        1
    );
    assert_eq!(
        reopened
            .handler_object_state_for_testing()
            .overlay_sizes_for_testing(),
        (0, 0, 0)
    );
    for id in [obj1_id, gas1_id] {
        assert_eq!(row(&first, id).map(|row| row.produced_at), Some(1));
    }
    for id in [obj2_id, gas2_id] {
        assert_eq!(row(&second, id), None);
    }

    // The handler replays commit 2. Its transaction is not executed again.
    let _watcher = ExecutionWatcher::start(reopened.clone());
    reopened.assign_commit_to_transactions(
        2,
        vec![TransactionKey::Digest(*second.transaction_digest())],
    );
    tokio::time::timeout(
        Duration::from_secs(10),
        reopened.wait_for_fully_executed_commit(2),
    )
    .await
    .expect("the replayed commit must become fully executed");
    for id in [obj2_id, gas2_id] {
        assert_eq!(row(&second, id).map(|row| row.produced_at), Some(2));
        assert_eq!(reopened.sync_ahead_record(&id).unwrap(), None);
    }
}

/// A checkpoint's bookkeeping becomes durable with the checkpoint, before its
/// commit flushes: a restart in between finds every row, record and
/// sheltered version on disk, although the overlays are gone and the
/// execution hook does not run again.
#[tokio::test]
async fn checkpoint_bookkeeping_is_durable_before_its_commit_flushes() {
    let (address_1, address_1_key): (Address, AccountPrivateKey) = get_key_pair();
    let (address_2, address_2_key): (Address, AccountPrivateKey) = get_key_pair();
    let obj1_id = ObjectId::random();
    let gas1_id = ObjectId::random();
    let obj2_id = ObjectId::random();
    let gas2_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![
            Object::with_id_owner_for_testing(obj1_id, address_1),
            Object::with_id_owner_for_testing(gas1_id, address_1),
            Object::with_id_owner_for_testing(obj2_id, address_2),
            Object::with_id_owner_for_testing(gas2_id, address_2),
        ],
        true,
    )
    .await;
    let state = s.epoch_store.handler_object_state_for_testing();

    // One checkpoint with a transaction state sync executed ahead of the
    // handler, and one the handler assigned to commit 1 before it executed.
    let consumed = [s.latest_ref(&obj1_id), s.latest_ref(&gas1_id)];
    let sync_ahead = s.transfer(
        &obj1_id,
        &gas1_id,
        address_1,
        &address_1_key,
        Address::random(),
    );
    let handler_known = s
        .execute_as_handler_known(
            vec![s.build_transfer(
                &obj2_id,
                &gas2_id,
                address_2,
                &address_2_key,
                Address::random(),
            )],
            1,
        )
        .remove(0);
    let records =
        [obj1_id, gas1_id].map(|id| (id, s.epoch_store.sync_ahead_record(&id).unwrap().unwrap()));
    s.epoch_store
        .persist_checkpoint_bookkeeping([&sync_ahead, &handler_known])
        .unwrap();

    // The entries left the overlays once durable.
    assert_eq!(state.overlay_sizes_for_testing().0, 0);
    for (id, record) in records {
        assert_eq!(
            s.epoch_store
                .durable_sync_ahead_record_for_testing(&id)
                .unwrap(),
            Some(record)
        );
    }

    // The node crashes before commit 1 flushes.
    let reopened = reopen(&s.authority, &s.epoch_store);
    reopened.set_effects_store(s.authority.get_transaction_cache_reader().clone());
    let reopened_state = reopened.handler_object_state_for_testing();
    assert_eq!(reopened_state.overlay_sizes_for_testing(), (0, 0, 0));
    for id in [obj2_id, gas2_id] {
        let row = reopened
            .handler_processed_object(&ObjectKey(id, handler_known.lamport_version()))
            .unwrap()
            .expect("the handler row must be durable with its checkpoint");
        assert_eq!(row.produced_at, 1);
    }
    for (id, record) in records {
        assert_eq!(reopened.sync_ahead_record(&id).unwrap(), Some(record));
    }
    let sheltered: Vec<Object> = consumed
        .iter()
        .map(|reference| {
            let object = reopened
                .sheltered_object(&ObjectKey::from(*reference))
                .unwrap()
                .expect("the consumed version must be sheltered durably");
            assert_eq!(object.digest(), reference.digest);
            object
        })
        .collect();

    // Re-executing the checkpoint after the crash re-inserts the same
    // entries, and its auxiliary batch writes the same rows and clears them.
    reopened
        .record_executed_transaction(
            &TransactionKey::Digest(*sync_ahead.transaction_digest()),
            &sync_ahead,
            &sheltered.as_slice(),
        )
        .unwrap();
    assert_ne!(reopened_state.overlay_sizes_for_testing(), (0, 0, 0));
    reopened
        .persist_checkpoint_bookkeeping([&sync_ahead, &handler_known])
        .unwrap();
    assert_eq!(reopened_state.overlay_sizes_for_testing(), (0, 0, 0));
    for (id, record) in records {
        assert_eq!(reopened.sync_ahead_record(&id).unwrap(), Some(record));
    }
}

/// On a validator whose handler keeps up, a checkpoint's bookkeeping is its
/// handler rows alone.
#[tokio::test]
async fn healthy_checkpoint_persists_only_handler_rows() {
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

    let consumed = [s.latest_ref(&obj_id), s.latest_ref(&gas_id)];
    let effects = s
        .execute_as_handler_known(
            vec![s.build_transfer(&obj_id, &gas_id, sender, &sender_key, Address::random())],
            1,
        )
        .remove(0);
    s.epoch_store
        .persist_checkpoint_bookkeeping([&effects])
        .unwrap();

    for (id, consumed_ref) in [obj_id, gas_id].into_iter().zip(consumed) {
        let row = s
            .epoch_store
            .durable_handler_processed_object_for_testing(&ObjectKey(id, effects.lamport_version()))
            .unwrap()
            .expect("the handler row must be durable with its checkpoint");
        assert_eq!(row.produced_at, 1);
        assert_eq!(
            s.epoch_store
                .durable_sync_ahead_record_for_testing(&id)
                .unwrap(),
            None
        );
        s.assert_not_sheltered(consumed_ref);
    }
}

/// A checkpoint whose bookkeeping holds no sync-ahead record is written
/// without the consensus quarantine lock, so a commit push or a quarantine
/// flush does not hold up checkpoint execution.
#[tokio::test]
async fn checkpoint_bookkeeping_without_a_record_does_not_wait_for_the_quarantine() {
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
    let effects = s
        .execute_as_handler_known(
            vec![s.build_transfer(&obj_id, &gas_id, sender, &sender_key, Address::random())],
            1,
        )
        .remove(0);

    let (written, wait_written) = std::sync::mpsc::channel();
    let mut bookkeeping_write = None;
    s.epoch_store.hold_consensus_quarantine_for_testing(|| {
        let epoch_store = s.epoch_store.clone();
        let effects = effects.clone();
        bookkeeping_write = Some(std::thread::spawn(move || {
            epoch_store
                .persist_checkpoint_bookkeeping([&effects])
                .unwrap();
            written.send(()).unwrap();
        }));
        wait_written.recv_timeout(Duration::from_secs(5)).expect(
            "checkpoint bookkeeping without a record must not wait for the quarantine lock",
        );
    });
    bookkeeping_write.unwrap().join().unwrap();

    for id in [obj_id, gas_id] {
        assert!(
            s.epoch_store
                .durable_handler_processed_object_for_testing(&ObjectKey(
                    id,
                    effects.lamport_version()
                ))
                .unwrap()
                .is_some()
        );
    }
    assert_eq!(
        s.epoch_store
            .handler_object_state_for_testing()
            .overlay_sizes_for_testing()
            .0,
        0
    );
}

/// A checkpoint whose bookkeeping holds a sync-ahead record waits for the
/// consensus quarantine lock, so its write cannot land between a flush
/// staging that record's deletion and writing its batch.
#[tokio::test]
async fn checkpoint_bookkeeping_with_a_record_waits_for_the_quarantine() {
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
    let effects = s.transfer(&obj_id, &gas_id, sender, &sender_key, Address::random());
    let records =
        [obj_id, gas_id].map(|id| (id, s.epoch_store.sync_ahead_record(&id).unwrap().unwrap()));

    let (written, wait_written) = std::sync::mpsc::channel();
    let mut bookkeeping_write = None;
    s.epoch_store.hold_consensus_quarantine_for_testing(|| {
        let epoch_store = s.epoch_store.clone();
        let effects = effects.clone();
        bookkeeping_write = Some(std::thread::spawn(move || {
            epoch_store
                .persist_checkpoint_bookkeeping([&effects])
                .unwrap();
            written.send(()).unwrap();
        }));
        assert!(
            wait_written
                .recv_timeout(Duration::from_millis(500))
                .is_err(),
            "a checkpoint batch with a record must wait for the quarantine lock"
        );
    });
    wait_written
        .recv_timeout(Duration::from_secs(30))
        .expect("the checkpoint batch proceeds once the quarantine lock is released");
    bookkeeping_write.unwrap().join().unwrap();

    for (id, record) in records {
        assert_eq!(
            s.epoch_store
                .durable_sync_ahead_record_for_testing(&id)
                .unwrap(),
            Some(record)
        );
    }
}

/// The checkpoint executor runs a checkpoint's executions before the previous
/// checkpoints' outputs are durable. A record that such an execution started
/// on an object the earlier checkpoints wrote handler-known goes with its own
/// checkpoint's batch, not theirs: a crash before their outputs must not
/// leave it durable on a version the store never got.
#[tokio::test]
async fn record_goes_with_the_checkpoint_that_wrote_it() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let obj_id = ObjectId::random();
    let handler_gas_id = ObjectId::random();
    let sync_gas_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![
            Object::with_id_owner_for_testing(obj_id, sender),
            Object::with_id_owner_for_testing(handler_gas_id, sender),
            Object::with_id_owner_for_testing(sync_gas_id, sender),
        ],
        true,
    )
    .await;

    // Two checkpoints of handler-known transfers, then one whose transfer ran
    // ahead of the handler on the second's output.
    let first = s
        .execute_as_handler_known(
            vec![s.build_transfer(&obj_id, &handler_gas_id, sender, &sender_key, sender)],
            1,
        )
        .remove(0);
    let second = s
        .execute_as_handler_known(
            vec![s.build_transfer(&obj_id, &handler_gas_id, sender, &sender_key, sender)],
            2,
        )
        .remove(0);
    let third = s.transfer(&obj_id, &sync_gas_id, sender, &sender_key, sender);
    s.assert_record(
        &obj_id,
        Some(second.lamport_version()),
        third.lamport_version(),
    );
    let record = s.epoch_store.sync_ahead_record(&obj_id).unwrap().unwrap();

    for (effects, index) in [(&first, 1), (&second, 2)] {
        s.epoch_store
            .persist_checkpoint_bookkeeping([effects])
            .unwrap();
        let row = s
            .epoch_store
            .durable_handler_processed_object_for_testing(&ObjectKey(
                obj_id,
                effects.lamport_version(),
            ))
            .unwrap()
            .expect("the handler row must be durable with its checkpoint");
        assert_eq!(row.produced_at, index);
        assert_eq!(
            s.epoch_store
                .durable_sync_ahead_record_for_testing(&obj_id)
                .unwrap(),
            None,
            "a record based on a later checkpoint's output must stay out of this batch"
        );
        assert_eq!(
            s.epoch_store.sync_ahead_record(&obj_id).unwrap(),
            Some(record)
        );
    }

    let state = s.epoch_store.handler_object_state_for_testing();
    let records_before = state.overlay_sizes_for_testing().1;
    s.epoch_store
        .persist_checkpoint_bookkeeping([&third])
        .unwrap();
    assert_eq!(
        s.epoch_store
            .durable_sync_ahead_record_for_testing(&obj_id)
            .unwrap(),
        Some(record)
    );
    // The object's record and the sync-ahead gas coin's.
    assert_eq!(state.overlay_sizes_for_testing().1, records_before - 2);
}

/// A record goes with every checkpoint whose executions wrote it, including
/// one that also holds the handler-known write of the record's base: the
/// base's row is then in the same batch.
#[tokio::test]
async fn record_goes_with_every_checkpoint_that_wrote_it() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let obj_id = ObjectId::random();
    let other_id = ObjectId::random();
    let handler_gas_id = ObjectId::random();
    let sync_gas_id = ObjectId::random();
    let other_sync_gas_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![
            Object::with_id_owner_for_testing(obj_id, sender),
            Object::with_id_owner_for_testing(other_id, sender),
            Object::with_id_owner_for_testing(handler_gas_id, sender),
            Object::with_id_owner_for_testing(sync_gas_id, sender),
            Object::with_id_owner_for_testing(other_sync_gas_id, sender),
        ],
        true,
    )
    .await;

    // Two sync-ahead transfers in separate checkpoints: the first
    // checkpoint's batch persists the chain so far.
    let genesis_version = s.latest_ref(&obj_id).version;
    let sync_1 = s.transfer(&obj_id, &sync_gas_id, sender, &sender_key, sender);
    let sync_2 = s.transfer(&obj_id, &sync_gas_id, sender, &sender_key, sender);
    s.epoch_store
        .persist_checkpoint_bookkeeping([&sync_1])
        .unwrap();
    assert_eq!(
        s.epoch_store
            .durable_sync_ahead_record_for_testing(&obj_id)
            .unwrap(),
        Some(SyncAheadRecord {
            base_version: Some(genesis_version),
            latest_created: sync_2.lamport_version(),
            initial_shared_version: None,
        })
    );

    // A handler-known transfer and a sync-ahead transfer on top of it in one
    // checkpoint, as when the builder merges the pending checkpoints of
    // several commits.
    let handler_known = s
        .execute_as_handler_known(
            vec![s.build_transfer(&other_id, &handler_gas_id, sender, &sender_key, sender)],
            1,
        )
        .remove(0);
    let sync_ahead = s.transfer(&other_id, &other_sync_gas_id, sender, &sender_key, sender);
    let record = s.epoch_store.sync_ahead_record(&other_id).unwrap().unwrap();
    assert_eq!(record.base_version, Some(handler_known.lamport_version()));
    s.epoch_store
        .persist_checkpoint_bookkeeping([&handler_known, &sync_ahead])
        .unwrap();
    assert_eq!(
        s.epoch_store
            .durable_sync_ahead_record_for_testing(&other_id)
            .unwrap(),
        Some(record)
    );
    assert!(
        s.epoch_store
            .durable_handler_processed_object_for_testing(&ObjectKey(
                other_id,
                handler_known.lamport_version(),
            ))
            .unwrap()
            .is_some(),
        "the base's row must be in the same batch as the record"
    );
}

/// A record that starts with no base - an unwrap run ahead of the handler -
/// stays out of the batch of the checkpoint that wrapped the object
/// handler-known.
#[tokio::test]
async fn record_of_an_unwrap_goes_with_the_unwrapping_checkpoint() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let create = s.handler_known_object_basics_call(
        "create",
        create_object_args(sender),
        &gas_id,
        sender,
        &sender_key,
        1,
    );
    let created_ref = *create.created()[0].reference();
    let wrap = s.handler_known_object_basics_call(
        "wrap",
        vec![CallArg::ImmutableOrOwned(created_ref)],
        &gas_id,
        sender,
        &sender_key,
        2,
    );
    let wrapper_ref = *wrap.created()[0].reference();
    let unwrap = s.object_basics_call(
        "unwrap",
        vec![CallArg::ImmutableOrOwned(wrapper_ref)],
        &gas_id,
        sender,
        &sender_key,
    );
    let wrapped_id = created_ref.object_id();
    s.assert_record(wrapped_id, None, unwrap.lamport_version());

    s.epoch_store
        .persist_checkpoint_bookkeeping([&create, &wrap])
        .unwrap();
    for id in [*wrapped_id, *wrapper_ref.object_id(), gas_id] {
        assert_eq!(
            s.epoch_store
                .durable_sync_ahead_record_for_testing(&id)
                .unwrap(),
            None,
            "no record of {id} may be durable before the unwrap's checkpoint"
        );
    }

    s.epoch_store
        .persist_checkpoint_bookkeeping([&unwrap])
        .unwrap();
    for id in [*wrapped_id, *wrapper_ref.object_id(), gas_id] {
        assert!(
            s.epoch_store
                .durable_sync_ahead_record_for_testing(&id)
                .unwrap()
                .is_some(),
            "the unwrap's checkpoint must persist the record of {id}"
        );
    }
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
    let effects = s.transfer(&obj_id, &gas_id, sender, &sender_key, Address::random());

    s.epoch_store
        .persist_checkpoint_bookkeeping([&effects])
        .unwrap();

    s.assert_no_handler_row(&obj_id, effects.lamport_version());
    assert_eq!(s.epoch_store.sync_ahead_record(&obj_id).unwrap(), None);
    s.assert_not_sheltered(obj_genesis_ref);
}

/// Only a committee member keeps the bookkeeping. A node outside the
/// committee runs no consensus handler, so every execution there would be
/// sync-ahead and nothing would ever clear its records and sheltered bytes.
#[rstest::rstest]
#[tokio::test]
async fn bookkeeping_is_kept_only_by_committee_members(#[values(true, false)] in_committee: bool) {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let [sync_obj_id, sync_gas_id, handler_obj_id, handler_gas_id] =
        std::array::from_fn(|_| ObjectId::random());
    let genesis_objects = [sync_obj_id, sync_gas_id, handler_obj_id, handler_gas_id]
        .map(|id| Object::with_id_owner_for_testing(id, sender))
        .to_vec();
    let s = if in_committee {
        setup_bookkeeping(genesis_objects, true).await
    } else {
        setup_bookkeeping_outside_committee(genesis_objects).await
    };

    let consumed_ref = s.latest_ref(&sync_obj_id);
    let sync_effects = s.transfer(
        &sync_obj_id,
        &sync_gas_id,
        sender,
        &sender_key,
        Address::random(),
    );
    let handler_effects = s
        .execute_as_handler_known(
            vec![s.build_transfer(
                &handler_obj_id,
                &handler_gas_id,
                sender,
                &sender_key,
                Address::random(),
            )],
            1,
        )
        .remove(0);
    s.epoch_store
        .persist_checkpoint_bookkeeping([&sync_effects, &handler_effects])
        .unwrap();

    assert_eq!(
        s.epoch_store
            .durable_sync_ahead_record_for_testing(&sync_obj_id)
            .unwrap()
            .is_some(),
        in_committee
    );
    assert_eq!(
        s.epoch_store
            .sheltered_object(&ObjectKey::from(consumed_ref))
            .unwrap()
            .is_some(),
        in_committee
    );
    assert_eq!(
        s.epoch_store
            .durable_handler_processed_object_for_testing(&ObjectKey(
                handler_obj_id,
                handler_effects.lamport_version()
            ))
            .unwrap()
            .is_some(),
        in_committee
    );
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

/// Genesis is the root of no consensus commit, so no commit would ever clear
/// bookkeeping its execution wrote; the objects it creates are epoch-start
/// state.
#[tokio::test]
async fn genesis_execution_leaves_no_bookkeeping() {
    let s = setup_bookkeeping(vec![], true).await;

    for id in [IOTA_FRAMEWORK_PACKAGE_ID, IOTA_SYSTEM_STATE_OBJECT_ID] {
        assert_eq!(s.epoch_store.sync_ahead_record(&id).unwrap(), None);
    }
    assert_eq!(
        s.epoch_store
            .handler_object_state_for_testing()
            .live_sync_ahead_records_count_for_testing(),
        0
    );
}

/// The change-epoch transaction is the root of no consensus commit, so no
/// commit would ever clear bookkeeping its execution wrote; what it writes is
/// the next epoch's epoch-start state.
#[tokio::test]
async fn change_epoch_execution_leaves_no_bookkeeping() {
    let s = setup_bookkeeping(vec![], true).await;

    // The checkpoint builder executes the transaction without committing it
    // and stores it for state sync; the checkpoint executor commits it.
    let (_, _, built_effects) = s
        .authority
        .create_and_execute_advance_epoch_tx(
            &s.epoch_store,
            &GasCostSummary::new(0, 0, 0, 0, 0),
            1, // checkpoint
            0, // epoch_start_timestamp_ms
            // One full score for the single-validator test committee.
            vec![u16::MAX as u64 + 1],
        )
        .await
        .expect("advance epoch tx must succeed");
    let tx = s
        .authority
        .get_transaction_cache_reader()
        .get_transaction_block(built_effects.transaction_digest())
        .expect("the checkpoint builder stores the change-epoch transaction");
    assert!(tx.data().transaction().is_end_of_epoch_tx());
    let effects = s.execute_with_assigned_shared_versions((*tx).clone());

    let written = handler_processed_upserts(&effects, 0);
    assert!(!written.is_empty());
    for (key, _) in written {
        assert_eq!(s.epoch_store.sync_ahead_record(&key.0).unwrap(), None);
        s.assert_no_handler_row(&key.0, key.1);
    }
    for consumed in effects.old_object_metadata() {
        s.assert_not_sheltered(*consumed.reference());
    }
    assert_eq!(
        s.epoch_store
            .handler_object_state_for_testing()
            .live_sync_ahead_records_count_for_testing(),
        0
    );
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

    let wrapper_ref = *wrap_effects.created()[0].reference();
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

    let unwrapped_ref = *unwrap_effects.unwrapped()[0].reference();
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
                .sync_ahead_record(created.reference().object_id())
                .unwrap(),
            Some(SyncAheadRecord {
                base_version: None,
                latest_created: start_effects.lamport_version(),
                initial_shared_version: None,
            }),
            "sync-ahead record mismatch for {:?} (owner {:?})",
            created.reference().object_id(),
            *created.owner()
        );
        s.assert_not_sheltered(*created.reference());
    }

    // The fixture really spans the owner kinds.
    let owners: Vec<_> = start_effects.created().iter().map(|o| *o.owner()).collect();
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
    let field_ref = *add_effects.created()[0].reference();
    assert!(matches!(add_effects.created()[0].owner(), Owner::Object(_)));

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
    let unwrapped_value = *remove_effects.unwrapped()[0].reference();
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
    let field_wrapper_ref = *add_effects.created()[0].reference();
    let value_after_add = add_effects
        .mutated()
        .into_iter()
        .find(|m| m.reference().object_id == *value_ref.object_id())
        .expect("adding the object field mutates the value object");
    assert!(matches!(value_after_add.owner(), Owner::Object(_)));

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
    s.assert_sheltered(*value_after_add.reference());
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
            CallArg::ImmutableOrOwned(*parent.reference()),
            CallArg::Receiving(*child.reference()),
        ],
        &gas_id,
        sender,
        &sender_key,
    );

    // The base stays `None`: the child was created by this same sync-ahead
    // chain (the `start` call), and the receive only extends the chain.
    s.assert_sheltered(*child.reference());
    s.assert_record(
        child.reference().object_id(),
        None,
        receive_effects.lamport_version(),
    );

    // The parent was mutated through its `&mut` argument: a declared input,
    // consumed and sheltered like any other.
    s.assert_sheltered(*parent.reference());
    s.assert_record(
        parent.reference().object_id(),
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
    let shared_ref = *share_effects.created()[0].reference();
    assert!(matches!(
        share_effects.created()[0].owner(),
        Owner::Shared(_)
    ));
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
            s.shared_arg(shared_ref.object_id()),
            CallArg::Pure(bcs::to_bytes(&42u64).unwrap()),
        ],
        &gas_id,
        sender,
        &sender_key,
    );
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
    let delete_effects = s.shared_object_basics_call(
        "delete",
        vec![s.shared_arg(shared_ref.object_id())],
        &gas_id,
        sender,
        &sender_key,
    );
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
async fn sync_ahead_delete_of_an_existing_shared_object_records_its_initial_shared_version() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    // The object exists before sync runs ahead: created by a commit the
    // handler processed, so it has a creation row and no record.
    let share_effects =
        s.handler_known_object_basics_call("share", vec![], &gas_id, sender, &sender_key, 5);
    let shared = share_effects.created()[0];
    let shared_id = shared.reference().object_id();
    let initial = initial_shared_version(shared.owner());
    assert_eq!(s.epoch_store.sync_ahead_record(shared_id).unwrap(), None);

    // Sync deletes it ahead of the handler. The record restores existence at
    // the consumed version and keeps the initial shared version, since the
    // bytes that carried the owner are gone and shared inputs are not
    // sheltered.
    let shared_before_delete = s.latest_ref(shared_id);
    let delete_effects = s.shared_object_basics_call(
        "delete",
        vec![s.shared_arg(shared_id)],
        &gas_id,
        sender,
        &sender_key,
    );
    assert_eq!(
        s.epoch_store.sync_ahead_record(shared_id).unwrap(),
        Some(SyncAheadRecord {
            base_version: Some(shared_before_delete.version),
            latest_created: delete_effects.lamport_version(),
            initial_shared_version: Some(initial),
        })
    );
    s.assert_no_handler_row(shared_id, delete_effects.lamport_version());
    s.assert_not_sheltered(shared_before_delete);
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
            CallArg::ImmutableOrOwned(*parent.reference()),
            CallArg::Receiving(*wrapper.reference()),
        ],
        &gas_id,
        sender,
        &sender_key,
    );

    // The consumed wrapper version is sheltered through the store fallback;
    // since the inner object's bytes live inside the wrapper's, they are
    // sheltered transitively - the only copy of them on this path.
    s.assert_sheltered(*wrapper.reference());
    s.assert_record(
        wrapper.reference().object_id(),
        None,
        unwrap_effects.lamport_version(),
    );
    assert!(
        s.store_object(
            wrapper.reference().object_id(),
            unwrap_effects.lamport_version()
        )
        .is_none()
    );

    // The inner object surfaces for the first time as `unwrapped`: its very
    // first store row is this version, so its record starts here - no base,
    // nothing sheltered at its id.
    let inner = *unwrap_effects.unwrapped()[0].reference();
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
            CallArg::ImmutableOrOwned(*parent.reference()),
            CallArg::Receiving(*child.reference()),
        ],
        &gas_id,
        sender,
        &sender_key,
    );

    s.assert_sheltered(*child.reference());
    s.assert_record(
        child.reference().object_id(),
        None,
        delete_effects.lamport_version(),
    );
    assert!(
        s.store_object(
            child.reference().object_id(),
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
// P-COOL deterministic-validation reader: shared inputs whose object is gone
// ---------------------------------------------------------------------------

#[track_caller]
fn assert_drops_with(verdict: SharedVerdict, kind: DropKind) {
    match verdict {
        SharedVerdict::Drop(reason) => assert_eq!(reason.kind(), kind),
        SharedVerdict::Exists(_) => panic!("expected a drop, got exists"),
        SharedVerdict::Deleted(version, digest) => {
            panic!("expected a drop, got deleted at {version} by {digest}")
        }
        SharedVerdict::Missing(reason) => panic!("expected a drop, got missing {reason:?}"),
    }
}

#[track_caller]
fn assert_deleted_by(verdict: SharedVerdict, delete_effects: &TransactionEffects) {
    match verdict {
        SharedVerdict::Deleted(version, digest) => {
            assert_eq!(version, delete_effects.lamport_version());
            assert_eq!(digest, *delete_effects.transaction_digest());
        }
        SharedVerdict::Exists(_) => panic!("expected deleted, got exists"),
        SharedVerdict::Drop(reason) => panic!("expected deleted, got drop {reason:?}"),
        SharedVerdict::Missing(reason) => panic!("expected deleted, got missing {reason:?}"),
    }
}

/// State sync deleted the shared object ahead of the handler. The bytes that
/// carried the owner are gone, so the record's initial shared version is what
/// a declared version is checked against. A wrong declaration must drop here
/// as it does on a validator that still holds the object.
#[tokio::test]
async fn reader_checks_the_declared_version_against_the_record_after_a_sync_ahead_delete() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let share_effects =
        s.handler_known_object_basics_call("share", vec![], &gas_id, sender, &sender_key, 5);
    let shared = share_effects.created()[0];
    let shared_id = shared.reference().object_id();
    let initial = initial_shared_version(shared.owner());
    let delete_effects = s.shared_object_basics_call(
        "delete",
        vec![s.shared_arg(shared_id)],
        &gas_id,
        sender,
        &sender_key,
    );
    // A version the object never had, so no row exists at that key.
    let wrong = Version::from_u64(1);
    assert!(wrong < initial);

    // Wrong declared version: no creation row at that key, the record
    // restores existence, the object is gone, the record's field decides.
    assert_drops_with(
        s.read_shared(12, shared_id, wrong),
        DropKind::SharedInitialVersionMismatch,
    );
    // Right declared version: the creation row proves it, the deletion has
    // no row because sync executed it, so it is kept as a deletion.
    assert_deleted_by(s.read_shared(12, shared_id, initial), &delete_effects);
}

/// The handler executed the deletion in the window above the horizon. The
/// tombstone row is the only place left that knows the initial shared
/// version, and a wrong declaration must drop at every horizon.
#[tokio::test]
async fn reader_checks_the_declared_version_against_the_deletion_row() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let share_effects =
        s.handler_known_object_basics_call("share", vec![], &gas_id, sender, &sender_key, 5);
    let shared = share_effects.created()[0];
    let shared_id = shared.reference().object_id();
    let initial = initial_shared_version(shared.owner());
    let delete_effects = s.handler_known_shared_object_basics_call(
        "delete",
        vec![s.shared_arg(shared_id)],
        &gas_id,
        sender,
        &sender_key,
        15,
    );
    // A version the object never had, so no row exists at that key. The
    // tombstone sits at `initial + 1`, and a row above the horizon answers
    // missing whatever its kind.
    let wrong = Version::from_u64(1);
    assert!(wrong < initial);

    // Commit 16, horizon 14: the deletion at 15 is not yet visible.
    assert_drops_with(
        s.read_shared(16, shared_id, wrong),
        DropKind::SharedInitialVersionMismatch,
    );
    assert_deleted_by(s.read_shared(16, shared_id, initial), &delete_effects);

    // Commit 18, horizon 16: every validator has executed the deletion.
    assert_drops_with(
        s.read_shared(18, shared_id, wrong),
        DropKind::SharedInitialVersionMismatch,
    );
    assert_drops_with(
        s.read_shared(18, shared_id, initial),
        DropKind::SharedDeletedAtOrBelowHorizon,
    );
}

/// RD-7. Execution smears a deleted shared object's marker onto every later
/// transaction that names it mutably, at that transaction's version, where
/// no handler row exists. The deletion must be decided from the rows, not
/// from the latest marker, or validators split by whether they executed the
/// smearing transaction.
#[tokio::test]
async fn smeared_deletion_marker_does_not_hide_a_deletion_at_or_below_the_horizon() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let share_effects =
        s.handler_known_object_basics_call("share", vec![], &gas_id, sender, &sender_key, 5);
    let shared = share_effects.created()[0];
    let shared_id = shared.reference().object_id();
    let initial = initial_shared_version(shared.owner());
    // Captured while the object is live: the argument of every later
    // transaction that names it.
    let shared_arg = s.shared_arg(shared_id);

    // Commit 10 deletes the object.
    let delete_effects = s.handler_known_shared_object_basics_call(
        "delete",
        vec![shared_arg.clone()],
        &gas_id,
        sender,
        &sender_key,
        10,
    );

    // Commit 12, horizon 10: the deletion is visible everywhere. This is the
    // answer of a validator that has not executed commit 11 yet.
    assert_drops_with(
        s.read_shared(12, shared_id, initial),
        DropKind::SharedDeletedAtOrBelowHorizon,
    );

    // Commit 11, horizon 9: the deletion is above the horizon, so a
    // transaction naming the object keeps as deleted, executes against the
    // deletion, fails, and smears the marker onto its own version.
    assert_deleted_by(s.read_shared(11, shared_id, initial), &delete_effects);
    let smear_tx = s.build_move_call(
        "object_basics",
        "set_value",
        vec![shared_arg, CallArg::Pure(bcs::to_bytes(&1u64).unwrap())],
        &gas_id,
        sender,
        &sender_key,
    );
    s.epoch_store
        .assign_commit_to_transactions(11, vec![TransactionKey::Digest(*smear_tx.digest())]);
    let smear_effects = s.execute_with_assigned_shared_versions_unchecked(smear_tx);
    assert!(!smear_effects.status().is_success());
    let (marker_version, marker_digest) = s
        .authority
        .get_object_cache_reader()
        .try_get_last_shared_object_deletion_info(shared_id, s.epoch_store.epoch())
        .unwrap()
        .expect("the deletion marker is there");
    assert_eq!(marker_version, smear_effects.lamport_version());
    assert_eq!(marker_digest, *smear_effects.transaction_digest());
    s.assert_no_handler_row(shared_id, marker_version);

    // Same commit 12, on the validator that executed commit 11: the latest
    // marker is the smear. The answer must not change.
    assert_drops_with(
        s.read_shared(12, shared_id, initial),
        DropKind::SharedDeletedAtOrBelowHorizon,
    );
    // And a keep above the horizon still names the deleting transaction, not
    // the smearing one.
    assert_deleted_by(s.read_shared(11, shared_id, initial), &delete_effects);
}

// ---------------------------------------------------------------------------
// P-COOL deterministic-validation loader
// ---------------------------------------------------------------------------

/// The deny check's package store answers as the reader does: the package is
/// there from epoch-start state, and absent once a record marks it as
/// published ahead of the handler.
#[tokio::test]
async fn package_store_view_follows_the_reader() {
    let (sender, _): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let reader = s.reader_at(12);
    let package = BackingPackageStore::get_package_object(&reader, &s.package_id)
        .unwrap()
        .expect("a genesis package is epoch-start state");
    assert_eq!(package.object().id(), s.package_id);

    s.record_package_published_ahead(&gas_id, sender);
    assert!(
        BackingPackageStore::get_package_object(&reader, &s.package_id)
            .unwrap()
            .is_none()
    );
}

/// A sync-ahead record marks the package as published ahead of the handler.
/// The record decides, and the loader stops at it naming the package input.
#[tokio::test]
async fn loader_stops_at_a_package_published_ahead_of_the_handler() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let share_effects =
        s.handler_known_object_basics_call("share", vec![], &gas_id, sender, &sender_key, 5);
    let shared = share_effects.created()[0];
    let shared_id = shared.reference().object_id();
    s.record_package_published_ahead(&gas_id, sender);
    let tx = s.build_move_call(
        "object_basics",
        "set_value",
        vec![
            s.shared_arg(shared_id),
            CallArg::Pure(bcs::to_bytes(&42u64).unwrap()),
        ],
        &gas_id,
        sender,
        &sender_key,
    );

    match s.read_inputs_at_commit(12, &tx) {
        InputResolution::Missing(InputObjectKind::MovePackage(id), reason) => {
            assert_eq!(id, s.package_id);
            assert_eq!(reason.kind(), MissingKind::PackageSyncPublished);
        }
        InputResolution::Missing(kind, reason) => panic!("wrong input {kind:?}: {reason:?}"),
        InputResolution::Drop(kind, reason) => panic!("dropped on {kind:?}: {reason:?}"),
        InputResolution::Loaded(_) => panic!("the package must not be visible"),
    }
}

/// Every input kind resolves to the read result the input checks expect:
/// the package from epoch-start state, the shared object at this
/// validator's latest version, the gas coin at the named reference.
#[tokio::test]
async fn loader_resolves_every_input_kind_at_a_commit() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let share_effects =
        s.handler_known_object_basics_call("share", vec![], &gas_id, sender, &sender_key, 5);
    let shared = share_effects.created()[0];
    let shared_id = shared.reference().object_id();
    let initial = initial_shared_version(shared.owner());
    let gas_ref = s.latest_ref(&gas_id);
    let tx = s.build_move_call(
        "object_basics",
        "set_value",
        vec![
            s.shared_arg(shared_id),
            CallArg::Pure(bcs::to_bytes(&42u64).unwrap()),
        ],
        &gas_id,
        sender,
        &sender_key,
    );

    let inputs = match s.read_inputs_at_commit(12, &tx) {
        InputResolution::Loaded(inputs) => inputs,
        InputResolution::Drop(kind, reason) => panic!("dropped on {kind:?}: {reason:?}"),
        InputResolution::Missing(kind, reason) => panic!("missing {kind:?}: {reason:?}"),
    };
    assert_eq!(inputs.len(), 3);
    for result in inputs.iter() {
        match (result.input_object_kind, &result.object) {
            (InputObjectKind::MovePackage(id), ObjectReadResultKind::Object(object)) => {
                assert_eq!(id, s.package_id);
                assert_eq!(object.id(), s.package_id);
            }
            (
                InputObjectKind::SharedMoveObject { id, .. },
                ObjectReadResultKind::Object(object),
            ) => {
                assert_eq!(&id, shared_id);
                assert_eq!(object.owner, Owner::Shared(initial));
            }
            (
                InputObjectKind::ImmOrOwnedMoveObject(reference),
                ObjectReadResultKind::Object(object),
            ) => {
                assert_eq!(reference, gas_ref);
                assert_eq!(object.object_ref(), gas_ref);
            }
            (kind, object) => panic!("unexpected result {kind:?}: {object:?}"),
        }
    }

    // A wrong declared initial version drops, naming the shared input.
    let wrong = s.build_move_call(
        "object_basics",
        "set_value",
        vec![
            CallArg::Shared(SharedObjectReference::new(
                *shared_id,
                Version::from_u64(1),
                true,
            )),
            CallArg::Pure(bcs::to_bytes(&42u64).unwrap()),
        ],
        &gas_id,
        sender,
        &sender_key,
    );
    match s.read_inputs_at_commit(12, &wrong) {
        InputResolution::Drop(InputObjectKind::SharedMoveObject { id, .. }, reason) => {
            assert_eq!(&id, shared_id);
            assert_eq!(reason.kind(), DropKind::SharedInitialVersionMismatch);
        }
        InputResolution::Drop(kind, reason) => panic!("wrong input {kind:?}: {reason:?}"),
        InputResolution::Missing(kind, reason) => panic!("missing {kind:?}: {reason:?}"),
        InputResolution::Loaded(_) => panic!("a wrong declaration must drop"),
    }
}

/// An owned input produced by a commit above the horizon is missing, and the
/// loader names that input.
#[tokio::test]
async fn loader_answers_missing_for_an_owned_input_above_the_horizon() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let share_effects =
        s.handler_known_object_basics_call("share", vec![], &gas_id, sender, &sender_key, 5);
    let shared = share_effects.created()[0];
    let shared_id = shared.reference().object_id();
    // Commit 11 mutates the gas coin. At commit 12 the horizon is 10.
    s.handler_known_shared_object_basics_call(
        "set_value",
        vec![
            s.shared_arg(shared_id),
            CallArg::Pure(bcs::to_bytes(&1u64).unwrap()),
        ],
        &gas_id,
        sender,
        &sender_key,
        11,
    );
    let gas_ref = s.latest_ref(&gas_id);
    let tx = s.build_move_call(
        "object_basics",
        "set_value",
        vec![
            s.shared_arg(shared_id),
            CallArg::Pure(bcs::to_bytes(&2u64).unwrap()),
        ],
        &gas_id,
        sender,
        &sender_key,
    );

    match s.read_inputs_at_commit(12, &tx) {
        InputResolution::Missing(InputObjectKind::ImmOrOwnedMoveObject(reference), reason) => {
            assert_eq!(reference, gas_ref);
            assert_eq!(reason.kind(), MissingKind::HandlerRowAboveHorizon);
        }
        InputResolution::Missing(kind, reason) => panic!("wrong input {kind:?}: {reason:?}"),
        InputResolution::Drop(kind, reason) => panic!("dropped on {kind:?}: {reason:?}"),
        InputResolution::Loaded(_) => panic!("the gas coin must not be visible"),
    }
    // At commit 13 the horizon reaches the producing commit.
    assert!(matches!(
        s.read_inputs_at_commit(13, &tx),
        InputResolution::Loaded(_)
    ));
}

/// The entry point passes the reader's verdicts through and answers `Keep`
/// with the owned references to lock once every check passed.
#[tokio::test]
async fn validation_at_commit_keeps_drops_and_reports_missing() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;
    let deny_config = &s.authority.config.transaction_deny_config;

    let share_effects =
        s.handler_known_object_basics_call("share", vec![], &gas_id, sender, &sender_key, 5);
    let shared = share_effects.created()[0];
    let shared_id = shared.reference().object_id();
    let set_value = |arg: CallArg, value: u64| {
        s.build_move_call(
            "object_basics",
            "set_value",
            vec![arg, CallArg::Pure(bcs::to_bytes(&value).unwrap())],
            &gas_id,
            sender,
            &sender_key,
        )
    };

    // Keep: every input visible at commit 12. Only the gas coin is locked.
    let gas_ref = s.latest_ref(&gas_id);
    let tx = set_value(s.shared_arg(shared_id), 1);
    match s
        .authority
        .handle_transaction_validation_checks_at_commit(
            &s.reader_at(12),
            &tx,
            &s.epoch_store,
            deny_config,
            VerifierLimitsSource::ProtocolConfig,
        )
        .unwrap()
    {
        ValidationAtCommit::Keep(owned) => assert_eq!(owned, vec![gas_ref]),
        ValidationAtCommit::Drop(kind, reason) => panic!("dropped on {kind:?}: {reason:?}"),
        ValidationAtCommit::Missing(kind, reason) => panic!("missing {kind:?}: {reason:?}"),
    }

    // Drop: a wrong declared initial version, named by the shared input.
    let wrong = set_value(
        CallArg::Shared(SharedObjectReference::new(
            *shared_id,
            Version::from_u64(1),
            true,
        )),
        2,
    );
    match s
        .authority
        .handle_transaction_validation_checks_at_commit(
            &s.reader_at(12),
            &wrong,
            &s.epoch_store,
            deny_config,
            VerifierLimitsSource::ProtocolConfig,
        )
        .unwrap()
    {
        ValidationAtCommit::Drop(InputObjectKind::SharedMoveObject { id, .. }, reason) => {
            assert_eq!(&id, shared_id);
            assert_eq!(reason.kind(), DropKind::SharedInitialVersionMismatch);
        }
        other => panic!("expected a drop on the shared input, got {other:?}"),
    }

    // Missing: the gas coin was produced at commit 11, above commit 12's
    // horizon.
    s.handler_known_shared_object_basics_call(
        "set_value",
        vec![
            s.shared_arg(shared_id),
            CallArg::Pure(bcs::to_bytes(&3u64).unwrap()),
        ],
        &gas_id,
        sender,
        &sender_key,
        11,
    );
    let gas_ref = s.latest_ref(&gas_id);
    let tx = set_value(s.shared_arg(shared_id), 4);
    match s
        .authority
        .handle_transaction_validation_checks_at_commit(
            &s.reader_at(12),
            &tx,
            &s.epoch_store,
            deny_config,
            VerifierLimitsSource::ProtocolConfig,
        )
        .unwrap()
    {
        ValidationAtCommit::Missing(InputObjectKind::ImmOrOwnedMoveObject(reference), reason) => {
            assert_eq!(reference, gas_ref);
            assert_eq!(reason.kind(), MissingKind::HandlerRowAboveHorizon);
        }
        other => panic!("expected missing on the gas coin, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// P-COOL deterministic-validation wiring
// ---------------------------------------------------------------------------

/// With the flag on, the validation loop reads inputs as of the commit. One
/// transaction keeps and locks its gas coin, one drops on a wrong shared
/// declaration, one is missing on a gas coin produced above the horizon. The
/// dropped ones carry the not-found error of their deciding input.
#[tokio::test]
async fn validation_loop_at_a_commit_keeps_drops_and_reports_missing() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_keep = ObjectId::random();
    let gas_wrong = ObjectId::random();
    let gas_missing = ObjectId::random();
    let s = setup_bookkeeping(
        vec![
            Object::with_id_owner_for_testing(gas_keep, sender),
            Object::with_id_owner_for_testing(gas_wrong, sender),
            Object::with_id_owner_for_testing(gas_missing, sender),
        ],
        true,
    )
    .await;

    let share_effects =
        s.handler_known_object_basics_call("share", vec![], &gas_keep, sender, &sender_key, 5);
    let shared = share_effects.created()[0];
    let shared_id = shared.reference().object_id();
    // Commit 11 mutates `gas_missing`. At commit 12 the horizon is 10.
    s.handler_known_shared_object_basics_call(
        "set_value",
        vec![
            s.shared_arg(shared_id),
            CallArg::Pure(bcs::to_bytes(&1u64).unwrap()),
        ],
        &gas_missing,
        sender,
        &sender_key,
        11,
    );
    let set_value = |arg: CallArg, gas_id: &ObjectId, value: u64| {
        s.build_move_call(
            "object_basics",
            "set_value",
            vec![arg, CallArg::Pure(bcs::to_bytes(&value).unwrap())],
            gas_id,
            sender,
            &sender_key,
        )
    };
    let keep = set_value(s.shared_arg(shared_id), &gas_keep, 2);
    let wrong = set_value(
        CallArg::Shared(SharedObjectReference::new(
            *shared_id,
            Version::from_u64(1),
            true,
        )),
        &gas_wrong,
        3,
    );
    let missing = set_value(s.shared_arg(shared_id), &gas_missing, 4);
    let keep_digest = *keep.digest();
    let wrong_digest = *wrong.digest();
    let missing_digest = *missing.digest();
    let gas_keep_ref = s.latest_ref(&gas_keep);
    let gas_missing_ref = s.latest_ref(&gas_missing);

    let mut transactions = vec![
        make_user_tx_v1_verified(keep),
        make_user_tx_v1_verified(wrong),
        make_user_tx_v1_verified(missing),
    ];
    let (dropped, locks, all_digests) = post_consensus_validation::validate_and_resolve_conflicts(
        &s.authority,
        &s.epoch_store,
        12,
        true,
        &mut transactions,
    )
    .await
    .unwrap();

    assert_eq!(all_digests, vec![keep_digest, wrong_digest, missing_digest]);
    assert_eq!(transactions.len(), 1);
    assert!(
        locks.contains_key(&gas_keep_ref),
        "the kept transaction locks its gas coin"
    );
    let verdicts = &s
        .authority
        .metrics
        .consensus_handler_validation_reader_verdicts;
    assert_eq!(verdicts.with_label_values(&["keep", "none"]).get(), 1);
    assert_eq!(
        verdicts
            .with_label_values(&["drop", "SharedInitialVersionMismatch"])
            .get(),
        1
    );
    assert_eq!(
        verdicts
            .with_label_values(&["missing", "HandlerRowAboveHorizon"])
            .get(),
        1
    );
    assert_eq!(dropped.len(), 2);
    assert_eq!(dropped[0].0, wrong_digest);
    assert!(
        matches!(
            &dropped[0].1,
            IotaError::UserInput {
                error: UserInputError::ObjectNotFound { object_id, version: None }
            } if object_id == shared_id
        ),
        "{:?}",
        dropped[0].1
    );
    assert_eq!(dropped[1].0, missing_digest);
    assert!(
        matches!(
            &dropped[1].1,
            IotaError::UserInput {
                error: UserInputError::ObjectNotFound { object_id, version: Some(version) }
            } if *object_id == gas_missing && *version == gas_missing_ref.version
        ),
        "{:?}",
        dropped[1].1
    );

    // Every candidate's decision is saved at the commit, with the error the
    // caller reports for a drop.
    let saved: Vec<_> = all_digests
        .iter()
        .map(|digest| s.epoch_store.post_consensus_verdict(12, *digest).unwrap())
        .collect();
    assert_eq!(
        saved,
        vec![
            Some(PostConsensusVerdict::Kept),
            Some(PostConsensusVerdict::Dropped(dropped[0].1.clone())),
            Some(PostConsensusVerdict::Dropped(dropped[1].1.clone())),
        ]
    );
}

/// The decisions of a commit and its candidate list read back as written,
/// before and after a restart.
#[tokio::test]
async fn post_consensus_verdicts_survive_a_restart() {
    use typed_store::Map;

    let mut s = setup_bookkeeping(vec![], true).await;
    let kept = TransactionDigest::random();
    let dropped = TransactionDigest::random();
    let error: IotaError = UserInputError::ObjectNotFound {
        object_id: ObjectId::random(),
        version: None,
    }
    .into();
    let verdicts = vec![
        (kept, PostConsensusVerdict::Kept),
        (dropped, PostConsensusVerdict::Dropped(error)),
    ];
    s.epoch_store
        .persist_post_consensus_verdicts(1, &verdicts)
        .unwrap();
    for reopened in [false, true] {
        if reopened {
            s.epoch_store = reopen(&s.authority, &s.epoch_store);
        }
        for (digest, verdict) in &verdicts {
            assert_eq!(
                s.epoch_store.post_consensus_verdict(1, *digest).unwrap(),
                Some(verdict.clone())
            );
        }
        assert_eq!(
            s.epoch_store
                .tables()
                .unwrap()
                .post_consensus_verdict_candidates
                .get(&1)
                .unwrap(),
            Some(vec![kept, dropped])
        );
        assert_eq!(s.epoch_store.post_consensus_verdict(2, kept).unwrap(), None);
    }
}

/// A commit's saved decisions are deleted in the batch that flushes its
/// output, together with its processed flags and the resume point, so they
/// exist exactly as long as the commit can be replayed.
#[tokio::test]
async fn decisions_retire_with_resume_progress_and_processed_state() {
    use typed_store::Map;

    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let object = ObjectId::random();
    let gas = ObjectId::random();
    let mut s = setup_bookkeeping(
        vec![
            Object::with_id_owner_for_testing(object, sender),
            Object::with_id_owner_for_testing(gas, sender),
        ],
        true,
    )
    .await;
    let tx = s.build_transfer(&object, &gas, sender, &key, Address::random());
    let mut transactions = vec![make_user_tx_v1_verified(tx.clone())];
    let (_, locks, _) = post_consensus_validation::validate_and_resolve_conflicts(
        &s.authority,
        &s.epoch_store,
        1,
        true,
        &mut transactions,
    )
    .await
    .unwrap();
    let key = make_user_tx_v1_verified(tx.clone()).0.key();
    let mut output = ConsensusCommitOutput::new(1, 1);
    output.set_owned_object_locks(locks);
    output.record_consensus_message_processed(key.clone());
    output.record_consensus_commit_stats(ExecutionIndicesWithStats {
        index: ExecutionIndices {
            sub_dag_index: 1,
            last_committed_round: 1,
            transaction_index: 1,
        },
        ..Default::default()
    });
    let mut batch = s.epoch_store.db_batch_for_test();
    output.write_to_batch(&s.epoch_store, &mut batch).unwrap();
    assert!(
        s.epoch_store
            .post_consensus_verdict(1, *tx.digest())
            .unwrap()
            .is_some()
    );
    assert!(!s.epoch_store.is_consensus_message_processed(&key).unwrap());
    batch.write().unwrap();
    let reopened = reopen(&s.authority, &s.epoch_store);
    assert!(
        reopened
            .post_consensus_verdict(1, *tx.digest())
            .unwrap()
            .is_none()
    );
    assert!(reopened.is_consensus_message_processed(&key).unwrap());
    assert_eq!(
        *reopened.subscribe_highest_fully_executed_commit().borrow(),
        1
    );

    // A later copy of the same transaction is dropped as processed before
    // validation, so it is no candidate at commit 2.
    s.epoch_store = reopened;
    s.initialize_randomness().await;
    assert!(s.process_at(2, std::slice::from_ref(&tx)).await.is_empty());
    assert_eq!(
        s.epoch_store
            .post_consensus_verdict(2, *tx.digest())
            .unwrap(),
        None
    );
    assert_eq!(
        s.epoch_store
            .tables()
            .unwrap()
            .post_consensus_verdict_candidates
            .get(&2)
            .unwrap(),
        Some(vec![])
    );
}

/// A transaction kept at commit 1 and deferred by congestion control is kept
/// again when it is reloaded at commit 2. After a restart both occurrences
/// replay from their saved decisions and schedule the same way.
#[tokio::test]
async fn executed_deferred_transaction_replays_both_occurrences() {
    let guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.enable_pcool_deterministic_validation_for_testing();
        config.set_per_object_congestion_control_mode_for_testing(
            iota_protocol_config::PerObjectCongestionControlMode::TotalTxCount,
        );
        config.set_max_accumulated_txn_cost_per_object_in_mysticeti_commit_for_testing(1);
        config.set_max_congestion_limit_overshoot_per_commit_for_testing(0);
        config.set_max_concurrent_execution_workers_for_testing(1);
        config.set_separate_gas_price_feedback_mechanism_for_randomness_for_testing(false);
        config.set_max_deferral_rounds_for_congestion_control_for_testing(10);
        config
    });
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let ids: Vec<_> = (0..4).map(|_| ObjectId::random()).collect();
    let mut s = setup_bookkeeping_with_config_guard(
        ids.iter()
            .map(|id| Object::with_id_owner_for_testing(*id, sender))
            .collect(),
        Some(guard),
    )
    .await;
    let transactions = vec![
        s.build_transfer(&ids[0], &ids[1], sender, &key, Address::random()),
        s.build_transfer(&ids[2], &ids[3], sender, &key, Address::random()),
    ];
    let first = s.process_at(1, &transactions).await;
    assert_eq!(first.len(), 1);
    let deferred = transactions
        .iter()
        .find(|tx| tx.digest() != first[0].digest())
        .unwrap()
        .clone();
    assert_eq!(
        s.epoch_store.get_all_deferred_transactions_for_test().len(),
        1
    );
    assert_eq!(
        s.epoch_store
            .post_consensus_verdict(1, *deferred.digest())
            .unwrap(),
        Some(PostConsensusVerdict::Kept)
    );
    let second = s.process_at(2, std::slice::from_ref(&deferred)).await;
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].digest(), deferred.digest());
    assert!(
        s.epoch_store
            .get_all_deferred_transactions_for_test()
            .is_empty()
    );
    assert_eq!(
        s.epoch_store
            .post_consensus_verdict(2, *deferred.digest())
            .unwrap(),
        Some(PostConsensusVerdict::Kept)
    );
    let roots = s.checkpoint_roots();
    let effects: Vec<_> = transactions
        .iter()
        .map(|tx| s.execute(tx.clone()))
        .collect();

    s.epoch_store = reopen(&s.authority, &s.epoch_store);
    s.epoch_store
        .set_effects_store(s.authority.get_transaction_cache_reader().clone());
    s.initialize_randomness().await;
    let replay_first = s.process_at(1, &transactions).await;
    assert_eq!(replay_first.len(), 1);
    assert_eq!(replay_first[0].digest(), first[0].digest());
    assert_eq!(
        s.epoch_store.get_all_deferred_transactions_for_test().len(),
        1
    );
    let replay_second = s.process_at(2, std::slice::from_ref(&deferred)).await;
    assert_eq!(replay_second.len(), 1);
    assert_eq!(replay_second[0].digest(), second[0].digest());
    assert!(
        s.epoch_store
            .get_all_deferred_transactions_for_test()
            .is_empty()
    );
    assert_eq!(roots, s.checkpoint_roots());

    // A later network copy is dropped as processed before validation, so it
    // is no candidate.
    let first_effects = effects
        .iter()
        .find(|effects| effects.transaction_digest() == first[0].digest())
        .unwrap();
    s.epoch_store
        .record_commit_fully_executed(1, &handler_processed_upserts(first_effects, 1))
        .unwrap();
    assert!(
        s.process_at(3, std::slice::from_ref(&deferred))
            .await
            .is_empty()
    );
    assert_eq!(
        s.epoch_store
            .post_consensus_verdict(3, *deferred.digest())
            .unwrap(),
        None
    );
}

/// A node whose state sync executed a transaction before its handler reached
/// the commit decides like a node that did not: the transaction drops at
/// commit 12, where its input is above the horizon, and is kept at commit 13.
/// After a restart both nodes replay the saved drop and keep, although the
/// transaction is executed by then.
#[tokio::test]
async fn sync_execution_does_not_change_the_accepted_occurrence() {
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let (other, other_key): (Address, AccountPrivateKey) = get_key_pair();
    let object = ObjectId::random();
    let gas = ObjectId::random();
    let other_gas = ObjectId::random();
    // The consumer pays with a coin the producer does not touch, so only one
    // of its inputs is above the horizon at commit 12 and the drop error names
    // it.
    let consumer_gas = ObjectId::random();
    let genesis = vec![
        Object::with_id_owner_for_testing(object, sender),
        Object::with_id_owner_for_testing(gas, sender),
        Object::with_id_owner_for_testing(other_gas, other),
        Object::with_id_owner_for_testing(consumer_gas, sender),
    ];
    let mut normal = setup_bookkeeping(genesis.clone(), true).await;
    // Shares the override `normal` holds.
    let mut synced = setup_bookkeeping_with_config_guard(genesis, None).await;
    assert!(
        synced
            .epoch_store
            .protocol_config()
            .pcool_deterministic_validation()
    );
    let producer = normal.build_transfer(&object, &gas, sender, &key, sender);
    normal.complete_empty_commit(10);
    synced.complete_empty_commit(10);
    normal
        .epoch_store
        .assign_commit_to_transactions(11, vec![TransactionKey::Digest(*producer.digest())]);
    let producer_effects = normal.execute(producer.clone());
    assert_eq!(producer_effects, synced.execute(producer.clone()));
    let consumer = normal.build_transfer(&object, &consumer_gas, sender, &key, Address::random());
    let contender = normal.build_transfer(&object, &other_gas, other, &other_key, other);
    let synced_effects = synced.execute(consumer.clone());

    let early = normal
        .validate_at(12, std::slice::from_ref(&consumer))
        .await;
    assert!(early.kept.is_empty());
    assert!(early.locks.is_empty());
    assert_eq!(early.dropped.len(), 1);
    assert_eq!(
        early,
        synced
            .validate_at(12, std::slice::from_ref(&consumer))
            .await
    );
    for node in [&normal, &synced] {
        assert!(
            node.process_at(12, std::slice::from_ref(&consumer))
                .await
                .is_empty()
        );
    }
    assert_eq!(normal.checkpoint_roots(), synced.checkpoint_roots());

    synced
        .epoch_store
        .assign_commit_to_transactions(11, vec![TransactionKey::Digest(*producer.digest())]);
    for node in [&normal, &synced] {
        node.epoch_store
            .record_commit_fully_executed(11, &handler_processed_upserts(&producer_effects, 11))
            .unwrap();
    }
    let candidates = [contender.clone(), consumer.clone()];
    let late = normal.validate_at(13, &candidates).await;
    assert_eq!(late.kept, vec![*consumer.digest()]);
    assert_eq!(late.locks.len(), 2);
    assert_eq!(late.dropped.len(), 1);
    assert_eq!(late, synced.validate_at(13, &candidates).await);
    for node in [&normal, &synced] {
        let scheduled = node.process_at(13, &candidates).await;
        assert_eq!(scheduled.len(), 1);
        assert_eq!(scheduled[0].digest(), consumer.digest());
    }
    let roots = normal.checkpoint_roots();
    assert_eq!(roots, synced.checkpoint_roots());
    let normal_effects = normal.execute(consumer.clone());
    assert_eq!(normal_effects, synced_effects);

    for node in [&normal, &synced] {
        node.epoch_store
            .persist_checkpoint_bookkeeping([&producer_effects, &normal_effects])
            .unwrap();
        // Only one epoch DB can be open while a store reopens.
        node.epoch_store.release_db_handles();
    }
    for node in [&mut normal, &mut synced] {
        let reopened = reopen(&node.authority, &node.epoch_store);
        reopened.set_effects_store(node.authority.get_transaction_cache_reader().clone());
        node.epoch_store = reopened;
        node.initialize_randomness().await;
        node.complete_empty_commit(11);
        assert_eq!(
            node.epoch_store
                .post_consensus_verdict(12, *consumer.digest())
                .unwrap(),
            Some(PostConsensusVerdict::Dropped(early.dropped[0].1.clone())),
        );
        assert_eq!(
            node.epoch_store
                .post_consensus_verdict(13, *consumer.digest())
                .unwrap(),
            Some(PostConsensusVerdict::Kept),
        );
        assert_eq!(
            early,
            node.validate_at(12, std::slice::from_ref(&consumer)).await
        );
        assert_eq!(late, node.validate_at(13, &candidates).await);
        assert!(
            node.process_at(12, std::slice::from_ref(&consumer))
                .await
                .is_empty()
        );
        assert_eq!(
            node.process_at(13, &candidates).await[0].digest(),
            consumer.digest()
        );
        assert_eq!(roots, node.checkpoint_roots());
        node.epoch_store.release_db_handles();
    }
}

/// Once commit 12 flushes, its saved drop is gone, and a retry of the dropped
/// transaction at a later commit is validated afresh, the same way on a store
/// that kept running and on one reopened after the flush.
#[tokio::test]
async fn retry_after_retired_drop_matches_reopened_store() {
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let object = ObjectId::random();
    let gas = ObjectId::random();
    // The consumer pays with a coin the producer does not touch, so only one
    // of its inputs is above the horizon and the drop error names it.
    let consumer_gas = ObjectId::random();
    let genesis = vec![
        Object::with_id_owner_for_testing(object, sender),
        Object::with_id_owner_for_testing(gas, sender),
        Object::with_id_owner_for_testing(consumer_gas, sender),
    ];
    let normal = setup_bookkeeping(genesis.clone(), true).await;
    // Shares the override `normal` holds.
    let mut restarted = setup_bookkeeping_with_config_guard(genesis, None).await;
    let producer = normal.build_transfer(&object, &gas, sender, &key, sender);
    for node in [&normal, &restarted] {
        node.complete_empty_commit(10);
        let effects = node
            .execute_as_handler_known(vec![producer.clone()], 13)
            .remove(0);
        node.epoch_store
            .persist_checkpoint_bookkeeping([&effects])
            .unwrap();
    }
    let consumer = normal.build_transfer(&object, &consumer_gas, sender, &key, Address::random());
    for node in [&normal, &restarted] {
        assert_eq!(
            node.validate_at(12, std::slice::from_ref(&consumer))
                .await
                .dropped
                .len(),
            1
        );
        let mut output = ConsensusCommitOutput::new(12, 12);
        output.record_consensus_commit_stats(ExecutionIndicesWithStats {
            index: ExecutionIndices {
                sub_dag_index: 12,
                last_committed_round: 12,
                transaction_index: 1,
            },
            ..Default::default()
        });
        let mut batch = node.epoch_store.db_batch_for_test();
        output
            .write_to_batch(&node.epoch_store, &mut batch)
            .unwrap();
        batch.write().unwrap();
        assert!(
            node.epoch_store
                .post_consensus_verdict(12, *consumer.digest())
                .unwrap()
                .is_none()
        );
    }
    normal.complete_empty_commit(12);
    let retry = normal
        .validate_at(14, std::slice::from_ref(&consumer))
        .await;
    assert_eq!(retry.dropped.len(), 1);
    normal
        .epoch_store
        .record_commit_fully_executed(13, &[])
        .unwrap();
    let accepted = normal
        .validate_at(15, std::slice::from_ref(&consumer))
        .await;
    assert_eq!(accepted.kept, vec![*consumer.digest()]);
    // Only one epoch DB can be open while a store reopens.
    normal.epoch_store.release_db_handles();
    restarted.epoch_store = reopen(&restarted.authority, &restarted.epoch_store);
    assert_eq!(
        retry,
        restarted
            .validate_at(14, std::slice::from_ref(&consumer))
            .await
    );
    restarted.complete_empty_commit(13);
    assert_eq!(
        accepted,
        restarted
            .validate_at(15, std::slice::from_ref(&consumer))
            .await
    );
}

/// A commit whose candidate list is saved but one of whose decisions is
/// missing is never validated again: the two are written in one batch, so
/// the store is corrupt.
#[tokio::test]
async fn incomplete_saved_decisions_are_not_revalidated() {
    use typed_store::Map;

    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let object = ObjectId::random();
    let gas = ObjectId::random();
    let s = setup_bookkeeping(
        vec![
            Object::with_id_owner_for_testing(object, sender),
            Object::with_id_owner_for_testing(gas, sender),
        ],
        true,
    )
    .await;
    let tx = s.build_transfer(&object, &gas, sender, &key, Address::random());
    assert_eq!(
        s.validate_at(1, std::slice::from_ref(&tx)).await.kept,
        vec![*tx.digest()]
    );
    s.epoch_store
        .tables()
        .unwrap()
        .post_consensus_verdicts
        .remove(&(1, *tx.digest()))
        .unwrap();
    let task = tokio::spawn(async move { s.validate_at(1, &[tx]).await });
    assert!(task.await.unwrap_err().is_panic());
}

/// A saved keep of a transaction that has not executed is validated again on
/// replay. When the new result agrees, the replay matches the first run; when
/// it is a drop, the node stops.
#[tokio::test]
async fn saved_keep_without_effects_revalidates_and_rejects_disagreement() {
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let object = ObjectId::random();
    let gas = ObjectId::random();
    let mut s = setup_bookkeeping(
        vec![
            Object::with_id_owner_for_testing(object, sender),
            Object::with_id_owner_for_testing(gas, sender),
        ],
        true,
    )
    .await;
    let tx = s.build_transfer(&object, &gas, sender, &key, Address::random());
    let result = s.validate_at(2, std::slice::from_ref(&tx)).await;
    assert_eq!(result.kept, vec![*tx.digest()]);
    assert!(
        !s.authority
            .get_transaction_cache_reader()
            .try_is_tx_already_executed(tx.digest())
            .unwrap()
    );
    s.epoch_store = reopen(&s.authority, &s.epoch_store);
    assert_eq!(result, s.validate_at(2, std::slice::from_ref(&tx)).await);

    let conflict = TransactionDigest::random();
    let mut output = ConsensusCommitOutput::new(1, 1);
    output.set_owned_object_locks([(s.latest_ref(&object), conflict)].into());
    s.epoch_store.push_consensus_output_for_tests(output);
    let authority = s.authority.clone();
    let epoch_store = s.epoch_store.clone();
    let digest = *tx.digest();
    let task = tokio::spawn(async move {
        post_consensus_validation::validate_and_resolve_conflicts(
            &authority,
            &epoch_store,
            2,
            true,
            &mut vec![make_user_tx_v1_verified(tx)],
        )
        .await
    });
    assert!(task.await.unwrap_err().is_panic());
    assert_eq!(
        s.epoch_store.post_consensus_verdict(2, digest).unwrap(),
        Some(PostConsensusVerdict::Kept)
    );
}

/// A Move authenticator's account object that the reader answered as deleted
/// is not a rejection at a commit. The check proceeds to the function
/// reference lookup, which here fails only because the random account has
/// none.
#[tokio::test]
async fn account_check_at_commit_tolerates_a_deleted_account_object() {
    let (sender, _): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let account_id = ObjectId::random();
    let version = Version::from_u64(7);
    let deleted = ObjectReadResult {
        input_object_kind: InputObjectKind::SharedMoveObject {
            id: account_id,
            initial_shared_version: Version::from_u64(3),
            mutable: false,
        },
        object: ObjectReadResultKind::DeletedSharedObject(version, TransactionDigest::random()),
    };

    let epoch_store = s.authority.epoch_store_for_testing();
    let error = s
        .authority
        .check_move_account_at_commit(
            account_id,
            Some(version),
            None,
            deleted,
            &Address::from(account_id),
            epoch_store.protocol_config(),
        )
        .expect_err("a random account has no authenticator function reference");
    assert!(
        matches!(
            error,
            IotaError::UserInput {
                error: UserInputError::MoveAuthenticatorNotFound { .. }
            }
        ),
        "{error:?}"
    );
}

// ---------------------------------------------------------------------------
// P-COOL deterministic-validation reader: owned inputs
// ---------------------------------------------------------------------------

#[track_caller]
fn assert_keeps(verdict: OwnedVerdict, reference: ObjectReference) {
    match verdict {
        OwnedVerdict::Keep(kept) => assert_eq!(kept.into_object().object_ref(), reference),
        OwnedVerdict::Drop(reason) => panic!("expected keep, got drop {reason:?}"),
        OwnedVerdict::Missing(reason) => panic!("expected keep, got missing {reason:?}"),
    }
}

#[track_caller]
fn assert_owned_missing(verdict: OwnedVerdict, kind: MissingKind) {
    match verdict {
        OwnedVerdict::Missing(reason) => assert_eq!(reason.kind(), kind),
        OwnedVerdict::Keep(_) => panic!("expected missing, got keep"),
        OwnedVerdict::Drop(reason) => panic!("expected missing, got drop {reason:?}"),
    }
}

#[track_caller]
fn assert_owned_drops(verdict: OwnedVerdict, kind: DropKind) {
    match verdict {
        OwnedVerdict::Drop(reason) => assert_eq!(reason.kind(), kind),
        OwnedVerdict::Keep(_) => panic!("expected drop, got keep"),
        OwnedVerdict::Missing(reason) => panic!("expected drop, got missing {reason:?}"),
    }
}

/// The reference of `id` in `effects`' mutated list.
fn mutated_ref(effects: &TransactionEffects, id: &ObjectId) -> ObjectReference {
    *effects
        .mutated()
        .iter()
        .find(|mutated| mutated.reference().object_id() == id)
        .expect("the object is in the mutated list")
        .reference()
}

/// RD-8, step 1. A dynamic object field is consumed through its parent,
/// which declares no lock on it, so Check #4 never drops a transaction that
/// names the child's old version. The reader must see the consumer's row
/// above that version and drop once it is at or below the horizon, without
/// loading bytes the pruner may have removed.
#[tokio::test]
async fn lock_free_consumed_version_drops_once_the_consumer_is_at_or_below_the_horizon() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;
    let parent_effects = s.handler_known_object_basics_call(
        "create",
        create_object_args(sender),
        &gas_id,
        sender,
        &sender_key,
        4,
    );
    let parent_id = *parent_effects.created()[0].reference().object_id();
    let child_effects = s.handler_known_object_basics_call(
        "create",
        create_object_args(sender),
        &gas_id,
        sender,
        &sender_key,
        5,
    );
    let child_id = *child_effects.created()[0].reference().object_id();

    // Commit 6 hangs the child off the parent. The child is a declared input
    // here, locked and consumed; its new version is object-owned.
    let add_effects = s.handler_known_object_basics_call(
        "add_ofield",
        vec![
            CallArg::ImmutableOrOwned(s.latest_ref(&parent_id)),
            CallArg::ImmutableOrOwned(s.latest_ref(&child_id)),
        ],
        &gas_id,
        sender,
        &sender_key,
        6,
    );
    let child_under_parent = mutated_ref(&add_effects, &child_id);
    assert_eq!(
        s.handler_processed_object(&child_id, child_under_parent.version)
            .kind,
        HandlerProcessedObjectKind::Live
    );

    // Commit 8 removes it through the parent alone. The child is loaded at
    // runtime, consumed without a lock, and handed back to the sender.
    let remove_effects = s.handler_known_object_basics_call(
        "remove_ofield",
        vec![CallArg::ImmutableOrOwned(s.latest_ref(&parent_id))],
        &gas_id,
        sender,
        &sender_key,
        8,
    );
    let child_after = mutated_ref(&remove_effects, &child_id);
    assert!(child_after.version > child_under_parent.version);
    assert_eq!(
        s.handler_processed_object(&child_id, child_after.version)
            .produced_at,
        8
    );

    // Commit 9, horizon 7: the consumption is above the horizon and invisible
    // on a slower validator, so the old version keeps everywhere.
    assert_keeps(s.read_owned(9, child_under_parent), child_under_parent);

    // Commit 12, horizon 10: every validator has executed the consumption.
    // The old version is superseded everywhere, whether or not its bytes
    // are still here.
    assert_owned_drops(
        s.read_owned(12, child_under_parent),
        DropKind::HandlerRowSuperseded,
    );
}

/// RD-8, step 1. No owned lock ever covers a shared object, so a shared
/// creation row named as an owned input reaches the reader. It must drop
/// from the row's created-shared flag at every horizon, as the deletion row
/// does, without loading bytes.
#[tokio::test]
async fn shared_creation_row_named_as_an_owned_input_drops() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;
    let share_effects =
        s.handler_known_object_basics_call("share", vec![], &gas_id, sender, &sender_key, 5);
    let shared_ref = *share_effects.created()[0].reference();

    // Commit 6, horizon 4: the row is above the horizon. Commit 12, horizon
    // 10: at or below. The flag decides first either way.
    assert_owned_drops(s.read_owned(6, shared_ref), DropKind::HandlerRowNotOwned);
    assert_owned_drops(s.read_owned(12, shared_ref), DropKind::HandlerRowNotOwned);
}

/// RD-8, step 1, the record side. State sync deleted a shared object ahead
/// of the handler; the record restores its base version with the initial
/// shared version the owner carried. Named as an owned input at that base,
/// it must drop from the record's field, not keep on `base_version == V`.
#[tokio::test]
async fn sync_ahead_record_of_a_shared_object_named_as_an_owned_input_drops() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;
    let share_effects =
        s.handler_known_object_basics_call("share", vec![], &gas_id, sender, &sender_key, 5);
    let shared_id = *share_effects.created()[0].reference().object_id();
    // A later version than the creation row's, so rule 1 finds no row and
    // the record decides.
    s.handler_known_shared_object_basics_call(
        "set_value",
        vec![
            s.shared_arg(&shared_id),
            CallArg::Pure(bcs::to_bytes(&1u64).unwrap()),
        ],
        &gas_id,
        sender,
        &sender_key,
        6,
    );
    let shared_before_delete = s.latest_ref(&shared_id);
    s.assert_no_handler_row(&shared_id, shared_before_delete.version);
    s.shared_object_basics_call(
        "delete",
        vec![s.shared_arg(&shared_id)],
        &gas_id,
        sender,
        &sender_key,
    );
    assert_eq!(
        s.epoch_store
            .sync_ahead_record(&shared_id)
            .unwrap()
            .unwrap()
            .base_version,
        Some(shared_before_delete.version)
    );

    assert_owned_drops(
        s.read_owned(12, shared_before_delete),
        DropKind::SyncAheadNotOwned,
    );
}

/// A row produced at the horizon decides. One commit later it is above the
/// horizon and answers missing.
#[tokio::test]
async fn owned_row_at_the_horizon_keeps_and_above_it_answers_missing() {
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

    let tx = s.build_transfer(&obj_id, &gas_id, sender, &sender_key, sender);
    s.execute_as_handler_known(vec![tx], 10);
    let produced = s.latest_ref(&obj_id);

    assert_keeps(s.read_owned(12, produced), produced);
    assert_owned_missing(
        s.read_owned(11, produced),
        MissingKind::HandlerRowAboveHorizon,
    );
}

/// The horizon is the protocol config's distance below the commit being
/// validated.
#[tokio::test]
async fn owned_row_horizon_follows_the_protocol_config_distance() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let obj_id = ObjectId::random();
    let gas_id = ObjectId::random();
    let config_guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.enable_pcool_deterministic_validation_for_testing();
        config.set_pcool_deterministic_validation_horizon_distance_for_testing(5);
        config
    });
    let s = setup_bookkeeping_with_config_guard(
        vec![
            Object::with_id_owner_for_testing(obj_id, sender),
            Object::with_id_owner_for_testing(gas_id, sender),
        ],
        Some(config_guard),
    )
    .await;

    let tx = s.build_transfer(&obj_id, &gas_id, sender, &sender_key, sender);
    s.execute_as_handler_known(vec![tx], 10);
    let produced = s.latest_ref(&obj_id);

    assert_keeps(s.read_owned(15, produced), produced);
    assert_owned_missing(
        s.read_owned(14, produced),
        MissingKind::HandlerRowAboveHorizon,
    );
}

/// The record restores a version the store no longer holds, and its bytes
/// come from the shelter. The consumed object never reaches the store here,
/// which is what a pruned version looks like to the keyed read.
#[tokio::test]
async fn owned_bytes_are_served_from_the_shelter_when_the_store_lacks_the_version() {
    let (sender, _): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let consumed = Object::with_id_owner_version_for_testing(
        ObjectId::random(),
        Version::from_u64(3),
        Owner::Address(sender),
    );
    let consumed_ref = consumed.object_ref();
    let transaction = SenderSignedTransaction::new(
        TestTransactionBuilder::new(sender, s.latest_ref(&gas_id), s.rgp)
            .transfer_iota(None, sender)
            .build(),
        vec![],
    );
    let effects = TestEffectsBuilder::new(&transaction)
        .with_mutated_objects([(
            consumed_ref.object_id,
            consumed_ref.version,
            Owner::Address(sender),
        )])
        .build();
    // The hook shelters every consumed input from the loaded inputs it is
    // given, the gas coin included.
    let loaded_inputs = std::collections::BTreeMap::from([
        (consumed_ref.object_id, consumed),
        (gas_id, s.authority.get_object(&gas_id).unwrap()),
    ]);
    s.epoch_store
        .record_executed_transaction(
            &TransactionKey::Digest(*effects.transaction_digest()),
            &effects,
            &loaded_inputs,
        )
        .unwrap();
    assert!(
        s.store_object(&consumed_ref.object_id, consumed_ref.version)
            .is_none()
    );
    s.assert_sheltered(consumed_ref);

    assert_keeps(s.read_owned(12, consumed_ref), consumed_ref);
}

/// An object the sync-ahead chain created answers missing at every commit:
/// no other validator is required to have it.
#[tokio::test]
async fn owned_sync_created_object_answers_missing() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let (created_ref, _) = s.create_object(&gas_id, sender, &sender_key);
    assert_owned_missing(
        s.read_owned(12, created_ref),
        MissingKind::SyncAheadCreatedId,
    );
    assert_owned_missing(
        s.read_owned(100, created_ref),
        MissingKind::SyncAheadCreatedId,
    );
}

/// Rule 3, the store fallback. Untouched epoch-start objects keep, owned or
/// immutable. A version consumed by a handler-known commit drops as
/// superseded, while the same consumption ahead of the handler keeps, since
/// the record restores it.
#[tokio::test]
async fn owned_store_fallback_keeps_untouched_and_drops_superseded_versions() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let untouched_id = ObjectId::random();
    let immutable_id = ObjectId::random();
    let handler_consumed_id = ObjectId::random();
    let sync_consumed_id = ObjectId::random();
    let gas_id = ObjectId::random();
    let s = setup_bookkeeping(
        vec![
            Object::with_id_owner_for_testing(untouched_id, sender),
            Object::immutable_with_id_for_testing(immutable_id),
            Object::with_id_owner_for_testing(handler_consumed_id, sender),
            Object::with_id_owner_for_testing(sync_consumed_id, sender),
            Object::with_id_owner_for_testing(gas_id, sender),
        ],
        true,
    )
    .await;
    let untouched = s.latest_ref(&untouched_id);
    let immutable = s.latest_ref(&immutable_id);
    let handler_consumed = s.latest_ref(&handler_consumed_id);
    let sync_consumed = s.latest_ref(&sync_consumed_id);

    let tx = s.build_transfer(&handler_consumed_id, &gas_id, sender, &sender_key, sender);
    s.execute_as_handler_known(vec![tx], 5);
    s.transfer(&sync_consumed_id, &gas_id, sender, &sender_key, sender);

    assert_keeps(s.read_owned(12, untouched), untouched);
    assert_keeps(s.read_owned(12, immutable), immutable);
    assert_owned_drops(
        s.read_owned(12, handler_consumed),
        DropKind::StoreSuperseded,
    );
    assert_keeps(s.read_owned(12, sync_consumed), sync_consumed);
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
            1,
            false,
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

// ---------------------------------------------------------------------------
// Verifier limits for published packages
// ---------------------------------------------------------------------------

/// An authority whose own `VerifierSigningConfig` rejects every package, and
/// a transaction that publishes the `object_basics` test package.
struct PublishSetup {
    authority: Arc<AuthorityState>,
    tx: TransactionEnvelope,
}

/// What an operator would write in the node YAML to tighten the signing-time
/// verifier. A one-tick meter limit fails the first function it meters.
const ONE_TICK_VERIFIER_SIGNING_CONFIG_YAML: &str = concat!(
    "max-per-fun-meter-units: 1\n",
    "max-per-mod-meter-units: 1\n",
    "max-per-pkg-meter-units: 1\n",
);

/// Builds an authority from `ONE_TICK_VERIFIER_SIGNING_CONFIG_YAML` and a
/// signed transaction publishing `object_basics` from a fresh sender's gas
/// coin. Nothing is executed or validated here; each test drives the checks
/// itself after setting the protocol flags.
async fn setup_publish_with_one_tick_node_limits() -> PublishSetup {
    let node_limits: VerifierSigningConfig =
        serde_yaml::from_str(ONE_TICK_VERIFIER_SIGNING_CONFIG_YAML).unwrap();
    let authority = TestAuthorityBuilder::new()
        .with_verifier_signing_config(node_limits)
        .build()
        .await;

    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    authority.insert_genesis_object(Object::with_id_owner_for_testing(gas_id, sender));

    let gas_ref = authority.get_object(&gas_id).unwrap().object_ref();
    let rgp = authority.reference_gas_price_for_testing().unwrap();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/unit_tests/data/object_basics");
    let tx_data = TestTransactionBuilder::new(sender, gas_ref, rgp)
        .publish(path)
        .build();

    PublishSetup {
        authority,
        tx: to_sender_signed_transaction(tx_data, &sender_key),
    }
}

/// Admission on this authority rejects the package under its own limits, so
/// the post-consensus verdict below is decided by where the limits come from.
async fn assert_node_limits_reject(setup: &PublishSetup) {
    let epoch_store = setup.authority.epoch_store_for_testing();
    let admission = setup
        .authority
        .handle_transaction_validation_checks(
            &VerifiedTransaction::new_unchecked(setup.tx.clone()),
            &epoch_store,
            &setup.authority.config.transaction_deny_config,
            false,
            VerifierLimitsSource::NodeConfig(&setup.authority.config.verifier_signing_config),
        )
        .await;

    assert!(
        matches!(
            admission,
            Err(IotaError::UserInput {
                error: UserInputError::PackageVerificationTimedout { .. }
            })
        ),
        "the node's own limits must reject the package: {admission:?}"
    );
}

/// With `pcool_verifier_limits_from_protocol_config` set, post-consensus
/// validation meters a published package with the protocol config's limits.
/// A validator whose own `VerifierSigningConfig` would reject the package
/// keeps it, as every validator on default settings does.
#[tokio::test]
async fn post_consensus_validation_meters_packages_with_protocol_limits() {
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config.set_pcool_verifier_limits_from_protocol_config_for_testing(true);
        config
    });
    let setup = setup_publish_with_one_tick_node_limits().await;
    assert_node_limits_reject(&setup).await;

    let epoch_store = setup.authority.epoch_store_for_testing();
    let mut transactions = vec![make_user_tx_v1(setup.tx.clone())];
    let (dropped, locks, _) = post_consensus_validation::validate_and_resolve_conflicts(
        &setup.authority,
        &epoch_store,
        1,
        false,
        &mut transactions,
    )
    .await
    .unwrap();

    assert!(
        dropped.is_empty(),
        "post-consensus validation dropped the package even though the protocol \
            config's limits should accept it: {dropped:?}"
    );
    assert_eq!(transactions.len(), 1);
    assert_eq!(locks.len(), 1, "only the gas coin is locked");
}

/// Without the flag, post-consensus validation still meters with this
/// validator's own `VerifierSigningConfig`: the package is dropped here
/// while validators on default settings keep it.
#[tokio::test]
async fn post_consensus_validation_meters_packages_with_node_limits_when_flag_disabled() {
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config.set_pcool_verifier_limits_from_protocol_config_for_testing(false);
        config
    });
    let setup = setup_publish_with_one_tick_node_limits().await;
    assert_node_limits_reject(&setup).await;

    let epoch_store = setup.authority.epoch_store_for_testing();
    let digest = *setup.tx.digest();
    let mut transactions = vec![make_user_tx_v1(setup.tx.clone())];
    let (dropped, locks, _) = post_consensus_validation::validate_and_resolve_conflicts(
        &setup.authority,
        &epoch_store,
        1,
        false,
        &mut transactions,
    )
    .await
    .unwrap();

    assert_eq!(dropped.len(), 1);
    assert_eq!(dropped[0].0, digest);
    assert!(matches!(
        &dropped[0].1,
        IotaError::UserInput {
            error: UserInputError::PackageVerificationTimedout { .. }
        }
    ));
    assert!(transactions.is_empty());
    assert!(locks.is_empty(), "dropped transaction must not take locks");
}

// ---------------------------------------------------------------------------
// P-COOL deterministic-validation wait on the execution frontier
// ---------------------------------------------------------------------------

/// Runs one commit through the commit boundary as the handler would, at
/// `index`, with `txs` as its user transactions. Owned arguments so the
/// future can be spawned and observed while it waits.
async fn process_commit_at(
    authority: Arc<AuthorityState>,
    epoch_store: Arc<crate::authority::authority_per_epoch_store::AuthorityPerEpochStore>,
    index: CommitIndex,
    txs: Vec<VerifiedTransaction>,
) -> IotaResult<Vec<Schedulable>> {
    epoch_store
        .process_consensus_transactions_and_commit_boundary(
            txs.into_iter().map(make_user_tx_v1_verified).collect(),
            &ExecutionIndicesWithStats::default(),
            &Arc::new(CheckpointServiceNoop {}),
            authority.get_object_cache_reader().as_ref(),
            &ConsensusCommitInfo::new_for_test(index, index, 0, true),
            &authority.metrics,
            &authority,
        )
        .await
        .map(|(schedulables, _)| schedulables)
}

/// The boundary must return promptly: a wait that blocks fails the test
/// through the paused clock's timeout.
async fn process_commit_without_waiting(s: &BookkeepingSetup, index: CommitIndex) {
    tokio::time::timeout(
        Duration::from_secs(1),
        process_commit_at(s.authority.clone(), s.epoch_store.clone(), index, vec![]),
    )
    .await
    .unwrap_or_else(|_| panic!("commit {index} must not wait"))
    .unwrap();
}

#[tokio::test(start_paused = true)]
async fn validation_does_not_wait_when_the_frontier_is_at_the_horizon() {
    let s = setup_bookkeeping(vec![], true).await;
    assert_eq!(s.highest_fully_executed_commit(), 0);

    // Horizon 0 with K = 2: the frontier is already there.
    process_commit_without_waiting(&s, 2).await;
}

#[tokio::test(start_paused = true)]
async fn validation_waits_until_the_horizon_is_fully_executed() {
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
    let _watcher = s.start_execution_watcher();

    // Commit 1 is assigned and its root has not executed, so the frontier
    // stays at 0.
    let tx = s.build_transfer(&obj_id, &gas_id, sender, &sender_key, Address::random());
    s.epoch_store
        .assign_commit_to_transactions(1, vec![TransactionKey::Digest(*tx.digest())]);

    // Commit 3 has horizon 1 and must block on it.
    let mut boundary = tokio::spawn(process_commit_at(
        s.authority.clone(),
        s.epoch_store.clone(),
        3,
        vec![],
    ));
    assert!(
        tokio::time::timeout(Duration::from_millis(200), &mut boundary)
            .await
            .is_err(),
        "commit 3 must wait for commit 1"
    );

    // The root executes, the watcher completes commit 1, the wait releases.
    s.execute(tx);
    tokio::time::timeout(Duration::from_secs(10), boundary)
        .await
        .expect("commit 3 must proceed once commit 1 is fully executed")
        .unwrap()
        .unwrap();
    assert!(s.highest_fully_executed_commit() >= 1);
}

#[tokio::test(start_paused = true)]
async fn validation_does_not_wait_after_the_final_round() {
    let s = setup_bookkeeping(vec![], true).await;
    assert_eq!(s.highest_fully_executed_commit(), 0);

    // After the final round no commit makes a checkpoint, so the frontier
    // never moves again and no commit may wait on it.
    s.epoch_store
        .get_reconfig_state_write_lock_guard()
        .close_all_tx();
    process_commit_without_waiting(&s, 5).await;
}

#[tokio::test(start_paused = true)]
async fn validation_does_not_wait_with_the_flag_off() {
    let s = setup_bookkeeping(vec![], false).await;
    assert_eq!(s.highest_fully_executed_commit(), 0);

    process_commit_without_waiting(&s, 5).await;
}

#[tokio::test(start_paused = true)]
async fn epoch_end_releases_a_pending_wait() {
    let s = setup_bookkeeping(vec![], true).await;
    assert_eq!(s.highest_fully_executed_commit(), 0);

    // Horizon 3 with the frontier at 0 and no watcher: the wait is pending.
    let mut boundary = tokio::spawn(process_commit_at(
        s.authority.clone(),
        s.epoch_store.clone(),
        5,
        vec![],
    ));
    assert!(
        tokio::time::timeout(Duration::from_millis(200), &mut boundary)
            .await
            .is_err(),
        "commit 5 must wait for commit 3"
    );

    // Terminating the epoch cuts the wait short. The boundary reports it,
    // and the handler stops on that error instead of panicking.
    s.epoch_store.epoch_terminated().await;
    let result = tokio::time::timeout(Duration::from_secs(10), boundary)
        .await
        .expect("the wait must end with the epoch")
        .unwrap();
    assert!(matches!(
        result,
        Err(IotaError::EpochEnded(epoch)) if epoch == s.epoch_store.epoch()
    ));
}
