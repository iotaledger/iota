// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for post-consensus transaction validation, owned-object
//! conflict resolution, and the rows of the P-COOL handler object state
//! written by executed transactions.
//!
//! Terms used here, as in `post_consensus_input_reader`:
//! - `C` is the commit being validated, and `K` is how many commits behind `C` the object state is
//!   read: the protocol config's `pcool_deterministic_validation_horizon_distance`, which these
//!   tests set to 4 unless a test sets another. Validation at `C` reads the state as of commit `C -
//!   K`, called the horizon. So "commit 14, horizon 10" is validation at commit 14 with `K = 4`.
//!   The reader treats a row produced above the horizon, such as one produced by commit 11, as
//!   missing.
//! - A row is one entry of the handler object state (`handler_object_state`). It records that a
//!   transaction in some commit produced version `V` of object `id`: the commit, the digest, and
//!   whether the object is live, deleted or wrapped at that version. "The row at `(id, V)`" is that
//!   entry.
//! - A transaction's commit is assigned once the consensus handler has recorded which commit the
//!   transaction belongs to (`assign_commit_to_transactions`). Executing a transaction whose commit
//!   is assigned writes its rows. A transaction executed from a synced checkpoint before the
//!   handler assigned its commit writes none.
//! - The roots of a commit are the transactions the handler put in the commit's pending
//!   checkpoints. The commit is fully executed once every root has effects, and the execution
//!   watcher is the task that waits for that.
//! - "After the final round" means after this validator stopped accepting user transactions for the
//!   epoch. Commits from then on make no checkpoint and do not wait for the horizon to be fully
//!   executed.

use std::{collections::BTreeSet, path::PathBuf, str::FromStr, sync::Arc, time::Duration};

use iota_config::verifier_signing_config::VerifierSigningConfig;
use iota_macros::sim_test;
use iota_protocol_config::{OverrideGuard, ProtocolConfig};
use iota_sdk_move_types::iota_framework::{
    account::AuthenticatorFunctionRefV1Key, deny_list::AddressKey, dynamic_field::Field,
    object::UID,
};
use iota_sdk_types::{
    Address, Argument, Command, DenyRuleSet, ExecutionStatus, GasCostSummary, Identifier,
    MoveAuthenticator, MoveAuthenticatorV1, MoveStruct, ObjectDigest, ObjectId, ObjectReference,
    OwnedObjectReference, Owner, ProgrammableTransaction, SharedObjectReference, StructTag,
    Transaction, TransactionDigest, TransactionEffects, TypeTag, UserSignature, Version,
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_transaction_checks::VerifierLimitsSource;
use iota_types::{
    IOTA_FRAMEWORK_PACKAGE_ID, IOTA_SYSTEM_STATE_OBJECT_ID,
    account_abstraction::authenticator_function::{
        AuthenticatorFunctionRefV1, derive_authenticator_function_ref_v1_dynamic_field_id,
    },
    attestation::{
        Attestation, AttestationData, AttestationRecord, AttestationVerdict, AttestedTransaction,
        SharedInputIndex,
    },
    base_types::{EpochId, random_object_ref},
    crypto::{AccountPrivateKey, get_key_pair},
    deny_list_v1::{check_address_denied_by_config, get_per_type_coin_deny_list_v1},
    dynamic_field::{DynamicFieldInfo, get_dynamic_field_object_from_store},
    effects::{TestEffectsBuilder, TransactionEffectsAPI},
    error::{ExecutionErrorKind, IotaError, IotaResult, UserInputError},
    executable_transaction::VerifiedExecutableTransaction,
    messages_consensus::{ConsensusTransaction, ConsensusTransactionKind},
    move_package,
    object::Object,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    storage::{BackingPackageStore, ObjectKey},
    transaction::{
        CallArg, InputObjectKind, ObjectReadResultKind, SenderSignedTransactionAPI,
        TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS, TEST_ONLY_GAS_UNIT_FOR_PUBLISH, TransactionAPI,
        TransactionEnvelope, TransactionKey, VerifiedTransaction,
    },
    utils::{
        make_move_authenticator_sig, to_sender_signed_transaction,
        to_sender_signed_transaction_with_optional_sponsor,
    },
};

use crate::{
    authority::{
        AuthorityState, ExecutionEnv,
        authority_per_epoch_store::{
            ExecutionIndices, ExecutionIndicesWithStats, LockDetails,
            authority_per_epoch_store_tests::reopen,
            consensus_quarantine::ConsensusCommitOutput,
            handler_object_state::{
                CommitIndex, HandlerProcessedObject, HandlerProcessedObjectKind,
                handler_processed_upserts,
            },
        },
        authority_test_utils::drop_attestations,
        authority_tests::{init_state_with_objects_and_object_basics, publish_object_basics},
        move_integration_tests::build_and_try_publish_test_package,
        shared_object_version_manager::{AssignedTxAndVersions, Schedulable},
        test_authority_builder::TestAuthorityBuilder,
    },
    checkpoints::CheckpointServiceNoop,
    consensus_handler::{
        ConsensusCommitInfo, ExecutionWatcher, SequencedConsensusTransaction,
        VerifiedSequencedConsensusTransaction,
    },
    execution_scheduler::{ExecutionSchedulerAPI, transaction_manager::AttestationSource},
    post_consensus_input_reader::{
        ClaimedAddressAtHorizon, DropKind, DynamicFieldAtHorizon, InputResolution, MissingKind,
        OwnedVerdict, SharedVerdict, ValidationAtCommit, reader::CommitIndexedReader,
    },
    post_consensus_validation::{self, PostConsensusDecision, SavedPostConsensusDecision},
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
        kind: ConsensusTransactionKind::UserTransactionV1(Box::new(AttestedTransaction::new(
            tx, None,
        ))),
        tracking_id: Default::default(),
    };
    VerifiedSequencedConsensusTransaction::new_test(consensus_tx)
}

/// Wraps a `VerifiedTransaction` in a `UserTransactionV1` consensus
/// transaction.
fn make_user_tx_v1_verified(tx: VerifiedTransaction) -> VerifiedSequencedConsensusTransaction {
    let consensus_tx = ConsensusTransaction {
        kind: ConsensusTransactionKind::UserTransactionV1(Box::new(AttestedTransaction::new(
            tx.into(),
            None,
        ))),
        tracking_id: Default::default(),
    };
    VerifiedSequencedConsensusTransaction::new_test(consensus_tx)
}

/// Wraps a `Transaction` in a `UserTransactionV1` consensus transaction
/// attested by `authority_index` with `computation_units`.
fn make_attested_user_tx(
    tx: TransactionEnvelope,
    authority_index: u8,
    computation_units: u64,
) -> VerifiedSequencedConsensusTransaction {
    make_attested_user_tx_with_versions(tx, authority_index, computation_units, vec![])
}

/// Like [`make_attested_user_tx()`], with `shared_object_versions` as the
/// versions the attestation records.
fn make_attested_user_tx_with_versions(
    tx: TransactionEnvelope,
    authority_index: u8,
    computation_units: u64,
    shared_object_versions: Vec<(SharedInputIndex, Version)>,
) -> VerifiedSequencedConsensusTransaction {
    let attestation = Attestation::Authority {
        payload: AttestationData::V1 {
            computation_units,
            shared_object_versions,
        },
        authority_index,
    };
    let attested = AttestedTransaction::new(tx, Some(attestation));
    let consensus_tx = ConsensusTransaction {
        kind: ConsensusTransactionKind::UserTransactionV1(Box::new(attested)),
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
            &Default::default(),
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
            &Default::default(),
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
    let (dropped_digest, error, _) = dropped.pop().unwrap();
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

/// Runs post-consensus validation on a transfer whose Move authenticator names
/// `account` as a shared object, checks that it is dropped, and returns the
/// error it was dropped with.
///
/// The drop keeps the transaction from execution, where the check of the
/// authenticator inputs would reject the account with `NotSharedObject`: an
/// error that halts the validator, not failure effects.
async fn drop_error_for_account_named_as_shared(account: Object) -> IotaError {
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let sender: Address = account.id().into();
    let gas_id = ObjectId::random();
    let authority = TestAuthorityBuilder::new()
        .with_starting_objects(&[account, Object::with_id_owner_for_testing(gas_id, sender)])
        .build()
        .await;
    let epoch_store = authority.epoch_store_for_testing();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let gas_ref = authority.get_object(&gas_id).unwrap().object_ref();
    let tx = TestTransactionBuilder::new(sender, gas_ref, rgp)
        .transfer_iota(None, Address::random())
        .build();
    let (authenticator, _) = make_move_authenticator_sig(sender);
    let tx = TransactionEnvelope::from_user_sig_data(tx, vec![authenticator]);
    let digest = *tx.digest();
    let mut transactions = vec![make_user_tx_v1(tx)];

    let (mut dropped, locks, _) = post_consensus_validation::validate_and_resolve_conflicts(
        &authority,
        &epoch_store,
        1,
        &Default::default(),
        &mut transactions,
    )
    .await
    .unwrap();

    assert!(transactions.is_empty(), "The transaction should be dropped");
    assert!(locks.is_empty(), "A dropped transaction acquires no locks");
    assert_eq!(dropped.len(), 1);
    let (dropped_digest, error, _) = dropped.pop().unwrap();
    assert_eq!(dropped_digest, digest);

    error
}

/// An immutable account named as shared is dropped with an error that names
/// the account.
#[tokio::test]
async fn test_immutable_account_named_as_shared_dropped() {
    let account = Object::immutable_for_testing();
    let account_id = account.id();

    let error = drop_error_for_account_named_as_shared(account).await;

    assert!(
        matches!(
            error,
            IotaError::UserInput {
                error: UserInputError::InputObjectNotAvailableAtCommit {
                    object_id,
                    version: None,
                }
            } if object_id == account_id
        ),
        "unexpected error: {error:?}"
    );
}

/// An address-owned account named as shared is dropped with an error that
/// names the account.
#[tokio::test]
async fn test_owned_account_named_as_shared_dropped() {
    let account = Object::with_owner_for_testing(Address::random());
    let account_id = account.id();

    let error = drop_error_for_account_named_as_shared(account).await;

    assert!(
        matches!(
            error,
            IotaError::UserInput {
                error: UserInputError::InputObjectNotAvailableAtCommit {
                    object_id,
                    version: None,
                }
            } if object_id == account_id
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
            &Default::default(),
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
            &Default::default(),
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
            &Default::default(),
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
            &Default::default(),
            &mut transactions,
        )
        .await
        .unwrap();

    let (dropped_digests, _): (Vec<TransactionDigest>, Vec<IotaError>) = dropped
        .into_iter()
        .map(|(digest, error, _)| (digest, error))
        .unzip();

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
/// dropped by the commit-indexed reader, for which the stale version is not
/// live at the commit (Check #5), and the fresh transaction is kept and
/// acquires the lock.
#[sim_test]
async fn test_stale_version_dropped_fresh_kept() {
    telemetry_subscribers::init_for_testing();

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
            &Default::default(),
            &mut transactions,
        )
        .await
        .unwrap();

    let (dropped_digests, dropped_errors): (Vec<TransactionDigest>, Vec<IotaError>) = dropped
        .into_iter()
        .map(|(digest, error, _)| (digest, error))
        .unzip();

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
                error: UserInputError::InputObjectNotAvailableAtCommit { .. }
            }
        ),
        "Stale tx should be dropped because its version is not available at the commit, got {:?}",
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
            &Default::default(),
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
            &Default::default(),
            &mut transactions,
        )
        .await
        .unwrap();

    let (dropped_digests, _): (Vec<TransactionDigest>, Vec<IotaError>) = dropped
        .into_iter()
        .map(|(digest, error, _)| (digest, error))
        .unzip();

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
            &Default::default(),
            &mut transactions,
        )
        .await
        .unwrap();

    let (dropped_digests, _): (Vec<TransactionDigest>, Vec<IotaError>) = dropped
        .into_iter()
        .map(|(digest, error, _)| (digest, error))
        .unzip();

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
            &Default::default(),
            &mut transactions,
        )
        .await
        .unwrap();

    let (dropped_digests, _): (Vec<TransactionDigest>, Vec<IotaError>) = dropped
        .into_iter()
        .map(|(digest, error, _)| (digest, error))
        .unzip();

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
            &Default::default(),
            &mut transactions,
        )
        .await
        .unwrap();

    let (dropped_digests, _): (Vec<TransactionDigest>, Vec<IotaError>) = dropped
        .into_iter()
        .map(|(digest, error, _)| (digest, error))
        .unzip();

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
            &Default::default(),
            &mut transactions,
        )
        .await
        .unwrap();

    let (dropped_digests, _): (Vec<TransactionDigest>, Vec<IotaError>) = dropped
        .into_iter()
        .map(|(digest, error, _)| (digest, error))
        .unzip();

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

    // The transaction is executed before the consensus handler processes the
    // commit that sequences it. With the P-COOL flow a committee validator no
    // longer executes synced checkpoints within the epoch, so an earlier
    // commit kept and executed it.
    let executable = VerifiedExecutableTransaction::new_from_checkpoint(
        verified_tx.clone(),
        epoch_store.epoch(),
        // checkpoint
        1,
    );
    epoch_store.assign_to_a_new_commit_for_testing(&[executable.key()]);
    authority
        .try_execute_immediately(&executable, ExecutionEnv::new(), &epoch_store)
        .unwrap();
    assert!(
        authority
            .get_transaction_cache_reader()
            .try_is_tx_already_executed(&tx_digest)
            .unwrap(),
        "precondition: transaction should be marked executed"
    );

    // Now the committee's consensus delivers the same transaction. Process the
    // commit exactly as the consensus handler would, which builds the pending
    // checkpoint for this commit.
    let seq_tx = SequencedConsensusTransaction::new_test(ConsensusTransaction {
        kind: ConsensusTransactionKind::UserTransactionV1(Box::new(AttestedTransaction::new(
            verified_tx.into(),
            None,
        ))),
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
            kind: ConsensusTransactionKind::UserTransactionV1(Box::new(AttestedTransaction::new(
                epoch_store.verify_transaction(tx).unwrap().into(),
                None,
            ))),
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
struct LockTierSetup {
    authority: Arc<crate::authority::AuthorityState>,
    epoch_store: Arc<crate::authority::authority_per_epoch_store::AuthorityPerEpochStore>,
    sender: Address,
    sender_key: AccountPrivateKey,
    recipient: Address,
    object_ref: ObjectReference,
    gas_ref: ObjectReference,
    rgp: u64,
}

async fn setup_lock_tier() -> LockTierSetup {
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

    /// Executes `verified_tx` in a commit of its own, as when an earlier
    /// commit kept and executed the same transaction.
    fn execute_in_an_earlier_commit(&self, verified_tx: &VerifiedTransaction) {
        let executable = VerifiedExecutableTransaction::new_from_checkpoint(
            verified_tx.clone(),
            self.epoch_store.epoch(),
            1,
        );
        self.epoch_store
            .assign_to_a_new_commit_for_testing(&[executable.key()]);
        self.authority
            .try_execute_immediately(&executable, ExecutionEnv::new(), &self.epoch_store)
            .unwrap();
    }

    /// Saves a keep for `verified_tx` at commit `index`, as the first run of
    /// that commit did before a restart, so validating the commit again
    /// repeats it.
    fn save_keep(&self, index: CommitIndex, verified_tx: &VerifiedTransaction) {
        self.epoch_store
            .persist_post_consensus_decisions(
                index,
                &[(*verified_tx.digest(), SavedPostConsensusDecision::kept())],
            )
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
        &Default::default(),
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

/// Body for the Check #1 retain case: an already-executed tx kept at the
/// replayed commit, finding its OWN digest as the lock holder in the given
/// tier, must NOT be dropped and must NOT trigger `fatal!`.
async fn run_same_digest_lock_retains_already_executed(tier: LockTier) {
    let s = setup_lock_tier().await;

    let tx = s.make_tx();
    let tx_digest = *tx.digest();

    // Order matters for the Persistent tier: the lock must be acquired *before*
    // we mark the tx as executed (otherwise the perpetual
    // `live_owned_object_markers` for its inputs are already consumed).
    s.seed_lock(tier, &tx);
    s.execute_in_an_earlier_commit(&tx);
    s.save_keep(1, &tx);

    let mut transactions = vec![make_user_tx_v1_verified(tx)];
    let (dropped, _, all_digests) = post_consensus_validation::validate_and_resolve_conflicts(
        &s.authority,
        &s.epoch_store,
        1,
        &Default::default(),
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

/// Body for the self-lock case: a transaction that finds its OWN digest as the
/// holder of its inputs' locks in the given tier, from an earlier commit, must
/// NOT be dropped. Without the self-exemption in Check #4 it would conflict
/// with its own lock and drop as `ObjectLockConflict`.
///
/// Unlike the already-executed case (Check #1), the transaction is NOT executed
/// here, so it flows through the full validation pass (Check #2-#5) and must
/// survive end-to-end and re-acquire its locks.
///
/// Dedup by digest already runs upstream in Check #0, so a same-digest lock
/// seen in Check #4 is never a same-commit duplicate.
async fn run_self_lock_is_not_a_conflict(tier: LockTier) {
    let setup = setup_lock_tier().await;

    let tx = setup.make_tx();
    let tx_digest = *tx.digest();

    // Seed the tx's own lock into the tier, as an earlier commit would.
    setup.seed_lock(tier, &tx);

    let mut transactions = vec![make_user_tx_v1_verified(tx)];
    let (dropped, locks, all_digests) = post_consensus_validation::validate_and_resolve_conflicts(
        &setup.authority,
        &setup.epoch_store,
        1,
        &Default::default(),
        &mut transactions,
    )
    .await
    .unwrap();

    assert!(
        dropped.is_empty(),
        "tx must not self-conflict on its own lock from an earlier commit"
    );
    assert_eq!(transactions.len(), 1, "tx must be retained");
    assert_eq!(all_digests, vec![tx_digest], "tx digest is reported");
    assert_eq!(
        locks.get(&setup.object_ref),
        Some(&tx_digest),
        "tx must re-acquire its own object lock"
    );
    assert_eq!(
        locks.get(&setup.gas_ref),
        Some(&tx_digest),
        "tx must re-acquire its own gas lock"
    );
}

#[tokio::test]
async fn tier2_quarantine_self_lock_is_not_a_conflict() {
    run_self_lock_is_not_a_conflict(LockTier::Quarantine).await;
}

#[tokio::test]
async fn tier3_persistent_self_lock_is_not_a_conflict() {
    run_self_lock_is_not_a_conflict(LockTier::Persistent).await;
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

/// Check #1 / `fatal!` invariant: when an already-executed transaction kept at
/// the replayed commit has an owned input locked by a DIFFERENT transaction
/// digest,
/// `validate_and_resolve_conflicts` must panic — the executed-but-out-locked
/// state is a real consistency violation, not a recoverable conflict.
///
/// Uses the Tier 2 (quarantine) seeding path; the helper is tier-agnostic, so a
/// single test covers both quarantine and persistent-DB code paths.
#[tokio::test]
#[should_panic(expected = "locked by a different transaction")]
async fn already_executed_tx_locked_by_different_digest_is_fatal() {
    let s = setup_lock_tier().await;

    // The tx the committee actually executed (by an earlier commit on this node).
    let tx = s.make_tx();
    s.execute_in_an_earlier_commit(&tx);
    s.save_keep(1, &tx);

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
        &Default::default(),
        &mut transactions,
    )
    .await;
}

/// An attested transaction whose `authority_index` matches the block's
/// `certificate_author_index` (both `0`) passes attestor verification (Check
/// #3) and is treated the same as an unattested one.
#[tokio::test]
async fn test_attested_tx_passes() {
    let _guard = enable_validator_attestation();

    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let recipient = get_key_pair::<AccountPrivateKey>().0;

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
    let digest = *tx.digest();

    // authority_index 0 == certificate_author_index 0 set by new_test → match.
    let protocol_config = epoch_store.protocol_config();
    let min_units = protocol_config
        .base_tx_cost_fixed()
        .min(protocol_config.gas_rounding_step());
    let mut transactions = vec![make_attested_user_tx(tx, 0, min_units)];

    let (dropped, locks, user_tx_digests) =
        post_consensus_validation::validate_and_resolve_conflicts(
            &authority,
            &epoch_store,
            1,
            &Default::default(),
            &mut transactions,
        )
        .await
        .unwrap();

    assert_eq!(
        transactions.len(),
        1,
        "an attested transaction with a matching attestor is kept"
    );
    assert!(dropped.is_empty(), "No errors expected");
    assert_eq!(
        locks.len(),
        2,
        "Locks for object and gas should be acquired"
    );
    assert_eq!(user_tx_digests, vec![digest]);
}

/// An attested transaction whose `authority_index` does not match the block's
/// `certificate_author_index` is dropped via Check #3. Critically, its digest
/// must still appear in `all_user_tx_digests` so the caller can release the
/// pre-consensus soft lock — this is the invariant the loop restructure
/// protects.
#[tokio::test]
async fn test_attested_tx_attestor_mismatch() {
    let _guard = enable_validator_attestation();

    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let recipient = get_key_pair::<AccountPrivateKey>().0;

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
    let digest = *tx.digest();

    // authority_index 1 != certificate_author_index 0 → mismatch.
    // Cost is also below the protocol floor (`min_units - 1`), so BOTH the
    // mismatch and the floor checks would fire. This pins the check order:
    // mismatch must be reported before the floor.
    let protocol_config = epoch_store.protocol_config();
    let min_units = protocol_config
        .base_tx_cost_fixed()
        .min(protocol_config.gas_rounding_step());
    let mut transactions = vec![make_attested_user_tx(tx, 1, min_units - 1)];

    let (dropped, locks, user_tx_digests) =
        post_consensus_validation::validate_and_resolve_conflicts(
            &authority,
            &epoch_store,
            1,
            &Default::default(),
            &mut transactions,
        )
        .await
        .unwrap();

    assert!(
        transactions.is_empty(),
        "a mismatched attestation removes the transaction from the batch"
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

/// An attested transaction whose attestation reports `computation_units` outside
/// the valid range is malformed and dropped via Check #3:
/// - below `min(base_tx_cost_fixed, gas_rounding_step)` → `AttestationUnitsBelowMinimum` (no honest
///   dry-run meters below the bucketization floor);
/// - above `gas_budget / gas_price` → `AttestationUnitsAboveBudget` (an honest dry-run cannot meter
///   more computation than the tx can pay for).
///
/// In both cases the digest must still surface in `all_user_tx_digests` for
/// soft-lock release.
#[tokio::test]
async fn test_attested_tx_cost_out_of_bounds() {
    let _guard = enable_validator_attestation();

    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();

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

    let reference_tx = make_transfer_object_transaction(
        object_ref,
        gas_ref,
        sender,
        &sender_key,
        Address::random(),
        rgp,
    );
    let protocol_config = epoch_store.protocol_config();
    let min_units = protocol_config
        .base_tx_cost_fixed()
        .min(protocol_config.gas_rounding_step());
    let ref_data = reference_tx.data().transaction();
    let max_units = ref_data.gas_budget() / ref_data.gas_price();

    // One case just below the minimum, one just above the maximum, each a
    // distinct transaction at a commit of its own.
    for (commit_index, (attested_units, expect_below)) in
        [(min_units - 1, true), (max_units + 1, false)]
            .into_iter()
            .enumerate()
    {
        let tx = make_transfer_object_transaction(
            object_ref,
            gas_ref,
            sender,
            &sender_key,
            Address::random(),
            rgp,
        );
        let digest = *tx.digest();
        let mut transactions = vec![make_attested_user_tx(tx, 0, attested_units)];

        let (dropped, locks, user_tx_digests) =
            post_consensus_validation::validate_and_resolve_conflicts(
                &authority,
                &epoch_store,
                commit_index as CommitIndex + 1,
                &Default::default(),
                &mut transactions,
            )
            .await
            .unwrap();

        assert!(
            transactions.is_empty(),
            "malformed units remove the transaction from the batch"
        );
        assert_eq!(
            dropped.len(),
            1,
            "malformed cost should produce one dropped entry"
        );
        match &dropped[0].1 {
            IotaError::AttestationUnitsBelowMinimum { actual, minimum } if expect_below => {
                assert_eq!(*actual, min_units - 1);
                assert_eq!(*minimum, min_units);
            }
            IotaError::AttestationUnitsAboveBudget { actual, maximum } if !expect_below => {
                assert_eq!(*actual, max_units + 1);
                assert_eq!(*maximum, max_units);
            }
            other => panic!("unexpected error for attested_units={attested_units}: {other:?}"),
        }
        assert!(
            locks.is_empty(),
            "no locks should be acquired for a dropped transaction"
        );
        assert_eq!(
            user_tx_digests,
            vec![digest],
            "digest must be collected before Check #3 for soft-lock release",
        );
    }
}

/// An attested transaction whose attestation records a version at an index
/// where the transaction names no shared object is malformed and dropped by
/// Check #3. A transfer signed with a key has no Move authenticator, so the
/// index of the account of authenticator 0 is such an index. The digest must
/// still surface in `all_user_tx_digests` for soft-lock release.
#[tokio::test]
async fn attested_version_at_index_without_shared_object_is_dropped() {
    let _guard = enable_validator_attestation();

    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();

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

    let tx = make_transfer_object_transaction(
        object_ref,
        gas_ref,
        sender,
        &sender_key,
        Address::random(),
        rgp,
    );
    let digest = *tx.digest();
    // The most units the transaction pays for, so Check #3 passes the units.
    let units = tx.data().transaction().gas_budget() / tx.data().transaction().gas_price();
    let index = SharedInputIndex::AuthenticatorAccount { authenticator: 0 };
    let mut transactions = vec![make_attested_user_tx_with_versions(
        tx,
        0,
        units,
        vec![(index, object_ref.version)],
    )];

    let (dropped, locks, user_tx_digests) =
        post_consensus_validation::validate_and_resolve_conflicts(
            &authority,
            &epoch_store,
            1,
            &Default::default(),
            &mut transactions,
        )
        .await
        .unwrap();

    assert!(
        transactions.is_empty(),
        "the malformed record removes the transaction from the batch"
    );
    assert_eq!(dropped.len(), 1, "one dropped entry");
    assert_eq!(
        dropped[0].1,
        IotaError::AttestationIndexNamesNoSharedObject { index }
    );
    assert!(
        locks.is_empty(),
        "no locks should be acquired for a dropped transaction"
    );
    assert_eq!(
        user_tx_digests,
        vec![digest],
        "digest must be collected before Check #3 for soft-lock release",
    );
}

/// Returns a transfer from the Move account `sender` whose gas `sponsor`
/// pays, signed with `authenticators` in that order. The gas coin is not in
/// the store: the tests that use it drop the transfer before its inputs are
/// read.
fn build_account_transfer(
    sender: ObjectId,
    sponsor: ObjectId,
    authenticators: Vec<MoveAuthenticatorV1>,
    gas_price: u64,
) -> TransactionEnvelope {
    let mut builder = ProgrammableTransactionBuilder::new();
    builder.transfer_iota(Address::random(), Some(1));
    let transfer = Transaction::new_programmable_allow_sponsor(
        sender.into(),
        vec![random_object_ref()],
        builder.finish(),
        TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS * gas_price,
        gas_price,
        sponsor.into(),
    );
    let signatures = authenticators
        .into_iter()
        .map(|authenticator| {
            UserSignature::MoveAuthenticator(MoveAuthenticator::from(authenticator))
        })
        .collect();

    TransactionEnvelope::from_user_sig_data(transfer, signatures)
}

/// Runs post-consensus validation on `tx`, attested by the block author with
/// `shared_object_versions` as its record and the most units `tx` pays for.
/// Asserts that `tx` is dropped without taking a lock, and returns the error
/// it is dropped with.
async fn expect_attested_drop(
    authority: &AuthorityState,
    tx: TransactionEnvelope,
    shared_object_versions: Vec<(SharedInputIndex, Version)>,
) -> IotaError {
    let epoch_store = authority.epoch_store_for_testing();
    let digest = *tx.digest();
    let units = tx.data().transaction().gas_budget() / tx.data().transaction().gas_price();
    let mut transactions = vec![make_attested_user_tx_with_versions(
        tx,
        0,
        units,
        shared_object_versions,
    )];

    let (mut dropped, locks, _) = post_consensus_validation::validate_and_resolve_conflicts(
        authority,
        &epoch_store,
        1,
        &Default::default(),
        &mut transactions,
    )
    .await
    .unwrap();

    assert!(transactions.is_empty(), "the transaction must be dropped");
    assert!(locks.is_empty(), "a dropped transaction acquires no locks");
    assert_eq!(dropped.len(), 1, "one dropped entry");
    let (dropped_digest, error, _) = dropped.pop().unwrap();
    assert_eq!(dropped_digest, digest);

    error
}

/// The record names the account of the Move authenticator twice, with the
/// same version.
#[tokio::test]
async fn attested_record_naming_index_twice_is_dropped() {
    let _guard = enable_validator_attestation();
    let authority = TestAuthorityBuilder::new().build().await;
    let rgp = authority.reference_gas_price_for_testing().unwrap();
    let account = ObjectId::random();
    let tx = build_account_transfer(account, account, vec![account_authenticator(account)], rgp);
    let index = SharedInputIndex::AuthenticatorAccount { authenticator: 0 };

    let error = expect_attested_drop(
        &authority,
        tx,
        vec![
            (index, Version::OBJECT_START),
            (index, Version::OBJECT_START),
        ],
    )
    .await;

    assert_eq!(error, IotaError::AttestationIndexRepeated { index });
}

/// The sponsor's Move authenticator takes the sender's account as an
/// argument, and the record gives the account a different version at each
/// of its two indexes.
#[tokio::test]
async fn attested_record_with_two_versions_of_one_object_is_dropped() {
    let _guard = enable_validator_attestation();
    let authority = TestAuthorityBuilder::new().build().await;
    let rgp = authority.reference_gas_price_for_testing().unwrap();
    let (sender, sponsor) = (ObjectId::random(), ObjectId::random());
    let sponsor_authenticator = MoveAuthenticatorV1::new(
        vec![CallArg::Shared(SharedObjectReference::new(
            sender,
            Version::OBJECT_START,
            false,
        ))],
        vec![],
        SharedObjectReference::new(sponsor, Version::OBJECT_START, false),
    );
    let tx = build_account_transfer(
        sender,
        sponsor,
        vec![account_authenticator(sender), sponsor_authenticator],
        rgp,
    );

    let error = expect_attested_drop(
        &authority,
        tx,
        vec![
            (
                SharedInputIndex::AuthenticatorAccount { authenticator: 0 },
                Version::from(1u64),
            ),
            (
                SharedInputIndex::AuthenticatorArgument {
                    authenticator: 1,
                    argument: 0,
                },
                Version::from(2u64),
            ),
            (
                SharedInputIndex::AuthenticatorAccount { authenticator: 1 },
                Version::from(1u64),
            ),
        ],
    )
    .await;

    assert_eq!(
        error,
        IotaError::AttestationObjectVersionsDiffer {
            object_id: sender,
            first: Version::from(1u64),
            second: Version::from(2u64),
        }
    );
}

/// The record has no entry for the account of the Move authenticator.
#[tokio::test]
async fn attested_record_missing_authenticator_input_is_dropped() {
    let _guard = enable_validator_attestation();
    let authority = TestAuthorityBuilder::new().build().await;
    let rgp = authority.reference_gas_price_for_testing().unwrap();
    let account = ObjectId::random();
    let tx = build_account_transfer(account, account, vec![account_authenticator(account)], rgp);

    let error = expect_attested_drop(&authority, tx, vec![]).await;

    assert_eq!(
        error,
        IotaError::AttestationAuthenticatorInputMissing {
            index: SharedInputIndex::AuthenticatorAccount { authenticator: 0 },
        }
    );
}

/// An attested transaction whose declared gas price or budget is out of the
/// protocol bounds is dropped post-consensus by the payload-only gas-bounds
/// check, before version assignment. The balance check still runs at execution.
/// Two representative cases: gas price below the reference gas price, and gas
/// budget above the protocol maximum.
#[tokio::test]
async fn test_attested_tx_gas_bounds_out_of_range_dropped() {
    let _guard = enable_validator_attestation();

    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let recipient = get_key_pair::<AccountPrivateKey>().0;

    let object_id = ObjectId::random();
    let gas_id = ObjectId::random();

    let (authority, _) = init_state_with_objects_and_object_basics(vec![
        Object::with_id_owner_for_testing(object_id, sender),
        Object::with_id_owner_for_testing(gas_id, sender),
    ])
    .await;

    let epoch_store = authority.epoch_store_for_testing();
    let rgp = authority.reference_gas_price_for_testing().unwrap();
    assert!(rgp > 0, "test requires a non-zero reference gas price");

    let protocol_config = epoch_store.protocol_config();
    let min_units = protocol_config
        .base_tx_cost_fixed()
        .min(protocol_config.gas_rounding_step());
    let max_tx_gas = protocol_config.max_tx_gas();

    let object_ref = authority.get_object(&object_id).unwrap().object_ref();
    let gas_ref = authority.get_object(&gas_id).unwrap().object_ref();

    // Gas price below the reference gas price → `GasPriceUnderRGP`.
    let price_under_rgp = make_transfer_object_transaction(
        object_ref,
        gas_ref,
        sender,
        &sender_key,
        recipient,
        rgp - 1,
    );
    // Gas budget above the protocol maximum → `GasBudgetTooHigh`.
    let budget_too_high = to_sender_signed_transaction(
        Transaction::new_transfer(recipient, object_ref, sender, gas_ref, max_tx_gas + 1, rgp),
        &sender_key,
    );

    // Each transaction is paired with the error it must be dropped for; a matcher
    // accepting either would pass even if both tripped the same bound. Each is
    // validated at a commit of its own.
    for (commit_index, (tx, expected_error)) in [
        (
            price_under_rgp,
            UserInputError::GasPriceUnderRGP {
                gas_price: rgp - 1,
                reference_gas_price: rgp,
            },
        ),
        (
            budget_too_high,
            UserInputError::GasBudgetTooHigh {
                gas_budget: max_tx_gas + 1,
                max_budget: max_tx_gas,
            },
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let digest = *tx.digest();
        let mut transactions = vec![make_attested_user_tx(tx, 0, min_units)];

        let (dropped, locks, user_tx_digests) =
            post_consensus_validation::validate_and_resolve_conflicts(
                &authority,
                &epoch_store,
                commit_index as CommitIndex + 1,
                &Default::default(),
                &mut transactions,
            )
            .await
            .unwrap();

        assert!(
            transactions.is_empty(),
            "out-of-bounds gas drops the attested transaction"
        );
        assert_eq!(dropped.len(), 1, "one dropped entry expected");
        assert_eq!(
            dropped[0].1,
            IotaError::UserInput {
                error: expected_error
            },
            "unexpected drop reason",
        );
        assert!(
            locks.is_empty(),
            "no locks should be acquired for a dropped transaction"
        );
        assert_eq!(user_tx_digests, vec![digest]);
    }
}

// ---------------------------------------------------------------------------
// Input checks for attested transactions
// ---------------------------------------------------------------------------

/// The input of a transfer that its signer does not own.
#[derive(Clone, Copy, Debug)]
enum ForeignInput {
    /// The transferred object.
    Object,
    /// The gas coin.
    Gas,
}

/// Turns validator attestation on while the guard lives.
fn enable_validator_attestation() -> OverrideGuard {
    ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config.set_enable_validator_attestation_for_testing(true);
        config.set_checkpoint_summary_version_specific_data_for_testing(2);
        config
    })
}

/// Turns validator attestation and the attestor cutoff on while the guard
/// lives. One refuted attestation of 10 units, 100 squared, takes an
/// attestor's whole score.
fn enable_attestor_cutoff() -> OverrideGuard {
    ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config.set_enable_validator_attestation_for_testing(true);
        config.set_checkpoint_summary_version_specific_data_for_testing(2);
        config.set_attestation_max_refuted_units_squared_for_testing(100);
        config
    })
}

/// A fresh authority holding `object` and `gas`, with their live references.
async fn setup_transfer_inputs(
    object: Object,
    gas: Object,
) -> (
    Arc<crate::authority::AuthorityState>,
    ObjectReference,
    ObjectReference,
) {
    let (object_id, gas_id) = (object.id(), gas.id());
    let (authority, _) = init_state_with_objects_and_object_basics(vec![object, gas]).await;
    let object_ref = authority.get_object(&object_id).unwrap().object_ref();
    let gas_ref = authority.get_object(&gas_id).unwrap().object_ref();
    (authority, object_ref, gas_ref)
}

/// A transfer validly signed by its sender whose `foreign` input belongs to
/// another address, referenced at its live version and digest.
async fn setup_foreign_input(
    foreign: ForeignInput,
) -> (Arc<crate::authority::AuthorityState>, TransactionEnvelope) {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let other_owner = Address::random();
    let (object_owner, gas_owner) = match foreign {
        ForeignInput::Object => (other_owner, sender),
        ForeignInput::Gas => (sender, other_owner),
    };
    let (authority, object_ref, gas_ref) = setup_transfer_inputs(
        Object::with_id_owner_for_testing(ObjectId::random(), object_owner),
        Object::with_id_owner_for_testing(ObjectId::random(), gas_owner),
    )
    .await;
    let rgp = authority.reference_gas_price_for_testing().unwrap();
    let tx = make_transfer_object_transaction(
        object_ref,
        gas_ref,
        sender,
        &sender_key,
        Address::random(),
        rgp,
    );
    (authority, tx)
}

/// `tx` attested by the block author of
/// `SequencedConsensusTransaction::new_test` (index 0) with the minimum units
/// Check #3 accepts.
fn attested_by_block_author(
    tx: TransactionEnvelope,
    protocol_config: &ProtocolConfig,
) -> ConsensusTransaction {
    let computation_units = protocol_config
        .base_tx_cost_fixed()
        .min(protocol_config.gas_rounding_step());
    ConsensusTransaction::new_attested_user_transaction(AttestedTransaction::new(
        tx,
        Some(Attestation::Authority {
            payload: AttestationData::V1 {
                computation_units,
                shared_object_versions: vec![],
            },
            authority_index: 0,
        }),
    ))
}

/// Runs one commit holding `consensus_tx` through the consensus handler and
/// returns the error `digest` was dropped with. Fails if it was scheduled.
async fn dropped_by_consensus_handler(
    authority: &crate::authority::AuthorityState,
    consensus_tx: ConsensusTransaction,
    digest: TransactionDigest,
) -> IotaError {
    let epoch_store = authority.epoch_store_for_testing();
    let (scheduled, _) = epoch_store
        .process_consensus_transactions_for_tests(
            vec![SequencedConsensusTransaction::new_test(consensus_tx)],
            &Arc::new(CheckpointServiceNoop {}),
            authority.get_object_cache_reader().as_ref(),
            &authority.metrics,
            // skip_consensus_commit_prologue_in_test
            true,
            authority,
        )
        .await
        .unwrap();
    assert!(
        scheduled
            .iter()
            .all(|scheduled| scheduled.as_tx().is_none_or(|tx| *tx.digest() != digest)),
        "transaction {digest:?} must not be scheduled"
    );
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        epoch_store.notify_read_dropped_digests(digest),
    )
    .await
    .expect("a dropped transaction has its error recorded")
}

/// An attested transaction spending another address's object or gas coin is
/// dropped post-consensus. At execution the same error is `fatal!`.
#[tokio::test]
async fn attested_tx_with_foreign_owned_input_is_dropped() {
    let _guard = enable_validator_attestation();

    for foreign in [ForeignInput::Object, ForeignInput::Gas] {
        let (authority, tx) = setup_foreign_input(foreign).await;
        let digest = *tx.digest();
        let consensus_tx =
            attested_by_block_author(tx, authority.epoch_store_for_testing().protocol_config());

        let error = dropped_by_consensus_handler(&authority, consensus_tx, digest).await;

        assert!(
            matches!(
                error,
                IotaError::UserInput {
                    error: UserInputError::IncorrectUserSignature { .. }
                }
            ),
            "{foreign:?}: expected an ownership error, got {error:?}"
        );
    }
}

/// Without an attestation the same transactions are dropped by the
/// `UserTransactionV1` checks, with the same error.
#[tokio::test]
async fn unattested_tx_with_foreign_owned_input_is_dropped() {
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    for foreign in [ForeignInput::Object, ForeignInput::Gas] {
        let (authority, tx) = setup_foreign_input(foreign).await;
        let digest = *tx.digest();

        let error = dropped_by_consensus_handler(
            &authority,
            ConsensusTransaction::new_user_transaction(tx),
            digest,
        )
        .await;

        assert!(
            matches!(
                error,
                IotaError::UserInput {
                    error: UserInputError::IncorrectUserSignature { .. }
                }
            ),
            "{foreign:?}: expected an ownership error, got {error:?}"
        );
    }
}

/// An attested transaction naming an input at its live version with another
/// digest is dropped post-consensus by the reader, as an unattested one is.
#[tokio::test]
async fn attested_tx_with_mismatched_digest_is_dropped() {
    let _guard = enable_validator_attestation();
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let (authority, object_ref, gas_ref) = setup_transfer_inputs(
        Object::with_id_owner_for_testing(ObjectId::random(), sender),
        Object::with_id_owner_for_testing(ObjectId::random(), sender),
    )
    .await;
    let mismatched_ref = ObjectReference::new(
        object_ref.object_id,
        object_ref.version,
        random_object_ref().digest,
    );
    let rgp = authority.reference_gas_price_for_testing().unwrap();
    let tx = make_transfer_object_transaction(
        mismatched_ref,
        gas_ref,
        sender,
        &sender_key,
        Address::random(),
        rgp,
    );
    let digest = *tx.digest();
    let consensus_tx =
        attested_by_block_author(tx, authority.epoch_store_for_testing().protocol_config());

    let error = dropped_by_consensus_handler(&authority, consensus_tx, digest).await;

    assert!(
        matches!(
            error,
            IotaError::UserInput {
                error: UserInputError::InputObjectNotAvailableAtCommit { .. }
            }
        ),
        "expected the reader's drop, got {error:?}"
    );
}

/// An attested transaction whose gas coin cannot cover its budget is dropped
/// post-consensus.
#[tokio::test]
async fn attested_tx_with_underfunded_gas_is_dropped() {
    let _guard = enable_validator_attestation();
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let (authority, object_ref, gas_ref) = setup_transfer_inputs(
        Object::with_id_owner_for_testing(ObjectId::random(), sender),
        Object::with_id_owner_gas_for_testing(ObjectId::random(), sender, 1),
    )
    .await;
    let rgp = authority.reference_gas_price_for_testing().unwrap();
    let tx = make_transfer_object_transaction(
        object_ref,
        gas_ref,
        sender,
        &sender_key,
        Address::random(),
        rgp,
    );
    let digest = *tx.digest();
    let consensus_tx =
        attested_by_block_author(tx, authority.epoch_store_for_testing().protocol_config());

    let error = dropped_by_consensus_handler(&authority, consensus_tx, digest).await;

    assert!(
        matches!(
            error,
            IotaError::UserInput {
                error: UserInputError::GasBalanceTooLow { .. }
            }
        ),
        "expected a balance error, got {error:?}"
    );
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

/// With the P-COOL flow, post-consensus validation drops a
/// transaction whose sender is denied by the consensus-governed active set.
#[sim_test]
async fn post_consensus_validation_uses_governance_rules() {
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
        &Default::default(),
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

/// With the P-COOL flow and active rules denying an unrelated
/// address, a non-denied sender's transaction is kept and takes its locks.
#[sim_test]
async fn post_consensus_validation_keeps_non_denied_transactions() {
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
        &Default::default(),
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

/// A sender denied by the active set is dropped, and after a newer-generation
/// proposal withdraws the rules the same sender's transaction is kept.
#[sim_test]
async fn post_consensus_validation_applies_relaxed_rules() {
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
        &Default::default(),
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
        // The next commit: commit 1 saved its decisions above.
        2,
        &Default::default(),
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
// P-COOL handler object state: rows written when a transaction executes
// (`record_executed_transaction`)
// ---------------------------------------------------------------------------

/// Authority and epoch store for the tests of the rows of the handler object
/// state. With `validation_enabled`, the P-COOL protocol flags are overridden
/// for as long as the setup lives. Rows are written for an executed
/// transaction only if its commit was assigned beforehand with
/// `assign_commit_to_transactions`.
struct HandlerObjectStateSetup {
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

/// What one validation of attested copies decided: the digest and block
/// author of each kept copy, and each drop with its decision.
#[derive(Debug, PartialEq, Eq)]
struct ValidatedCopies {
    kept: Vec<(TransactionDigest, u32)>,
    dropped: Vec<(TransactionDigest, IotaError, PostConsensusDecision)>,
}

async fn setup_handler_object_state(
    genesis_objects: Vec<Object>,
    validation_enabled: bool,
) -> HandlerObjectStateSetup {
    let config_guard = (!validation_enabled).then(|| {
        ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_enable_pcool_flow_for_testing(false);
            config
        })
    });
    setup_handler_object_state_with_config_guard(genesis_objects, config_guard).await
}

/// Like [`setup_handler_object_state`] with the flags on, but for a node whose authority
/// key is not in the committee, as on a full node.
async fn setup_handler_object_state_outside_committee(
    genesis_objects: Vec<Object>,
) -> HandlerObjectStateSetup {
    // The builder's genesis committee holds its own generated validator keys,
    // so a node signing with a fresh key is outside it.
    let (_, keypair): (_, iota_types::crypto::AuthorityKeyPair) = get_key_pair();
    build_handler_object_state_setup(genesis_objects, None, Some(&keypair), None).await
}

/// Like [`setup_handler_object_state`], under the protocol config override the caller
/// installed. The guard lives as long as the setup.
async fn setup_handler_object_state_with_config_guard(
    genesis_objects: Vec<Object>,
    config_guard: Option<OverrideGuard>,
) -> HandlerObjectStateSetup {
    build_handler_object_state_setup(genesis_objects, config_guard, None, None).await
}

/// Like [`setup_handler_object_state`] with the flags on, but for a validator
/// whose local config is `deny_config`.
async fn setup_handler_object_state_with_deny_config(
    genesis_objects: Vec<Object>,
    deny_config: iota_config::transaction_deny_config::TransactionDenyConfig,
) -> HandlerObjectStateSetup {
    build_handler_object_state_setup(genesis_objects, None, None, Some(deny_config)).await
}

async fn build_handler_object_state_setup(
    genesis_objects: Vec<Object>,
    _config_guard: Option<OverrideGuard>,
    keypair: Option<&iota_types::crypto::AuthorityKeyPair>,
    deny_config: Option<iota_config::transaction_deny_config::TransactionDenyConfig>,
) -> HandlerObjectStateSetup {
    let mut builder = match keypair {
        Some(keypair) => TestAuthorityBuilder::new().with_keypair(keypair),
        None => TestAuthorityBuilder::new(),
    };
    if let Some(deny_config) = deny_config {
        builder = builder.with_transaction_deny_config(deny_config);
    }
    let authority = builder.build().await;
    for object in genesis_objects {
        authority.insert_genesis_object(object);
    }
    let (authority, package) = publish_object_basics(authority).await;
    let epoch_store = (*authority.epoch_store_for_testing()).clone();
    let rgp = authority.reference_gas_price_for_testing().unwrap();
    HandlerObjectStateSetup {
        authority,
        epoch_store,
        package_id: package.object_id,
        rgp,
        _config_guard,
    }
}

/// Like [`setup_handler_object_state`] with the flags on, but with the given test
/// package instead of `object_basics`, inserted at genesis like it, so the
/// package is epoch-start state. `gas_id` is created at genesis for `sender`
/// and funds the test's transactions.
async fn setup_handler_object_state_with_package(
    package_name: &str,
    sender: Address,
    gas_id: ObjectId,
) -> HandlerObjectStateSetup {
    setup_handler_object_state_with_package_and_config_guard(package_name, sender, gas_id, None)
        .await
}

/// [`setup_handler_object_state_with_package`] under the protocol config override the
/// caller installed, if any. The guard lives as long as the setup.
async fn setup_handler_object_state_with_package_and_config_guard(
    package_name: &str,
    sender: Address,
    gas_id: ObjectId,
    _config_guard: Option<OverrideGuard>,
) -> HandlerObjectStateSetup {
    let authority = TestAuthorityBuilder::new().build().await;
    authority.insert_genesis_object(Object::with_id_owner_for_testing(gas_id, sender));
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/unit_tests/data")
        .join(package_name);
    let modules: Vec<_> = iota_move_build::BuildConfig::new_for_testing()
        .build(&path)
        .unwrap()
        .get_modules()
        .cloned()
        .collect();
    let package = Object::new_package_for_testing(
        &modules,
        TransactionDigest::GENESIS_MARKER,
        iota_framework::BuiltInFramework::genesis_move_packages(),
    )
    .unwrap();
    let package_id = package.id();
    authority.insert_genesis_object(package);
    let epoch_store = (*authority.epoch_store_for_testing()).clone();
    let rgp = authority.reference_gas_price_for_testing().unwrap();
    HandlerObjectStateSetup {
        authority,
        epoch_store,
        package_id,
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

impl HandlerObjectStateSetup {
    /// The latest reference of `id` in the authority's view.
    fn latest_ref(&self, id: &ObjectId) -> ObjectReference {
        self.authority.get_object(id).unwrap().object_ref()
    }

    /// A keyed read of `(id, version)` from the object store, the way
    /// validation's content read would issue it, including the version the
    /// commit moved into this epoch's historic objects bucket; `None` for
    /// tombstones.
    fn store_object(&self, id: &ObjectId, version: Version) -> Option<Object> {
        self.authority
            .get_object_cache_reader()
            .try_get_object_by_key_with_historic_fallback(id, version, self.epoch_store.epoch())
            .unwrap()
    }

    /// Executes `tx` and checks that it succeeded. `new_from_checkpoint` is
    /// used only because it builds an executable transaction without a
    /// certificate. With the handler object state kept, the handler must have
    /// assigned the transaction's commit first, as
    /// [`Self::execute_with_assigned_commit`] does; otherwise
    /// `record_executed_transaction` panics.
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

    /// The row at `(id, version)`, which must exist.
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

    /// Asserts that there is no row at `(id, version)`.
    #[track_caller]
    fn assert_no_handler_processed_object(&self, id: &ObjectId, version: Version) {
        assert_eq!(
            self.epoch_store
                .handler_processed_object(&ObjectKey(*id, version))
                .unwrap(),
            None
        );
    }

    /// Assigns commit `index` to `txs`, then executes them in order, so their
    /// rows are written. A commit is assigned once, so every transaction of a
    /// commit goes in one call.
    fn execute_with_assigned_commit(
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

    /// [`Self::execute_with_assigned_commit`] of a call into the `object_basics`
    /// module.
    fn assigned_commit_object_basics_call(
        &self,
        function: &'static str,
        args: Vec<CallArg>,
        gas_id: &ObjectId,
        sender: Address,
        sender_key: &AccountPrivateKey,
        index: CommitIndex,
    ) -> TransactionEffects {
        self.assigned_commit_move_call(
            "object_basics",
            function,
            args,
            gas_id,
            sender,
            sender_key,
            index,
        )
    }

    /// [`Self::execute_with_assigned_commit`] of a call into `module::function`
    /// of the published test package.
    fn assigned_commit_move_call(
        &self,
        module: &'static str,
        function: &'static str,
        args: Vec<CallArg>,
        gas_id: &ObjectId,
        sender: Address,
        sender_key: &AccountPrivateKey,
        index: CommitIndex,
    ) -> TransactionEffects {
        let tx = self.build_move_call(module, function, args, gas_id, sender, sender_key);
        self.execute_with_assigned_commit(vec![tx], index).remove(0)
    }

    /// Commits the outputs of `committed` to the database in order, as the
    /// checkpoint executor does. `committed` must hold, oldest first, every
    /// uncommitted transaction that wrote the objects they touched.
    fn commit_outputs(&self, committed: &[TransactionDigest]) {
        self.commit_outputs_in_epoch(self.epoch_store.epoch(), committed);
    }

    /// [`Self::commit_outputs`] in `epoch`, the one the transactions executed
    /// in: the cache records the epoch a version was superseded in and expects
    /// the commit to name the same one.
    fn commit_outputs_in_epoch(&self, epoch: EpochId, committed: &[TransactionDigest]) {
        let cache_commit = self.authority.get_cache_commit();
        let batch = cache_commit.build_db_batch(epoch, 1, committed);
        cache_commit.commit_transaction_outputs(epoch, batch, committed);
    }

    /// [`Self::commit_outputs`] of every executed transaction not committed
    /// yet, oldest first and epoch by epoch, and clears the execution caches,
    /// as after the checkpoints holding them executed and a restart.
    fn commit_all_executed_and_clear_caches(&self) {
        let cache = self.authority.get_transaction_cache_reader();
        let mut uncommitted: Vec<_> = self
            .authority
            .uncommitted_transactions_for_testing()
            .into_iter()
            .map(|digest| {
                let effects = cache
                    .get_executed_effects(&digest)
                    .expect("an uncommitted transaction was executed");
                (effects.epoch(), effects.lamport_version(), digest)
            })
            .collect();
        uncommitted.sort();
        for (epoch, chunk) in uncommitted.chunk_by(|a, b| a.0 == b.0).map(|c| (c[0].0, c)) {
            let digests: Vec<_> = chunk.iter().map(|(_, _, digest)| *digest).collect();
            self.commit_outputs_in_epoch(epoch, &digests);
        }
        self.authority.clear_execution_caches_for_testing();
    }

    /// [`Self::commit_outputs`] of `committed`, then removes every version
    /// `consumer` consumed from the epoch's historic objects bucket the
    /// commit moved it to, as on a validator that lost those bytes. The
    /// removal from the live table finds nothing left there. `consumer` must
    /// be one of `committed`.
    async fn prune_versions_consumed_by(
        &self,
        committed: &[TransactionDigest],
        consumer: &TransactionEffects,
    ) {
        self.commit_outputs(committed);
        self.authority
            .database_for_testing()
            .prune_objects_immediately_for_testing(vec![consumer.clone()])
            .await
            .unwrap();
        let bucket = self
            .authority
            .get_historic_objects()
            .ensure(self.epoch_store.epoch())
            .unwrap();
        let mut batch = bucket.objects.batch();
        batch
            .delete_batch_tagged(
                &bucket.objects,
                consumer
                    .modified_at_versions()
                    .iter()
                    .map(|modified| ObjectKey(*modified.object_id(), modified.version())),
            )
            .unwrap();
        batch.write().unwrap();
        for modified in consumer.modified_at_versions() {
            self.authority
                .evict_committed_object_for_testing(modified.object_id());
            assert!(
                self.store_object(modified.object_id(), modified.version())
                    .is_none()
            );
        }
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

    /// Builds a call into the `object_basics` module taking shared-object
    /// inputs and executes it as a root of commit `index`, with the shared
    /// versions assigned directly rather than through consensus.
    fn assigned_commit_shared_object_basics_call(
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

    /// [`Self::execute_with_assigned_shared_versions`] without asserting
    /// execution success, for scenarios exercising failed transactions.
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

    /// Executes `tx` with the shared object versions the handler assigned it
    /// in `assigned_versions`, without asserting execution success.
    fn execute_with_handler_versions(
        &self,
        tx: VerifiedTransaction,
        assigned_versions: AssignedTxAndVersions,
    ) -> TransactionEffects {
        let executable = self.executable(tx);
        let versions = assigned_versions
            .into_map()
            .remove(&executable.key())
            .expect("the handler must have assigned the transaction's shared versions");
        self.execute_executable(
            &executable,
            ExecutionEnv::new().with_assigned_versions(versions),
        )
    }

    /// A verified sponsored `object_basics::create` call: the sender is the
    /// account `authenticator` names, with the authenticator in place of its
    /// signature, and `sponsor` pays the gas from `gas_id`. The call takes no
    /// owned inputs, so the gas coin is its only owned object.
    fn build_account_authenticated_call(
        &self,
        authenticator: MoveAuthenticatorV1,
        gas_id: &ObjectId,
        sponsor: Address,
        sponsor_key: &AccountPrivateKey,
    ) -> VerifiedTransaction {
        self.epoch_store
            .verify_transaction(self.account_authenticated_call(
                authenticator,
                gas_id,
                sponsor,
                sponsor_key,
            ))
            .unwrap()
    }

    /// [`Self::build_account_authenticated_call`] before verification, for a
    /// transaction admission would refuse and a byzantine submitter can still
    /// sequence.
    fn account_authenticated_call(
        &self,
        authenticator: MoveAuthenticatorV1,
        gas_id: &ObjectId,
        sponsor: Address,
        sponsor_key: &AccountPrivateKey,
    ) -> TransactionEnvelope {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder
            .move_call(
                self.package_id,
                Identifier::from_static("object_basics"),
                Identifier::from_static("create"),
                vec![],
                create_object_args(sponsor),
            )
            .unwrap();
        self.account_authenticated_programmable(
            authenticator,
            builder.finish(),
            gas_id,
            sponsor,
            sponsor_key,
        )
    }

    /// A verified sponsored transfer of one nano from the gas coin `gas_id`
    /// to `sponsor`, sent by the account `authenticator` names. It names no
    /// package, so the only package the deny rules can meet is the one that
    /// holds the account's authenticate function.
    fn build_account_authenticated_transfer(
        &self,
        authenticator: MoveAuthenticatorV1,
        gas_id: &ObjectId,
        sponsor: Address,
        sponsor_key: &AccountPrivateKey,
    ) -> VerifiedTransaction {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.transfer_iota(sponsor, Some(1));
        self.epoch_store
            .verify_transaction(self.account_authenticated_programmable(
                authenticator,
                builder.finish(),
                gas_id,
                sponsor,
                sponsor_key,
            ))
            .unwrap()
    }

    /// `programmable` sent by the account `authenticator` names, with the
    /// authenticator in place of its signature, and paid by `sponsor` from
    /// `gas_id`.
    fn account_authenticated_programmable(
        &self,
        authenticator: MoveAuthenticatorV1,
        programmable: ProgrammableTransaction,
        gas_id: &ObjectId,
        sponsor: Address,
        sponsor_key: &AccountPrivateKey,
    ) -> TransactionEnvelope {
        let tx = Transaction::new_programmable_allow_sponsor(
            authenticator.address(),
            vec![self.latest_ref(gas_id)],
            programmable,
            TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS * self.rgp,
            self.rgp,
            sponsor,
        );
        to_sender_signed_transaction_with_optional_sponsor(
            tx,
            UserSignature::MoveAuthenticator(MoveAuthenticator::from(authenticator)),
            Some(sponsor_key),
        )
    }

    /// Executes a call naming the deleted shared object `id`, created at
    /// `initial_shared_version`, as a mutable input. The call aborts, and
    /// execution writes a deletion marker for `id` at the call's version
    /// with no row for `id`. The call is assigned to commit `index`.
    fn name_deleted_shared_object(
        &self,
        id: &ObjectId,
        initial_shared_version: Version,
        gas_id: &ObjectId,
        sender: Address,
        sender_key: &AccountPrivateKey,
        index: CommitIndex,
    ) -> TransactionEffects {
        let tx = self.build_move_call(
            "object_basics",
            "delete",
            vec![CallArg::Shared(SharedObjectReference::new(
                *id,
                initial_shared_version,
                true,
            ))],
            gas_id,
            sender,
            sender_key,
        );
        self.epoch_store
            .assign_commit_to_transactions(index, vec![TransactionKey::Digest(*tx.digest())]);
        let effects = self.execute_with_assigned_shared_versions_unchecked(tx);
        assert!(
            effects
                .deleted_mutably_accessed_shared_objects()
                .contains(id),
            "the call must name {id} as a deleted shared input"
        );
        effects
    }

    /// How many reads of `kind` the readers of this epoch answered from an
    /// earlier read at the same commit.
    fn reused_reads(&self, kind: &str) -> u64 {
        self.epoch_store
            .metrics
            .consensus_handler_validation_reader_reused_verdicts
            .with_label_values(&[kind])
            .get()
    }

    /// The commit-indexed reader as of `commit_index`.
    fn reader_at(&self, commit_index: CommitIndex) -> CommitIndexedReader {
        CommitIndexedReader::new(
            self.authority.get_object_cache_reader().clone(),
            self.epoch_store.clone(),
            commit_index,
        )
    }

    /// The commit-indexed reader's result for the owned input `reference`,
    /// as of `commit_index`.
    fn read_owned(&self, commit_index: CommitIndex, reference: ObjectReference) -> OwnedVerdict {
        self.reader_at(commit_index).read_owned(reference).unwrap()
    }

    /// The commit-indexed reader's result for shared input `id` declared at
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
        let kinds = tx
            .collect_all_input_object_kind_for_reading(
                self.epoch_store
                    .protocol_config()
                    .move_authenticator_type_arg_packages_as_inputs(),
            )
            .unwrap();
        TransactionInputLoader::new(self.authority.get_object_cache_reader().clone())
            .read_objects_at_commit(&self.reader_at(commit_index), &kinds)
            .unwrap()
    }

    /// Records the row of a publish of the `object_basics` package,
    /// which the fixture inserts at genesis, by commit `index`, so the reader
    /// treats the package as published by that commit.
    fn record_package_published_at(&self, index: CommitIndex) {
        let package = self.authority.get_object(&self.package_id).unwrap();
        self.epoch_store
            .assign_commit_to_transactions(index, vec![]);
        self.epoch_store
            .record_commit_fully_executed(
                index,
                &[(
                    ObjectKey::from(package.object_ref()),
                    HandlerProcessedObject {
                        digest: package.digest(),
                        kind: HandlerProcessedObjectKind::Live,
                        produced_at: index,
                        initial_shared_version: None,
                        replaced_version: None,
                        deleted_by: None,
                    },
                )],
                Default::default(),
            )
            .unwrap();
    }

    /// Stores `object` and records its row at commit `index` as the write of
    /// a transaction of that commit that replaced `replaced_version`, so
    /// the reader treats the object as written by that commit.
    fn record_write_at(&self, object: Object, replaced_version: Version, index: CommitIndex) {
        let row = (
            ObjectKey::from(object.object_ref()),
            HandlerProcessedObject {
                digest: object.digest(),
                kind: HandlerProcessedObjectKind::Live,
                produced_at: index,
                initial_shared_version: None,
                replaced_version: Some(replaced_version),
                deleted_by: None,
            },
        );
        self.epoch_store
            .assign_commit_to_transactions(index, vec![]);
        self.epoch_store
            .record_commit_fully_executed(index, &[row], Default::default())
            .unwrap();
        self.authority.insert_genesis_object(object);
    }

    /// Validation of `tx` at commit `index` under `deny_rules`.
    fn validate_at_commit_under(
        &self,
        index: CommitIndex,
        tx: &VerifiedTransaction,
        deny_rules: &DenyRuleSet,
    ) -> IotaResult<ValidationAtCommit> {
        self.authority
            .handle_transaction_validation_checks_at_commit(
                &self.reader_at(index),
                tx,
                &self.epoch_store,
                deny_rules,
                VerifierLimitsSource::ProtocolConfig,
                false,
            )
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

    /// Creates a fresh `object_basics::Object` owned by `sender` at commit
    /// `index`; returns the created reference and the effects.
    fn create_object(
        &self,
        gas_id: &ObjectId,
        sender: Address,
        sender_key: &AccountPrivateKey,
        index: CommitIndex,
    ) -> (ObjectReference, TransactionEffects) {
        let effects = self.assigned_commit_object_basics_call(
            "create",
            create_object_args(sender),
            gas_id,
            sender,
            sender_key,
            index,
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

    fn highest_fully_executed_commit(&self) -> CommitIndex {
        *self
            .epoch_store
            .subscribe_highest_fully_executed_commit()
            .unwrap()
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

    /// Assigns commit `index` with no roots and marks it fully executed.
    fn complete_empty_commit(&self, index: CommitIndex) {
        self.epoch_store
            .assign_commit_to_transactions(index, vec![]);
        self.epoch_store
            .record_commit_fully_executed(index, &[], Default::default())
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
        self.process_at_with_versions(index, transactions).await.0
    }

    /// [`Self::process_at`], also returning the versions the handler
    /// assigned, for executing a scheduled transaction with them.
    async fn process_at_with_versions(
        &self,
        index: CommitIndex,
        transactions: &[VerifiedTransaction],
    ) -> (Vec<VerifiedExecutableTransaction>, AssignedTxAndVersions) {
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
        let (scheduled, assigned_versions, _) = self
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
        let scheduled = scheduled
            .into_iter()
            .map(|tx| tx.as_tx().unwrap().clone())
            .collect();
        (scheduled, assigned_versions)
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
            &Default::default(),
            &mut sequenced,
        )
        .await
        .unwrap();
        ValidationResult {
            kept: sequenced
                .iter()
                .map(|tx| tx.0.transaction.user_transaction_digest().unwrap())
                .collect(),
            dropped: dropped
                .into_iter()
                .map(|(digest, error, _)| (digest, error))
                .collect(),
            locks,
        }
    }

    /// `tx` attested by `author` in `author`'s block, with the minimum units
    /// Check #3 accepts.
    fn attested_copy(
        &self,
        tx: &VerifiedTransaction,
        author: u8,
    ) -> VerifiedSequencedConsensusTransaction {
        let protocol_config = self.epoch_store.protocol_config();
        let units = protocol_config
            .base_tx_cost_fixed()
            .min(protocol_config.gas_rounding_step());
        let mut copy = make_attested_user_tx(tx.clone().into(), author, units);
        copy.0.certificate_author_index = author.into();
        copy
    }

    /// Executes `tx`, whose commit is assigned, the way execution from a
    /// certified checkpoint does when the checkpoint records `attestor`'s
    /// attestation of it as refuted with `units`.
    fn execute_refuted(&self, tx: VerifiedTransaction, attestor: u8, units: u64) {
        let attestation = AttestationSource::Certified(Some(AttestationRecord {
            attestor,
            verdict: AttestationVerdict::Refuted { units },
        }));
        let effects = self.execute_executable(
            &self.executable(tx),
            ExecutionEnv::new().with_attestation(attestation),
        );
        assert!(effects.status().is_success(), "{:?}", effects.status());
    }

    /// Validates the consensus `copies` at commit `index` with deterministic
    /// validation on.
    async fn validate_copies_at(
        &self,
        index: CommitIndex,
        mut copies: Vec<VerifiedSequencedConsensusTransaction>,
    ) -> ValidatedCopies {
        let (dropped, _, _) = post_consensus_validation::validate_and_resolve_conflicts(
            &self.authority,
            &self.epoch_store,
            index,
            &Default::default(),
            &mut copies,
        )
        .await
        .unwrap();
        ValidatedCopies {
            kept: copies
                .iter()
                .map(|tx| {
                    (
                        tx.0.transaction.user_transaction_digest().unwrap(),
                        tx.0.certificate_author_index,
                    )
                })
                .collect(),
            dropped,
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

    /// Marks commit `index` fully executed with the rows its roots' `effects`
    /// write, as the execution watcher does once every root has executed.
    fn complete_commit(&self, index: CommitIndex, effects: &[&TransactionEffects]) {
        let upserts: Vec<_> = effects
            .iter()
            .flat_map(|effects| handler_processed_upserts(effects, index))
            .collect();
        self.epoch_store
            .record_commit_fully_executed(index, &upserts, Default::default())
            .unwrap();
    }

    /// Reopens the epoch store as a restart does and prepares it for commit
    /// processing. Every other epoch store of the test must have released its
    /// DB handles first.
    async fn restart_epoch_store(&mut self) {
        let reopened = reopen(&self.authority, &self.epoch_store);
        reopened.set_effects_store(self.authority.get_transaction_cache_reader().clone());
        self.epoch_store = reopened;
        self.initialize_randomness().await;
    }

    /// [`Self::execute_with_assigned_commit`] of one transaction that fails at
    /// execution.
    fn execute_failing_with_assigned_commit(
        &self,
        tx: VerifiedTransaction,
        index: CommitIndex,
    ) -> TransactionEffects {
        self.epoch_store
            .assign_commit_to_transactions(index, vec![TransactionKey::Digest(*tx.digest())]);
        let effects = self.execute_unchecked(tx);
        assert!(effects.status().is_failure(), "{:?}", effects.status());
        effects
    }

    /// A verified `object_basics::remove_field` call on `parent_id`, which
    /// aborts at execution: no field was ever added to the parent.
    fn build_aborting_call(
        &self,
        parent_id: &ObjectId,
        gas_id: &ObjectId,
        sender: Address,
        sender_key: &AccountPrivateKey,
    ) -> VerifiedTransaction {
        self.build_move_call(
            "object_basics",
            "remove_field",
            vec![CallArg::ImmutableOrOwned(self.latest_ref(parent_id))],
            gas_id,
            sender,
            sender_key,
        )
    }

    /// `K`, the protocol config's horizon distance: validation at commit `C`
    /// reads the object state as of commit `C - K`.
    fn horizon_distance(&self) -> CommitIndex {
        self.epoch_store
            .protocol_config()
            .pcool_deterministic_validation_horizon_distance()
    }

    /// Moves the authority to the next epoch under the same protocol config
    /// and prepares the new epoch store for commit processing. Releases the
    /// override this setup holds, since the epoch change installs its own;
    /// the caller reinstalls one afterwards if the test needs it.
    async fn advance_epoch(&mut self) {
        drop(self._config_guard.take());
        self.authority.reconfigure_for_testing().await;
        self.epoch_store = self.authority.epoch_store_for_testing().clone();
        self.initialize_randomness().await;
    }

    /// Hands `executable`, which the handler kept at a commit, to the
    /// execution scheduler with the versions the handler assigned, as the
    /// handler's scheduler task does, and returns its effects, or `None`
    /// when it has not executed within a few seconds.
    async fn schedule_kept_transaction(
        &self,
        executable: VerifiedExecutableTransaction,
        assigned_versions: AssignedTxAndVersions,
    ) -> Option<TransactionEffects> {
        let digest = *executable.digest();
        let assigned_versions = assigned_versions
            .into_map()
            .remove(&executable.key())
            .unwrap_or_default();
        self.authority.execution_scheduler().enqueue(
            vec![(
                Schedulable::Transaction(executable),
                ExecutionEnv::new().with_assigned_versions(assigned_versions),
            )],
            &self.epoch_store,
        );
        tokio::time::timeout(
            Duration::from_secs(5),
            self.authority
                .get_transaction_cache_reader()
                .try_notify_read_executed_effects("", &[digest]),
        )
        .await
        .ok()
        .map(|effects| effects.unwrap().remove(0))
    }

    /// A verified `object_basics::create` call of `value` that the account
    /// `authenticator` names sends and pays for with its gas coin `gas_id`.
    /// Calls that differ only in `value` have different digests.
    fn build_self_paid_account_call(
        &self,
        authenticator: MoveAuthenticatorV1,
        gas_id: &ObjectId,
        value: u64,
    ) -> VerifiedTransaction {
        let account = authenticator.address();
        let mut builder = ProgrammableTransactionBuilder::new();
        builder
            .move_call(
                self.package_id,
                Identifier::from_static("object_basics"),
                Identifier::from_static("create"),
                vec![],
                vec![
                    CallArg::Pure(bcs::to_bytes(&value).unwrap()),
                    CallArg::Pure(bcs::to_bytes(&account).unwrap()),
                ],
            )
            .unwrap();
        let tx = Transaction::new_programmable(
            account,
            vec![self.latest_ref(gas_id)],
            builder.finish(),
            TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS * self.rgp,
            self.rgp,
        );
        let tx = to_sender_signed_transaction_with_optional_sponsor(
            tx,
            UserSignature::MoveAuthenticator(MoveAuthenticator::from(authenticator)),
            None::<&AccountPrivateKey>,
        );
        self.epoch_store.verify_transaction(tx).unwrap()
    }
}

#[tokio::test]
async fn assigned_commit_transaction_writes_handler_processed_only() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let obj_id = ObjectId::random();
    let gas_id = ObjectId::random();

    let s = setup_handler_object_state(
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

    // The handler assigned the transaction's commit before execution, so the
    // execution writes rows.
    let gas_genesis_ref = s.latest_ref(&gas_id);
    let effects = s.execute_with_assigned_commit(vec![tx], 7).remove(0);

    // The gas coin is a written object like any other.
    for consumed_ref in [obj_genesis_ref, gas_genesis_ref] {
        let id = consumed_ref.object_id();
        let row = s.handler_processed_object(id, effects.lamport_version());
        assert_eq!(row.produced_at, 7);
        assert_eq!(row.kind, HandlerProcessedObjectKind::Live);
    }
}

#[tokio::test]
async fn assigned_commit_delete_writes_a_deleted_tombstone_row() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let create_effects = s.assigned_commit_object_basics_call(
        "create",
        create_object_args(sender),
        &gas_id,
        sender,
        &sender_key,
        3,
    );
    let created_ref = *create_effects.created()[0].reference();
    let created_row = s.handler_processed_object(created_ref.object_id(), created_ref.version);
    assert_eq!(created_row.kind, HandlerProcessedObjectKind::Live);

    // The deletion in a later commit writes a tombstone at the deletion
    // version, carrying the store's deleted-object digest and naming the
    // deleted version as `replaced_version`; the live row of the deleted
    // version stays.
    let delete_effects = s.assigned_commit_object_basics_call(
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
            replaced_version: Some(created_ref.version),
            deleted_by: Some(*delete_effects.transaction_digest()),
        }
    );
    assert_eq!(
        s.handler_processed_object(created_ref.object_id(), created_ref.version),
        created_row
    );
}

#[tokio::test]
async fn assigned_commit_wrap_and_unwrap_write_a_wrapped_tombstone_and_a_new_live_row() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let (created_ref, _) = s.create_object(&gas_id, sender, &sender_key, 4);
    let created_row = s.handler_processed_object(created_ref.object_id(), created_ref.version);
    // The wrap writes a tombstone at the wrap version and leaves the live row
    // of the wrapped version in place.
    let wrap_effects = s.assigned_commit_object_basics_call(
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
            replaced_version: Some(created_ref.version),
            deleted_by: None,
        }
    );
    assert_eq!(
        s.handler_processed_object(created_ref.object_id(), created_ref.version),
        created_row
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
    let unwrap_effects = s.assigned_commit_object_basics_call(
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
            replaced_version: None,
            deleted_by: None,
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
            replaced_version: Some(wrapper_ref.version),
            deleted_by: Some(*unwrap_effects.transaction_digest()),
        }
    );
}

#[tokio::test]
async fn assigned_commit_share_records_the_initial_shared_version() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let share_effects =
        s.assigned_commit_object_basics_call("share", vec![], &gas_id, sender, &sender_key, 5);
    let shared = share_effects.created()[0];

    // The row written for a newly shared object holds its initial shared
    // version, so a later read can tell the object was created as shared.
    assert_eq!(
        s.handler_processed_object(shared.reference().object_id(), shared.reference().version),
        HandlerProcessedObject {
            digest: shared.reference().digest,
            kind: HandlerProcessedObjectKind::Live,
            produced_at: 5,
            initial_shared_version: Some(initial_shared_version(shared.owner())),
            replaced_version: None,
            deleted_by: None,
        }
    );
}

#[tokio::test]
async fn assigned_commit_shared_delete_keeps_the_initial_shared_version_on_the_tombstone_row() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let share_effects =
        s.assigned_commit_object_basics_call("share", vec![], &gas_id, sender, &sender_key, 5);
    let shared = share_effects.created()[0];
    let shared_id = shared.reference().object_id();
    let initial = initial_shared_version(shared.owner());

    // Once the object is gone, the tombstone row is the only place this
    // validator can still read the initial shared version a transaction
    // declares against, and the deleting transaction a kept input is handed
    // on with.
    let delete_effects = s.assigned_commit_shared_object_basics_call(
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
            replaced_version: Some(shared.reference().version),
            deleted_by: Some(*delete_effects.transaction_digest()),
        }
    );
}

#[tokio::test(start_paused = true)]
async fn watcher_completes_a_commit_once_its_roots_execute() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let obj_id = ObjectId::random();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(obj_id, sender),
            Object::with_id_owner_for_testing(gas_id, sender),
        ],
        true,
    )
    .await;
    let _watcher = s.start_execution_watcher();
    let state = s.epoch_store.handler_object_state_for_testing();

    // The handler assigns the commit before its transaction executes, and the
    // execution watcher must wait for the transaction's effects.
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
    let s = setup_handler_object_state(
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

    // The later commit's transaction executes first. Completing commit 2 now would
    // claim commit 1 executed too, so the watcher must hold it back.
    s.execute(second);
    s.assert_commit_not_fully_executed(1).await;
    assert_eq!(state.commit_index_of(&second_key), Some(2));

    s.execute(first);
    s.wait_for_fully_executed_commit(2).await;
    assert_eq!(state.commit_index_of(&first_key), None);
    assert_eq!(state.commit_index_of(&second_key), None);
}

/// The flush reaches a commit before the watcher does, as for a commit
/// whose checkpoint executes before the watcher starts. The flush completes
/// the commit from its own batch; the watcher reaching it afterwards must
/// change nothing.
#[tokio::test]
async fn watcher_changes_nothing_for_a_commit_the_flush_completed_first() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let obj_id = ObjectId::random();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(obj_id, sender),
            Object::with_id_owner_for_testing(gas_id, sender),
        ],
        true,
    )
    .await;
    let state = s.epoch_store.handler_object_state_for_testing();

    // The handler assigns commit 1 and its root executes; its checkpoint
    // executes and the commit flushes before any watcher runs.
    let tx = s.build_transfer(&obj_id, &gas_id, sender, &sender_key, Address::random());
    let key = TransactionKey::Digest(*tx.digest());
    let effects = s.execute_with_assigned_commit(vec![tx], 1).remove(0);
    let version = effects.lamport_version();
    let written = s
        .epoch_store
        .flush_commit_through_quarantine_for_testing(1, vec![*effects.transaction_digest()])
        .unwrap();

    assert_eq!(written, handler_processed_upserts(&effects, 1));
    assert_eq!(state.overlay_size_for_testing(), 0);
    for id in [obj_id, gas_id] {
        let row = s
            .epoch_store
            .durable_handler_processed_object_for_testing(&ObjectKey(id, version))
            .unwrap()
            .expect("the flush must write the commit's rows");
        assert_eq!(row.produced_at, 1);
    }
    assert_eq!(state.commit_index_of(&key), None);
    assert_eq!(s.highest_fully_executed_commit(), 1);

    // The watcher now receives commit 1, then an empty commit 2; once commit
    // 2 is complete, commit 1 has been handled. Had the watcher completed
    // commit 1 again, its rows would stay in memory with no flush left to
    // remove them.
    let _watcher = s.start_execution_watcher();
    s.epoch_store.assign_commit_to_transactions(2, vec![]);
    s.wait_for_fully_executed_commit(2).await;
    assert_eq!(state.overlay_size_for_testing(), 0);
}

/// A flush that derives a commit's rows from effects fails when a root has
/// none, so the batch is never written without that root's rows and the
/// commit stays waiting to be completed.
#[tokio::test]
async fn flush_fails_when_a_root_has_no_effects() {
    let s = setup_handler_object_state(vec![], true).await;
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

/// The checkpoint executor's batch writes a completed commit's rows to the
/// table before the commit flushes, so the flush has none of them left to
/// write.
#[tokio::test]
async fn flush_skips_rows_the_checkpoint_batch_made_durable() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let obj_id = ObjectId::random();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
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
        .execute_with_assigned_commit(
            vec![s.build_transfer(&obj_id, &gas_id, sender, &sender_key, Address::random())],
            1,
        )
        .remove(0);
    s.wait_for_fully_executed_commit(1).await;
    s.epoch_store
        .persist_checkpoint_bookkeeping([&effects])
        .unwrap();
    assert_eq!(state.overlay_size_for_testing(), 0);

    let written = s
        .epoch_store
        .flush_commit_through_quarantine_for_testing(1, vec![*effects.transaction_digest()])
        .unwrap();
    assert_eq!(written, vec![]);
    assert_eq!(state.overlay_size_for_testing(), 0);
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
/// completed writes all of the commit's rows and removes them from memory.
#[tokio::test]
async fn flush_writes_completed_commit_rows_still_held_in_memory() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let obj_id = ObjectId::random();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
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
        .execute_with_assigned_commit(
            vec![s.build_transfer(&obj_id, &gas_id, sender, &sender_key, Address::random())],
            1,
        )
        .remove(0);
    s.wait_for_fully_executed_commit(1).await;
    assert_eq!(state.overlay_size_for_testing(), 2);

    let written = s
        .epoch_store
        .flush_commit_through_quarantine_for_testing(1, vec![*effects.transaction_digest()])
        .unwrap();
    assert_eq!(written, handler_processed_upserts(&effects, 1));
    assert_eq!(state.overlay_size_for_testing(), 0);
    for (key, row) in written {
        assert_eq!(
            s.epoch_store
                .durable_handler_processed_object_for_testing(&key)
                .unwrap(),
            Some(row)
        );
    }
}

/// The checkpoint executor's batch writes a commit's rows to the table
/// before the watcher completes the commit, so the watcher does not put them
/// in memory again and the commit's flush has none of them left to write.
#[tokio::test]
async fn watcher_skips_rows_the_checkpoint_batch_made_durable() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let obj_id = ObjectId::random();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(obj_id, sender),
            Object::with_id_owner_for_testing(gas_id, sender),
        ],
        true,
    )
    .await;
    let state = s.epoch_store.handler_object_state_for_testing();

    let effects = s
        .execute_with_assigned_commit(
            vec![s.build_transfer(&obj_id, &gas_id, sender, &sender_key, Address::random())],
            1,
        )
        .remove(0);
    s.epoch_store
        .persist_checkpoint_bookkeeping([&effects])
        .unwrap();
    assert_eq!(state.overlay_size_for_testing(), 0);

    let _watcher = s.start_execution_watcher();
    s.wait_for_fully_executed_commit(1).await;
    assert_eq!(state.overlay_size_for_testing(), 0);

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
/// flushed. After the restart no rows are held in memory and the watcher has not
/// completed the replayed commit, so its flush derives every row from
/// effects and writes them.
#[tokio::test]
async fn replayed_flush_after_restart_writes_rows_from_effects() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let obj_id = ObjectId::random();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(obj_id, sender),
            Object::with_id_owner_for_testing(gas_id, sender),
        ],
        true,
    )
    .await;

    let watcher = s.start_execution_watcher();
    let effects = s
        .execute_with_assigned_commit(
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
            .overlay_size_for_testing(),
        0
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

/// A commit executed before a crash but not yet flushed loses its rows held
/// in memory, and its transactions are not executed again. When the handler
/// replays the commit, the watcher must
/// restore the rows from the effects already on disk.
#[tokio::test]
async fn watcher_restores_rows_of_a_replayed_commit_after_restart() {
    let (address_1, address_1_key): (Address, AccountPrivateKey) = get_key_pair();
    let (address_2, address_2_key): (Address, AccountPrivateKey) = get_key_pair();
    let obj1_id = ObjectId::random();
    let gas1_id = ObjectId::random();
    let obj2_id = ObjectId::random();
    let gas2_id = ObjectId::random();
    let s = setup_handler_object_state(
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
        .execute_with_assigned_commit(
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
        .execute_with_assigned_commit(
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
    // commit 2's were only held in memory and are gone.
    assert_eq!(
        *reopened
            .subscribe_highest_fully_executed_commit()
            .unwrap()
            .borrow(),
        1
    );
    assert_eq!(
        reopened
            .handler_object_state_for_testing()
            .overlay_size_for_testing(),
        0
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
    }
}

/// An attestor whose refuted totals take its whole score is cut off from the
/// commit whose horizon reaches them. Its attested copies drop as copies, so
/// another author's copy of the same transaction is kept, and an attestor
/// below the threshold is not cut off. After a restart the commit replays its
/// saved decisions, and a later commit reads the totals back from the table.
#[tokio::test]
async fn cut_off_attestor_copies_drop_once_the_horizon_reaches_its_refuted_totals() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let inputs: Vec<(ObjectId, ObjectId)> = (0..5)
        .map(|_| (ObjectId::random(), ObjectId::random()))
        .collect();
    let mut s = setup_handler_object_state_with_config_guard(
        inputs
            .iter()
            .flat_map(|(object, gas)| [*object, *gas])
            .map(|id| Object::with_id_owner_for_testing(id, sender))
            .collect(),
        Some(enable_attestor_cutoff()),
    )
    .await;
    let transfer = |(object, gas): &(ObjectId, ObjectId)| {
        s.build_transfer(object, gas, sender, &sender_key, Address::random())
    };
    let (refuted_0, refuted_2) = (transfer(&inputs[0]), transfer(&inputs[1]));
    let (t1, t2, t3) = (
        transfer(&inputs[2]),
        transfer(&inputs[3]),
        transfer(&inputs[4]),
    );
    let k = s.horizon_distance();

    // Commit 1 refutes attestor 0 with 10 units, 100 squared, which takes its
    // whole score, and attestor 2 with 9, 81 squared, which does not.
    let watcher = s.start_execution_watcher();
    let roots = vec![*refuted_0.digest(), *refuted_2.digest()];
    s.epoch_store.assign_commit_to_transactions(
        1,
        roots
            .iter()
            .map(|digest| TransactionKey::Digest(*digest))
            .collect(),
    );
    s.execute_refuted(refuted_0, 0, 10);
    s.execute_refuted(refuted_2, 2, 9);
    s.wait_for_fully_executed_commit(1).await;

    // Commit `k` reads as of the horizon 0, before commit 1.
    assert_eq!(
        s.validate_copies_at(k, vec![s.attested_copy(&t1, 0)]).await,
        ValidatedCopies {
            kept: vec![(*t1.digest(), 0)],
            dropped: vec![],
        }
    );

    // Commit `k + 1` reads as of the horizon 1.
    let copies = |s: &HandlerObjectStateSetup| {
        vec![
            s.attested_copy(&t1, 0),
            s.attested_copy(&t2, 0),
            s.attested_copy(&t2, 1),
            s.attested_copy(&t3, 2),
        ]
    };
    let cut_off = ValidatedCopies {
        kept: vec![(*t2.digest(), 1), (*t3.digest(), 2)],
        dropped: vec![(
            *t1.digest(),
            IotaError::AttestorCutOff { attestor: 0 },
            PostConsensusDecision::DroppedCopy,
        )],
    };
    let unused_before = input_reads(&s.authority, "unused");
    assert_eq!(s.validate_copies_at(k + 1, copies(&s)).await, cut_off);
    // The cut-off attestor's copy is not read ahead.
    assert_eq!(input_reads(&s.authority, "unused"), unused_before);
    assert_eq!(
        s.authority
            .metrics
            .consensus_handler_validation_attestor_cut_off_transactions
            .get(),
        2
    );

    // The node restarts once commit 1 has flushed.
    s.epoch_store
        .flush_commit_through_quarantine_for_testing(1, roots)
        .unwrap();
    drop(watcher);
    s.restart_epoch_store().await;
    assert_eq!(s.validate_copies_at(k + 1, copies(&s)).await, cut_off);
    s.complete_empty_commit(2);
    assert_eq!(
        s.validate_copies_at(k + 2, vec![s.attested_copy(&t1, 0)])
            .await,
        ValidatedCopies {
            kept: vec![],
            dropped: cut_off.dropped,
        }
    );
}

/// The flush completes a commit with refuted roots before the watcher
/// reaches it, deriving the commit's totals from the roots' records. The
/// watcher reaching the commit afterwards must not count them again.
#[tokio::test]
async fn flush_records_refuted_totals_the_watcher_does_not_count_again() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let obj1_id = ObjectId::random();
    let gas1_id = ObjectId::random();
    let obj2_id = ObjectId::random();
    let gas2_id = ObjectId::random();
    let s = setup_handler_object_state_with_config_guard(
        vec![
            Object::with_id_owner_for_testing(obj1_id, sender),
            Object::with_id_owner_for_testing(gas1_id, sender),
            Object::with_id_owner_for_testing(obj2_id, sender),
            Object::with_id_owner_for_testing(gas2_id, sender),
        ],
        Some(enable_attestor_cutoff()),
    )
    .await;

    // Commit 1 refutes attestor 0 with 10 units, 100 squared, which takes its
    // whole score, and attestor 2 with 8, 64 squared, which would only if
    // counted twice. It flushes before any watcher runs.
    let first = s.build_transfer(&obj1_id, &gas1_id, sender, &sender_key, Address::random());
    let second = s.build_transfer(&obj2_id, &gas2_id, sender, &sender_key, Address::random());
    let roots = vec![*first.digest(), *second.digest()];
    s.epoch_store.assign_commit_to_transactions(
        1,
        roots
            .iter()
            .map(|digest| TransactionKey::Digest(*digest))
            .collect(),
    );
    s.execute_refuted(first, 0, 10);
    s.execute_refuted(second, 2, 8);
    s.epoch_store
        .flush_commit_through_quarantine_for_testing(1, roots)
        .unwrap();
    assert_eq!(
        s.epoch_store.attestors_cut_off_as_of(1),
        BTreeSet::from([0])
    );

    // The watcher now receives commit 1, then an empty commit 2.
    let _watcher = s.start_execution_watcher();
    s.epoch_store.assign_commit_to_transactions(2, vec![]);
    s.wait_for_fully_executed_commit(2).await;
    assert_eq!(
        s.epoch_store.attestors_cut_off_as_of(2),
        BTreeSet::from([0])
    );
}

/// A commit the watcher completed before a crash, but that had not flushed,
/// loses its refuted totals with the process. When the handler replays the
/// commit, the watcher must restore them from the records on disk, on top of
/// the totals the table holds for the flushed commits.
#[tokio::test]
async fn watcher_restores_refuted_totals_of_a_replayed_commit_after_restart() {
    let (address_1, address_1_key): (Address, AccountPrivateKey) = get_key_pair();
    let (address_2, address_2_key): (Address, AccountPrivateKey) = get_key_pair();
    let obj1_id = ObjectId::random();
    let gas1_id = ObjectId::random();
    let obj2_id = ObjectId::random();
    let gas2_id = ObjectId::random();
    let s = setup_handler_object_state_with_config_guard(
        vec![
            Object::with_id_owner_for_testing(obj1_id, address_1),
            Object::with_id_owner_for_testing(gas1_id, address_1),
            Object::with_id_owner_for_testing(obj2_id, address_2),
            Object::with_id_owner_for_testing(gas2_id, address_2),
        ],
        Some(enable_attestor_cutoff()),
    )
    .await;

    // Attestor 0 is refuted with 6 units, 36 squared, in commit 1, which
    // flushes, and with 8, 64 squared, in commit 2, whose output is still in
    // the quarantine when the node crashes: 100 together, its whole score.
    let watcher = s.start_execution_watcher();
    let first = s.build_transfer(
        &obj1_id,
        &gas1_id,
        address_1,
        &address_1_key,
        Address::random(),
    );
    let first_digest = *first.digest();
    s.epoch_store
        .assign_commit_to_transactions(1, vec![TransactionKey::Digest(first_digest)]);
    s.execute_refuted(first, 0, 6);
    s.wait_for_fully_executed_commit(1).await;
    s.epoch_store
        .flush_commit_through_quarantine_for_testing(1, vec![first_digest])
        .unwrap();
    let second = s.build_transfer(
        &obj2_id,
        &gas2_id,
        address_2,
        &address_2_key,
        Address::random(),
    );
    let second_key = TransactionKey::Digest(*second.digest());
    s.epoch_store
        .assign_commit_to_transactions(2, vec![second_key]);
    s.execute_refuted(second, 0, 8);
    s.wait_for_fully_executed_commit(2).await;
    assert_eq!(
        s.epoch_store.attestors_cut_off_as_of(2),
        BTreeSet::from([0])
    );
    drop(watcher);

    let reopened = reopen(&s.authority, &s.epoch_store);
    reopened.set_effects_store(s.authority.get_transaction_cache_reader().clone());
    assert_eq!(reopened.attestors_cut_off_as_of(1), BTreeSet::new());

    // The handler replays commit 2. Its transaction is not executed again.
    let _watcher = ExecutionWatcher::start(reopened.clone());
    reopened.assign_commit_to_transactions(2, vec![second_key]);
    tokio::time::timeout(
        Duration::from_secs(10),
        reopened.wait_for_fully_executed_commit(2),
    )
    .await
    .expect("the replayed commit must become fully executed");
    assert_eq!(reopened.attestors_cut_off_as_of(2), BTreeSet::from([0]));
}

/// A checkpoint's rows are written to the table with the checkpoint, before
/// its commit flushes: a restart in between finds every row on disk,
/// although the rows held in memory are gone.
#[tokio::test]
async fn checkpoint_bookkeeping_is_durable_before_its_commit_flushes() {
    let (address_1, address_1_key): (Address, AccountPrivateKey) = get_key_pair();
    let (address_2, address_2_key): (Address, AccountPrivateKey) = get_key_pair();
    let obj1_id = ObjectId::random();
    let gas1_id = ObjectId::random();
    let obj2_id = ObjectId::random();
    let gas2_id = ObjectId::random();
    let s = setup_handler_object_state(
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

    // One checkpoint holding the roots of commits 1 and 2.
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
    let first = s.execute_with_assigned_commit(vec![first], 1).remove(0);
    let second = s.execute_with_assigned_commit(vec![second], 2).remove(0);
    s.epoch_store
        .persist_checkpoint_bookkeeping([&first, &second])
        .unwrap();

    // The rows are removed from memory once written to the table.
    assert_eq!(state.overlay_size_for_testing(), 0);

    // The node crashes before either commit flushes.
    let reopened = reopen(&s.authority, &s.epoch_store);
    reopened.set_effects_store(s.authority.get_transaction_cache_reader().clone());
    let reopened_state = reopened.handler_object_state_for_testing();
    assert_eq!(reopened_state.overlay_size_for_testing(), 0);
    for (effects, index, ids) in [
        (&first, 1, [obj1_id, gas1_id]),
        (&second, 2, [obj2_id, gas2_id]),
    ] {
        for id in ids {
            let row = reopened
                .handler_processed_object(&ObjectKey(id, effects.lamport_version()))
                .unwrap()
                .expect("the handler row must be durable with its checkpoint");
            assert_eq!(row.produced_at, index);
        }
    }

    // The handler replays both commits. Re-executing the checkpoint's
    // transactions re-inserts the same rows, and its batch writes them
    // again and clears them.
    reopened.assign_commit_to_transactions(1, vec![first_key]);
    reopened.assign_commit_to_transactions(2, vec![second_key]);
    reopened.record_executed_transaction(&first_key, &first);
    reopened.record_executed_transaction(&second_key, &second);
    assert_eq!(reopened_state.overlay_size_for_testing(), 4);
    reopened
        .persist_checkpoint_bookkeeping([&first, &second])
        .unwrap();
    assert_eq!(reopened_state.overlay_size_for_testing(), 0);
}

/// A checkpoint's rows are written without the consensus
/// quarantine lock, so a commit push or a quarantine flush does not hold up
/// checkpoint execution.
#[tokio::test]
async fn checkpoint_bookkeeping_does_not_wait_for_the_quarantine() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let obj_id = ObjectId::random();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(obj_id, sender),
            Object::with_id_owner_for_testing(gas_id, sender),
        ],
        true,
    )
    .await;
    let effects = s
        .execute_with_assigned_commit(
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
        wait_written
            .recv_timeout(Duration::from_secs(5))
            .expect("checkpoint bookkeeping must not wait for the quarantine lock");
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
            .overlay_size_for_testing(),
        0
    );
}

/// The rows of later checkpoints are in the table while the outputs of an
/// earlier checkpoint are not written yet, as on a running node whose
/// executor works on several checkpoints at once. The reader answers as it
/// does after each checkpoint's outputs are written.
#[tokio::test]
async fn rows_written_ahead_of_an_earlier_checkpoints_outputs_read_the_same() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let obj_id = ObjectId::random();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(obj_id, sender),
            Object::with_id_owner_for_testing(gas_id, sender),
        ],
        true,
    )
    .await;

    // Three checkpoints of one commit each. Each transfer consumes the
    // outputs of the one before.
    let mut references = vec![s.latest_ref(&gas_id)];
    let mut effects = Vec::new();
    for commit in 1..=3 {
        let transfer = s.build_transfer(&obj_id, &gas_id, sender, &sender_key, sender);
        effects.push(
            s.execute_with_assigned_commit(vec![transfer], commit)
                .remove(0),
        );
        references.push(s.latest_ref(&gas_id));
    }
    // The rows of all three are written, in checkpoint order, before the
    // outputs of the first.
    for effects in &effects {
        s.epoch_store
            .persist_checkpoint_bookkeeping([effects])
            .unwrap();
    }

    let horizon_distance = s.horizon_distance();
    // Every version of the gas coin as of every horizon from before the
    // first commit to the last.
    let verdicts = || {
        let mut verdicts = Vec::new();
        for horizon in 0..=3 {
            for reference in &references {
                verdicts.push(match s.read_owned(horizon + horizon_distance, *reference) {
                    OwnedVerdict::Keep(_) => "keep",
                    OwnedVerdict::Drop(_) => "drop",
                    OwnedVerdict::Missing(_) => "missing",
                });
            }
        }
        verdicts
    };
    let with_rows_ahead = verdicts();
    // As of the last commit only the latest version is kept.
    assert_eq!(with_rows_ahead[12..], ["drop", "drop", "drop", "keep"]);

    for effects in &effects {
        s.commit_outputs(&[*effects.transaction_digest()]);
        assert_eq!(verdicts(), with_rows_ahead);
    }
}

#[tokio::test]
async fn bookkeeping_disabled_writes_nothing() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let obj_id = ObjectId::random();
    let gas_id = ObjectId::random();

    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(obj_id, sender),
            Object::with_id_owner_for_testing(gas_id, sender),
        ],
        false,
    )
    .await;

    let effects = s.transfer(&obj_id, &gas_id, sender, &sender_key, Address::random());

    s.epoch_store
        .persist_checkpoint_bookkeeping([&effects])
        .unwrap();

    s.assert_no_handler_processed_object(&obj_id, effects.lamport_version());
}

/// Only a committee member keeps the handler object state. A node outside the
/// committee runs no consensus handler, so nothing would ever assign its
/// executions to a commit or clear their rows.
#[rstest::rstest]
#[tokio::test]
async fn bookkeeping_is_kept_only_by_committee_members(#[values(true, false)] in_committee: bool) {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let [obj_id, gas_id] = std::array::from_fn(|_| ObjectId::random());
    let genesis_objects = [obj_id, gas_id]
        .map(|id| Object::with_id_owner_for_testing(id, sender))
        .to_vec();
    let s = if in_committee {
        setup_handler_object_state(genesis_objects, true).await
    } else {
        setup_handler_object_state_outside_committee(genesis_objects).await
    };

    let effects = s
        .execute_with_assigned_commit(
            vec![s.build_transfer(&obj_id, &gas_id, sender, &sender_key, Address::random())],
            1,
        )
        .remove(0);
    s.epoch_store
        .persist_checkpoint_bookkeeping([&effects])
        .unwrap();

    assert_eq!(
        s.epoch_store
            .durable_handler_processed_object_for_testing(&ObjectKey(
                obj_id,
                effects.lamport_version()
            ))
            .unwrap()
            .is_some(),
        in_committee
    );
}

/// Genesis is the root of no consensus commit, so no commit would ever clear
/// rows its execution wrote. The objects it creates are epoch-start state.
#[tokio::test]
async fn genesis_execution_leaves_no_bookkeeping() {
    let s = setup_handler_object_state(vec![], true).await;

    for id in [IOTA_FRAMEWORK_PACKAGE_ID, IOTA_SYSTEM_STATE_OBJECT_ID] {
        s.assert_no_handler_processed_object(&id, s.latest_ref(&id).version);
    }
    assert_eq!(
        s.epoch_store
            .handler_object_state_for_testing()
            .overlay_size_for_testing(),
        0
    );
}

/// The change-epoch transaction is the root of no consensus commit, so no
/// commit would ever clear rows its execution wrote. What it writes is the
/// next epoch's epoch-start state. An execution whose commit the handler has
/// not assigned panics, so this also shows that `record_executed_transaction`
/// is not called for it.
#[tokio::test]
async fn change_epoch_execution_leaves_no_bookkeeping() {
    let s = setup_handler_object_state(vec![], true).await;

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
        s.assert_no_handler_processed_object(&key.0, key.1);
    }
}

// ---------------------------------------------------------------------------
// P-COOL deterministic-validation reader: shared inputs whose object is gone
// ---------------------------------------------------------------------------

#[track_caller]
fn assert_drops_with(verdict: SharedVerdict, kind: DropKind) {
    match verdict {
        SharedVerdict::Drop(reason) => assert_eq!(reason.kind(), kind),
        SharedVerdict::Exists => panic!("expected a drop, got exists"),
        SharedVerdict::Deleted(version, digest) => {
            panic!("expected a drop, got deleted at {version} by {digest}")
        }
        SharedVerdict::Missing(reason) => panic!("expected a drop, got missing {reason:?}"),
    }
}

#[track_caller]
fn assert_shared_missing(verdict: SharedVerdict, kind: MissingKind) {
    match verdict {
        SharedVerdict::Missing(reason) => assert_eq!(reason.kind(), kind),
        SharedVerdict::Exists => panic!("expected missing, got exists"),
        SharedVerdict::Deleted(version, digest) => {
            panic!("expected missing, got deleted at {version} by {digest}")
        }
        SharedVerdict::Drop(reason) => panic!("expected missing, got drop {reason:?}"),
    }
}

/// `verdict` is `Deleted` with `delete_effects`' version and digest.
#[track_caller]
fn assert_deleted_by(verdict: SharedVerdict, delete_effects: &TransactionEffects) {
    let digest = assert_deleted_at(verdict, delete_effects.lamport_version());
    assert_eq!(digest, *delete_effects.transaction_digest());
}

/// `verdict` is `Deleted` at `expected_version`. Returns its digest.
#[track_caller]
fn assert_deleted_at(verdict: SharedVerdict, expected_version: Version) -> TransactionDigest {
    match verdict {
        SharedVerdict::Deleted(version, digest) => {
            assert_eq!(version, expected_version);
            digest
        }
        SharedVerdict::Exists => panic!("expected deleted, got exists"),
        SharedVerdict::Drop(reason) => panic!("expected deleted, got drop {reason:?}"),
        SharedVerdict::Missing(reason) => panic!("expected deleted, got missing {reason:?}"),
    }
}

/// Commit 15 deletes a shared object. The tombstone row is then the only
/// place that still records the initial shared version, and a wrong declared
/// version must drop whether the deletion is above the horizon or not.
///
/// Later transactions naming the deleted object write newer deletion records
/// in the store and no row, so the latest deletion record differs between a
/// validator that executed them and one that did not. The results must not,
/// nor the version and transaction they carry: the reads before and after
/// those transactions stand for those two
/// validators.
#[tokio::test]
async fn reader_checks_the_declared_version_against_the_deletion_row() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let share_effects =
        s.assigned_commit_object_basics_call("share", vec![], &gas_id, sender, &sender_key, 5);
    let shared = share_effects.created()[0];
    let shared_id = shared.reference().object_id();
    let initial = initial_shared_version(shared.owner());
    let delete_effects = s.assigned_commit_shared_object_basics_call(
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

    // Commit 18, horizon 14: the deletion at 15 is not yet visible.
    assert_drops_with(
        s.read_shared(18, shared_id, wrong),
        DropKind::SharedInitialVersionMismatch,
    );
    assert_deleted_by(s.read_shared(18, shared_id, initial), &delete_effects);

    // Commit 20, horizon 16: every validator has executed the deletion.
    assert_drops_with(
        s.read_shared(20, shared_id, wrong),
        DropKind::SharedInitialVersionMismatch,
    );
    assert_drops_with(
        s.read_shared(20, shared_id, initial),
        DropKind::SharedDeletedAtOrBelowHorizon,
    );

    // A later transaction naming the deleted object, which the handler
    // assigned to commit 16, writes a newer marker and no row.
    let handler_smear_effects =
        s.name_deleted_shared_object(shared_id, initial, &gas_id, sender, &sender_key, 16);
    s.assert_no_handler_processed_object(shared_id, handler_smear_effects.lamport_version());

    // The tombstone row decides at both commits, and gives the version and
    // the deleting transaction.
    assert_drops_with(
        s.read_shared(18, shared_id, wrong),
        DropKind::SharedInitialVersionMismatch,
    );
    assert_deleted_by(s.read_shared(18, shared_id, initial), &delete_effects);
    assert_drops_with(
        s.read_shared(20, shared_id, wrong),
        DropKind::SharedInitialVersionMismatch,
    );
    assert_drops_with(
        s.read_shared(20, shared_id, initial),
        DropKind::SharedDeletedAtOrBelowHorizon,
    );
}

/// An id the store does not have answers missing, as a shared object created
/// above the horizon does on a validator that executed its creation: both
/// sides of the split answer missing.
#[tokio::test]
async fn reader_answers_missing_for_a_shared_id_the_store_does_not_have() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    assert_shared_missing(
        s.read_shared(12, &ObjectId::random(), Version::from_u64(1)),
        MissingKind::SharedNotFound,
    );

    let share_effects =
        s.assigned_commit_object_basics_call("share", vec![], &gas_id, sender, &sender_key, 11);
    let shared = share_effects.created()[0];
    assert_shared_missing(
        s.read_shared(
            12,
            shared.reference().object_id(),
            initial_shared_version(shared.owner()),
        ),
        MissingKind::SharedCreationAboveHorizon,
    );
}

/// Execution of a shared object's deletion at commit 11 writes the handler's
/// tombstone row, then the tombstone, then the deletion marker. Validation of
/// commit 14 runs alongside it and here reads the object between the
/// tombstone and the marker. The tombstone row answers the deletion with its
/// version and transaction, as it does once the marker is written, and the
/// read never fails.
#[tokio::test]
async fn reader_keeps_a_shared_deletion_from_its_tombstone_row_before_the_marker() {
    use iota_types::effects::TestEffectsBuilder;

    use crate::transaction_outputs::TransactionOutputs;

    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let deleter_gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(gas_id, sender),
            Object::with_id_owner_for_testing(deleter_gas_id, sender),
        ],
        true,
    )
    .await;
    let share_effects =
        s.assigned_commit_object_basics_call("share", vec![], &gas_id, sender, &sender_key, 5);
    let shared_ref = *share_effects.created()[0].reference();
    let shared_id = shared_ref.object_id;
    let initial = initial_shared_version(share_effects.created()[0].owner());

    let deletion = s.build_move_call(
        "object_basics",
        "delete",
        vec![s.shared_arg(&shared_id)],
        &deleter_gas_id,
        sender,
        &sender_key,
    );
    let deletion_effects = TestEffectsBuilder::new(deletion.data())
        .with_shared_input_versions([(shared_id, shared_ref.version)].into())
        .with_deleted_objects_owned_by([(shared_id, shared_ref.version, Owner::Shared(initial))])
        .build();
    let key = TransactionKey::Digest(*deletion.digest());
    s.epoch_store.assign_commit_to_transactions(11, vec![key]);
    s.epoch_store
        .record_executed_transaction(&key, &deletion_effects);
    s.authority.get_cache_writer().write_transaction_outputs(
        s.epoch_store.epoch(),
        Arc::new(TransactionOutputs {
            transaction: Arc::new(deletion),
            effects: deletion_effects.clone(),
            events: Default::default(),
            superseded: vec![],
            markers: vec![],
            wrapped: vec![],
            deleted: vec![ObjectKey(shared_id, deletion_effects.lamport_version())],
            live_object_markers_to_delete: vec![],
            new_live_object_markers_to_init: vec![],
            written: Default::default(),
            attestation_record: None,
        }),
    );
    assert!(s.authority.get_object(&shared_id).is_none());
    assert_eq!(
        s.authority
            .get_object_cache_reader()
            .try_get_last_shared_object_deletion_info(&shared_id, s.epoch_store.epoch())
            .unwrap(),
        None
    );

    // Commit 14, horizon 10: the deletion at 11 is above the horizon.
    assert_deleted_by(s.read_shared(14, &shared_id, initial), &deletion_effects);
}

/// A reader that answered a shared input as existing keeps that answer for
/// the rest of its commit, also once a deletion above the horizon executes
/// meanwhile. A new reader at the same commit answers the deletion. Both are
/// kept inputs, so the commit's decisions are the same either way.
#[tokio::test]
async fn reader_keeps_its_first_shared_verdict_through_a_deletion_above_the_horizon() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let deleter_gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(gas_id, sender),
            Object::with_id_owner_for_testing(deleter_gas_id, sender),
        ],
        true,
    )
    .await;
    let share_effects =
        s.assigned_commit_object_basics_call("share", vec![], &gas_id, sender, &sender_key, 5);
    let shared_ref = *share_effects.created()[0].reference();
    let shared_id = shared_ref.object_id;
    let initial = initial_shared_version(share_effects.created()[0].owner());
    let reused = || s.reused_reads("shared");

    // Commit 14, horizon 10: shared at 5, so it exists.
    let reader = s.reader_at(14);
    assert_eq!(
        reader.read_shared(shared_id, initial).unwrap(),
        SharedVerdict::Exists
    );

    let deletion_effects = s.assigned_commit_shared_object_basics_call(
        "delete",
        vec![s.shared_arg(&shared_id)],
        &deleter_gas_id,
        sender,
        &sender_key,
        11,
    );

    assert_eq!(reused(), 0);
    assert_eq!(
        reader.read_shared(shared_id, initial).unwrap(),
        SharedVerdict::Exists
    );
    assert_eq!(reused(), 1);
    assert_deleted_by(s.read_shared(14, &shared_id, initial), &deletion_effects);
    assert_eq!(reused(), 1);
}

// ---------------------------------------------------------------------------
// P-COOL deterministic-validation loader
// ---------------------------------------------------------------------------

/// The deny check's package store answers as the reader does: the package is
/// there from epoch-start state, and absent once a row shows it published
/// above the horizon.
#[tokio::test]
async fn package_store_view_follows_the_reader() {
    let (sender, _): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let reader = s.reader_at(12);
    let package = BackingPackageStore::get_package_object(&reader, &s.package_id)
        .unwrap()
        .expect("a genesis package is epoch-start state");
    assert_eq!(package.object().id(), s.package_id);

    s.record_package_published_at(11);
    // A reader answers a package once per commit, so the row is read
    // through a new one.
    let reader = s.reader_at(12);
    assert!(
        BackingPackageStore::get_package_object(&reader, &s.package_id)
            .unwrap()
            .is_none()
    );
}

/// A row shows the package published above the horizon. The row decides,
/// and the loader stops at it naming the package input.
#[tokio::test]
async fn loader_stops_at_a_package_published_above_the_horizon() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let share_effects =
        s.assigned_commit_object_basics_call("share", vec![], &gas_id, sender, &sender_key, 5);
    let shared = share_effects.created()[0];
    let shared_id = shared.reference().object_id();
    s.record_package_published_at(11);
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
            assert_eq!(reason.kind(), MissingKind::PackageAboveHorizon);
        }
        InputResolution::Missing(kind, reason) => panic!("wrong input {kind:?}: {reason:?}"),
        InputResolution::Drop(kind, reason) => panic!("dropped on {kind:?}: {reason:?}"),
        InputResolution::Loaded(_) => panic!("the package must not be visible"),
    }
}

/// Every input kind resolves to the read result the input checks expect:
/// the package from epoch-start state, the gas coin at the named reference,
/// and the shared object as an unread shared object, since its contents are
/// not read.
#[tokio::test]
async fn loader_resolves_every_input_kind_at_a_commit() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let share_effects =
        s.assigned_commit_object_basics_call("share", vec![], &gas_id, sender, &sender_key, 5);
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
                InputObjectKind::SharedMoveObject {
                    id,
                    initial_shared_version,
                    ..
                },
                ObjectReadResultKind::UnreadSharedObject,
            ) => {
                assert_eq!(&id, shared_id);
                assert_eq!(initial_shared_version, initial);
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
    let s = setup_handler_object_state(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let share_effects =
        s.assigned_commit_object_basics_call("share", vec![], &gas_id, sender, &sender_key, 5);
    let shared = share_effects.created()[0];
    let shared_id = shared.reference().object_id();
    // Commit 11 mutates the gas coin. At commit 14 the horizon is 10.
    s.assigned_commit_shared_object_basics_call(
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

    match s.read_inputs_at_commit(14, &tx) {
        InputResolution::Missing(InputObjectKind::ImmOrOwnedMoveObject(reference), reason) => {
            assert_eq!(reference, gas_ref);
            assert_eq!(reason.kind(), MissingKind::HandlerRowAboveHorizon);
        }
        InputResolution::Missing(kind, reason) => panic!("wrong input {kind:?}: {reason:?}"),
        InputResolution::Drop(kind, reason) => panic!("dropped on {kind:?}: {reason:?}"),
        InputResolution::Loaded(_) => panic!("the gas coin must not be visible"),
    }
    // At commit 15 the horizon reaches the producing commit.
    assert!(matches!(
        s.read_inputs_at_commit(15, &tx),
        InputResolution::Loaded(_)
    ));
}

/// Validation at a commit passes the reader's results through, and answers
/// `Keep` with the owned references to lock once every check passed.
#[tokio::test]
async fn validation_at_commit_keeps_drops_and_reports_missing() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;
    let deny_config = &s.authority.config.transaction_deny_config;

    let share_effects =
        s.assigned_commit_object_basics_call("share", vec![], &gas_id, sender, &sender_key, 5);
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
            false,
        )
        .unwrap()
    {
        ValidationAtCommit::Keep(owned) => assert_eq!(owned, vec![gas_ref]),
        ValidationAtCommit::Drop(kind, reason) => panic!("dropped on {kind:?}: {reason:?}"),
        ValidationAtCommit::Missing(kind, reason) => panic!("missing {kind:?}: {reason:?}"),
        ValidationAtCommit::AuthenticatorPackageDenied(error) => panic!("denied: {error:?}"),
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
            false,
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
    s.assigned_commit_shared_object_basics_call(
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
            false,
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

/// With a denied package in the deny rules, the deny check loads the package
/// of every call. A call into a package published above the horizon answers
/// missing, as without denied packages, and not as an object not found.
#[tokio::test]
async fn denied_packages_leave_a_package_above_the_horizon_missing() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;
    let deny_config = iota_config::transaction_deny_config::TransactionDenyConfigBuilder::new()
        .add_denied_package(ObjectId::random())
        .build();

    let share_effects =
        s.assigned_commit_object_basics_call("share", vec![], &gas_id, sender, &sender_key, 5);
    let shared = share_effects.created()[0];
    let shared_id = shared.reference().object_id();
    s.record_package_published_at(11);
    let tx = s.build_move_call(
        "object_basics",
        "set_value",
        vec![
            s.shared_arg(shared_id),
            CallArg::Pure(bcs::to_bytes(&1u64).unwrap()),
        ],
        &gas_id,
        sender,
        &sender_key,
    );

    match s
        .authority
        .handle_transaction_validation_checks_at_commit(
            &s.reader_at(12),
            &tx,
            &s.epoch_store,
            &deny_config,
            VerifierLimitsSource::ProtocolConfig,
            false,
        )
        .unwrap()
    {
        ValidationAtCommit::Missing(InputObjectKind::MovePackage(id), reason) => {
            assert_eq!(id, s.package_id);
            assert_eq!(reason.kind(), MissingKind::PackageAboveHorizon);
        }
        other => panic!("expected missing on the package, got {other:?}"),
    }
}

/// Receiving objects go through the commit-indexed reader as owned inputs do,
/// without the sender check, so a kept one was live at the horizon on every
/// validator and execution can always run it. A reference nobody can execute
/// is never kept: the scheduler would wait on it forever and the commit would
/// never complete. Receiving an input still rejects. The same holds for an
/// attested transaction.
#[tokio::test]
async fn validation_at_commit_reads_receiving_objects_through_the_reader() {
    use std::collections::HashSet;

    use iota_types::storage::InputKey;

    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state_with_package("tto", sender, gas_id).await;
    let deny_config = &s.authority.config.transaction_deny_config;

    // Three parent and child pairs. The first child is received at commit 6,
    // at or below the horizon of commit 12, the second at commit 11, above
    // it, the third not at all.
    let start = |index| {
        let effects =
            s.assigned_commit_move_call("M1", "start", vec![], &gas_id, sender, &sender_key, index);
        let (parent, child) = parent_and_child(effects.created());
        (*parent.reference(), *child.reference(), effects.created())
    };
    let receive = |parent: ObjectReference, child: ObjectReference, index| {
        s.assigned_commit_move_call(
            "M1",
            "receiver",
            vec![CallArg::ImmutableOrOwned(parent), CallArg::Receiving(child)],
            &gas_id,
            sender,
            &sender_key,
            index,
        );
    };
    let (parent_a, child_a, _) = start(5);
    receive(parent_a, child_a, 6);
    let (parent_b, child_b, _) = start(7);
    let (parent_c, child_c, created_c) = start(8);
    // `start` also creates a shared object, an immutable one and the child of
    // a dynamic object field, none of which an address owns.
    let created_with = |owned: fn(&Owner) -> bool| {
        created_c
            .iter()
            .find(|object| owned(object.owner()))
            .map(|object| *object.reference())
            .expect("`start` creates an object with that owner")
    };
    let shared = created_with(|owner| matches!(owner, Owner::Shared(_)));
    let immutable = created_with(|owner| matches!(owner, Owner::Immutable));
    let object_owned = created_with(|owner| matches!(owner, Owner::Object(_)));
    // The gas coin as of commit 8: the receive at commit 11 consumes it above
    // the horizon, which keeps it, where its newer version would be missing.
    let gas_ref = s.latest_ref(&gas_id);
    receive(parent_b, child_b, 11);

    // `M1::receiver(parent_c, Receiving(receiving))`, with `extra_input`
    // appended as an unused input. Validation does not pair a child with its
    // parent, so the one untouched parent serves every case.
    let validate = |receiving: ObjectReference, extra_input: Option<CallArg>, attested: bool| {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder
            .move_call(
                s.package_id,
                Identifier::from_static("M1"),
                Identifier::from_static("receiver"),
                vec![],
                vec![
                    CallArg::ImmutableOrOwned(parent_c),
                    CallArg::Receiving(receiving),
                ],
            )
            .unwrap();
        let mut pt = builder.finish();
        pt.inputs.extend(extra_input);
        let tx = Transaction::new_programmable(
            sender,
            vec![gas_ref],
            pt,
            TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS * s.rgp,
            s.rgp,
        );
        let tx = s
            .epoch_store
            .verify_transaction(to_sender_signed_transaction(tx, &sender_key))
            .unwrap();
        s.authority.handle_transaction_validation_checks_at_commit(
            &s.reader_at(12),
            &tx,
            &s.epoch_store,
            deny_config,
            VerifierLimitsSource::ProtocolConfig,
            attested,
        )
    };

    // An attested transaction's receiving objects take the same path, so
    // every verdict holds for both.
    for attested in [false, true] {
        // Live at the horizon: kept, and not among the owned objects to lock.
        match validate(child_c, None, attested).unwrap() {
            ValidationAtCommit::Keep(owned) => assert_eq!(owned, vec![parent_c, gas_ref]),
            other => panic!("expected the receive of a live object to keep, got {other:?}"),
        }

        // Received at commit 6: the receive's row above the version drops it.
        match validate(child_a, None, attested).unwrap() {
            ValidationAtCommit::Drop(InputObjectKind::ImmOrOwnedMoveObject(reference), reason) => {
                assert_eq!(reference, child_a);
                assert_eq!(reason.kind(), DropKind::HandlerRowSuperseded);
            }
            other => panic!("expected the receive of a consumed version to drop, got {other:?}"),
        }

        // Received at commit 11: kept, as on a validator that has not executed
        // the receive. Execution fails it.
        match validate(child_b, None, attested).unwrap() {
            ValidationAtCommit::Keep(owned) => assert_eq!(owned, vec![parent_c, gas_ref]),
            other => panic!("expected the receive above the horizon to keep, got {other:?}"),
        }

        // A wrong digest at a live version drops.
        let wrong_digest =
            ObjectReference::new(child_c.object_id, child_c.version, ObjectDigest::random());
        match validate(wrong_digest, None, attested).unwrap() {
            ValidationAtCommit::Drop(InputObjectKind::ImmOrOwnedMoveObject(reference), reason) => {
                assert_eq!(reference, wrong_digest);
                assert_eq!(reason.kind(), DropKind::HandlerRowDigestMismatch);
            }
            other => panic!("expected a wrong digest to drop, got {other:?}"),
        }

        // Never existed: missing, so not kept. The scheduler never finds such a
        // key available, so a kept transaction would never execute.
        let unseen = ObjectReference::new(ObjectId::random(), child_c.version, child_c.digest);
        match validate(unseen, None, attested).unwrap() {
            ValidationAtCommit::Missing(
                InputObjectKind::ImmOrOwnedMoveObject(reference),
                reason,
            ) => {
                assert_eq!(reference, unseen);
                assert_eq!(reason.kind(), MissingKind::StoreNotFound);
            }
            other => {
                panic!("expected the receive of an unseen object to be missing, got {other:?}")
            }
        }
        let unseen_key = InputKey::VersionedObject {
            id: unseen.object_id,
            version: unseen.version,
        };
        assert_eq!(
            s.authority
                .get_object_cache_reader()
                .multi_input_objects_available(
                    &[unseen_key],
                    &HashSet::from([unseen_key]),
                    s.epoch_store.epoch(),
                ),
            vec![false],
            "the scheduler must never find the unseen receiving object available"
        );

        // A shared object's creation version drops from its creation row, as
        // when it is named as an owned input.
        match validate(shared, None, attested).unwrap() {
            ValidationAtCommit::Drop(InputObjectKind::ImmOrOwnedMoveObject(reference), reason) => {
                assert_eq!(reference, shared);
                assert_eq!(reason.kind(), DropKind::HandlerRowSharedObject);
            }
            other => panic!("expected the receive of a shared object to drop, got {other:?}"),
        }

        // An immutable object and an object owned by another object are live at
        // the horizon, and the receiving checks reject them from their bytes.
        let result = validate(immutable, None, attested);
        assert!(
            matches!(
                result,
                Err(IotaError::UserInput {
                    error: UserInputError::MutableParameterExpected { object_id }
                }) if object_id == immutable.object_id
            ),
            "receiving an immutable object must reject, got {result:?}"
        );
        let result = validate(object_owned, None, attested);
        assert!(
            matches!(
                result,
                Err(IotaError::UserInput {
                    error: UserInputError::InvalidChildObjectArgument { child_id, .. }
                }) if child_id == object_owned.object_id
            ),
            "receiving an object owned by an object must reject, got {result:?}"
        );

        // Receiving an input still rejects.
        let result = validate(child_c, Some(CallArg::Receiving(parent_c)), attested);
        assert!(
            matches!(
                result,
                Err(IotaError::UserInput {
                    error: UserInputError::DuplicateObjectRefInput
                })
            ),
            "receiving an input must reject, got {result:?}"
        );
    }
}

/// A receive run through the handler's path at `commit`. When kept, it is
/// handed to the execution scheduler, and `effects` is `None` when it has not
/// executed within a few seconds. `watcher` must stay alive for the commit
/// to become fully executed.
struct ScheduledReceive {
    setup: HandlerObjectStateSetup,
    watcher: ExecutionWatcher,
    commit: CommitIndex,
    kept: bool,
    effects: Option<TransactionEffects>,
}

/// Executes `M1::start` as the root of commit 1 with the execution watcher
/// running, and with `consume` set, `M1::<consume>(parent, Receiving(child))`
/// as the root of commit 2, with `child` the object `start` made receivable.
/// Then runs `M1::receiver(parent, Receiving(receiving(child)))` through the
/// handler's processing of the first commit whose horizon reaches commit
/// `horizon`, and hands it, if kept, to the execution scheduler with the
/// versions the handler assigned, as the handler's scheduler task does.
async fn schedule_receive_through_the_handler(
    consume: Option<&'static str>,
    receiving: impl FnOnce(ObjectReference) -> ObjectReference,
    horizon: CommitIndex,
) -> ScheduledReceive {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state_with_package("tto", sender, gas_id).await;
    let watcher = s.start_execution_watcher();

    let start_effects =
        s.assigned_commit_move_call("M1", "start", vec![], &gas_id, sender, &sender_key, 1);
    s.wait_for_fully_executed_commit(1).await;
    let (parent, child) = parent_and_child(start_effects.created());
    let parent_id = parent.reference().object_id;
    if let Some(consume) = consume {
        let consume_effects = s.assigned_commit_move_call(
            "M1",
            consume,
            vec![
                CallArg::ImmutableOrOwned(*parent.reference()),
                CallArg::Receiving(*child.reference()),
            ],
            &gas_id,
            sender,
            &sender_key,
            2,
        );
        assert!(
            consume_effects.status().is_success(),
            "{:?}",
            consume_effects.status()
        );
        s.wait_for_fully_executed_commit(2).await;
    }
    let receive = s.build_move_call(
        "M1",
        "receiver",
        vec![
            CallArg::ImmutableOrOwned(s.latest_ref(&parent_id)),
            CallArg::Receiving(receiving(*child.reference())),
        ],
        &gas_id,
        sender,
        &sender_key,
    );

    let commit = horizon + s.horizon_distance();
    let (scheduled, assigned_versions) = s
        .process_at_with_versions(commit, std::slice::from_ref(&receive))
        .await;
    let Some(executable) = scheduled.into_iter().next() else {
        return ScheduledReceive {
            setup: s,
            watcher,
            commit,
            kept: false,
            effects: None,
        };
    };
    let effects = s
        .schedule_kept_transaction(executable, assigned_versions)
        .await;
    ScheduledReceive {
        setup: s,
        watcher,
        commit,
        kept: true,
        effects,
    }
}

/// Asserts that a kept receive executed and aborted in `iota::transfer` as
/// unable to receive, with the sender charged.
fn assert_receive_aborted(s: &HandlerObjectStateSetup, effects: Option<TransactionEffects>) {
    let effects = effects.unwrap_or_else(|| {
        panic!(
            "the receive must execute: {} transaction(s) still pending in the execution \
             scheduler, highest fully executed commit {}",
            s.authority.execution_scheduler().num_pending_transactions(),
            s.highest_fully_executed_commit(),
        )
    });
    assert!(
        matches!(
            effects.status(),
            ExecutionStatus::Failure {
                error: ExecutionErrorKind::MoveAbort { location, code: 3 },
                ..
            } if location.module.as_str() == "transfer"
        ),
        "the receive must abort in `iota::transfer` as unable to receive, got {:?}",
        effects.status()
    );
    assert!(effects.gas_cost_summary().gas_used() > 0);
}

/// A receive naming a version of the child that never existed is not kept at
/// the commit: no validator could make it ready for execution, so keeping it
/// would stop the commit from ever becoming fully executed.
#[tokio::test]
async fn receive_of_a_version_that_never_existed_is_not_kept() {
    let ScheduledReceive {
        setup: s,
        watcher: _watcher,
        commit,
        kept,
        ..
    } = schedule_receive_through_the_handler(
        None,
        |child| ObjectReference::new(child.object_id, child.version + 1_000, child.digest),
        1,
    )
    .await;

    assert!(!kept, "the receive must not be kept at commit {commit}");
    s.wait_for_fully_executed_commit(commit).await;
}

/// A receive naming a version of the child that a commit above the horizon
/// consumed, by deleting or by wrapping it, is kept: the version was live at
/// the horizon. Execution finds the receiving object available from this
/// epoch's marker, and the receive aborts with the sender charged. The parent
/// is shared, as the consumption mutates an owned parent's version above the
/// horizon, and another gas coin pays for the consumption, so every other
/// input of the receive is live at the horizon.
///
/// With `committed`, the consumption's checkpoint has committed and the caches
/// are cleared, so the received version is read from this epoch's bucket.
#[tokio::test]
async fn receive_of_a_version_consumed_above_the_horizon_is_kept_and_aborts() {
    for (consume, committed) in [
        ("deleter", false),
        ("deleter", true),
        ("wrapper", false),
        ("wrapper", true),
    ] {
        let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
        let gas_id = ObjectId::random();
        let consumer_gas_id = ObjectId::random();
        let s = setup_handler_object_state_with_package("tto_shared", sender, gas_id).await;
        s.authority
            .insert_genesis_object(Object::with_id_owner_for_testing(consumer_gas_id, sender));
        let _watcher = s.start_execution_watcher();

        // `M::start` as the root of commit 1, then
        // `M::<consume>(parent, Receiving(child))` as the root of commit 2.
        let start_effects =
            s.assigned_commit_move_call("M", "start", vec![], &gas_id, sender, &sender_key, 1);
        s.wait_for_fully_executed_commit(1).await;
        let (parent, child) = parent_and_child(start_effects.created());
        let parent_id = parent.reference().object_id;
        let child = *child.reference();
        let consumption = s.build_move_call(
            "M",
            consume,
            vec![s.shared_arg(&parent_id), CallArg::Receiving(child)],
            &consumer_gas_id,
            sender,
            &sender_key,
        );
        let consumption_digest = *consumption.digest();
        s.epoch_store
            .assign_commit_to_transactions(2, vec![TransactionKey::Digest(consumption_digest)]);
        s.execute_with_assigned_shared_versions(consumption);
        s.wait_for_fully_executed_commit(2).await;
        if committed {
            s.commit_outputs(&[*start_effects.transaction_digest(), consumption_digest]);
            s.authority.clear_execution_caches_for_testing();
            assert!(
                s.authority
                    .get_object_cache_reader()
                    .get_object_by_key(&child.object_id, child.version)
                    .is_none(),
                "the commit moves the received version out of the live state"
            );
        }

        let commit = 1 + s.horizon_distance();
        assert!(
            commit > 2,
            "the consumption at commit 2 must be above the horizon of commit {commit}"
        );
        let receive = s.build_move_call(
            "M",
            "receiver",
            vec![s.shared_arg(&parent_id), CallArg::Receiving(child)],
            &gas_id,
            sender,
            &sender_key,
        );
        let (scheduled, assigned_versions) = s
            .process_at_with_versions(commit, std::slice::from_ref(&receive))
            .await;
        let executable = scheduled
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("the receive after `{consume}` must be kept"));
        let effects = s
            .schedule_kept_transaction(executable, assigned_versions)
            .await;
        assert_receive_aborted(&s, effects);
        s.wait_for_fully_executed_commit(commit).await;
    }
}

/// The same receive at the first commit whose horizon reaches the
/// consumption is dropped, and that commit becomes fully executed.
#[tokio::test]
async fn receive_of_a_version_consumed_at_the_horizon_is_dropped() {
    for consume in ["deleter", "wrapper"] {
        let ScheduledReceive {
            setup: s,
            watcher: _watcher,
            commit,
            kept,
            ..
        } = schedule_receive_through_the_handler(Some(consume), |child| child, 2).await;

        assert!(!kept, "the receive after `{consume}` must not be kept");
        s.wait_for_fully_executed_commit(commit).await;
    }
}

/// The same receive naming the child's current version executes and receives
/// the child, so the commit becomes fully executed.
#[tokio::test]
async fn kept_receive_of_the_current_version_executes_through_the_scheduler() {
    let ScheduledReceive {
        setup: s,
        watcher: _watcher,
        commit,
        kept,
        effects,
    } = schedule_receive_through_the_handler(None, |child| child, 1).await;

    assert!(kept, "the receive must be kept at commit {commit}");
    let effects = effects.expect("the receive must execute");
    assert!(effects.status().is_success(), "{:?}", effects.status());
    s.wait_for_fully_executed_commit(commit).await;
}

/// A receive naming the version of a child that a receive deleted in the
/// previous epoch is dropped at a commit of the next epoch on every
/// validator, whether it has pruned the deleted version or not: the deletion
/// marker belongs to the previous epoch, so a kept receive would leave the
/// scheduler's view of the receiving object to whether the deleted version is
/// still stored. The version was superseded before the epoch, which the
/// store's tombstone and the absence of any row show on both validators.
#[tokio::test]
async fn receive_of_a_version_deleted_last_epoch_is_dropped_on_every_node() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let mut pruning = setup_handler_object_state_with_package("tto", sender, gas_id).await;
    let mut keeping = setup_handler_object_state_with_package("tto", sender, gas_id).await;

    // `M1::start` as the root of commit 1, then
    // `M1::deleter(parent, Receiving(child))` as the root of commit 2, which
    // receives the child and deletes it.
    let delete_the_child = |s: &HandlerObjectStateSetup| {
        let start_effects =
            s.assigned_commit_move_call("M1", "start", vec![], &gas_id, sender, &sender_key, 1);
        s.complete_commit(1, &[&start_effects]);
        let (parent, child) = parent_and_child(start_effects.created());
        let delete_effects = s.assigned_commit_move_call(
            "M1",
            "deleter",
            vec![
                CallArg::ImmutableOrOwned(*parent.reference()),
                CallArg::Receiving(*child.reference()),
            ],
            &gas_id,
            sender,
            &sender_key,
            2,
        );
        s.complete_commit(2, &[&delete_effects]);
        assert_eq!(
            delete_effects
                .deleted()
                .iter()
                .map(|deleted| deleted.object_id)
                .collect::<Vec<_>>(),
            vec![child.reference().object_id]
        );
        (
            parent.reference().object_id,
            *child.reference(),
            vec![
                *start_effects.transaction_digest(),
                *delete_effects.transaction_digest(),
            ],
            delete_effects,
        )
    };
    let (parent_id, child, committed, delete_effects) = delete_the_child(&pruning);
    assert_eq!(
        delete_the_child(&keeping).1,
        child,
        "both validators must hold the same child"
    );

    pruning
        .prune_versions_consumed_by(&committed, &delete_effects)
        .await;
    keeping.commit_outputs(&committed);
    assert!(
        keeping
            .store_object(&child.object_id, child.version)
            .is_some()
    );

    for s in [&mut pruning, &mut keeping] {
        s.advance_epoch().await;
        assert_eq!(s.epoch_store.epoch(), 1);
        assert!(s.epoch_store.protocol_config().enable_pcool_flow());
    }

    // The same transaction on both validators: their stores agree on the
    // parent's and the gas coin's latest versions.
    let receive = pruning.build_move_call(
        "M1",
        "receiver",
        vec![
            CallArg::ImmutableOrOwned(pruning.latest_ref(&parent_id)),
            CallArg::Receiving(child),
        ],
        &gas_id,
        sender,
        &sender_key,
    );
    assert_eq!(
        keeping.latest_ref(&parent_id),
        pruning.latest_ref(&parent_id)
    );
    assert_eq!(keeping.latest_ref(&gas_id), pruning.latest_ref(&gas_id));

    for s in [&pruning, &keeping] {
        let _watcher = s.start_execution_watcher();
        let (scheduled, _) = s
            .process_at_with_versions(1, std::slice::from_ref(&receive))
            .await;
        assert!(
            scheduled.is_empty(),
            "the receive must not be kept at commit 1 of the new epoch"
        );
        assert_eq!(
            s.epoch_store
                .post_consensus_decision(1, *receive.digest())
                .unwrap()
                .map(|saved| saved.decision),
            Some(PostConsensusDecision::Dropped)
        );
        s.wait_for_fully_executed_commit(1).await;
    }
}

// ---------------------------------------------------------------------------
// P-COOL deterministic-validation checks at a commit: shared inputs
// ---------------------------------------------------------------------------

/// A live shared object with the given id, in place of a system object the
/// test authority does not create. Its contents are never read before the
/// rules under test reject it.
fn shared_object_with_id(id: ObjectId) -> Object {
    Object::with_id_owner_version_for_testing(
        id,
        Version::OBJECT_START,
        Owner::Shared(Version::OBJECT_START),
    )
}

/// Post-consensus validation does not read a live shared input, but the rules
/// that limit a user transaction's use of system objects still apply, with the
/// same error a validator gives before submission. The clock and the
/// randomness state can be inputs only as immutable. The authenticator state
/// and the deny rules object cannot be inputs at all, mutable or not.
#[tokio::test]
async fn validation_at_commit_drops_a_forbidden_system_object_input() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(gas_id, sender),
            shared_object_with_id(ObjectId::AUTHENTICATOR_STATE),
            shared_object_with_id(ObjectId::TRANSACTION_DENY_RULES),
        ],
        true,
    )
    .await;
    let deny_config = &s.authority.config.transaction_deny_config;

    let randomness_version = initial_shared_version(
        &s.authority
            .get_object(&ObjectId::RANDOMNESS_STATE)
            .unwrap()
            .owner,
    );
    let declared = |object_id, version, mutable| {
        CallArg::Shared(SharedObjectReference::new(object_id, version, mutable))
    };
    let immutable_parameter_expected =
        |object_id| UserInputError::ImmutableParameterExpected { object_id };
    let inaccessible_system_object =
        |object_id| UserInputError::InaccessibleSystemObject { object_id };
    let cases = [
        (
            CallArg::CLOCK_MUTABLE,
            immutable_parameter_expected(ObjectId::CLOCK),
        ),
        (
            declared(ObjectId::RANDOMNESS_STATE, randomness_version, true),
            immutable_parameter_expected(ObjectId::RANDOMNESS_STATE),
        ),
        (
            declared(ObjectId::AUTHENTICATOR_STATE, Version::OBJECT_START, false),
            inaccessible_system_object(ObjectId::AUTHENTICATOR_STATE),
        ),
        (
            declared(ObjectId::AUTHENTICATOR_STATE, Version::OBJECT_START, true),
            inaccessible_system_object(ObjectId::AUTHENTICATOR_STATE),
        ),
        (
            declared(
                ObjectId::TRANSACTION_DENY_RULES,
                Version::OBJECT_START,
                false,
            ),
            inaccessible_system_object(ObjectId::TRANSACTION_DENY_RULES),
        ),
        (
            declared(
                ObjectId::TRANSACTION_DENY_RULES,
                Version::OBJECT_START,
                true,
            ),
            inaccessible_system_object(ObjectId::TRANSACTION_DENY_RULES),
        ),
    ];

    for (input, expected) in cases {
        let tx = s.build_move_call(
            "object_basics",
            "use_clock",
            vec![input.clone()],
            &gas_id,
            sender,
            &sender_key,
        );

        let error = s
            .authority
            .handle_transaction_validation_checks(&tx, &s.epoch_store, deny_config)
            .await
            .expect_err("a validator must reject the input before submission");
        assert_eq!(
            UserInputError::try_from(error).unwrap(),
            expected,
            "{input:?}"
        );

        let error = s
            .authority
            .handle_transaction_validation_checks_at_commit(
                &s.reader_at(12),
                &tx,
                &s.epoch_store,
                deny_config,
                VerifierLimitsSource::ProtocolConfig,
                false,
            )
            .expect_err("post-consensus validation must drop the input");
        assert_eq!(
            UserInputError::try_from(error).unwrap(),
            expected,
            "{input:?}"
        );
    }
}

/// Only a mutable clock is rejected. A user transaction that declares the
/// clock immutable is kept.
#[tokio::test]
async fn validation_at_commit_keeps_a_clock_declared_immutable() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;
    let deny_config = &s.authority.config.transaction_deny_config;

    let tx = s.build_move_call(
        "object_basics",
        "use_clock",
        vec![CallArg::CLOCK_IMMUTABLE],
        &gas_id,
        sender,
        &sender_key,
    );
    let gas_ref = s.latest_ref(&gas_id);

    match s
        .authority
        .handle_transaction_validation_checks_at_commit(
            &s.reader_at(12),
            &tx,
            &s.epoch_store,
            deny_config,
            VerifierLimitsSource::ProtocolConfig,
            false,
        )
        .unwrap()
    {
        ValidationAtCommit::Keep(owned) => assert_eq!(
            owned,
            vec![gas_ref],
            "the gas coin must be the only object the handler locks"
        ),
        other => panic!("expected keep, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// P-COOL deterministic-validation checks at a commit: the Move authenticator
// ---------------------------------------------------------------------------

/// Post-consensus validation does not resolve a Move authenticator's account,
/// but it requires the account to have its authenticator function field at
/// the horizon. A transaction whose account has no authenticator function
/// never leaves an honest validator: the checks it runs before submitting to
/// consensus reject it. A faulty validator can still propose it, and then
/// post-consensus validation reports the account missing, so the transaction
/// does not reach execution.
#[tokio::test]
async fn validation_at_commit_reports_an_object_that_is_not_an_account_missing() {
    let (sponsor, sponsor_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![Object::with_id_owner_for_testing(gas_id, sponsor)],
        true,
    )
    .await;
    let deny_config = &s.authority.config.transaction_deny_config;

    // A shared object that was never made an account.
    let share_effects =
        s.assigned_commit_object_basics_call("share", vec![], &gas_id, sponsor, &sponsor_key, 5);
    let account_id = *share_effects.created()[0].reference().object_id();
    let account_start_version =
        initial_shared_version(&s.authority.get_object(&account_id).unwrap().owner);
    let tx = s.build_account_authenticated_call(
        MoveAuthenticatorV1::new(
            vec![],
            vec![],
            SharedObjectReference::new(account_id, account_start_version, false),
        ),
        &gas_id,
        sponsor,
        &sponsor_key,
    );

    // The checks a validator runs before submitting to consensus, called with
    // the arguments of its submission path, resolve the account and reject
    // the transaction.
    let error = s
        .authority
        .handle_transaction_validation_checks(&tx, &s.epoch_store, deny_config)
        .await
        .expect_err("a validator must reject an account without a function before submission");
    assert!(
        matches!(
            error,
            IotaError::UserInput {
                error: UserInputError::MoveAuthenticatorNotFound { .. }
            }
        ),
        "{error:?}"
    );

    // A faulty validator skips those checks and proposes the transaction
    // anyway. Every validator reports the account missing.
    match s
        .authority
        .handle_transaction_validation_checks_at_commit(
            &s.reader_at(12),
            &tx,
            &s.epoch_store,
            deny_config,
            VerifierLimitsSource::ProtocolConfig,
            false,
        )
        .unwrap()
    {
        ValidationAtCommit::Missing(InputObjectKind::SharedMoveObject { id, .. }, reason) => {
            assert_eq!(id, account_id);
            assert_eq!(reason.kind(), MissingKind::AuthenticatorFunctionNotFound);
        }
        other => panic!("expected the account missing, got {other:?}"),
    }
}

/// A Move authenticator needs its account to be a shared object, but a
/// transaction can still name an object owned by an address as its account.
/// A validator rejects such a transaction before submission, and
/// post-consensus validation drops it when reading the account finds an object
/// that is not shared, so it never reaches execution.
#[tokio::test]
async fn validation_at_commit_drops_an_account_owned_by_an_address() {
    let (sponsor, sponsor_key): (Address, AccountPrivateKey) = get_key_pair();
    let (owner, _): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let account_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(gas_id, sponsor),
            Object::with_id_owner_for_testing(account_id, owner),
        ],
        true,
    )
    .await;
    let deny_config = &s.authority.config.transaction_deny_config;

    let tx = s.build_account_authenticated_call(
        MoveAuthenticatorV1::new(
            vec![],
            vec![],
            SharedObjectReference::new(account_id, s.latest_ref(&account_id).version(), false),
        ),
        &gas_id,
        sponsor,
        &sponsor_key,
    );

    let error = s
        .authority
        .handle_transaction_validation_checks(&tx, &s.epoch_store, deny_config)
        .await
        .expect_err("a validator must reject an address-owned account before submission");
    assert!(
        matches!(
            error,
            IotaError::UserInput {
                error: UserInputError::AccountObjectNotSupported { object_id }
            } if object_id == account_id
        ),
        "{error:?}"
    );

    match s
        .authority
        .handle_transaction_validation_checks_at_commit(
            &s.reader_at(12),
            &tx,
            &s.epoch_store,
            deny_config,
            VerifierLimitsSource::ProtocolConfig,
            false,
        )
        .unwrap()
    {
        ValidationAtCommit::Drop(InputObjectKind::SharedMoveObject { id, .. }, reason) => {
            assert_eq!(id, account_id);
            assert_eq!(reason.kind(), DropKind::SharedNotCreatedShared);
        }
        other => panic!("expected a drop on the account, got {other:?}"),
    }
}

/// A Move authenticator needs its account to be a shared object, but a
/// transaction can still name an immutable object as its account. A validator
/// rejects such a transaction before submission, and post-consensus validation
/// drops it when reading the account finds an object that is not shared, so
/// it never reaches execution.
#[tokio::test]
async fn validation_at_commit_drops_an_immutable_account() {
    let (sponsor, sponsor_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let account = Object::immutable_for_testing();
    let account_id = account.id();
    let s = setup_handler_object_state(
        vec![Object::with_id_owner_for_testing(gas_id, sponsor), account],
        true,
    )
    .await;
    let deny_config = &s.authority.config.transaction_deny_config;

    let tx = s.build_account_authenticated_call(
        MoveAuthenticatorV1::new(
            vec![],
            vec![],
            SharedObjectReference::new(account_id, s.latest_ref(&account_id).version(), false),
        ),
        &gas_id,
        sponsor,
        &sponsor_key,
    );

    let error = s
        .authority
        .handle_transaction_validation_checks(&tx, &s.epoch_store, deny_config)
        .await
        .expect_err("a validator must reject an immutable account before submission");
    assert!(
        matches!(
            error,
            IotaError::UserInput {
                error: UserInputError::ImmutableAccountObjectNotSupported { object_id }
            } if object_id == account_id
        ),
        "{error:?}"
    );

    match s
        .authority
        .handle_transaction_validation_checks_at_commit(
            &s.reader_at(12),
            &tx,
            &s.epoch_store,
            deny_config,
            VerifierLimitsSource::ProtocolConfig,
            false,
        )
        .unwrap()
    {
        ValidationAtCommit::Drop(InputObjectKind::SharedMoveObject { id, .. }, reason) => {
            assert_eq!(id, account_id);
            assert_eq!(reason.kind(), DropKind::SharedNotCreatedShared);
        }
        other => panic!("expected a drop on the account, got {other:?}"),
    }
}

/// Post-consensus validation keeps a transaction whose account object was
/// deleted by a commit that not every validator has executed yet. The
/// account's authenticator function field outlives the object, so the account
/// still has it at the horizon. Those
/// validators that have not executed yet cannot see the deletion, so keeping
/// it is the only result every validator can reach. The check of the Move
/// authenticator's input objects skips a deleted shared object, and execution
/// fails the transaction with `InputObjectDeleted`.
#[tokio::test]
async fn validation_at_commit_keeps_a_deleted_account_that_execution_rejects() {
    let (sponsor, sponsor_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let setup_gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(gas_id, sponsor),
            Object::with_id_owner_for_testing(setup_gas_id, sponsor),
        ],
        true,
    )
    .await;
    let deny_config = &s.authority.config.transaction_deny_config;

    // The account is shared and deleted with a second gas coin. Had commit 15
    // written the call's gas coin, that coin would be missing at commit 18.
    let share_effects = s.assigned_commit_object_basics_call(
        "share",
        vec![],
        &setup_gas_id,
        sponsor,
        &sponsor_key,
        5,
    );
    let shared = share_effects.created()[0];
    let account_id = *shared.reference().object_id();
    let account_start_version = initial_shared_version(shared.owner());
    // Its authenticator function field, as epoch-start state, makes it an
    // account at every horizon. The function is never called.
    s.authority.insert_genesis_object(authenticator_field(
        &s.authority.get_object(&account_id).unwrap(),
        "authenticate",
    ));
    s.assigned_commit_shared_object_basics_call(
        "delete",
        vec![s.shared_arg(&account_id)],
        &setup_gas_id,
        sponsor,
        &sponsor_key,
        15,
    );
    let tx = s.build_account_authenticated_call(
        MoveAuthenticatorV1::new(
            vec![],
            vec![],
            SharedObjectReference::new(account_id, account_start_version, false),
        ),
        &gas_id,
        sponsor,
        &sponsor_key,
    );
    let gas_ref = s.latest_ref(&gas_id);

    // Commit 18, horizon 14: the deletion at 15 is not yet visible to every
    // validator.
    match s
        .authority
        .handle_transaction_validation_checks_at_commit(
            &s.reader_at(18),
            &tx,
            &s.epoch_store,
            deny_config,
            VerifierLimitsSource::ProtocolConfig,
            false,
        )
        .unwrap()
    {
        ValidationAtCommit::Keep(owned) => assert_eq!(
            owned,
            vec![gas_ref],
            "the gas coin must be the only object the handler locks"
        ),
        other => panic!("expected keep, got {other:?}"),
    }

    s.epoch_store
        .assign_commit_to_transactions(18, vec![TransactionKey::Digest(*tx.digest())]);
    let effects = s.execute_with_assigned_shared_versions_unchecked(tx);
    assert_eq!(
        effects.status(),
        &ExecutionStatus::Failure {
            error: ExecutionErrorKind::InputObjectDeleted,
            command: None,
        },
        "execution must fail the kept transaction because its account was deleted"
    );
}

/// The dynamic field of the account `account_id`, at `version`, naming
/// `object_basics::authenticate` of `package` as the account's authenticate
/// function.
fn authenticator_function_ref_field(
    account_id: ObjectId,
    package: ObjectId,
    version: Version,
) -> Object {
    let field_id = derive_authenticator_function_ref_v1_dynamic_field_id(account_id).unwrap();
    let field = Field {
        id: UID::from(field_id),
        name: AuthenticatorFunctionRefV1Key::default(),
        value: AuthenticatorFunctionRefV1 {
            package,
            module: "object_basics".to_string(),
            function: "authenticate".to_string(),
        },
    };
    let field_type = DynamicFieldInfo::dynamic_field_type(
        StructTag::new_authenticator_function_ref_v1_key().into(),
        AuthenticatorFunctionRefV1::type_(StructTag::new_gas_coin()).into(),
    );
    Object::new_move(
        MoveStruct::new(field_type.into(), version, bcs::to_bytes(&field).unwrap()).unwrap(),
        Owner::Object(account_id),
        TransactionDigest::GENESIS_MARKER,
    )
}

/// Deny rules that deny `package` and nothing else.
fn deny_package(package: ObjectId) -> DenyRuleSet {
    DenyRuleSet {
        denied_packages: [package].into(),
        ..Default::default()
    }
}

/// Asserts that the deny check of the authenticator's package rejected the
/// transaction for `package`.
#[track_caller]
fn assert_authenticator_package_denied(result: IotaResult<ValidationAtCommit>, package: ObjectId) {
    match result {
        Ok(ValidationAtCommit::AuthenticatorPackageDenied(IotaError::UserInput {
            error: UserInputError::TransactionDenied { error },
        })) => assert_eq!(
            error,
            format!(
                "Access to package {package} from a Move authenticator is temporarily disabled"
            )
        ),
        other => panic!("expected a denial of package {package}, got {other:?}"),
    }
}

/// The error of a denial of an authenticator package at the commit.
#[track_caller]
fn authenticator_package_denial(result: &IotaResult<ValidationAtCommit>) -> IotaError {
    match result {
        Ok(ValidationAtCommit::AuthenticatorPackageDenied(error)) => error.clone(),
        other => panic!("expected a denial of an authenticator package, got {other:?}"),
    }
}

/// Asserts that validation at the commit kept the transaction.
#[track_caller]
fn assert_kept(result: IotaResult<ValidationAtCommit>) {
    assert!(
        matches!(result, Ok(ValidationAtCommit::Keep(_))),
        "expected keep, got {result:?}"
    );
}

/// A shared account whose authenticator function field names the
/// `object_basics` package, on a setup with the P-COOL flow on, and a sponsored
/// transfer the account sends. Returns the setup and the transfer.
async fn setup_account_with_authenticator_in_object_basics(
    config_guard: Option<OverrideGuard>,
) -> (HandlerObjectStateSetup, VerifiedTransaction) {
    let (sponsor, sponsor_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let account_id = ObjectId::random();
    let s = setup_handler_object_state_with_config_guard(
        vec![
            Object::with_id_owner_for_testing(gas_id, sponsor),
            shared_object_with_id(account_id),
        ],
        config_guard,
    )
    .await;
    s.authority
        .insert_genesis_object(authenticator_function_ref_field(
            account_id,
            s.package_id,
            Version::OBJECT_START,
        ));
    let tx = s.build_account_authenticated_transfer(
        account_authenticator(account_id),
        &gas_id,
        sponsor,
        &sponsor_key,
    );
    (s, tx)
}

/// Post-consensus validation does not resolve the account, but it still
/// checks the package that holds the account's authenticate function, and that
/// package's dependencies, against the deny rules, as a validator does before
/// submitting to consensus. It reads the account's authenticator function
/// field as of the horizon. A denial drops the transaction with the error
/// admission gives.
#[tokio::test]
async fn validation_at_commit_checks_the_authenticator_package_against_the_deny_rules() {
    let (s, tx) = setup_account_with_authenticator_in_object_basics(None).await;

    for denied in [s.package_id, ObjectId::FRAMEWORK] {
        assert_authenticator_package_denied(
            s.validate_at_commit_under(12, &tx, &deny_package(denied)),
            denied,
        );
        let at_admission = s
            .authority
            .handle_transaction_validation_checks(&tx, &s.epoch_store, &deny_package(denied))
            .await
            .map(|_| ())
            .unwrap_err();
        assert_eq!(
            authenticator_package_denial(&s.validate_at_commit_under(
                12,
                &tx,
                &deny_package(denied)
            )),
            at_admission,
            "admission and the commit must deny with the same error"
        );
    }

    assert_kept(s.validate_at_commit_under(12, &tx, &deny_package(ObjectId::random())));
}

/// A package that post-consensus validation does not see at the commit cannot
/// be checked against the deny rules, and it is not a reason to drop the
/// transaction either: execution reports it. Whether the package is unknown or
/// published above the horizon, every validator then keeps the transaction.
/// Once the horizon reaches the publish, the package is checked.
#[tokio::test]
async fn validation_at_commit_leaves_an_authenticator_package_it_does_not_see_to_execution() {
    let (sponsor, sponsor_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let late_account = ObjectId::random();
    let unknown_account = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(gas_id, sponsor),
            shared_object_with_id(late_account),
            shared_object_with_id(unknown_account),
        ],
        true,
    )
    .await;
    let unknown = ObjectId::random();
    for (account_id, package) in [(late_account, s.package_id), (unknown_account, unknown)] {
        s.authority
            .insert_genesis_object(authenticator_function_ref_field(
                account_id,
                package,
                Version::OBJECT_START,
            ));
    }
    let transfer_by = |account_id| {
        s.build_account_authenticated_transfer(
            account_authenticator(account_id),
            &gas_id,
            sponsor,
            &sponsor_key,
        )
    };

    let by_unknown = transfer_by(unknown_account);
    assert_kept(s.validate_at_commit_under(12, &by_unknown, &deny_package(unknown)));

    // Published by commit 11, above the horizon of commit 12.
    s.record_package_published_at(11);
    let by_late = transfer_by(late_account);
    assert_kept(s.validate_at_commit_under(12, &by_late, &deny_package(s.package_id)));
    assert_authenticator_package_denied(
        s.validate_at_commit_under(
            11 + s.horizon_distance(),
            &by_late,
            &deny_package(s.package_id),
        ),
        s.package_id,
    );
}

/// Commit 11 rotates the account's authenticate function from a denied
/// package to an allowed one, recorded as the new version of the account's
/// authenticator function field that the rotation writes. At commit 14,
/// horizon 10, a validator that has executed commit 11 and one that has not
/// both read the function the account named before the rotation, so both drop
/// the transaction alike. Once the horizon reaches the rotation, the
/// transaction is kept.
#[tokio::test]
async fn authenticator_function_rotated_above_the_horizon_gets_one_verdict_on_every_validator() {
    let (sponsor, sponsor_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let account_id = ObjectId::random();
    let genesis = vec![
        Object::with_id_owner_for_testing(gas_id, sponsor),
        shared_object_with_id(account_id),
    ];
    let rotated = setup_handler_object_state(genesis.clone(), true).await;
    let not_rotated = setup_handler_object_state(genesis, true).await;
    assert_eq!(rotated.package_id, not_rotated.package_id);
    let denied = rotated.package_id;
    for node in [&rotated, &not_rotated] {
        node.authority
            .insert_genesis_object(authenticator_function_ref_field(
                account_id,
                denied,
                Version::OBJECT_START,
            ));
    }
    rotated.record_write_at(
        authenticator_function_ref_field(account_id, ObjectId::SYSTEM, Version::from_u64(2)),
        Version::OBJECT_START,
        11,
    );
    let tx = rotated.build_account_authenticated_transfer(
        account_authenticator(account_id),
        &gas_id,
        sponsor,
        &sponsor_key,
    );

    let on_rotated = rotated.validate_at_commit_under(14, &tx, &deny_package(denied));
    let on_not_rotated = not_rotated.validate_at_commit_under(14, &tx, &deny_package(denied));
    assert_eq!(
        authenticator_package_denial(&on_rotated),
        authenticator_package_denial(&on_not_rotated),
        "a validator that executed the rotation and one that did not must reach the same verdict"
    );
    assert_authenticator_package_denied(on_rotated, denied);

    assert_kept(rotated.validate_at_commit_under(
        11 + rotated.horizon_distance(),
        &tx,
        &deny_package(denied),
    ));
}

/// Commit 11 rotates the account's authenticate function from an allowed
/// package to a denied one. A rotation to a denied package above the horizon
/// is not caught at the commit: at commit 14, horizon 10, a validator that has
/// executed commit 11 and one that has not both read the function the account
/// named before the rotation, so both keep the transaction. Once the horizon
/// reaches the rotation, the transaction is dropped.
#[tokio::test]
async fn authenticator_function_rotated_to_a_denied_package_above_the_horizon_is_not_caught() {
    let (sponsor, sponsor_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let account_id = ObjectId::random();
    let genesis = vec![
        Object::with_id_owner_for_testing(gas_id, sponsor),
        shared_object_with_id(account_id),
    ];
    let rotated = setup_handler_object_state(genesis.clone(), true).await;
    let not_rotated = setup_handler_object_state(genesis, true).await;
    assert_eq!(rotated.package_id, not_rotated.package_id);
    let denied = rotated.package_id;
    for node in [&rotated, &not_rotated] {
        node.authority
            .insert_genesis_object(authenticator_function_ref_field(
                account_id,
                ObjectId::SYSTEM,
                Version::OBJECT_START,
            ));
    }
    rotated.record_write_at(
        authenticator_function_ref_field(account_id, denied, Version::from_u64(2)),
        Version::OBJECT_START,
        11,
    );
    let tx = rotated.build_account_authenticated_transfer(
        account_authenticator(account_id),
        &gas_id,
        sponsor,
        &sponsor_key,
    );

    assert_kept(rotated.validate_at_commit_under(14, &tx, &deny_package(denied)));
    assert_kept(not_rotated.validate_at_commit_under(14, &tx, &deny_package(denied)));

    assert_authenticator_package_denied(
        rotated.validate_at_commit_under(
            11 + rotated.horizon_distance(),
            &tx,
            &deny_package(denied),
        ),
        denied,
    );
}

/// The account's authenticator function field is written above the horizon,
/// and the store no longer holds the version that write replaced, which the
/// read as of the horizon needs. Validation at the commit fails with a storage
/// error instead of keeping or dropping the transaction, whether or not a
/// package is denied, since the account check reads the same field. Once the
/// horizon reaches the write, the transaction is kept.
#[tokio::test]
async fn authenticator_function_field_missing_from_the_store_is_a_storage_error() {
    let (sponsor, sponsor_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let account_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(gas_id, sponsor),
            shared_object_with_id(account_id),
        ],
        true,
    )
    .await;
    s.record_write_at(
        authenticator_function_ref_field(account_id, s.package_id, Version::from_u64(2)),
        Version::OBJECT_START,
        11,
    );
    let tx = s.build_account_authenticated_transfer(
        account_authenticator(account_id),
        &gas_id,
        sponsor,
        &sponsor_key,
    );
    let unrelated = deny_package(ObjectId::random());

    for rules in [&unrelated, &DenyRuleSet::default()] {
        match s.validate_at_commit_under(12, &tx, rules) {
            Err(error @ IotaError::Storage(_)) => assert!(error.is_storage_or_epoch_error()),
            other => panic!("expected a storage error, got {other:?}"),
        }
    }

    assert_kept(s.validate_at_commit_under(11 + s.horizon_distance(), &tx, &unrelated));
}

/// A transaction sent by one account and sponsored by another carries a Move
/// authenticator for each, the sender's first. Post-consensus validation
/// checks the package of each account's authenticate function, so a denial of
/// the sponsor's package alone drops the transaction with the error admission
/// gives for that package. With neither package denied, the transaction is
/// kept.
#[tokio::test]
async fn validation_at_commit_checks_the_authenticator_package_of_a_move_account_sponsor() {
    let gas_id = ObjectId::random();
    let sender_account = ObjectId::random();
    let sponsor_account = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(gas_id, sponsor_account.into()),
            shared_object_with_id(sender_account),
            shared_object_with_id(sponsor_account),
        ],
        true,
    )
    .await;
    for (account_id, package) in [
        (sender_account, ObjectId::SYSTEM),
        (sponsor_account, s.package_id),
    ] {
        s.authority
            .insert_genesis_object(authenticator_function_ref_field(
                account_id,
                package,
                Version::OBJECT_START,
            ));
    }
    let mut builder = ProgrammableTransactionBuilder::new();
    builder.transfer_iota(sponsor_account.into(), Some(1));
    let transfer = Transaction::new_programmable_allow_sponsor(
        sender_account.into(),
        vec![s.latest_ref(&gas_id)],
        builder.finish(),
        TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS * s.rgp,
        s.rgp,
        sponsor_account.into(),
    );
    let signatures = [sender_account, sponsor_account]
        .map(|account_id| {
            UserSignature::MoveAuthenticator(MoveAuthenticator::from(account_authenticator(
                account_id,
            )))
        })
        .to_vec();
    let tx = s
        .epoch_store
        .verify_transaction(TransactionEnvelope::from_user_sig_data(
            transfer, signatures,
        ))
        .unwrap();
    assert_eq!(tx.move_authenticators().len(), 2);

    let denied = s.package_id;
    let at_commit = s.validate_at_commit_under(12, &tx, &deny_package(denied));
    let at_admission = s
        .authority
        .handle_transaction_validation_checks(&tx, &s.epoch_store, &deny_package(denied))
        .await
        .map(|_| ())
        .unwrap_err();
    assert_eq!(
        authenticator_package_denial(&at_commit),
        at_admission,
        "admission and the commit must deny with the same error"
    );
    assert_authenticator_package_denied(at_commit, denied);

    assert_kept(s.validate_at_commit_under(12, &tx, &deny_package(ObjectId::random())));
}

/// With `deny_authenticator_packages` off, neither admission nor
/// post-consensus validation checks the package of the account's authenticate
/// function, so the commit keeps the transaction although that package is
/// denied.
#[tokio::test]
async fn validation_at_commit_does_not_check_the_authenticator_package_with_the_flag_off() {
    let config_guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_deny_authenticator_packages_for_testing(false);
        config
    });
    let (s, tx) = setup_account_with_authenticator_in_object_basics(Some(config_guard)).await;

    assert_kept(s.validate_at_commit_under(12, &tx, &deny_package(s.package_id)));
}

/// [`setup_account_with_authenticator_in_object_basics`], but the account's
/// authenticator names a type of the `object_basics` package as its type
/// argument, and the package's publish is recorded at commit 11. The transfer
/// names no package itself.
async fn setup_account_with_object_basics_type_arg_published_at_11(
    config_guard: Option<OverrideGuard>,
) -> (HandlerObjectStateSetup, VerifiedTransaction) {
    let (sponsor, sponsor_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let account_id = ObjectId::random();
    let s = setup_handler_object_state_with_config_guard(
        vec![
            Object::with_id_owner_for_testing(gas_id, sponsor),
            shared_object_with_id(account_id),
        ],
        config_guard,
    )
    .await;
    s.authority
        .insert_genesis_object(authenticator_function_ref_field(
            account_id,
            s.package_id,
            Version::OBJECT_START,
        ));
    let authenticator = MoveAuthenticatorV1::new(
        vec![],
        vec![TypeTag::from_str(&format!("{}::object_basics::Object", s.package_id)).unwrap()],
        SharedObjectReference::new(account_id, Version::OBJECT_START, false),
    );
    let tx = s.build_account_authenticated_transfer(authenticator, &gas_id, sponsor, &sponsor_key);
    s.record_package_published_at(11);
    (s, tx)
}

/// A package a Move authenticator's type arguments name is an input of the
/// authenticator. Published above the horizon, it is missing at the commit, as
/// the package of a call would be. Once the horizon reaches the publish, the
/// transaction is kept.
#[tokio::test]
async fn validation_at_commit_reports_an_authenticator_type_arg_package_above_the_horizon_missing()
{
    let (s, tx) = setup_account_with_object_basics_type_arg_published_at_11(None).await;

    match s.validate_at_commit_under(12, &tx, &DenyRuleSet::default()) {
        Ok(ValidationAtCommit::Missing(InputObjectKind::MovePackage(id), reason)) => {
            assert_eq!(id, s.package_id);
            assert_eq!(reason.kind(), MissingKind::PackageAboveHorizon);
        }
        other => panic!("expected missing on the package, got {other:?}"),
    }
    assert_kept(s.validate_at_commit_under(15, &tx, &DenyRuleSet::default()));
}

/// With `move_authenticator_type_arg_packages_as_inputs` off, the packages a
/// Move authenticator's type arguments name are not inputs, so one published
/// above the horizon does not hold the transaction back.
#[tokio::test]
async fn validation_at_commit_does_not_read_authenticator_type_arg_packages_with_the_flag_off() {
    let config_guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_move_authenticator_type_arg_packages_as_inputs_for_testing(false);
        config
    });
    let (s, tx) =
        setup_account_with_object_basics_type_arg_published_at_11(Some(config_guard)).await;

    assert_kept(s.validate_at_commit_under(12, &tx, &DenyRuleSet::default()));
}

/// A Move authenticator naming a mutable shared object as a call argument is
/// refused at admission, and a byzantine submitter can still sequence it. At
/// commit 18, horizon 14, the object is already deleted at commit 15 on one
/// validator and still live on another. Both must drop the transaction on the
/// declaration, which no object read can change.
#[tokio::test]
async fn validation_loop_drops_a_mutable_shared_authenticator_argument_on_every_validator() {
    for deletion_executed in [false, true] {
        let (sponsor, sponsor_key): (Address, AccountPrivateKey) = get_key_pair();
        let gas_id = ObjectId::random();
        let setup_gas_id = ObjectId::random();
        let s = setup_handler_object_state(
            vec![
                Object::with_id_owner_for_testing(gas_id, sponsor),
                Object::with_id_owner_for_testing(setup_gas_id, sponsor),
            ],
            true,
        )
        .await;

        let share = |index| {
            let effects = s.assigned_commit_object_basics_call(
                "share",
                vec![],
                &setup_gas_id,
                sponsor,
                &sponsor_key,
                index,
            );
            let shared = effects.created()[0];
            (
                *shared.reference().object_id(),
                initial_shared_version(shared.owner()),
            )
        };
        let (account_id, account_start_version) = share(5);
        let (s_id, s_start_version) = share(6);
        if deletion_executed {
            s.assigned_commit_shared_object_basics_call(
                "delete",
                vec![s.shared_arg(&s_id)],
                &setup_gas_id,
                sponsor,
                &sponsor_key,
                15,
            );
        }

        let tx = VerifiedTransaction::new_unchecked(s.account_authenticated_call(
            MoveAuthenticatorV1::new(
                vec![CallArg::Shared(SharedObjectReference::new(
                    s_id,
                    s_start_version,
                    true,
                ))],
                vec![],
                SharedObjectReference::new(account_id, account_start_version, false),
            ),
            &gas_id,
            sponsor,
            &sponsor_key,
        ));
        let digest = *tx.digest();
        let mut transactions = vec![make_user_tx_v1_verified(tx)];
        let (dropped, locks, _) = post_consensus_validation::validate_and_resolve_conflicts(
            &s.authority,
            &s.epoch_store,
            18,
            &Default::default(),
            &mut transactions,
        )
        .await
        .unwrap();

        assert!(
            transactions.is_empty(),
            "kept with deletion executed: {deletion_executed}"
        );
        assert!(locks.is_empty(), "deletion executed: {deletion_executed}");
        assert_eq!(dropped.len(), 1, "deletion executed: {deletion_executed}");
        assert_eq!(dropped[0].0, digest);
        assert!(
            matches!(
                &dropped[0].1,
                IotaError::UserInput {
                    error: UserInputError::MutableSharedIsInMoveAuthenticatorInput { object_id }
                } if *object_id == s_id
            ),
            "deletion executed: {deletion_executed}, got {:?}",
            dropped[0].1
        );
    }
}

/// The rules that limit a Move authenticator's shared inputs stay in
/// post-consensus validation, although the input is not read there. None of
/// them can be mutable, the clock included, and the authenticator state cannot
/// be an input at all, mutable or not.
#[tokio::test]
async fn validation_at_commit_drops_a_forbidden_shared_authenticator_input() {
    let (sponsor, sponsor_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(gas_id, sponsor),
            shared_object_with_id(ObjectId::AUTHENTICATOR_STATE),
        ],
        true,
    )
    .await;
    let deny_config = &s.authority.config.transaction_deny_config;

    // The account's authenticator function is not looked up here, so a shared
    // object that was never made an account will do.
    let share_effects =
        s.assigned_commit_object_basics_call("share", vec![], &gas_id, sponsor, &sponsor_key, 5);
    let account_id = *share_effects.created()[0].reference().object_id();
    let account_start_version =
        initial_shared_version(&s.authority.get_object(&account_id).unwrap().owner);

    let declared = |object_id, mutable| {
        CallArg::Shared(SharedObjectReference::new(
            object_id,
            Version::OBJECT_START,
            mutable,
        ))
    };
    let cases = [
        (
            declared(ObjectId::CLOCK, true),
            UserInputError::MutableSharedIsInMoveAuthenticatorInput {
                object_id: ObjectId::CLOCK,
            },
        ),
        (
            declared(ObjectId::AUTHENTICATOR_STATE, false),
            UserInputError::InaccessibleSystemObject {
                object_id: ObjectId::AUTHENTICATOR_STATE,
            },
        ),
        (
            declared(ObjectId::AUTHENTICATOR_STATE, true),
            UserInputError::InaccessibleSystemObject {
                object_id: ObjectId::AUTHENTICATOR_STATE,
            },
        ),
    ];

    for (input, expected) in cases {
        // Admission refuses these declarations in `validity_check`, so the
        // transaction is built unverified to reach the checks under test.
        let tx = VerifiedTransaction::new_unchecked(s.account_authenticated_call(
            MoveAuthenticatorV1::new(
                vec![input.clone()],
                vec![],
                SharedObjectReference::new(account_id, account_start_version, false),
            ),
            &gas_id,
            sponsor,
            &sponsor_key,
        ));

        let error = s
            .authority
            .handle_transaction_validation_checks_at_commit(
                &s.reader_at(12),
                &tx,
                &s.epoch_store,
                deny_config,
                VerifierLimitsSource::ProtocolConfig,
                false,
            )
            .expect_err("post-consensus validation must drop the input");
        assert_eq!(
            UserInputError::try_from(error).unwrap(),
            expected,
            "{input:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// P-COOL deterministic-validation wiring
// ---------------------------------------------------------------------------

/// With the flag on, the validation loop reads inputs as of the commit. One
/// transaction keeps and locks its gas coin, one drops on a wrong shared
/// declaration, one is missing on a gas coin produced above the horizon. The
/// drop carries the error for an input not available at the commit, which is
/// final, and the missing one the not-found error, which is retried.
#[tokio::test]
async fn validation_loop_at_a_commit_keeps_drops_and_reports_missing() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_keep = ObjectId::random();
    let gas_wrong = ObjectId::random();
    let gas_missing = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(gas_keep, sender),
            Object::with_id_owner_for_testing(gas_wrong, sender),
            Object::with_id_owner_for_testing(gas_missing, sender),
        ],
        true,
    )
    .await;

    let share_effects =
        s.assigned_commit_object_basics_call("share", vec![], &gas_keep, sender, &sender_key, 5);
    let shared = share_effects.created()[0];
    let shared_id = shared.reference().object_id();
    // Commit 11 mutates `gas_missing`. At commit 12 the horizon is 10.
    s.assigned_commit_shared_object_basics_call(
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
        &Default::default(),
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
                error: UserInputError::InputObjectNotAvailableAtCommit { object_id, version: None }
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
    // caller reports for a drop. The input not visible at the horizon yet is
    // saved as `DroppedInputNotVisibleYet`, so the drop is not final for the digest.
    let saved: Vec<_> = all_digests
        .iter()
        .map(|digest| s.epoch_store.post_consensus_decision(12, *digest).unwrap())
        .collect();
    assert_eq!(
        saved,
        vec![
            Some(SavedPostConsensusDecision::kept()),
            Some(SavedPostConsensusDecision::dropped(
                PostConsensusDecision::Dropped,
                &dropped[0].1
            )),
            Some(SavedPostConsensusDecision::dropped(
                PostConsensusDecision::DroppedInputNotVisibleYet,
                &dropped[1].1
            )),
        ]
    );
}

/// Copies of one transaction in a commit, the first with a Move
/// authenticator that fails validation, then the honest copy twice: the
/// honest copy is validated after the failing one and kept, and its repeat is
/// not validated. A replay of the commit keeps the same copy. A commit whose
/// decisions were saved when only the first copy was validated replays the
/// saved drop.
#[tokio::test]
async fn later_copy_is_validated_after_a_copy_fails_on_its_authenticator() {
    use typed_store::Map;

    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let object = ObjectId::random();
    let gas = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(object, sender),
            Object::with_id_owner_for_testing(gas, sender),
        ],
        true,
    )
    .await;
    let honest = s.build_transfer(&object, &gas, sender, &key, Address::random());
    let digest = *honest.digest();
    // Declaring a mutable shared input fails the authenticator's validity
    // check.
    let failing = VerifiedTransaction::new_unchecked(TransactionEnvelope::new(
        iota_sdk_types::SenderSignedTransaction::new(
            honest.data().transaction().clone(),
            vec![UserSignature::MoveAuthenticator(MoveAuthenticator::from(
                MoveAuthenticatorV1::new(
                    vec![CallArg::Shared(SharedObjectReference::new(
                        ObjectId::random(),
                        Version::from(1),
                        true,
                    ))],
                    vec![],
                    SharedObjectReference::new(sender.into(), Version::from(1), false),
                ),
            ))],
        ),
    ));
    assert_eq!(*failing.digest(), digest);
    let copies = || {
        vec![
            make_user_tx_v1_verified(failing.clone()),
            make_user_tx_v1_verified(honest.clone()),
            make_user_tx_v1_verified(honest.clone()),
        ]
    };
    let retained_signatures = |transactions: &[VerifiedSequencedConsensusTransaction]| {
        transactions
            .iter()
            .map(|tx| match &tx.0.transaction {
                crate::consensus_handler::SequencedConsensusTransactionKind::External(ext) => ext
                    .kind
                    .as_user_transaction()
                    .unwrap()
                    .data()
                    .signatures()
                    .to_vec(),
                crate::consensus_handler::SequencedConsensusTransactionKind::System(_) => {
                    unreachable!("only user transactions are validated here")
                }
            })
            .collect::<Vec<_>>()
    };

    // The first run, then a replay from the saved decisions.
    for _ in 0..2 {
        let mut transactions = copies();
        let (dropped, _, digests) = post_consensus_validation::validate_and_resolve_conflicts(
            &s.authority,
            &s.epoch_store,
            1,
            &Default::default(),
            &mut transactions,
        )
        .await
        .unwrap();
        assert!(dropped.is_empty(), "{dropped:?}");
        assert_eq!(digests, vec![digest]);
        assert_eq!(
            retained_signatures(&transactions),
            vec![honest.data().signatures().to_vec()]
        );
        assert_eq!(
            s.epoch_store.post_consensus_decision(1, digest).unwrap(),
            Some(SavedPostConsensusDecision::kept())
        );
        assert_eq!(
            s.epoch_store
                .tables()
                .unwrap()
                .post_consensus_decision_candidates
                .get(&1)
                .unwrap(),
            Some(vec![digest, digest])
        );
    }
    // On the first run the honest copy was read ahead although a copy of its
    // digest came before it. The replay read it in the loop.
    assert_eq!(input_reads(&s.authority, "ahead"), 1);
    assert_eq!(input_reads(&s.authority, "inline"), 1);
    assert_eq!(input_reads(&s.authority, "unused"), 0);

    let error: IotaError = UserInputError::Unsupported("saved drop".to_string()).into();
    s.epoch_store
        .persist_post_consensus_decisions(
            2,
            &[(
                digest,
                SavedPostConsensusDecision::dropped(PostConsensusDecision::Dropped, &error),
            )],
        )
        .unwrap();
    let mut transactions = copies();
    let (dropped, _, _) = post_consensus_validation::validate_and_resolve_conflicts(
        &s.authority,
        &s.epoch_store,
        2,
        &Default::default(),
        &mut transactions,
    )
    .await
    .unwrap();
    assert_eq!(
        dropped,
        vec![(digest, error, PostConsensusDecision::Dropped)]
    );
    assert!(transactions.is_empty());
}

/// The decisions of a commit and its candidate list read back as written,
/// before and after a restart.
#[tokio::test]
async fn post_consensus_decisions_survive_a_restart() {
    use typed_store::Map;

    let mut s = setup_handler_object_state(vec![], true).await;
    let kept = TransactionDigest::random();
    let dropped = TransactionDigest::random();
    let error: IotaError = UserInputError::ObjectNotFound {
        object_id: ObjectId::random(),
        version: None,
    }
    .into();
    let decisions = vec![
        (kept, SavedPostConsensusDecision::kept()),
        (
            dropped,
            SavedPostConsensusDecision::dropped(PostConsensusDecision::Dropped, &error),
        ),
    ];
    s.epoch_store
        .persist_post_consensus_decisions(1, &decisions)
        .unwrap();
    for reopened in [false, true] {
        if reopened {
            s.epoch_store = reopen(&s.authority, &s.epoch_store);
        }
        for (digest, decision) in &decisions {
            assert_eq!(
                s.epoch_store.post_consensus_decision(1, *digest).unwrap(),
                Some(decision.clone())
            );
        }
        assert_eq!(
            s.epoch_store
                .tables()
                .unwrap()
                .post_consensus_decision_candidates
                .get(&1)
                .unwrap(),
            Some(vec![kept, dropped])
        );
        assert_eq!(
            s.epoch_store.post_consensus_decision(2, kept).unwrap(),
            None
        );
    }
}

/// The saved form of a decision does not depend on how `IotaError` is
/// encoded: a binary of another version within the epoch reads the same
/// bytes.
#[test]
fn saved_post_consensus_decision_encoding_is_stable() {
    assert_eq!(bcs::to_bytes(&PostConsensusDecision::Kept).unwrap(), [0]);
    assert_eq!(bcs::to_bytes(&PostConsensusDecision::Dropped).unwrap(), [1]);
    assert_eq!(
        bcs::to_bytes(&PostConsensusDecision::DroppedInputNotVisibleYet).unwrap(),
        [2]
    );
    assert_eq!(
        bcs::to_bytes(&SavedPostConsensusDecision {
            decision: PostConsensusDecision::Dropped,
            error: vec![7, 8],
        })
        .unwrap(),
        [1, 2, 7, 8]
    );
}

/// A saved drop whose error this binary cannot decode is still repeated on
/// replay, with `IotaError::Unknown` in place of the error.
#[tokio::test]
async fn saved_drop_with_an_undecodable_error_is_repeated() {
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let object = ObjectId::random();
    let gas = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(object, sender),
            Object::with_id_owner_for_testing(gas, sender),
        ],
        true,
    )
    .await;
    let tx = s.build_transfer(&object, &gas, sender, &key, Address::random());
    s.epoch_store
        .persist_post_consensus_decisions(
            1,
            &[(
                *tx.digest(),
                SavedPostConsensusDecision {
                    decision: PostConsensusDecision::Dropped,
                    error: vec![0xff; 4],
                },
            )],
        )
        .unwrap();
    let result = s.validate_at(1, std::slice::from_ref(&tx)).await;
    assert!(result.kept.is_empty());
    assert_eq!(result.dropped.len(), 1);
    assert_eq!(result.dropped[0].0, *tx.digest());
    assert!(
        matches!(result.dropped[0].1, IotaError::Unknown(_)),
        "{:?}",
        result.dropped[0].1
    );
}

/// A commit's saved decisions are deleted in the batch that flushes its
/// output, together with its processed flags and the resume point, so they
/// stay as long as the commit can be replayed, and the highest saved commit
/// is the last validated commit above the resume point.
#[tokio::test]
async fn decisions_retire_with_resume_progress_and_processed_state() {
    use typed_store::Map;

    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let object = ObjectId::random();
    let gas = ObjectId::random();
    let mut s = setup_handler_object_state(
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
        &Default::default(),
        &mut transactions,
    )
    .await
    .unwrap();
    assert_eq!(s.epoch_store.last_validated_commit().unwrap(), Some(1));
    // Commit 3 is validated and not flushed: its decisions outlive the batch.
    s.epoch_store
        .persist_post_consensus_decisions(
            3,
            &[(
                TransactionDigest::random(),
                SavedPostConsensusDecision::kept(),
            )],
        )
        .unwrap();
    assert_eq!(s.epoch_store.last_validated_commit().unwrap(), Some(3));
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
            .post_consensus_decision(1, *tx.digest())
            .unwrap()
            .is_some()
    );
    assert!(!s.epoch_store.is_consensus_message_processed(&key).unwrap());
    batch.write().unwrap();
    let reopened = reopen(&s.authority, &s.epoch_store);
    assert!(
        reopened
            .post_consensus_decision(1, *tx.digest())
            .unwrap()
            .is_none()
    );
    assert_eq!(reopened.last_validated_commit().unwrap(), Some(3));
    assert!(reopened.is_consensus_message_processed(&key).unwrap());
    assert_eq!(
        *reopened
            .subscribe_highest_fully_executed_commit()
            .unwrap()
            .borrow(),
        1
    );

    // A later copy of the same transaction is dropped as processed before
    // validation, so it is no candidate at commit 2.
    s.epoch_store = reopened;
    s.initialize_randomness().await;
    assert!(s.process_at(2, std::slice::from_ref(&tx)).await.is_empty());
    assert_eq!(
        s.epoch_store
            .post_consensus_decision(2, *tx.digest())
            .unwrap(),
        None
    );
    assert_eq!(
        s.epoch_store
            .tables()
            .unwrap()
            .post_consensus_decision_candidates
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
    let mut s = setup_handler_object_state_with_config_guard(
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
            .post_consensus_decision(1, *deferred.digest())
            .unwrap(),
        Some(SavedPostConsensusDecision::kept())
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
            .post_consensus_decision(2, *deferred.digest())
            .unwrap(),
        Some(SavedPostConsensusDecision::kept())
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
        .record_commit_fully_executed(
            1,
            &handler_processed_upserts(first_effects, 1),
            Default::default(),
        )
        .unwrap();
    assert!(
        s.process_at(3, std::slice::from_ref(&deferred))
            .await
            .is_empty()
    );
    assert_eq!(
        s.epoch_store
            .post_consensus_decision(3, *deferred.digest())
            .unwrap(),
        None
    );
}

/// A transaction kept at commit 1 and deferred by congestion control is kept
/// at commit 2 although a deny rule activated in between denies its sender:
/// it is not validated again and keeps the locks it took at commit 1. The
/// replay of both commits after a restart repeats these decisions, and once
/// the transaction executes, its sender can spend the new versions of its
/// inputs.
#[tokio::test]
async fn deferred_transaction_is_kept_without_being_validated_again() {
    let guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
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
    let mut s = setup_handler_object_state_with_config_guard(
        ids.iter()
            .map(|id| Object::with_id_owner_for_testing(*id, sender))
            .collect(),
        Some(guard),
    )
    .await;
    // Transfers to the sender itself, so it still owns both inputs of the
    // deferred transaction after the transaction executes.
    let transactions = vec![
        s.build_transfer(&ids[0], &ids[1], sender, &key, sender),
        s.build_transfer(&ids[2], &ids[3], sender, &key, sender),
    ];
    let first = s.process_at(1, &transactions).await;
    assert_eq!(first.len(), 1);
    let (kept, deferred, object_id, gas_id) = if first[0].digest() == transactions[0].digest() {
        (
            transactions[0].clone(),
            transactions[1].clone(),
            ids[2],
            ids[3],
        )
    } else {
        (
            transactions[1].clone(),
            transactions[0].clone(),
            ids[0],
            ids[1],
        )
    };
    let deferred_inputs = [s.latest_ref(&object_id), s.latest_ref(&gas_id)];
    assert_eq!(
        s.epoch_store.get_all_deferred_transactions_for_test().len(),
        1
    );
    let deny_sender = iota_sdk_types::DenyRuleSet {
        denied_addresses: [sender].into(),
        ..Default::default()
    };

    for restarted in [false, true] {
        if restarted {
            s.restart_epoch_store().await;
            let replay_first = s.process_at(1, &transactions).await;
            assert_eq!(replay_first.len(), 1);
            assert_eq!(replay_first[0].digest(), kept.digest());
        }
        activate_deny_rules(&s.epoch_store, deny_sender.clone(), 1);
        let second = s.process_at(2, &[]).await;
        assert_eq!(second.len(), 1, "restarted: {restarted}");
        assert_eq!(second[0].digest(), deferred.digest());
        assert_eq!(
            s.epoch_store
                .post_consensus_decision(2, *deferred.digest())
                .unwrap(),
            Some(SavedPostConsensusDecision::kept())
        );
        for input in &deferred_inputs {
            assert_eq!(
                s.epoch_store.get_quarantined_owned_object_lock(input),
                Some(*deferred.digest()),
                "the lock taken at commit 1 must stay with the deferred transaction"
            );
        }
    }

    activate_deny_rules(&s.epoch_store, Default::default(), 2);
    let kept_effects = s.execute(kept);
    let deferred_effects = s.execute(deferred);
    s.complete_commit(1, &[&kept_effects]);
    s.complete_commit(2, &[&deferred_effects]);
    let next = s.build_transfer(&object_id, &gas_id, sender, &key, Address::random());
    // Commit 6 is the first whose horizon covers the outputs of commit 2.
    let scheduled = s.process_at(6, std::slice::from_ref(&next)).await;
    assert_eq!(scheduled.len(), 1);
    assert_eq!(scheduled[0].digest(), next.digest());
}

/// A transaction on the shared object `S` is kept at commit 5 and deferred by
/// congestion control, since a deletion of `S` in the same commit takes the
/// only slot on `S`. It is kept at commit 9, whose horizon covers the
/// deletion at commit 5: it is not validated again and keeps the locks it
/// took at commit 5. The replay of both commits after a restart repeats these
/// decisions. Execution fails the transaction because `S` is deleted and
/// charges its gas, and its sender can then spend the new versions of its
/// owned inputs.
#[tokio::test]
async fn deferred_transaction_on_a_shared_object_deleted_meanwhile_is_kept() {
    let guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
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
    let deletion_gas = ObjectId::random();
    let gas = ObjectId::random();
    let mut s = setup_handler_object_state_with_config_guard(
        [deletion_gas, gas]
            .into_iter()
            .map(|id| Object::with_id_owner_for_testing(id, sender))
            .collect(),
        Some(guard),
    )
    .await;

    let setup_effects = s.execute_with_assigned_commit(
        vec![
            s.build_move_call(
                "object_basics",
                "share",
                vec![],
                &deletion_gas,
                sender,
                &key,
            ),
            s.build_move_call(
                "object_basics",
                "create",
                create_object_args(sender),
                &gas,
                sender,
                &key,
            ),
        ],
        1,
    );
    s.complete_commit(1, &setup_effects.iter().collect::<Vec<_>>());
    s.epoch_store
        .persist_checkpoint_bookkeeping(&setup_effects)
        .unwrap();
    let shared_id = *setup_effects[0].created()[0].reference().object_id();
    let object_id = *setup_effects[1].created()[0].reference().object_id();

    // The higher gas price orders the deletion first in commit 5.
    let deletion = {
        let gas_price = 2 * s.rgp;
        let tx = Transaction::new_move_call(
            sender,
            s.package_id,
            Identifier::from_static("object_basics"),
            Identifier::from_static("delete"),
            vec![],
            s.latest_ref(&deletion_gas),
            vec![s.shared_arg(&shared_id)],
            TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS * gas_price,
            gas_price,
        )
        .unwrap();
        s.epoch_store
            .verify_transaction(to_sender_signed_transaction(tx, &key))
            .unwrap()
    };
    let deferred = s.build_move_call(
        "object_basics",
        "update",
        vec![
            s.shared_arg(&shared_id),
            CallArg::ImmutableOrOwned(s.latest_ref(&object_id)),
        ],
        &gas,
        sender,
        &key,
    );
    let deferred_inputs = [s.latest_ref(&object_id), s.latest_ref(&gas)];
    let candidates = [deletion.clone(), deferred.clone()];

    let assert_kept_with_its_locks =
        |s: &HandlerObjectStateSetup, index: CommitIndex, restarted: bool| {
            assert_eq!(
                s.epoch_store
                    .post_consensus_decision(index, *deferred.digest())
                    .unwrap(),
                Some(SavedPostConsensusDecision::kept()),
                "commit {index}, restarted: {restarted}"
            );
            for input in &deferred_inputs {
                assert_eq!(
                    s.epoch_store.get_quarantined_owned_object_lock(input),
                    Some(*deferred.digest()),
                    "commit {index}, restarted: {restarted}"
                );
            }
        };
    let mut deletion_effects = None;
    let mut reload_versions = None;
    for restarted in [false, true] {
        if restarted {
            s.restart_epoch_store().await;
            s.complete_empty_commit(1);
        }
        let (first, versions) = s.process_at_with_versions(5, &candidates).await;
        assert_eq!(
            first.iter().map(|tx| *tx.digest()).collect::<Vec<_>>(),
            vec![*deletion.digest()],
            "restarted: {restarted}"
        );
        assert_eq!(
            s.epoch_store.get_all_deferred_transactions_for_test().len(),
            1
        );
        assert_kept_with_its_locks(&s, 5, restarted);

        let deletion_effects = deletion_effects.get_or_insert_with(|| {
            let effects = s.execute_with_handler_versions(deletion.clone(), versions);
            assert!(effects.status().is_success(), "{:?}", effects.status());
            s.epoch_store
                .persist_checkpoint_bookkeeping([&effects])
                .unwrap();
            effects
        });
        s.complete_commit(5, &[deletion_effects]);

        // Commit 9 is the first whose horizon covers the deletion at commit 5.
        let (second, versions) = s.process_at_with_versions(9, &[]).await;
        assert_eq!(
            second.iter().map(|tx| *tx.digest()).collect::<Vec<_>>(),
            vec![*deferred.digest()],
            "restarted: {restarted}"
        );
        assert_kept_with_its_locks(&s, 9, restarted);
        reload_versions = Some(versions);
    }

    let gas_before = s.latest_ref(&gas);
    let deferred_effects = s.execute_with_handler_versions(deferred, reload_versions.unwrap());
    assert_eq!(
        deferred_effects.status(),
        &ExecutionStatus::Failure {
            error: ExecutionErrorKind::InputObjectDeleted,
            command: None,
        }
    );
    assert_ne!(
        deferred_effects.gas_cost_summary(),
        &GasCostSummary::default()
    );
    assert!(s.latest_ref(&gas).version > gas_before.version);
    s.complete_commit(9, &[&deferred_effects]);

    let next = s.build_transfer(&object_id, &gas, sender, &key, Address::random());
    // Commit 13 is the first whose horizon covers the outputs of commit 9.
    let scheduled = s.process_at(13, std::slice::from_ref(&next)).await;
    assert_eq!(
        scheduled.iter().map(|tx| *tx.digest()).collect::<Vec<_>>(),
        vec![*next.digest()]
    );
}

/// Under P-COOL, a transaction among the previously deferred ones is kept
/// without checks and takes no locks, while the same transaction sequenced anew
/// is validated and dropped.
#[tokio::test]
async fn previously_deferred_transaction_is_kept_without_checks() {
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let object = ObjectId::random();
    let gas = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(object, sender),
            Object::with_id_owner_for_testing(gas, sender),
        ],
        true,
    )
    .await;
    let tx = s.build_transfer(&object, &gas, sender, &key, Address::random());
    let digest = *tx.digest();
    activate_deny_rules(
        &s.epoch_store,
        iota_sdk_types::DenyRuleSet {
            denied_addresses: [sender].into(),
            ..Default::default()
        },
        1,
    );
    let previously_deferred = std::collections::HashMap::from([(
        digest,
        (
            crate::authority::transaction_deferral::DeferralKey::new_for_consensus_round(2, 1),
            None,
        ),
    )]);

    let mut transactions = vec![make_user_tx_v1_verified(tx.clone())];
    let (dropped, locks, all_digests) = post_consensus_validation::validate_and_resolve_conflicts(
        &s.authority,
        &s.epoch_store,
        2,
        &previously_deferred,
        &mut transactions,
    )
    .await
    .unwrap();
    assert!(dropped.is_empty());
    assert_eq!(transactions.len(), 1);
    assert!(locks.is_empty());
    assert_eq!(all_digests, vec![digest]);
    assert_eq!(
        s.epoch_store.post_consensus_decision(2, digest).unwrap(),
        Some(SavedPostConsensusDecision::kept())
    );

    let mut transactions = vec![make_user_tx_v1_verified(tx)];
    let (dropped, _, _) = post_consensus_validation::validate_and_resolve_conflicts(
        &s.authority,
        &s.epoch_store,
        3,
        &Default::default(),
        &mut transactions,
    )
    .await
    .unwrap();
    assert!(transactions.is_empty());
    assert_eq!(dropped.len(), 1);
    assert!(
        matches!(
            &dropped[0].1,
            IotaError::UserInput {
                error: UserInputError::TransactionDenied { .. }
            }
        ),
        "{:?}",
        dropped[0].1
    );
}

/// The protocol config override under which congestion control schedules one
/// transaction per commit and defers the others.
fn one_transaction_per_commit() -> OverrideGuard {
    ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_per_object_congestion_control_mode_for_testing(
            iota_protocol_config::PerObjectCongestionControlMode::TotalTxCount,
        );
        config.set_max_accumulated_txn_cost_per_object_in_mysticeti_commit_for_testing(1);
        config.set_max_congestion_limit_overshoot_per_commit_for_testing(0);
        config.set_max_concurrent_execution_workers_for_testing(1);
        config.set_separate_gas_price_feedback_mechanism_for_randomness_for_testing(false);
        config.set_max_deferral_rounds_for_congestion_control_for_testing(10);
        config
    })
}

/// A transaction kept at commit 1 and deferred by congestion control holds
/// the locks it took there in the lock table once commit 1 flushes. A
/// competitor naming the same inputs at commit 2 is dropped on them, while the
/// deferred transaction is reloaded and kept.
#[tokio::test]
async fn deferred_transaction_lock_blocks_a_competitor_after_its_commit_flushes() {
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let ids: Vec<_> = (0..4).map(|_| ObjectId::random()).collect();
    let s = setup_handler_object_state_with_config_guard(
        ids.iter()
            .map(|id| Object::with_id_owner_for_testing(*id, sender))
            .collect(),
        Some(one_transaction_per_commit()),
    )
    .await;
    let transactions = vec![
        s.build_transfer(&ids[0], &ids[1], sender, &key, Address::random()),
        s.build_transfer(&ids[2], &ids[3], sender, &key, Address::random()),
    ];
    let first = s.process_at(1, &transactions).await;
    assert_eq!(first.len(), 1);
    let (kept, deferred, object_id, gas_id) = if first[0].digest() == transactions[0].digest() {
        (
            transactions[0].clone(),
            transactions[1].clone(),
            ids[2],
            ids[3],
        )
    } else {
        (
            transactions[1].clone(),
            transactions[0].clone(),
            ids[0],
            ids[1],
        )
    };
    let deferred_digest = *deferred.digest();
    let deferred_inputs = [s.latest_ref(&object_id), s.latest_ref(&gas_id)];

    let kept_effects = s.execute(kept);
    let height = s
        .checkpoint_roots()
        .into_iter()
        .map(|(height, _)| height)
        .max()
        .expect("commit 1 has a pending checkpoint");
    s.epoch_store
        .flush_pushed_commits_for_testing(height, 1, vec![*kept_effects.transaction_digest()])
        .unwrap();
    let tables = s.epoch_store.tables().unwrap();
    for input in &deferred_inputs {
        assert_eq!(s.epoch_store.get_quarantined_owned_object_lock(input), None);
        assert_eq!(
            tables.get_locked_transaction(input).unwrap(),
            Some(deferred_digest),
            "the flush must move the deferred transaction's lock to the lock table"
        );
    }

    let competitor = s.build_transfer(&object_id, &gas_id, sender, &key, Address::random());
    let second = s.process_at(2, std::slice::from_ref(&competitor)).await;
    assert_eq!(
        second.iter().map(|tx| *tx.digest()).collect::<Vec<_>>(),
        vec![deferred_digest]
    );
    let verdict = s
        .epoch_store
        .post_consensus_decision(2, *competitor.digest())
        .unwrap()
        .map(|saved| (saved.decision, saved.error()));
    assert!(
        matches!(
            &verdict,
            Some((PostConsensusDecision::Dropped, IotaError::ObjectLockConflict {
                obj_ref,
                pending_transaction,
            })) if deferred_inputs.contains(obj_ref) && *pending_transaction == deferred_digest
        ),
        "{verdict:?}"
    );
}

/// An account's transaction that pays its own gas, kept at commit 1 behind a
/// transfer with a higher gas price and deferred by congestion control, is
/// reloaded and kept at commit 2 and executes through the scheduler. The
/// account never authorized it, so it fails without a charge and its gas coin
/// moves to its next version, and the account's next transaction naming that
/// version is kept once the horizon reaches commit 2.
#[tokio::test]
async fn deferred_unauthorized_account_transaction_moves_its_gas_coin_at_execution() {
    let (other, other_key): (Address, AccountPrivateKey) = get_key_pair();
    let other_ids = [ObjectId::random(), ObjectId::random()];
    let (s, account_id, gas_id) = setup_unauthorized_gas_owner(
        Some(one_transaction_per_commit()),
        other_ids
            .iter()
            .map(|id| Object::with_id_owner_for_testing(*id, other))
            .collect(),
    )
    .await;
    let _watcher = s.start_execution_watcher();
    let transfer = s
        .epoch_store
        .verify_transaction(make_transfer_object_transaction(
            s.latest_ref(&other_ids[0]),
            s.latest_ref(&other_ids[1]),
            other,
            &other_key,
            Address::random(),
            2 * s.rgp,
        ))
        .unwrap();
    let unauthorized =
        s.build_self_paid_account_call(account_authenticator(account_id), &gas_id, 1);
    let unauthorized_digest = *unauthorized.digest();

    let (first, assigned_versions) = s
        .process_at_with_versions(1, &[transfer.clone(), unauthorized])
        .await;
    assert_eq!(
        first.iter().map(|tx| *tx.digest()).collect::<Vec<_>>(),
        vec![*transfer.digest()]
    );
    assert_eq!(
        s.epoch_store
            .post_consensus_decision(1, unauthorized_digest)
            .unwrap(),
        Some(SavedPostConsensusDecision::kept())
    );
    let transfer_effects = s
        .schedule_kept_transaction(first[0].clone(), assigned_versions)
        .await
        .expect("the transfer must execute");
    assert!(
        transfer_effects.status().is_success(),
        "{:?}",
        transfer_effects.status()
    );
    s.wait_for_fully_executed_commit(1).await;

    let (second, assigned_versions) = s.process_at_with_versions(2, &[]).await;
    assert_eq!(
        second.iter().map(|tx| *tx.digest()).collect::<Vec<_>>(),
        vec![unauthorized_digest]
    );
    assert_eq!(
        s.epoch_store
            .post_consensus_decision(2, unauthorized_digest)
            .unwrap(),
        Some(SavedPostConsensusDecision::kept())
    );
    let effects = s
        .schedule_kept_transaction(second[0].clone(), assigned_versions)
        .await
        .expect("the deferred transaction must execute");
    assert!(effects.status().is_failure(), "{:?}", effects.status());
    assert_eq!(
        effects.gas_cost_summary(),
        &GasCostSummary::default(),
        "nobody may be charged"
    );
    assert_eq!(
        s.latest_ref(&gas_id).version,
        effects.lamport_version(),
        "the gas coin must move to its next version"
    );
    s.wait_for_fully_executed_commit(2).await;

    let next = s.build_self_paid_account_call(account_authenticator(account_id), &gas_id, 2);
    let scheduled = s
        .process_at(2 + s.horizon_distance(), std::slice::from_ref(&next))
        .await;
    assert_eq!(
        scheduled.iter().map(|tx| *tx.digest()).collect::<Vec<_>>(),
        vec![*next.digest()]
    );
}

/// A receive of the child of a shared parent, kept at commit `C` behind a
/// transaction that deletes, wraps or receives the same child at `C` and
/// deferred by congestion control, is reloaded and kept at `C + K`, whose
/// horizon covers the consumption, without being validated again. It executes
/// through the scheduler and aborts with the sender charged.
#[tokio::test]
async fn receive_deferred_behind_a_consumption_of_its_child_is_kept_and_aborts() {
    for consume in ["deleter", "wrapper", "receiver"] {
        let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
        let gas_id = ObjectId::random();
        let consumer_gas_id = ObjectId::random();
        let s = setup_handler_object_state_with_package_and_config_guard(
            "tto_shared",
            sender,
            gas_id,
            Some(one_transaction_per_commit()),
        )
        .await;
        s.authority
            .insert_genesis_object(Object::with_id_owner_for_testing(consumer_gas_id, sender));
        let _watcher = s.start_execution_watcher();

        let start_effects =
            s.assigned_commit_move_call("M", "start", vec![], &gas_id, sender, &sender_key, 1);
        s.wait_for_fully_executed_commit(1).await;
        let (parent, child) = parent_and_child(start_effects.created());
        let parent_id = parent.reference().object_id;
        let child = *child.reference();

        // The higher gas price orders the consumption first.
        let consumption = {
            let gas_price = 2 * s.rgp;
            let tx = Transaction::new_move_call(
                sender,
                s.package_id,
                Identifier::from_static("M"),
                Identifier::from_static(consume),
                vec![],
                s.latest_ref(&consumer_gas_id),
                vec![s.shared_arg(&parent_id), CallArg::Receiving(child)],
                TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS * gas_price,
                gas_price,
            )
            .unwrap();
            s.epoch_store
                .verify_transaction(to_sender_signed_transaction(tx, &sender_key))
                .unwrap()
        };
        let receive = s.build_move_call(
            "M",
            "receiver",
            vec![s.shared_arg(&parent_id), CallArg::Receiving(child)],
            &gas_id,
            sender,
            &sender_key,
        );
        let receive_digest = *receive.digest();

        let commit = 1 + s.horizon_distance();
        let (first, assigned_versions) = s
            .process_at_with_versions(commit, &[consumption.clone(), receive])
            .await;
        assert_eq!(
            first.iter().map(|tx| *tx.digest()).collect::<Vec<_>>(),
            vec![*consumption.digest()],
            "{consume}"
        );
        assert_eq!(
            s.epoch_store
                .post_consensus_decision(commit, receive_digest)
                .unwrap(),
            Some(SavedPostConsensusDecision::kept()),
            "{consume}"
        );
        let consumption_effects = s
            .schedule_kept_transaction(first[0].clone(), assigned_versions)
            .await
            .expect("the consumption must execute");
        assert!(
            consumption_effects.status().is_success(),
            "{consume}: {:?}",
            consumption_effects.status()
        );
        s.wait_for_fully_executed_commit(commit).await;

        let reload_at = commit + s.horizon_distance();
        let (second, assigned_versions) = s.process_at_with_versions(reload_at, &[]).await;
        assert_eq!(
            second.iter().map(|tx| *tx.digest()).collect::<Vec<_>>(),
            vec![receive_digest],
            "{consume}"
        );
        assert_eq!(
            s.epoch_store
                .post_consensus_decision(reload_at, receive_digest)
                .unwrap(),
            Some(SavedPostConsensusDecision::kept()),
            "{consume}"
        );
        let effects = s
            .schedule_kept_transaction(second[0].clone(), assigned_versions)
            .await;
        assert_receive_aborted(&s, effects);
        s.wait_for_fully_executed_commit(reload_at).await;
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
    let normal = setup_handler_object_state(genesis.clone(), true).await;
    let mut restarted = setup_handler_object_state(genesis, true).await;
    let producer = normal.build_transfer(&object, &gas, sender, &key, sender);
    for node in [&normal, &restarted] {
        node.complete_empty_commit(10);
        let effects = node
            .execute_with_assigned_commit(vec![producer.clone()], 13)
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
                .post_consensus_decision(12, *consumer.digest())
                .unwrap()
                .is_none()
        );
    }
    normal.complete_empty_commit(12);
    let retry = normal
        .validate_at(16, std::slice::from_ref(&consumer))
        .await;
    assert_eq!(retry.dropped.len(), 1);
    normal
        .epoch_store
        .record_commit_fully_executed(13, &[], Default::default())
        .unwrap();
    let accepted = normal
        .validate_at(17, std::slice::from_ref(&consumer))
        .await;
    assert_eq!(accepted.kept, vec![*consumer.digest()]);
    // Only one epoch DB can be open while a store reopens.
    normal.epoch_store.release_db_handles();
    restarted.epoch_store = reopen(&restarted.authority, &restarted.epoch_store);
    assert_eq!(
        retry,
        restarted
            .validate_at(16, std::slice::from_ref(&consumer))
            .await
    );
    restarted.complete_empty_commit(13);
    assert_eq!(
        accepted,
        restarted
            .validate_at(17, std::slice::from_ref(&consumer))
            .await
    );
}

/// Commit 1 keeps a transfer and drops a double spend of the same object.
/// The transfer executes and its outputs and rows become durable,
/// and the node crashes before commit 1 flushes. The replay of commit 1
/// schedules the same transaction and leaves the same decisions, although
/// this validator's store has already moved past the version both name.
#[tokio::test]
async fn replay_of_an_executed_durable_unflushed_commit_repeats_its_decisions() {
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let object = ObjectId::random();
    let gas = ObjectId::random();
    let contender_gas = ObjectId::random();
    let mut s = setup_handler_object_state(
        [object, gas, contender_gas]
            .into_iter()
            .map(|id| Object::with_id_owner_for_testing(id, sender))
            .collect(),
        true,
    )
    .await;
    let kept = s.build_transfer(&object, &gas, sender, &key, Address::random());
    let contender = s.build_transfer(&object, &contender_gas, sender, &key, Address::random());
    let candidates = [kept.clone(), contender.clone()];

    let scheduled = s.process_at(1, &candidates).await;
    assert_eq!(
        scheduled.iter().map(|tx| *tx.digest()).collect::<Vec<_>>(),
        vec![*kept.digest()]
    );
    let saved = |s: &HandlerObjectStateSetup| {
        candidates
            .iter()
            .map(|tx| {
                s.epoch_store
                    .post_consensus_decision(1, *tx.digest())
                    .unwrap()
                    .map(|saved| saved.decision)
            })
            .collect::<Vec<_>>()
    };
    let decisions = saved(&s);
    assert_eq!(decisions[0], Some(PostConsensusDecision::Kept));
    assert_eq!(decisions[1], Some(PostConsensusDecision::Dropped));

    let effects = s.execute(kept.clone());
    let epoch = s.epoch_store.epoch();
    let cache_commit = s.authority.get_cache_commit();
    let batch = cache_commit.build_db_batch(epoch, 1, &[*kept.digest()]);
    s.epoch_store
        .persist_checkpoint_bookkeeping([&effects])
        .unwrap();
    cache_commit.commit_transaction_outputs(epoch, batch, &[*kept.digest()]);

    s.restart_epoch_store().await;
    let replayed = s.process_at(1, &candidates).await;
    assert_eq!(
        replayed.iter().map(|tx| *tx.digest()).collect::<Vec<_>>(),
        vec![*kept.digest()]
    );
    assert_eq!(saved(&s), decisions);
}

/// Commit 1 keeps a transfer, which executes. The version it consumed then
/// loses its bytes, and the node crashes before commit 1 flushes. The
/// replay of commit 1 keeps the transfer again from its saved decision,
/// without the consumed version's bytes, which a fresh read would need.
#[tokio::test]
async fn replay_after_the_consumed_version_is_pruned_repeats_the_saved_keep() {
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let object = ObjectId::random();
    let gas = ObjectId::random();
    let mut s = setup_handler_object_state(
        [object, gas]
            .into_iter()
            .map(|id| Object::with_id_owner_for_testing(id, sender))
            .collect(),
        true,
    )
    .await;
    let consumed = s.latest_ref(&object);
    let tx = s.build_transfer(&object, &gas, sender, &key, Address::random());
    assert_eq!(s.process_at(1, std::slice::from_ref(&tx)).await.len(), 1);
    let effects = s.execute(tx.clone());
    s.epoch_store
        .persist_checkpoint_bookkeeping([&effects])
        .unwrap();
    s.prune_versions_consumed_by(&[*tx.digest()], &effects)
        .await;
    assert!(
        s.store_object(&consumed.object_id, consumed.version)
            .is_none()
    );

    s.restart_epoch_store().await;
    let replayed = s.process_at(1, std::slice::from_ref(&tx)).await;
    assert_eq!(replayed.len(), 1);
    assert_eq!(replayed[0].digest(), tx.digest());
    assert_eq!(
        s.epoch_store
            .post_consensus_decision(1, *tx.digest())
            .unwrap()
            .map(|saved| saved.decision),
        Some(PostConsensusDecision::Kept)
    );
}

/// A transfer naming a version produced at commit 13 is dropped at commit 12,
/// where that version is above the horizon, and its retry is kept at commit
/// 17. After a crash before either commit flushes, both replay with their
/// saved decisions: the drop again, and the retry kept.
#[tokio::test]
async fn missing_drop_and_its_kept_retry_replay_after_a_restart() {
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let object = ObjectId::random();
    let gas = ObjectId::random();
    let consumer_gas = ObjectId::random();
    let mut s = setup_handler_object_state(
        [object, gas, consumer_gas]
            .into_iter()
            .map(|id| Object::with_id_owner_for_testing(id, sender))
            .collect(),
        true,
    )
    .await;
    let producer = s.build_transfer(&object, &gas, sender, &key, sender);
    let producer_key = TransactionKey::Digest(*producer.digest());
    s.complete_empty_commit(10);
    let produced = s.execute_with_assigned_commit(vec![producer], 13).remove(0);
    s.epoch_store
        .persist_checkpoint_bookkeeping([&produced])
        .unwrap();
    let consumer = s.build_transfer(&object, &consumer_gas, sender, &key, Address::random());

    /// Processes commit 12 with the consumer, completes commits 12 and 13,
    /// processes commit 17 with the consumer again, and returns the two saved
    /// decisions.
    async fn run(
        s: &HandlerObjectStateSetup,
        consumer: &VerifiedTransaction,
        produced: &TransactionEffects,
    ) -> (Option<PostConsensusDecision>, Option<PostConsensusDecision>) {
        assert!(
            s.process_at(12, std::slice::from_ref(consumer))
                .await
                .is_empty()
        );
        s.complete_commit(12, &[]);
        s.complete_commit(13, &[produced]);
        let retried = s.process_at(17, std::slice::from_ref(consumer)).await;
        assert_eq!(retried.len(), 1);
        assert_eq!(retried[0].digest(), consumer.digest());
        (
            s.epoch_store
                .post_consensus_decision(12, *consumer.digest())
                .unwrap()
                .map(|saved| saved.decision),
            s.epoch_store
                .post_consensus_decision(17, *consumer.digest())
                .unwrap()
                .map(|saved| saved.decision),
        )
    }
    let (dropped, kept) = run(&s, &consumer, &produced).await;
    assert_eq!(
        dropped,
        Some(PostConsensusDecision::DroppedInputNotVisibleYet)
    );
    assert_eq!(kept, Some(PostConsensusDecision::Kept));

    s.restart_epoch_store().await;
    s.complete_empty_commit(10);
    s.epoch_store
        .assign_commit_to_transactions(13, vec![producer_key]);
    assert_eq!(run(&s, &consumer, &produced).await, (dropped, kept));
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
    let s = setup_handler_object_state(
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
        .post_consensus_decisions
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
    let mut s = setup_handler_object_state(
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
            &Default::default(),
            &mut vec![make_user_tx_v1_verified(tx)],
        )
        .await
    });
    assert!(task.await.unwrap_err().is_panic());
    assert_eq!(
        s.epoch_store.post_consensus_decision(2, digest).unwrap(),
        Some(SavedPostConsensusDecision::kept())
    );
}

/// Saves the switch to synced checkpoints after commit `last`, then executes
/// `deletion` as a synced checkpoint does, without a commit assigned by the
/// handler, checks that it leaves no row, and restarts the epoch
/// store.
async fn switch_execute_synced_and_restart(
    s: &mut HandlerObjectStateSetup,
    last: CommitIndex,
    deletion: VerifiedTransaction,
) {
    s.epoch_store.record_handled_commit(last);
    s.epoch_store.start_executing_synced_checkpoints().await;
    let effects = s.execute_with_assigned_shared_versions(deletion);
    let written = handler_processed_upserts(&effects, last);
    assert!(!written.is_empty(), "{effects:?}");
    for (ObjectKey(id, version), _) in written {
        s.assert_no_handler_processed_object(&id, version);
    }
    s.epoch_store = reopen(&s.authority, &s.epoch_store);
    assert_eq!(
        s.epoch_store
            .last_commit_before_synced_checkpoint_execution()
            .unwrap(),
        Some(last)
    );
}

/// The replay keeps the same transactions without reading their inputs, so
/// it registers no locks for them.
#[track_caller]
fn assert_repeats_the_saved_keep(first: &ValidationResult, replayed: ValidationResult) {
    assert_eq!(replayed.kept, first.kept);
    assert_eq!(replayed.dropped, first.dropped);
    assert!(replayed.locks.is_empty(), "{:?}", replayed.locks);
}

/// Commit 7 keeps a transaction on a shared object created at commit 3, and
/// the transaction is deferred. The node switches to synced checkpoints after
/// commit 7, a synced checkpoint deletes the object, and the node restarts.
/// The replay of commit 7 keeps the transaction from its saved decision,
/// without reading the object, whose deletion has no row.
#[tokio::test]
async fn replay_up_to_the_switch_repeats_a_saved_keep_without_reading_inputs() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let deleter_gas = ObjectId::random();
    let mut s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(gas_id, sender),
            Object::with_id_owner_for_testing(deleter_gas, sender),
        ],
        true,
    )
    .await;
    let share_effects =
        s.assigned_commit_object_basics_call("share", vec![], &gas_id, sender, &sender_key, 3);
    let shared_id = *share_effects.created()[0].reference().object_id();
    let kept = s.build_move_call(
        "object_basics",
        "set_value",
        vec![
            s.shared_arg(&shared_id),
            CallArg::Pure(bcs::to_bytes(&1u64).unwrap()),
        ],
        &gas_id,
        sender,
        &sender_key,
    );
    let result = s.validate_at(7, std::slice::from_ref(&kept)).await;
    assert_eq!(result.kept, vec![*kept.digest()]);

    let deletion = s.build_move_call(
        "object_basics",
        "delete",
        vec![s.shared_arg(&shared_id)],
        &deleter_gas,
        sender,
        &sender_key,
    );
    switch_execute_synced_and_restart(&mut s, 7, deletion).await;
    assert!(s.authority.get_object(&shared_id).is_none());

    assert_repeats_the_saved_keep(&result, s.validate_at(7, std::slice::from_ref(&kept)).await);
}

/// As [`replay_up_to_the_switch_repeats_a_saved_keep_without_reading_inputs`],
/// with a shared object created before the epoch, which a fresh read would
/// find missing.
#[tokio::test]
async fn replay_up_to_the_switch_repeats_a_saved_keep_on_an_epoch_start_object() {
    let (mut s, _, coin) =
        setup_validators_with_a_shared_regulated_coin(RegulatedCoinOptions::default()).await;
    let kept = coin.destroy_shared_coin(&s, coin.other, &coin.other_key, &coin.other_gas);
    let result = s.validate_at(7, std::slice::from_ref(&kept)).await;
    assert_eq!(result.kept, vec![*kept.digest()]);

    let deletion =
        coin.destroy_shared_coin(&s, coin.publisher, &coin.publisher_key, &coin.publisher_gas);
    switch_execute_synced_and_restart(&mut s, 7, deletion).await;
    assert!(s.authority.get_object(&coin.coin_id).is_none());

    assert_repeats_the_saved_keep(&result, s.validate_at(7, std::slice::from_ref(&kept)).await);
}

/// Once the switch after commit 7 is saved, validating a later commit stops
/// the node: the consensus handler processes no commit after it.
#[tokio::test]
async fn validation_after_the_saved_switch_stops_the_node() {
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let object = ObjectId::random();
    let gas = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(object, sender),
            Object::with_id_owner_for_testing(gas, sender),
        ],
        true,
    )
    .await;
    let tx = s.build_transfer(&object, &gas, sender, &key, Address::random());
    s.epoch_store.record_handled_commit(7);
    s.epoch_store.start_executing_synced_checkpoints().await;
    let task = tokio::spawn(async move { s.validate_at(8, &[tx]).await });
    assert!(task.await.unwrap_err().is_panic());
}

/// With the flag on, the lock check runs before the reader. A version
/// consumed by a locked transaction of a commit above the horizon `C - K` is
/// kept by the reader alone, since the consumer's row is above it, so the lock
/// is what drops a contender for it, and the reader never runs. Once the
/// consumer's commit is at or below the horizon, the reader alone drops the
/// version as well.
#[tokio::test]
async fn validation_loop_at_a_commit_checks_locks_before_the_reader() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let obj_id = ObjectId::random();
    let gas1 = ObjectId::random();
    let gas2 = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(obj_id, sender),
            Object::with_id_owner_for_testing(gas1, sender),
            Object::with_id_owner_for_testing(gas2, sender),
        ],
        true,
    )
    .await;

    let first = s.build_transfer(&obj_id, &gas1, sender, &sender_key, sender);
    s.execute_with_assigned_commit(vec![first], 5);
    let x_v1 = s.latest_ref(&obj_id);

    // The winner holds its locks in the quarantine and executes at commit 10.
    let winner = s.build_transfer(&obj_id, &gas1, sender, &sender_key, sender);
    let winner_digest = *winner.digest();
    seed_quarantined_lock(&s.epoch_store, x_v1, winner_digest);
    seed_quarantined_lock(&s.epoch_store, s.latest_ref(&gas1), winner_digest);
    s.execute_with_assigned_commit(vec![winner], 10);

    // Commit 13, horizon 9: the winner's row is above the horizon.
    assert_keeps(s.read_owned(13, x_v1), x_v1);

    let contender = s
        .epoch_store
        .verify_transaction(make_transfer_object_transaction(
            x_v1,
            s.latest_ref(&gas2),
            sender,
            &sender_key,
            Address::random(),
            s.rgp,
        ))
        .unwrap();
    let contender_digest = *contender.digest();
    let mut transactions = vec![make_user_tx_v1_verified(contender)];
    let (dropped, locks, all_digests) = post_consensus_validation::validate_and_resolve_conflicts(
        &s.authority,
        &s.epoch_store,
        13,
        &Default::default(),
        &mut transactions,
    )
    .await
    .unwrap();

    assert_eq!(all_digests, vec![contender_digest]);
    assert!(transactions.is_empty());
    assert!(locks.is_empty());
    assert_eq!(dropped.len(), 1);
    assert_eq!(dropped[0].0, contender_digest);
    assert!(
        matches!(
            &dropped[0].1,
            IotaError::ObjectLockConflict { obj_ref, pending_transaction }
                if *obj_ref == x_v1 && *pending_transaction == winner_digest
        ),
        "{:?}",
        dropped[0].1
    );
    let verdicts = &s
        .authority
        .metrics
        .consensus_handler_validation_reader_verdicts;
    for labels in [
        ["keep", "none"],
        ["drop", "HandlerRowSuperseded"],
        ["missing", "HandlerRowAboveHorizon"],
    ] {
        assert_eq!(verdicts.with_label_values(&labels).get(), 0, "{labels:?}");
    }

    // Commit 14, horizon 10: the winner's row decides.
    assert_owned_drops(s.read_owned(14, x_v1), DropKind::HandlerRowSuperseded);
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

/// A row produced by commit 10 is at the horizon of commit 14 and keeps. For
/// commit 13, whose horizon is 9, the same row is above the horizon and
/// answers missing.
#[tokio::test]
async fn owned_row_at_the_horizon_keeps_and_above_it_answers_missing() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let obj_id = ObjectId::random();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(obj_id, sender),
            Object::with_id_owner_for_testing(gas_id, sender),
        ],
        true,
    )
    .await;

    let tx = s.build_transfer(&obj_id, &gas_id, sender, &sender_key, sender);
    s.execute_with_assigned_commit(vec![tx], 10);
    let produced = s.latest_ref(&obj_id);

    assert_keeps(s.read_owned(14, produced), produced);
    assert_owned_missing(
        s.read_owned(13, produced),
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
        config.set_pcool_deterministic_validation_horizon_distance_for_testing(5);
        config
    });
    let s = setup_handler_object_state_with_config_guard(
        vec![
            Object::with_id_owner_for_testing(obj_id, sender),
            Object::with_id_owner_for_testing(gas_id, sender),
        ],
        Some(config_guard),
    )
    .await;

    let tx = s.build_transfer(&obj_id, &gas_id, sender, &sender_key, sender);
    s.execute_with_assigned_commit(vec![tx], 10);
    let produced = s.latest_ref(&obj_id);

    assert_keeps(s.read_owned(15, produced), produced);
    assert_owned_missing(
        s.read_owned(14, produced),
        MissingKind::HandlerRowAboveHorizon,
    );
}

/// A receive takes no lock on the received version. The receive's row of
/// the child above that version decides: at or below the horizon the version
/// drops, whether or not this validator still holds its bytes, and the
/// loader drops a transaction naming it. Above the horizon it is kept, as on
/// a validator that has not executed the receive. This epoch's historic
/// objects bucket keeps its bytes; removed by hand from there, the load fails
/// with a storage error.
#[tokio::test]
async fn owned_received_version_drops_from_the_receive_row() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state_with_package("tto", sender, gas_id).await;
    let publish = s
        .authority
        .get_object(&s.package_id)
        .unwrap()
        .previous_transaction;

    let start_effects =
        s.assigned_commit_move_call("M1", "start", vec![], &gas_id, sender, &sender_key, 5);
    let (parent, child) = parent_and_child(start_effects.created());
    let child_ref = *child.reference();
    let receive_effects = s.assigned_commit_move_call(
        "M1",
        "receiver",
        vec![
            CallArg::ImmutableOrOwned(*parent.reference()),
            CallArg::Receiving(child_ref),
        ],
        &gas_id,
        sender,
        &sender_key,
        7,
    );

    assert_keeps(s.read_owned(10, child_ref), child_ref);
    assert_owned_drops(s.read_owned(11, child_ref), DropKind::HandlerRowSuperseded);

    let tx = s
        .epoch_store
        .verify_transaction(make_transfer_object_transaction(
            child_ref,
            s.latest_ref(&gas_id),
            sender,
            &sender_key,
            sender,
            s.rgp,
        ))
        .unwrap();
    match s.read_inputs_at_commit(11, &tx) {
        InputResolution::Drop(InputObjectKind::ImmOrOwnedMoveObject(reference), reason) => {
            assert_eq!(reference, child_ref);
            assert_eq!(reason.kind(), DropKind::HandlerRowSuperseded);
        }
        InputResolution::Drop(kind, reason) => panic!("dropped on {kind:?}: {reason:?}"),
        InputResolution::Missing(kind, reason) => panic!("missing on {kind:?}: {reason:?}"),
        InputResolution::Loaded(_) => panic!("loaded a version the receive consumed"),
    }

    s.prune_versions_consumed_by(
        &[
            publish,
            *start_effects.transaction_digest(),
            *receive_effects.transaction_digest(),
        ],
        &receive_effects,
    )
    .await;
    assert_owned_drops(s.read_owned(11, child_ref), DropKind::HandlerRowSuperseded);
    assert!(matches!(
        s.reader_at(10).read_owned(child_ref),
        Err(IotaError::Storage(_))
    ));
}

/// A dynamic-field child is consumed through its parent without a lock. The
/// tombstone row the removal writes above the child's version drops it, with
/// or without its bytes.
#[tokio::test]
async fn owned_dynamic_field_child_drops_from_the_row_of_its_removal() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let create_args = || create_object_args(sender);
    let parent_effects = s.assigned_commit_object_basics_call(
        "create",
        create_args(),
        &gas_id,
        sender,
        &sender_key,
        2,
    );
    let value_effects = s.assigned_commit_object_basics_call(
        "create",
        create_args(),
        &gas_id,
        sender,
        &sender_key,
        3,
    );
    let parent_id = *parent_effects.created()[0].reference().object_id();
    let add_effects = s.assigned_commit_object_basics_call(
        "add_field",
        vec![
            CallArg::ImmutableOrOwned(s.latest_ref(&parent_id)),
            CallArg::ImmutableOrOwned(*value_effects.created()[0].reference()),
        ],
        &gas_id,
        sender,
        &sender_key,
        5,
    );
    let field_ref = *add_effects.created()[0].reference();
    assert!(matches!(add_effects.created()[0].owner(), Owner::Object(_)));
    let remove_effects = s.assigned_commit_object_basics_call(
        "remove_field",
        vec![CallArg::ImmutableOrOwned(s.latest_ref(&parent_id))],
        &gas_id,
        sender,
        &sender_key,
        6,
    );

    assert_keeps(s.read_owned(9, field_ref), field_ref);
    assert_owned_drops(s.read_owned(10, field_ref), DropKind::HandlerRowSuperseded);

    s.prune_versions_consumed_by(
        &[
            *parent_effects.transaction_digest(),
            *value_effects.transaction_digest(),
            *add_effects.transaction_digest(),
            *remove_effects.transaction_digest(),
        ],
        &remove_effects,
    )
    .await;
    assert_owned_drops(s.read_owned(10, field_ref), DropKind::HandlerRowSuperseded);
}

/// Shared mutations write no rows, so a shared object's creation version
/// named as owned drops from the creation row itself, before and after a
/// mutation consumed it and its bytes were removed.
#[tokio::test]
async fn owned_read_of_a_shared_creation_version_drops() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;

    let share_effects =
        s.assigned_commit_object_basics_call("share", vec![], &gas_id, sender, &sender_key, 5);
    let shared_ref = *share_effects.created()[0].reference();
    assert_owned_drops(
        s.read_owned(9, shared_ref),
        DropKind::HandlerRowSharedObject,
    );

    let mutate_effects = s.assigned_commit_shared_object_basics_call(
        "set_value",
        vec![
            s.shared_arg(shared_ref.object_id()),
            CallArg::Pure(bcs::to_bytes(&1u64).unwrap()),
        ],
        &gas_id,
        sender,
        &sender_key,
        6,
    );
    s.prune_versions_consumed_by(
        &[
            *share_effects.transaction_digest(),
            *mutate_effects.transaction_digest(),
        ],
        &mutate_effects,
    )
    .await;
    assert_owned_drops(
        s.read_owned(10, shared_ref),
        DropKind::HandlerRowSharedObject,
    );
}

/// With no row for an owned input, the reader falls back to the store.
/// Objects untouched since the epoch started keep, owned or immutable. A
/// version consumed by a commit the handler has assigned, at or below the
/// horizon, drops from the consuming commit's row. An id the store does not
/// have answers missing.
#[tokio::test]
async fn owned_store_fallback_keeps_untouched_and_drops_superseded_versions() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let untouched_id = ObjectId::random();
    let immutable_id = ObjectId::random();
    let handler_consumed_id = ObjectId::random();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(untouched_id, sender),
            Object::immutable_with_id_for_testing(immutable_id),
            Object::with_id_owner_for_testing(handler_consumed_id, sender),
            Object::with_id_owner_for_testing(gas_id, sender),
        ],
        true,
    )
    .await;
    let untouched = s.latest_ref(&untouched_id);
    let immutable = s.latest_ref(&immutable_id);
    let handler_consumed = s.latest_ref(&handler_consumed_id);

    let tx = s.build_transfer(&handler_consumed_id, &gas_id, sender, &sender_key, sender);
    s.execute_with_assigned_commit(vec![tx], 5);

    assert_keeps(s.read_owned(12, untouched), untouched);
    assert_keeps(s.read_owned(12, immutable), immutable);
    assert_owned_drops(
        s.read_owned(12, handler_consumed),
        DropKind::HandlerRowSuperseded,
    );
    let unknown = ObjectReference::new(
        ObjectId::random(),
        Version::from_u64(1),
        ObjectDigest::random(),
    );
    assert_owned_missing(s.read_owned(12, unknown), MissingKind::StoreNotFound);
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
}

/// Builds the state above.
async fn setup_immutable_input() -> ImmutableInputSetup {
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

    /// Executes `tx` in a commit of its own, as when an earlier commit kept
    /// and executed the same transaction.
    fn execute_in_an_earlier_commit(&self, tx: &VerifiedTransaction) {
        let executable = VerifiedExecutableTransaction::new_from_checkpoint(
            tx.clone(),
            self.epoch_store.epoch(),
            1,
        );
        self.epoch_store
            .assign_to_a_new_commit_for_testing(&[executable.key()]);
        self.authority
            .try_execute_immediately(&executable, ExecutionEnv::new(), &self.epoch_store)
            .unwrap();
    }

    /// Saves a keep for each of `txs` at commit 1, in that order, as the first
    /// run of the commit did before a restart.
    fn save_keeps(&self, txs: &[&VerifiedTransaction]) {
        let decisions: Vec<_> = txs
            .iter()
            .map(|tx| (*tx.digest(), SavedPostConsensusDecision::kept()))
            .collect();
        self.epoch_store
            .persist_post_consensus_decisions(1, &decisions)
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
            &Default::default(),
            transactions,
        )
        .await
        .unwrap();
        (
            dropped
                .into_iter()
                .map(|(digest, error, _)| (digest, error))
                .collect(),
            locks,
        )
    }
}

/// An immutable object is read-only and cannot be double-spent, so referencing
/// it must not acquire an owned-object lock: any number of transactions in the
/// same epoch may take it as an input (issue #12602).
#[tokio::test]
async fn test_immutable_object_input_not_locked() {
    telemetry_subscribers::init_for_testing();
    let s = setup_immutable_input().await;

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

/// The same for attested transactions: their lock set comes from the loaded
/// objects as well, so an immutable input is never locked (issue #12809).
#[tokio::test]
async fn test_attested_tx_immutable_object_input_not_locked() {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_validator_attestation();
    let s = setup_immutable_input().await;
    let protocol_config = s.epoch_store.protocol_config();
    let min_units = protocol_config
        .base_tx_cost_fixed()
        .min(protocol_config.gas_rounding_step());

    let tx1 = s.read_immutable(s.gas1_ref);
    let tx2 = s.read_immutable(s.gas2_ref);
    let mut transactions = vec![
        make_attested_user_tx(tx1.into_inner(), 0, min_units),
        make_attested_user_tx(tx2.into_inner(), 0, min_units),
    ];

    let (dropped, locks) = s.resolve(&mut transactions).await;

    assert!(
        dropped.is_empty(),
        "no attested transaction may be dropped over an immutable input: {dropped:?}"
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

/// The already-executed branch locks the transaction's owned inputs and leaves
/// the immutable input free. A later reader of the same immutable object is
/// then kept instead of hitting the winner-out-locked `fatal!`.
#[tokio::test]
async fn test_already_executed_tx_does_not_lock_immutable_input() {
    telemetry_subscribers::init_for_testing();
    let s = setup_immutable_input().await;

    let executed = s.read_immutable_with_owned(s.gas1_ref);
    s.execute_in_an_earlier_commit(&executed);

    let later_reader = s.read_immutable(s.gas2_ref);
    // The already-executed branch takes only a transaction with a keep saved
    // at this commit. Every candidate of a replayed commit needs a saved
    // decision, so the reader gets one too; not executed, it is validated
    // again.
    s.save_keeps(&[&executed, &later_reader]);
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
/// the post-consensus result below is decided by where the limits come from.
async fn assert_node_limits_reject(setup: &PublishSetup) {
    let epoch_store = setup.authority.epoch_store_for_testing();
    let admission = setup
        .authority
        .handle_transaction_validation_checks(
            &VerifiedTransaction::new_unchecked(setup.tx.clone()),
            &epoch_store,
            &setup.authority.config.transaction_deny_config,
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

/// Post-consensus validation meters a published package with the protocol
/// config's limits.
/// A validator whose own `VerifierSigningConfig` would reject the package
/// keeps it, as every validator on default settings does.
#[tokio::test]
async fn post_consensus_validation_meters_packages_with_protocol_limits() {
    let setup = setup_publish_with_one_tick_node_limits().await;
    assert_node_limits_reject(&setup).await;

    let epoch_store = setup.authority.epoch_store_for_testing();
    let mut transactions = vec![make_user_tx_v1(setup.tx.clone())];
    let (dropped, locks, _) = post_consensus_validation::validate_and_resolve_conflicts(
        &setup.authority,
        &epoch_store,
        1,
        &Default::default(),
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

// ---------------------------------------------------------------------------
// P-COOL post-consensus validation waits for the horizon to be fully executed
//
// The horizon distance K is 4 here: commit C reads state as of commit C - 4
// and waits until the highest fully executed commit has reached it.
// ---------------------------------------------------------------------------

/// Runs one commit through `process_consensus_transactions_and_commit_boundary`
/// as the handler would, at `index`, with `txs` as its user transactions.
/// Owned arguments so the future can be spawned and observed while it waits.
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
        .map(|(schedulables, ..)| drop_attestations(schedulables))
        .map_err(|error| match error {
            crate::epoch::randomness::AdvanceDkgError::Iota(error) => error,
            crate::epoch::randomness::AdvanceDkgError::RuntimeShutdown => {
                panic!("runtime shutdown during a test commit")
            }
        })
}

/// The call must return promptly: a wait that blocks fails the test through
/// the paused clock's timeout.
async fn process_commit_without_waiting(s: &HandlerObjectStateSetup, index: CommitIndex) {
    tokio::time::timeout(
        Duration::from_secs(1),
        process_commit_at(s.authority.clone(), s.epoch_store.clone(), index, vec![]),
    )
    .await
    .unwrap_or_else(|_| panic!("commit {index} must not wait"))
    .unwrap();
}

#[tokio::test(start_paused = true)]
async fn validation_does_not_wait_when_the_horizon_is_already_fully_executed() {
    let s = setup_handler_object_state(vec![], true).await;
    assert_eq!(s.highest_fully_executed_commit(), 0);

    // Commit 4 has horizon 0, which is fully executed from the start.
    process_commit_without_waiting(&s, 4).await;
}

#[tokio::test(start_paused = true)]
async fn validation_waits_until_the_horizon_is_fully_executed() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let obj_id = ObjectId::random();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(obj_id, sender),
            Object::with_id_owner_for_testing(gas_id, sender),
        ],
        true,
    )
    .await;
    let _watcher = s.start_execution_watcher();

    // Commit 1 is assigned and its only transaction has not executed, so the
    // highest fully executed commit stays at 0.
    let tx = s.build_transfer(&obj_id, &gas_id, sender, &sender_key, Address::random());
    s.epoch_store
        .assign_commit_to_transactions(1, vec![TransactionKey::Digest(*tx.digest())]);

    // Commit 5 has horizon 1 and must block on it.
    let mut processing = tokio::spawn(process_commit_at(
        s.authority.clone(),
        s.epoch_store.clone(),
        5,
        vec![],
    ));
    assert!(
        tokio::time::timeout(Duration::from_millis(200), &mut processing)
            .await
            .is_err(),
        "commit 5 must wait for commit 1"
    );

    // The transaction executes, the execution watcher marks commit 1 fully
    // executed, and the wait ends.
    s.execute(tx);
    tokio::time::timeout(Duration::from_secs(10), processing)
        .await
        .expect("commit 5 must proceed once commit 1 is fully executed")
        .unwrap()
        .unwrap();
    assert!(s.highest_fully_executed_commit() >= 1);
}

#[tokio::test(start_paused = true)]
async fn validation_does_not_wait_after_the_final_round() {
    let s = setup_handler_object_state(vec![], true).await;
    assert_eq!(s.highest_fully_executed_commit(), 0);

    // After the final round no commit makes a checkpoint, so the highest fully
    // executed commit never moves again and no commit may wait on it.
    s.epoch_store
        .get_reconfig_state_write_lock_guard()
        .close_all_tx();
    process_commit_without_waiting(&s, 5).await;
}

#[tokio::test(start_paused = true)]
async fn validation_does_not_wait_with_the_flag_off() {
    let s = setup_handler_object_state(vec![], false).await;
    assert!(
        s.epoch_store
            .subscribe_highest_fully_executed_commit()
            .is_none()
    );

    process_commit_without_waiting(&s, 5).await;
}

#[tokio::test(start_paused = true)]
async fn epoch_end_releases_a_pending_wait() {
    let s = setup_handler_object_state(vec![], true).await;
    assert_eq!(s.highest_fully_executed_commit(), 0);

    // Commit 7 has horizon 3. Nothing has executed and no execution watcher
    // runs, so the wait is pending.
    let mut processing = tokio::spawn(process_commit_at(
        s.authority.clone(),
        s.epoch_store.clone(),
        7,
        vec![],
    ));
    assert!(
        tokio::time::timeout(Duration::from_millis(200), &mut processing)
            .await
            .is_err(),
        "commit 7 must wait for commit 3"
    );

    // Terminating the epoch cuts the wait short. The call returns the error,
    // and the handler stops on it instead of panicking.
    s.epoch_store.epoch_terminated().await;
    let result = tokio::time::timeout(Duration::from_secs(10), processing)
        .await
        .expect("the wait must end with the epoch")
        .unwrap();
    assert!(matches!(
        result,
        Err(IotaError::EpochEnded(epoch)) if epoch == s.epoch_store.epoch()
    ));
}

/// A [`HandlerObjectStateSetup`] with a transaction that failed at execution in commit
/// `failed_at`, and a spare gas coin its sender owns that the failure did not
/// touch.
struct FailedHoldingTransaction {
    s: HandlerObjectStateSetup,
    sender: Address,
    sender_key: AccountPrivateKey,
    holding_tx: TransactionDigest,
    /// The holding transaction's gas coin at the version its failed execution
    /// consumed.
    consumed_gas: ObjectReference,
    spare_gas_id: ObjectId,
}

async fn setup_failed_holding_transaction(failed_at: CommitIndex) -> FailedHoldingTransaction {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let spare_gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(gas_id, sender),
            Object::with_id_owner_for_testing(spare_gas_id, sender),
        ],
        true,
    )
    .await;
    let (parent_ref, _) = s.create_object(&gas_id, sender, &sender_key, failed_at - 1);
    let consumed_gas = s.latest_ref(&gas_id);
    let failing = s.build_aborting_call(parent_ref.object_id(), &gas_id, sender, &sender_key);
    let holding_tx = *failing.digest();
    s.execute_failing_with_assigned_commit(failing, failed_at);
    FailedHoldingTransaction {
        s,
        sender,
        sender_key,
        holding_tx,
        consumed_gas,
        spare_gas_id,
    }
}

/// A transaction naming a version a failed transaction consumed is dropped on
/// the failed transaction's lock, also once the horizon reaches the commit of
/// the failure.
#[tokio::test]
async fn transaction_naming_a_version_a_failed_transaction_consumed_is_dropped_on_its_lock() {
    let failed_at = 5;
    let f = setup_failed_holding_transaction(failed_at).await;
    seed_quarantined_lock(&f.s.epoch_store, f.consumed_gas, f.holding_tx);
    let contender = make_transfer_object_transaction(
        f.consumed_gas,
        f.s.latest_ref(&f.spare_gas_id),
        f.sender,
        &f.sender_key,
        Address::random(),
        f.s.rgp,
    );
    let contender = f.s.epoch_store.verify_transaction(contender).unwrap();
    let contender_digest = *contender.digest();

    let mut transactions = vec![make_user_tx_v1_verified(contender)];
    let (dropped, _, _) = post_consensus_validation::validate_and_resolve_conflicts(
        &f.s.authority,
        &f.s.epoch_store,
        failed_at + f.s.horizon_distance(),
        &Default::default(),
        &mut transactions,
    )
    .await
    .unwrap();

    assert!(transactions.is_empty());
    assert!(
        matches!(
            dropped.as_slice(),
            [(digest, IotaError::ObjectLockConflict { obj_ref, pending_transaction }, _)]
                if *digest == contender_digest
                    && *obj_ref == f.consumed_gas
                    && *pending_transaction == f.holding_tx
        ),
        "{dropped:?}"
    );
}

/// A [`HandlerObjectStateSetup`] under `config_guard`, if any, with an account
/// since the epoch started whose authenticator function does not exist, a gas
/// coin owned by the account's address, and `other_objects` at genesis.
/// Returns the setup, the account's id and the coin's id. A transaction naming
/// the account and paying with the coin fails at execution, since its
/// authenticator cannot run, so the account never authorized it.
async fn setup_unauthorized_gas_owner(
    config_guard: Option<OverrideGuard>,
    other_objects: Vec<Object>,
) -> (HandlerObjectStateSetup, ObjectId, ObjectId) {
    let account_id = ObjectId::random();
    let gas_id = ObjectId::random();
    let account = shared_object_with_id(account_id);
    let mut genesis_objects = vec![
        authenticator_field(&account, "missing_function"),
        account,
        Object::with_id_owner_for_testing(gas_id, account_id.into()),
    ];
    genesis_objects.extend(other_objects);
    let s = setup_handler_object_state_with_config_guard(genesis_objects, config_guard).await;
    (s, account_id, gas_id)
}

/// The authenticator function field that makes the shared object `account` an
/// account, naming `function` of a module that is not published. Inserted as
/// epoch-start state, it is the account's field at every horizon. Execution
/// resolves the account from it, and its authenticator fails, as the function
/// does not exist.
fn authenticator_field(account: &Object, function: &str) -> Object {
    let account_id = account.id();
    let field_id = derive_authenticator_function_ref_v1_dynamic_field_id(account_id).unwrap();
    let account_type = account.data.as_opt_struct().unwrap().struct_tag().clone();
    let value = AuthenticatorFunctionRefV1 {
        package: ObjectId::random(),
        module: "account".to_string(),
        function: function.to_string(),
    };
    let field_type = StructTag::new_dynamic_field(
        StructTag::new_authenticator_function_ref_v1_key(),
        AuthenticatorFunctionRefV1::type_(account_type),
    );
    let contents = bcs::to_bytes(&(field_id, false, value)).unwrap();
    Object::new_move(
        MoveStruct::new(field_type.into(), Version::OBJECT_START, contents).unwrap(),
        Owner::Object(account_id),
        TransactionDigest::GENESIS_MARKER,
    )
}

/// The authenticator naming the shared object `account_id` as an account.
fn account_authenticator(account_id: ObjectId) -> MoveAuthenticatorV1 {
    MoveAuthenticatorV1::new(
        vec![],
        vec![],
        SharedObjectReference::new(account_id, Version::OBJECT_START, false),
    )
}

/// An account that pays its own gas but did not authorize the transaction is
/// not charged, and its gas coin moves to its next version. Once the horizon
/// reaches the commit of the failure, the account's next transaction, naming
/// the coin at its new version, is kept, while one naming the version the
/// failure consumed is still dropped on the failed transaction's lock.
#[tokio::test]
async fn gas_coin_of_an_unauthorized_transaction_moves_to_its_next_version() {
    let (s, account_id, gas_id) = setup_unauthorized_gas_owner(None, vec![]).await;
    let failed_at = 5;
    let gas_ref = s.latest_ref(&gas_id);
    let unauthorized =
        s.build_self_paid_account_call(account_authenticator(account_id), &gas_id, 1);
    let unauthorized_digest = *unauthorized.digest();
    let stale = s.build_self_paid_account_call(account_authenticator(account_id), &gas_id, 2);
    let stale_digest = *stale.digest();

    // Post-consensus validation keeps the transaction and locks the coin, and
    // the handler stores that lock.
    let mut transactions = vec![make_user_tx_v1_verified(unauthorized.clone())];
    let (dropped, locks, _) = post_consensus_validation::validate_and_resolve_conflicts(
        &s.authority,
        &s.epoch_store,
        failed_at,
        &Default::default(),
        &mut transactions,
    )
    .await
    .unwrap();
    assert!(dropped.is_empty(), "{dropped:?}");
    assert_eq!(locks.get(&gas_ref), Some(&unauthorized_digest));
    seed_quarantined_lock(&s.epoch_store, gas_ref, unauthorized_digest);

    s.epoch_store.assign_commit_to_transactions(
        failed_at,
        vec![TransactionKey::Digest(unauthorized_digest)],
    );
    let effects = s.execute_with_assigned_shared_versions_unchecked(unauthorized);
    assert!(effects.status().is_failure(), "{:?}", effects.status());
    assert_eq!(
        effects.gas_cost_summary(),
        &GasCostSummary::default(),
        "nobody may be charged"
    );
    assert_eq!(
        s.latest_ref(&gas_id).version,
        effects.lamport_version(),
        "the gas coin must move to its next version"
    );

    let next = s.build_self_paid_account_call(account_authenticator(account_id), &gas_id, 3);
    let next_digest = *next.digest();
    let mut transactions = vec![
        make_user_tx_v1_verified(stale),
        make_user_tx_v1_verified(next),
    ];
    let (dropped, locks, _) = post_consensus_validation::validate_and_resolve_conflicts(
        &s.authority,
        &s.epoch_store,
        failed_at + s.horizon_distance(),
        &Default::default(),
        &mut transactions,
    )
    .await
    .unwrap();
    assert!(
        matches!(
            dropped.as_slice(),
            [(digest, IotaError::ObjectLockConflict { obj_ref, pending_transaction }, _)]
                if *digest == stale_digest
                    && *obj_ref == gas_ref
                    && *pending_transaction == unauthorized_digest
        ),
        "{dropped:?}"
    );
    assert_eq!(transactions.len(), 1);
    assert_eq!(locks.get(&s.latest_ref(&gas_id)), Some(&next_digest));
}

/// Keeps `tx` at `commit`, stores the lock it takes on the gas coin `gas_id`
/// as the handler does, and executes it as a root of that commit. It fails
/// without a charge and leaves the coin live at its next version.
async fn keep_and_fail(
    s: &HandlerObjectStateSetup,
    commit: CommitIndex,
    tx: VerifiedTransaction,
    gas_id: ObjectId,
) {
    let digest = *tx.digest();
    let gas_ref = s.latest_ref(&gas_id);
    let result = s.validate_at(commit, std::slice::from_ref(&tx)).await;
    assert_eq!(result.kept, vec![digest], "{:?}", result.dropped);
    seed_quarantined_lock(&s.epoch_store, gas_ref, digest);
    s.epoch_store
        .assign_commit_to_transactions(commit, vec![TransactionKey::Digest(digest)]);
    let effects = s.execute_with_assigned_shared_versions_unchecked(tx);
    assert_eq!(effects.gas_cost_summary(), &GasCostSummary::default());
    let next_gas_ref = s.latest_ref(&gas_id);
    assert_eq!(next_gas_ref.version, effects.lamport_version());
    s.authority
        .get_object_cache_reader()
        .try_check_owned_objects_are_live(&[next_gas_ref])
        .expect("the gas coin must stay usable");
}

/// Each failure of a transaction the account did not authorize moves its gas
/// coin to its next version, so the account's next transaction naming the coin
/// is kept once the horizon reaches the commit of the failure.
#[tokio::test]
async fn consecutive_failures_each_move_the_gas_coin_on() {
    let (s, account_id, gas_id) = setup_unauthorized_gas_owner(None, vec![]).await;
    let mut commit = 5;
    for value in 1..=3 {
        let tx = s.build_self_paid_account_call(account_authenticator(account_id), &gas_id, value);
        keep_and_fail(&s, commit, tx, gas_id).await;
        commit += s.horizon_distance();
    }
}

/// A validator that executed an unauthorized transaction replays its commit,
/// as after a restart. The replay retains the executed transaction and locks
/// the gas coin it names, like the validators that kept the transaction without
/// having executed it, so a contender naming the coin in the same commit stays
/// dropped.
#[tokio::test]
async fn already_executed_transaction_locks_the_gas_coin_it_names() {
    let (s, account_id, gas_id) = setup_unauthorized_gas_owner(None, vec![]).await;
    let gas_ref = s.latest_ref(&gas_id);
    let unauthorized =
        s.build_self_paid_account_call(account_authenticator(account_id), &gas_id, 1);
    let unauthorized_digest = *unauthorized.digest();
    let contender = s.build_self_paid_account_call(account_authenticator(account_id), &gas_id, 2);
    let contender_digest = *contender.digest();
    let candidates = [unauthorized.clone(), contender];

    // The handler keeps the transaction and drops the contender on its lock,
    // and the transaction executes without a gas charge.
    let (scheduled, assigned_versions) = s.process_at_with_versions(1, &candidates).await;
    assert_eq!(
        scheduled.iter().map(|tx| *tx.digest()).collect::<Vec<_>>(),
        vec![unauthorized_digest]
    );
    let executable = s.executable(unauthorized);
    let effects = s.execute_executable(
        &executable,
        ExecutionEnv::new().with_assigned_versions(
            assigned_versions
                .into_map()
                .remove(&executable.key())
                .expect("the handler assigned the transaction's shared versions"),
        ),
    );
    assert!(effects.status().is_failure(), "{:?}", effects.status());
    assert_eq!(effects.gas_cost_summary(), &GasCostSummary::default());

    let replayed = s.validate_at(1, &candidates).await;
    assert_eq!(
        replayed.kept,
        vec![unauthorized_digest],
        "only the executed transaction may be kept"
    );
    assert_eq!(replayed.locks.get(&gas_ref), Some(&unauthorized_digest));
    assert!(
        matches!(
            replayed.dropped.as_slice(),
            [(digest, IotaError::ObjectLockConflict { obj_ref, pending_transaction })]
                if *digest == contender_digest
                    && *obj_ref == gas_ref
                    && *pending_transaction == unauthorized_digest
        ),
        "{:?}",
        replayed.dropped
    );
}

// ---------------------------------------------------------------------------
// P-COOL post-consensus validation: nodes at different execution progress
// ---------------------------------------------------------------------------

/// How the consumer in [`consumed_version_drops_the_same_whatever_the_consumer_execution`]
/// consumes the object a contender names.
#[derive(Clone, Copy, Debug)]
enum Consumption {
    /// A transfer of an object that exists since the epoch started. There
    /// is no row at the consumed version.
    EpochStartTransfer,
    /// A transfer of an object created in a commit the handler has assigned.
    Transfer,
    /// A deletion of an object created in a commit the handler has assigned.
    Deletion,
    /// A wrap of an object created in a commit the handler has assigned.
    Wrap,
}

/// An object consumed at commit 5 and a contender naming the consumed
/// version at every later commit. Two nodes: one executed the consumer when
/// its handler reached commit 5, and one has not executed it while the
/// horizon is below commit 5. Both drop the contender with the same error at
/// every commit, keep the consumer's outputs from the first commit whose
/// horizon reaches commit 5 and drop them before, and once both have
/// executed the consumer, the reader alone drops the consumed version on
/// both.
#[rstest::rstest]
#[tokio::test]
async fn consumed_version_drops_the_same_whatever_the_consumer_execution(
    #[values(
        Consumption::EpochStartTransfer,
        Consumption::Transfer,
        Consumption::Deletion,
        Consumption::Wrap
    )]
    consumption: Consumption,
) {
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let epoch_start_object = ObjectId::random();
    let consumer_gas = ObjectId::random();
    let contender_gas = ObjectId::random();
    let probe_gas = ObjectId::random();
    let genesis: Vec<_> = [epoch_start_object, consumer_gas, contender_gas, probe_gas]
        .into_iter()
        .map(|id| Object::with_id_owner_for_testing(id, sender))
        .collect();
    let ran = setup_handler_object_state(genesis.clone(), true).await;
    let lagging = setup_handler_object_state(genesis, true).await;
    let nodes = [&ran, &lagging];

    // Commit 1 creates the object the consumer takes, commit 2 is empty.
    let create = ran.build_move_call(
        "object_basics",
        "create",
        create_object_args(sender),
        &consumer_gas,
        sender,
        &key,
    );
    let mut created = None;
    for node in nodes {
        let effects = node
            .execute_with_assigned_commit(vec![create.clone()], 1)
            .remove(0);
        node.complete_commit(1, &[&effects]);
        node.complete_empty_commit(2);
        created = Some(*effects.created()[0].reference());
    }
    let created = created.unwrap();
    let consumed = match consumption {
        Consumption::EpochStartTransfer => ran.latest_ref(&epoch_start_object),
        _ => created,
    };
    let object_arg = CallArg::ImmutableOrOwned(consumed);
    let consumer = match consumption {
        Consumption::EpochStartTransfer => ran.build_transfer(
            &epoch_start_object,
            &consumer_gas,
            sender,
            &key,
            Address::random(),
        ),
        Consumption::Transfer => ran.build_move_call(
            "object_basics",
            "transfer",
            vec![
                object_arg,
                CallArg::Pure(bcs::to_bytes(&Address::random()).unwrap()),
            ],
            &consumer_gas,
            sender,
            &key,
        ),
        Consumption::Deletion => ran.build_move_call(
            "object_basics",
            "delete",
            vec![object_arg],
            &consumer_gas,
            sender,
            &key,
        ),
        Consumption::Wrap => ran.build_move_call(
            "object_basics",
            "wrap",
            vec![object_arg],
            &consumer_gas,
            sender,
            &key,
        ),
    };
    let stale = ran
        .epoch_store
        .verify_transaction(make_transfer_object_transaction(
            consumed,
            ran.latest_ref(&contender_gas),
            sender,
            &key,
            Address::random(),
            ran.rgp,
        ))
        .unwrap();

    for node in nodes {
        let scheduled = node.process_at(5, std::slice::from_ref(&consumer)).await;
        assert_eq!(scheduled.len(), 1);
        assert_eq!(scheduled[0].digest(), consumer.digest());
    }
    let effects = ran.execute(consumer.clone());
    ran.complete_commit(5, &[&effects]);
    ran.prune_versions_consumed_by(&[*create.digest(), *consumer.digest()], &effects)
        .await;
    // Names the consumer's gas output, which `lagging` does not hold yet.
    let fresh = ran
        .epoch_store
        .verify_transaction(make_transfer_object_transaction(
            ran.latest_ref(&consumer_gas),
            ran.latest_ref(&probe_gas),
            sender,
            &key,
            Address::random(),
            ran.rgp,
        ))
        .unwrap();
    let candidates = [stale.clone(), fresh.clone()];

    // Commit 6, horizon 2: `lagging` may validate it without executing the
    // consumer.
    let at_6 = ran.validate_at(6, &candidates).await;
    assert!(at_6.kept.is_empty());
    assert_eq!(at_6.dropped.len(), 2);
    assert!(
        matches!(
            &at_6.dropped[0].1,
            IotaError::ObjectLockConflict { obj_ref, pending_transaction }
                if *obj_ref == consumed && pending_transaction == consumer.digest()
        ),
        "{:?}",
        at_6.dropped[0].1
    );
    assert_eq!(at_6, lagging.validate_at(6, &candidates).await);

    // Commits 9 and 10 have the horizon at or above commit 5, so every node
    // has executed the consumer by then.
    lagging.complete_commit(5, &[&lagging.execute(consumer.clone())]);
    for index in [9, 10] {
        let result = ran.validate_at(index, &candidates).await;
        assert_eq!(result.kept, vec![*fresh.digest()]);
        assert_eq!(result.dropped, at_6.dropped[..1].to_vec());
        assert_eq!(result, lagging.validate_at(index, &candidates).await);
        for node in nodes {
            assert_owned_drops(
                node.read_owned(index, consumed),
                DropKind::HandlerRowSuperseded,
            );
        }
    }
}

/// A version consumed in the previous epoch drops in the next one through
/// the store's latest reference, with the consumed version pruned or still
/// stored. The consumer's output keeps.
#[rstest::rstest]
#[tokio::test]
async fn version_consumed_last_epoch_drops_on_every_node(#[values(true, false)] pruned: bool) {
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let object = ObjectId::random();
    let gas = ObjectId::random();
    let contender_gas = ObjectId::random();
    let mut s = setup_handler_object_state(
        [object, gas, contender_gas]
            .into_iter()
            .map(|id| Object::with_id_owner_for_testing(id, sender))
            .collect(),
        true,
    )
    .await;

    let consumed = s.latest_ref(&object);
    let consumer = s.build_transfer(&object, &gas, sender, &key, sender);
    let effects = s
        .execute_with_assigned_commit(vec![consumer.clone()], 5)
        .remove(0);
    s.complete_commit(5, &[&effects]);
    if pruned {
        s.prune_versions_consumed_by(&[*consumer.digest()], &effects)
            .await;
    }
    let produced = s.latest_ref(&object);

    // The override goes with the epoch: reconfiguration installs its own.
    drop(s._config_guard.take());
    s.authority.reconfigure_for_testing().await;
    s.epoch_store = (*s.authority.epoch_store_for_testing()).clone();
    assert_eq!(s.epoch_store.epoch(), 1);
    assert!(s.epoch_store.protocol_config().enable_pcool_flow());

    assert_owned_drops(s.read_owned(3, consumed), DropKind::StoreSuperseded);
    assert_keeps(s.read_owned(3, produced), produced);
    let transfer = |reference: ObjectReference| {
        s.epoch_store
            .verify_transaction(make_transfer_object_transaction(
                reference,
                s.latest_ref(&contender_gas),
                sender,
                &key,
                Address::random(),
                s.rgp,
            ))
            .unwrap()
    };
    let stale = transfer(consumed);
    let fresh = transfer(produced);
    let result = s.validate_at(3, &[stale.clone(), fresh.clone()]).await;
    assert_eq!(result.kept, vec![*fresh.digest()]);
    assert_eq!(result.dropped.len(), 1);
    assert_eq!(result.dropped[0].0, *stale.digest());
    assert!(
        matches!(
            &result.dropped[0].1,
            IotaError::UserInput {
                error: UserInputError::InputObjectNotAvailableAtCommit {
                    object_id,
                    version: Some(version),
                }
            } if *object_id == object && *version == consumed.version
        ),
        "{:?}",
        result.dropped[0].1
    );
}

/// A crash after a checkpoint's rows are written to the table and before its
/// outputs are. The restart loses the rows held in memory, the handler
/// replays the commit, and executing the checkpoint again writes the same
/// rows. At every point the consumed versions keep with their bytes
/// and the produced one answers missing, as its row is above the horizon.
/// The object store is not restarted here, so the outputs stay readable from
/// its cache; the cluster test
/// `test_crash_after_checkpoint_bookkeeping_write_recovers` loses them.
#[tokio::test]
async fn crash_between_checkpoint_bookkeeping_and_outputs_keeps_every_read() {
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let object = ObjectId::random();
    let gas = ObjectId::random();
    let mut s = setup_handler_object_state(
        [object, gas]
            .into_iter()
            .map(|id| Object::with_id_owner_for_testing(id, sender))
            .collect(),
        true,
    )
    .await;

    let consumed = [s.latest_ref(&object), s.latest_ref(&gas)];
    let tx = s.build_transfer(&object, &gas, sender, &key, sender);
    let tx_key = TransactionKey::Digest(*tx.digest());
    let effects = s.execute_with_assigned_commit(vec![tx], 2).remove(0);
    let produced = s.latest_ref(&object);
    s.epoch_store
        .persist_checkpoint_bookkeeping([&effects])
        .unwrap();
    let check_reads = |s: &HandlerObjectStateSetup| {
        for reference in consumed {
            assert_keeps(s.read_owned(3, reference), reference);
        }
        assert_owned_missing(
            s.read_owned(3, produced),
            MissingKind::HandlerRowAboveHorizon,
        );
    };
    check_reads(&s);

    // The crash loses the rows held in memory.
    s.restart_epoch_store().await;
    assert_eq!(
        s.epoch_store
            .handler_object_state_for_testing()
            .overlay_size_for_testing(),
        0
    );
    check_reads(&s);

    // The handler replays commit 2, and the checkpoint executes again and
    // its batch is written again.
    s.epoch_store.assign_commit_to_transactions(2, vec![tx_key]);
    s.epoch_store.record_executed_transaction(&tx_key, &effects);
    s.epoch_store
        .persist_checkpoint_bookkeeping([&effects])
        .unwrap();
    check_reads(&s);
}

// ---------------------------------------------------------------------------
// P-COOL post-consensus validation: execution progress and replays
// ---------------------------------------------------------------------------

/// A regulated coin type, a shared zero coin of it, an address its deny list
/// denies from the current epoch on, and an owned coin of that address and of
/// another one. The same state exists on every validator built by
/// [`setup_validators_with_a_shared_regulated_coin`].
struct SharedRegulatedCoin {
    coin_type: TypeTag,
    coin_id: ObjectId,
    deny_cap_id: ObjectId,
    publisher: Address,
    publisher_key: AccountPrivateKey,
    publisher_gas: ObjectId,
    denied: Address,
    denied_key: AccountPrivateKey,
    denied_gas: ObjectId,
    denied_coin: ObjectId,
    other: Address,
    other_key: AccountPrivateKey,
    other_gas: ObjectId,
    other_coin: ObjectId,
}

/// How [`setup_validators_with_a_shared_regulated_coin`] builds the coin's
/// state.
#[derive(Clone, Copy, Default)]
struct RegulatedCoinOptions {
    /// Shares the coin at this commit of epoch 1, with the commit assigned by
    /// the handler, instead of before the epoch.
    share_at: Option<CommitIndex>,
}

impl SharedRegulatedCoin {
    /// A call of `0x2::coin::destroy_zero` on the shared coin by `sender`,
    /// paid with `gas_id` on `s`.
    fn destroy_shared_coin(
        &self,
        s: &HandlerObjectStateSetup,
        sender: Address,
        sender_key: &AccountPrivateKey,
        gas_id: &ObjectId,
    ) -> VerifiedTransaction {
        let tx = TestTransactionBuilder::new(sender, s.latest_ref(gas_id), s.rgp)
            .move_call(
                ObjectId::FRAMEWORK,
                "coin",
                "destroy_zero",
                vec![s.shared_arg(&self.coin_id)],
            )
            .with_type_args(vec![self.coin_type.clone()])
            .build();
        s.epoch_store
            .verify_transaction(to_sender_signed_transaction(tx, sender_key))
            .unwrap()
    }

    /// Executes the publisher's deletion of the shared coin on `s` as a root
    /// of commit `deleting_commit`, and returns its effects.
    fn delete_shared_coin(
        &self,
        s: &HandlerObjectStateSetup,
        deleting_commit: CommitIndex,
    ) -> TransactionEffects {
        let deletion =
            self.destroy_shared_coin(s, self.publisher, &self.publisher_key, &self.publisher_gas);
        s.epoch_store.assign_commit_to_transactions(
            deleting_commit,
            vec![TransactionKey::Digest(*deletion.digest())],
        );
        let effects = s.execute_with_assigned_shared_versions(deletion);
        assert!(s.authority.get_object(&self.coin_id).is_none());
        effects
    }

    /// Executes `0x2::coin::<function>` of the deny list for the coin type
    /// on `s` with the publisher's deny cap, assigned to commit `index`, and
    /// returns its effects. `address` is the argument of the add and remove
    /// functions.
    fn call_deny_list(
        &self,
        s: &HandlerObjectStateSetup,
        function: &str,
        address: Option<Address>,
        index: CommitIndex,
    ) -> TransactionEffects {
        let mut args = vec![
            s.shared_arg(&ObjectId::DENY_LIST),
            CallArg::ImmutableOrOwned(s.latest_ref(&self.deny_cap_id)),
        ];
        args.extend(address.map(|address| CallArg::Pure(bcs::to_bytes(&address).unwrap())));
        let tx =
            TestTransactionBuilder::new(self.publisher, s.latest_ref(&self.publisher_gas), s.rgp)
                .move_call(ObjectId::FRAMEWORK, "coin", function, args)
                .with_type_args(vec![self.coin_type.clone()])
                .build();
        let tx = s
            .epoch_store
            .verify_transaction(to_sender_signed_transaction(tx, &self.publisher_key))
            .unwrap();
        s.epoch_store
            .assign_commit_to_transactions(index, vec![TransactionKey::Digest(*tx.digest())]);
        s.execute_with_assigned_shared_versions(tx)
    }

    /// The drop of `transaction` for a denial of `address` for the coin type.
    fn address_denied(
        &self,
        transaction: &VerifiedTransaction,
        address: Address,
    ) -> Vec<(TransactionDigest, IotaError)> {
        vec![(
            *transaction.digest(),
            UserInputError::AddressDeniedForCoin {
                address,
                coin_type: self.coin_type.to_canonical_string(false),
            }
            .into(),
        )]
    }

    /// A transfer of the owned regulated coin `coin` by its owner `sender`,
    /// paid with `gas_id` on `s`.
    fn transfer_owned_coin(
        &self,
        s: &HandlerObjectStateSetup,
        sender: Address,
        sender_key: &AccountPrivateKey,
        gas_id: &ObjectId,
        coin: &ObjectId,
    ) -> VerifiedTransaction {
        s.build_transfer(coin, gas_id, sender, sender_key, self.publisher)
    }
}

/// Shares a zero coin of `coin_type` on `s`, paid by the publisher's
/// `share_gas`, assigned to commit `share_at` when given and to a new commit
/// otherwise. Returns the shared coin's reference.
fn share_zero_regulated_coin(
    s: &HandlerObjectStateSetup,
    coin_type: &TypeTag,
    publisher: Address,
    publisher_key: &AccountPrivateKey,
    share_gas: &ObjectId,
    share_at: Option<CommitIndex>,
) -> ObjectReference {
    let mut builder = ProgrammableTransactionBuilder::new();
    let zero = builder.programmable_move_call(
        ObjectId::FRAMEWORK,
        Identifier::from_static("coin"),
        Identifier::from_static("zero"),
        vec![coin_type.clone()],
        vec![],
    );
    builder.programmable_move_call(
        ObjectId::FRAMEWORK,
        Identifier::from_static("transfer"),
        Identifier::from_static("public_share_object"),
        vec![TypeTag::Struct(Box::new(StructTag::new(
            ObjectId::FRAMEWORK,
            Identifier::from_static("coin"),
            Identifier::from_static("Coin"),
            vec![coin_type.clone()],
        )))],
        vec![zero],
    );
    let share = TestTransactionBuilder::new(publisher, s.latest_ref(share_gas), s.rgp)
        .programmable(builder.finish())
        .build();
    let share = s
        .epoch_store
        .verify_transaction(to_sender_signed_transaction(share, publisher_key))
        .unwrap();
    let share_effects = match share_at {
        Some(index) => s.execute_with_assigned_commit(vec![share], index).remove(0),
        None => {
            s.epoch_store
                .assign_to_a_new_commit_for_testing(&[TransactionKey::Digest(*share.digest())]);
            s.execute(share)
        }
    };
    share_effects
        .created()
        .iter()
        .find(|created| matches!(created.owner(), Owner::Shared(_)))
        .map(|created| *created.reference())
        .expect("the share transaction creates the shared coin")
}

/// Two validators of one chain at epoch 1, with the P-COOL flow on and the
/// state of [`SharedRegulatedCoin`], built as `options` asks. Epoch 0 publishes
/// the coin, hands out the owned coins and denies the address, each in a commit
/// of its own; the epoch change makes the denial active for the epoch-gated
/// read.
async fn setup_validators_with_a_shared_regulated_coin(
    options: RegulatedCoinOptions,
) -> (
    HandlerObjectStateSetup,
    HandlerObjectStateSetup,
    SharedRegulatedCoin,
) {
    let (publisher, publisher_key): (Address, AccountPrivateKey) = get_key_pair();
    let (denied, denied_key): (Address, AccountPrivateKey) = get_key_pair();
    let (other, other_key): (Address, AccountPrivateKey) = get_key_pair();
    let publisher_gas = ObjectId::random();
    let share_gas = ObjectId::random();
    let denied_gas = ObjectId::random();
    let other_gas = ObjectId::random();
    let genesis = vec![
        Object::with_id_owner_for_testing(publisher_gas, publisher),
        Object::with_id_owner_for_testing(share_gas, publisher),
        Object::with_id_owner_for_testing(denied_gas, denied),
        Object::with_id_owner_for_testing(other_gas, other),
    ];

    let mut validators = Vec::new();
    let mut coins = Vec::new();
    for _ in 0..2 {
        let s = build_handler_object_state_setup(genesis.clone(), None, None, None).await;
        let (_, publish_effects) = build_and_try_publish_test_package(
            &s.authority,
            &publisher,
            &publisher_key,
            &publisher_gas,
            "coin_deny_list_v1_mintable",
            TEST_ONLY_GAS_UNIT_FOR_PUBLISH * s.rgp,
            s.rgp,
            false,
        )
        .await;
        let mut package_id = None;
        let mut deny_cap = None;
        let mut minted = None;
        for created in publish_effects.created() {
            let object = s
                .authority
                .get_object(&created.reference().object_id)
                .unwrap();
            if object.is_package() {
                package_id = Some(object.id());
            } else if object.data.opt_object_type().unwrap().is_deny_cap_v1() {
                deny_cap = Some(object.object_ref());
            } else if !object.is_gas_coin() && object.opt_coin_type().is_some() {
                minted = Some(object.object_ref());
            }
        }
        let deny_cap = deny_cap.expect("the publish creates the deny cap");
        let coin_type = TypeTag::Struct(Box::new(StructTag::new(
            package_id.expect("the publish creates the coin package"),
            Identifier::from_static("regulated_coin"),
            Identifier::from_static("REGULATED_COIN"),
            vec![],
        )));

        let mut builder = ProgrammableTransactionBuilder::new();
        builder
            .pay(
                vec![minted.expect("the publish mints a coin")],
                vec![denied, other],
                vec![10, 10],
            )
            .unwrap();
        let pay = TestTransactionBuilder::new(publisher, s.latest_ref(&publisher_gas), s.rgp)
            .programmable(builder.finish())
            .build();
        let pay = s
            .epoch_store
            .verify_transaction(to_sender_signed_transaction(pay, &publisher_key))
            .unwrap();
        s.epoch_store
            .assign_to_a_new_commit_for_testing(&[TransactionKey::Digest(*pay.digest())]);
        let pay_effects = s.execute(pay);
        let coin_of = |owner: Address| {
            pay_effects
                .created()
                .iter()
                .find(|created| *created.owner() == Owner::Address(owner))
                .map(|created| *created.reference().object_id())
                .expect("the payment creates a coin for each recipient")
        };
        let owned_coins = (coin_of(denied), coin_of(other));

        let deny_args = vec![
            s.shared_arg(&ObjectId::DENY_LIST),
            CallArg::ImmutableOrOwned(deny_cap),
            CallArg::Pure(bcs::to_bytes(&denied).unwrap()),
        ];
        let deny = TestTransactionBuilder::new(publisher, s.latest_ref(&publisher_gas), s.rgp)
            .move_call(ObjectId::FRAMEWORK, "coin", "deny_list_v1_add", deny_args)
            .with_type_args(vec![coin_type.clone()])
            .build();
        let deny = s
            .epoch_store
            .verify_transaction(to_sender_signed_transaction(deny, &publisher_key))
            .unwrap();
        s.epoch_store
            .assign_to_a_new_commit_for_testing(&[TransactionKey::Digest(*deny.digest())]);
        s.execute_with_assigned_shared_versions(deny);
        let shared_before_the_epoch = options.share_at.is_none().then(|| {
            share_zero_regulated_coin(&s, &coin_type, publisher, &publisher_key, &share_gas, None)
        });
        validators.push(s);
        coins.push((
            coin_type,
            deny_cap.object_id,
            owned_coins,
            shared_before_the_epoch,
        ));
    }

    for s in &mut validators {
        s.authority.reconfigure_for_testing().await;
        s.epoch_store = s.authority.epoch_store_for_testing().clone();
        assert_eq!(s.epoch_store.epoch(), 1);
        assert!(s.epoch_store.pcool_bookkeeping_enabled());
    }

    let mut shared_coins = Vec::new();
    for (s, (coin_type, _, _, shared_before_the_epoch)) in validators.iter().zip(&coins) {
        shared_coins.push(shared_before_the_epoch.unwrap_or_else(|| {
            share_zero_regulated_coin(
                s,
                coin_type,
                publisher,
                &publisher_key,
                &share_gas,
                options.share_at,
            )
        }));
        let config = get_per_type_coin_deny_list_v1(
            &coin_type.to_canonical_string(false),
            &s.authority.get_object_store(),
        )
        .expect("the deny list holds a config for the coin type");
        let epoch = Some(s.epoch_store.epoch());
        let store = s.authority.get_object_store();
        assert!(
            check_address_denied_by_config(&config, denied, &store, epoch),
            "the denial must be active for the epoch-gated read"
        );
    }
    assert_eq!(
        coins[0], coins[1],
        "both validators must hold the same coins"
    );
    assert_eq!(
        shared_coins[0], shared_coins[1],
        "both validators must hold the same shared coin"
    );
    let (coin_type, deny_cap_id, (denied_coin, other_coin), _) = coins.remove(0);
    let second = validators.pop().unwrap();
    let first = validators.pop().unwrap();
    (
        first,
        second,
        SharedRegulatedCoin {
            coin_type,
            coin_id: shared_coins[0].object_id,
            deny_cap_id,
            publisher,
            publisher_key,
            publisher_gas,
            denied,
            denied_key,
            denied_gas,
            denied_coin,
            other,
            other_key,
            other_gas,
            other_coin,
        },
    )
}

/// Validates, at commit 14 (horizon 10), a transaction by the denied address
/// naming the shared regulated coin, on a validator that has executed the
/// coin's deletion and on one that has not. `deleting_commit` is the commit
/// the deletion is assigned to on the first validator, above the horizon.
/// With `commit_the_deletion`, the deletion's outputs are committed there,
/// which moves the coin's last version into this epoch's bucket, and the
/// caches are cleared.
async fn run_shared_coin_deletion_verdict_comparison(
    options: RegulatedCoinOptions,
    deleting_commit: CommitIndex,
    commit_the_deletion: bool,
) {
    let (deleted, not_deleted, coin) = setup_validators_with_a_shared_regulated_coin(options).await;
    let transaction =
        coin.destroy_shared_coin(&deleted, coin.denied, &coin.denied_key, &coin.denied_gas);
    let last_version = deleted
        .authority
        .get_object(&coin.coin_id)
        .unwrap()
        .version();

    coin.delete_shared_coin(&deleted, deleting_commit);
    assert!(not_deleted.authority.get_object(&coin.coin_id).is_some());
    if commit_the_deletion {
        deleted.commit_all_executed_and_clear_caches();
        assert!(
            deleted
                .authority
                .get_object_cache_reader()
                .get_object_by_key(&coin.coin_id, last_version)
                .is_none(),
            "the commit moves the coin's last version out of the live state"
        );
    }

    let on_deleted = deleted
        .validate_at(14, std::slice::from_ref(&transaction))
        .await;
    let on_not_deleted = not_deleted
        .validate_at(14, std::slice::from_ref(&transaction))
        .await;
    assert_eq!(
        on_not_deleted.dropped,
        coin.address_denied(&transaction, coin.denied),
        "the deny check at the commit must see the shared coin's type"
    );
    assert_eq!(
        on_deleted, on_not_deleted,
        "a validator that executed the deletion of the shared coin and one that did not must \
         reach the same verdict"
    );
}

/// A shared coin of a regulated type is deleted by commit 11, which is above
/// the horizon of commit 14. Commit 14 holds a transaction naming the coin,
/// sent by an address the coin type's deny list denies. Every validator must
/// keep or drop it alike, whether or not it has executed the deletion.
#[tokio::test]
async fn shared_coin_deleted_above_the_horizon_gets_one_verdict_on_every_validator() {
    run_shared_coin_deletion_verdict_comparison(RegulatedCoinOptions::default(), 11, false).await;
}

/// As [`shared_coin_deleted_above_the_horizon_gets_one_verdict_on_every_validator`],
/// with the deletion's checkpoint committed and the caches cleared: the type
/// read of the coin's last version finds it in this epoch's bucket.
#[tokio::test]
async fn shared_coin_deleted_and_committed_above_the_horizon_gets_one_verdict_on_every_validator() {
    run_shared_coin_deletion_verdict_comparison(RegulatedCoinOptions::default(), 11, true).await;
}

/// A validator validates commit 12 after executing the deletion of a shared
/// regulated coin at commit 11, and saves its decision for a transaction by a
/// denied address naming the coin. It then crashes before the outputs of
/// commit 11 are durable. On replay the deletion is not executed, as on the
/// second validator here, and the commit must replay without stopping the
/// node, with the saved decision.
#[tokio::test]
async fn replay_after_losing_a_shared_coin_deletion_repeats_the_saved_decision() {
    run_shared_coin_deletion_replay(RegulatedCoinOptions::default()).await;
}

/// Validates commit 12 on a validator that executed the shared coin's
/// deletion at commit 11, then replays commit 12 with that saved decision on
/// a validator without the deletion, as after a crash that lost commit 11's
/// outputs. The replay must not stop the node and must repeat the decision.
async fn run_shared_coin_deletion_replay(options: RegulatedCoinOptions) {
    let (deleted, replayed, coin) = setup_validators_with_a_shared_regulated_coin(options).await;
    let transaction =
        coin.destroy_shared_coin(&deleted, coin.denied, &coin.denied_key, &coin.denied_gas);
    let digest = *transaction.digest();
    coin.delete_shared_coin(&deleted, 11);
    let first_run = deleted
        .validate_at(12, std::slice::from_ref(&transaction))
        .await;
    let saved = deleted
        .epoch_store
        .post_consensus_decision(12, digest)
        .unwrap()
        .expect("the first run saves its decision");

    replayed
        .epoch_store
        .persist_post_consensus_decisions(12, &[(digest, saved)])
        .unwrap();
    let authority = replayed.authority.clone();
    let epoch_store = replayed.epoch_store.clone();
    let replay = tokio::spawn(async move {
        let mut transactions = vec![make_user_tx_v1_verified(transaction)];
        post_consensus_validation::validate_and_resolve_conflicts(
            &authority,
            &epoch_store,
            12,
            &Default::default(),
            &mut transactions,
        )
        .await
        .map(|(dropped, _, _)| {
            dropped
                .into_iter()
                .map(|(digest, error, _)| (digest, error))
                .collect::<Vec<_>>()
        })
    })
    .await
    .expect("the replay of commit 12 must not stop the node")
    .unwrap();
    assert_eq!(replay, first_run.dropped);
}

/// After the final round, commits do not wait for the horizon to be fully
/// executed. Here commit 20 carries a transaction on a shared object whose
/// deletion at commit 15, at or below its horizon, this validator had not
/// executed yet; no decision is saved for it. The deletion then executes and
/// becomes durable, and the node restarts before the epoch ends. The replay
/// of commit 20 must not stop the node.
#[tokio::test]
async fn replay_after_the_final_round_survives_a_shared_deletion_executed_late() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let deleter_gas = ObjectId::random();
    let mut s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(gas_id, sender),
            Object::with_id_owner_for_testing(deleter_gas, sender),
        ],
        true,
    )
    .await;
    let share_effects =
        s.assigned_commit_object_basics_call("share", vec![], &gas_id, sender, &sender_key, 5);
    let shared_id = *share_effects.created()[0].reference().object_id();
    let transaction = s.build_move_call(
        "object_basics",
        "set_value",
        vec![
            s.shared_arg(&shared_id),
            CallArg::Pure(bcs::to_bytes(&1u64).unwrap()),
        ],
        &gas_id,
        sender,
        &sender_key,
    );
    let digest = *transaction.digest();

    s.epoch_store
        .get_reconfig_state_write_lock_guard()
        .close_all_tx();
    process_commit_at(
        s.authority.clone(),
        s.epoch_store.clone(),
        20,
        vec![transaction.clone()],
    )
    .await
    .unwrap();
    assert_eq!(
        s.epoch_store.post_consensus_decision(20, digest).unwrap(),
        None
    );

    let delete_effects = s.assigned_commit_shared_object_basics_call(
        "delete",
        vec![s.shared_arg(&shared_id)],
        &deleter_gas,
        sender,
        &sender_key,
        15,
    );
    s.epoch_store
        .persist_checkpoint_bookkeeping([&share_effects, &delete_effects])
        .unwrap();

    s.epoch_store = reopen(&s.authority, &s.epoch_store);
    s.initialize_randomness().await;
    s.epoch_store
        .get_reconfig_state_write_lock_guard()
        .close_all_tx();
    tokio::spawn(process_commit_at(
        s.authority.clone(),
        s.epoch_store.clone(),
        20,
        vec![transaction],
    ))
    .await
    .expect("the replay of commit 20 must not stop the node")
    .unwrap();
}

/// After the final round, a commit does not wait for the horizon to be fully
/// executed. A transaction at or below the horizon can then have its handler
/// row written while its output is not in the object store yet. The commit
/// carrying a transaction naming that output must be processed without a
/// storage error, which would stop the consensus handler.
#[tokio::test]
async fn commit_after_the_final_round_decides_on_a_row_whose_output_is_not_stored() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let object_id = ObjectId::random();
    let producer_gas = ObjectId::random();
    let consumer_gas = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(object_id, sender),
            Object::with_id_owner_for_testing(producer_gas, sender),
            Object::with_id_owner_for_testing(consumer_gas, sender),
        ],
        true,
    )
    .await;

    // The producer at commit 9 has executed and its rows are recorded, and
    // its output has not reached the object store.
    let producer = s.build_transfer(&object_id, &producer_gas, sender, &sender_key, sender);
    let producer_effects = TestEffectsBuilder::new(producer.data()).build();
    let producer_key = TransactionKey::Digest(*producer.digest());
    s.epoch_store
        .assign_commit_to_transactions(9, vec![producer_key]);
    s.epoch_store
        .record_executed_transaction(&producer_key, &producer_effects);
    let produced = producer_effects
        .mutated()
        .into_iter()
        .map(|owned| *owned.reference())
        .find(|reference| reference.object_id == object_id)
        .expect("the producer mutates the object");
    assert!(s.store_object(&object_id, produced.version).is_none());

    let consumer = s
        .epoch_store
        .verify_transaction(make_transfer_object_transaction(
            produced,
            s.latest_ref(&consumer_gas),
            sender,
            &sender_key,
            Address::random(),
            s.rgp,
        ))
        .unwrap();
    s.epoch_store
        .get_reconfig_state_write_lock_guard()
        .close_all_tx();
    // Commit 13, horizon 9: the producer's row is at the horizon.
    let result = process_commit_at(
        s.authority.clone(),
        s.epoch_store.clone(),
        13,
        vec![consumer],
    )
    .await;
    assert!(
        result.is_ok(),
        "validation after the final round must not fail with {:?}",
        result.err()
    );
}

/// A transaction on a shared object is kept at commit 12 and never executes.
/// A later commit deletes the object, and the deletion is durable when the
/// node restarts. The replay of commit 12 reads the object as deleted where
/// the first run read it as existing, and must still repeat the saved keep.
#[tokio::test]
async fn replay_of_a_kept_transaction_survives_a_later_deletion_of_its_shared_input() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let deleter_gas = ObjectId::random();
    let mut s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(gas_id, sender),
            Object::with_id_owner_for_testing(deleter_gas, sender),
        ],
        true,
    )
    .await;
    let share_effects =
        s.assigned_commit_object_basics_call("share", vec![], &gas_id, sender, &sender_key, 5);
    let shared_id = *share_effects.created()[0].reference().object_id();
    let transaction = s.build_move_call(
        "object_basics",
        "set_value",
        vec![
            s.shared_arg(&shared_id),
            CallArg::Pure(bcs::to_bytes(&1u64).unwrap()),
        ],
        &gas_id,
        sender,
        &sender_key,
    );
    let first_run = s.validate_at(12, std::slice::from_ref(&transaction)).await;
    assert_eq!(first_run.kept, vec![*transaction.digest()]);

    let delete_effects = s.assigned_commit_shared_object_basics_call(
        "delete",
        vec![s.shared_arg(&shared_id)],
        &deleter_gas,
        sender,
        &sender_key,
        15,
    );
    s.epoch_store
        .persist_checkpoint_bookkeeping([&share_effects, &delete_effects])
        .unwrap();
    s.epoch_store = reopen(&s.authority, &s.epoch_store);
    assert!(matches!(
        s.read_shared(
            12,
            &shared_id,
            initial_shared_version(share_effects.created()[0].owner())
        ),
        SharedVerdict::Deleted(..)
    ));

    assert_eq!(
        first_run,
        s.validate_at(12, std::slice::from_ref(&transaction)).await
    );
}

/// On `s`, each in a commit the handler has assigned: creates a parent and a
/// value at commits 2 and 3,
/// adds the value as a dynamic field of the parent at commit 5 and removes it
/// through the parent at commit 6, which consumes the field without a lock.
/// `gas_id` pays for all of it. Returns the consumed field version, the
/// digests of the four transactions in order, and the removal's effects.
fn consume_a_dynamic_field_child_without_a_lock(
    s: &HandlerObjectStateSetup,
    gas_id: &ObjectId,
    sender: Address,
    sender_key: &AccountPrivateKey,
) -> (ObjectReference, Vec<TransactionDigest>, TransactionEffects) {
    let parent_effects = s.assigned_commit_object_basics_call(
        "create",
        create_object_args(sender),
        gas_id,
        sender,
        sender_key,
        2,
    );
    let value_effects = s.assigned_commit_object_basics_call(
        "create",
        create_object_args(sender),
        gas_id,
        sender,
        sender_key,
        3,
    );
    let parent_id = *parent_effects.created()[0].reference().object_id();
    let add_effects = s.assigned_commit_object_basics_call(
        "add_field",
        vec![
            CallArg::ImmutableOrOwned(s.latest_ref(&parent_id)),
            CallArg::ImmutableOrOwned(*value_effects.created()[0].reference()),
        ],
        gas_id,
        sender,
        sender_key,
        5,
    );
    let remove_effects = s.assigned_commit_object_basics_call(
        "remove_field",
        vec![CallArg::ImmutableOrOwned(s.latest_ref(&parent_id))],
        gas_id,
        sender,
        sender_key,
        6,
    );
    let digests = [
        &parent_effects,
        &value_effects,
        &add_effects,
        &remove_effects,
    ]
    .map(|effects| *effects.transaction_digest())
    .to_vec();
    (
        *add_effects.created()[0].reference(),
        digests,
        remove_effects,
    )
}

/// The execution watcher fails while completing commit 1, and validation of
/// commit 3 waits for its horizon, commit 1. Once the watcher stops, the wait
/// before validation must not depend on checkpoint certification by peers.
/// The check that the boundary finishes also passes when the watcher's
/// error stops the node instead, since the spawned task then ends with a
/// panic; that outcome is acceptable.
#[tokio::test(start_paused = true)]
async fn validation_wait_ends_after_the_execution_watcher_fails() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let obj_id = ObjectId::random();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(obj_id, sender),
            Object::with_id_owner_for_testing(gas_id, sender),
        ],
        true,
    )
    .await;
    let _watcher = s.start_execution_watcher();
    s.epoch_store.fail_next_watcher_completion_for_testing();

    let tx = s.build_transfer(&obj_id, &gas_id, sender, &sender_key, Address::random());
    s.epoch_store
        .assign_commit_to_transactions(1, vec![TransactionKey::Digest(*tx.digest())]);
    let boundary = tokio::spawn(process_commit_at(
        s.authority.clone(),
        s.epoch_store.clone(),
        3,
        vec![],
    ));
    s.execute(tx);

    assert!(
        tokio::time::timeout(Duration::from_secs(3_600), boundary)
            .await
            .is_ok(),
        "commit 3 must not wait forever once the execution watcher failed"
    );
}

/// The handler executes commit 9's transaction, whose row stays in memory
/// and whose output stays in the object cache. After the final round,
/// commit 13 (horizon 9) carries a transfer of that output, does not wait,
/// and saves no decision for it. The node crashes before commit 9's
/// checkpoint is durable, losing the row and the output. The replay of commit
/// 13 must not stop the node.
#[tokio::test]
async fn replay_after_the_final_round_survives_losing_an_uncommitted_producer() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let object_id = ObjectId::random();
    let producer_gas = ObjectId::random();
    let consumer_gas = ObjectId::random();
    let mut s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(object_id, sender),
            Object::with_id_owner_for_testing(producer_gas, sender),
            Object::with_id_owner_for_testing(consumer_gas, sender),
        ],
        true,
    )
    .await;
    let before = s.latest_ref(&object_id);
    let producer = s.build_transfer(&object_id, &producer_gas, sender, &sender_key, sender);
    let producer_digest = *producer.digest();
    s.execute_with_assigned_commit(vec![producer], 9);
    let consumer = s.build_transfer(
        &object_id,
        &consumer_gas,
        sender,
        &sender_key,
        Address::random(),
    );

    s.epoch_store
        .get_reconfig_state_write_lock_guard()
        .close_all_tx();
    process_commit_at(
        s.authority.clone(),
        s.epoch_store.clone(),
        13,
        vec![consumer.clone()],
    )
    .await
    .unwrap();
    assert_eq!(
        s.epoch_store
            .post_consensus_decision(13, *consumer.digest())
            .unwrap(),
        None
    );

    // The crash: the producer's uncommitted output and the rows held in
    // memory go.
    // Clearing the end-of-epoch state also clears the durable marker table,
    // which a crash does not; the owned producer writes no marker.
    s.authority
        .get_reconfig_api()
        .revert_state_update(&producer_digest);
    {
        let guard = s.authority.execution_lock_for_reconfiguration().await;
        s.authority
            .get_reconfig_api()
            .clear_state_end_of_epoch(&guard);
    }
    s.epoch_store = reopen(&s.authority, &s.epoch_store);
    assert_eq!(s.latest_ref(&object_id), before);
    s.initialize_randomness().await;
    s.epoch_store
        .get_reconfig_state_write_lock_guard()
        .close_all_tx();
    tokio::spawn(process_commit_at(
        s.authority.clone(),
        s.epoch_store.clone(),
        13,
        vec![consumer],
    ))
    .await
    .expect("the replay of commit 13 must not stop the node")
    .unwrap();
}

/// As [`shared_coin_deleted_above_the_horizon_gets_one_verdict_on_every_validator`],
/// with the coin shared at commit 5 of the current epoch, so the reader finds
/// it through its creation row.
#[tokio::test]
async fn shared_coin_created_this_epoch_and_deleted_above_the_horizon_gets_one_verdict() {
    run_shared_coin_deletion_verdict_comparison(
        RegulatedCoinOptions { share_at: Some(5) },
        11,
        false,
    )
    .await;
}

/// As [`replay_after_losing_a_shared_coin_deletion_repeats_the_saved_decision`],
/// with the coin shared at commit 5 of the current epoch.
#[tokio::test]
async fn replay_after_losing_the_deletion_of_a_coin_shared_this_epoch_repeats_the_saved_decision() {
    run_shared_coin_deletion_replay(RegulatedCoinOptions { share_at: Some(5) }).await;
}

/// The denied address's entry is removed at commit 11, on one validator only.
/// The deny list is read at the commit as of its horizon, so the removal
/// counts from commit 15 on. At commit 14 (horizon 10) a transfer of the
/// address's own regulated coin is dropped on both validators, whether or
/// not they executed the removal. At commit 15 (horizon 11) it is kept.
#[tokio::test]
async fn deny_entry_removed_at_a_commit_counts_from_the_horizon_distance_after_it() {
    let (removed, not_removed, coin) =
        setup_validators_with_a_shared_regulated_coin(RegulatedCoinOptions::default()).await;
    let transfer = coin.transfer_owned_coin(
        &removed,
        coin.denied,
        &coin.denied_key,
        &coin.denied_gas,
        &coin.denied_coin,
    );
    coin.call_deny_list(&removed, "deny_list_v1_remove", Some(coin.denied), 11);

    let on_removed = removed
        .validate_at(14, std::slice::from_ref(&transfer))
        .await;
    let on_not_removed = not_removed
        .validate_at(14, std::slice::from_ref(&transfer))
        .await;
    assert_eq!(
        on_not_removed.dropped,
        coin.address_denied(&transfer, coin.denied)
    );
    assert_eq!(on_removed, on_not_removed);

    coin.call_deny_list(&not_removed, "deny_list_v1_remove", Some(coin.denied), 11);
    for s in [&removed, &not_removed] {
        let at_15 = s.validate_at(15, std::slice::from_ref(&transfer)).await;
        assert_eq!(at_15.kept, vec![*transfer.digest()]);
    }
}

/// An address is denied at commit 11, on one validator only. A transfer of
/// its regulated coin is kept at commit 14 (horizon 10) on both validators,
/// and dropped at commit 15 (horizon 11) once both have executed the denial.
#[tokio::test]
async fn deny_entry_added_at_a_commit_counts_from_the_horizon_distance_after_it() {
    let (denied_first, denied_later, coin) =
        setup_validators_with_a_shared_regulated_coin(RegulatedCoinOptions::default()).await;
    let transfer = coin.transfer_owned_coin(
        &denied_first,
        coin.other,
        &coin.other_key,
        &coin.other_gas,
        &coin.other_coin,
    );
    coin.call_deny_list(&denied_first, "deny_list_v1_add", Some(coin.other), 11);

    let on_denied = denied_first
        .validate_at(14, std::slice::from_ref(&transfer))
        .await;
    let on_not_denied = denied_later
        .validate_at(14, std::slice::from_ref(&transfer))
        .await;
    assert_eq!(on_not_denied.kept, vec![*transfer.digest()]);
    assert_eq!(on_denied, on_not_denied);

    coin.call_deny_list(&denied_later, "deny_list_v1_add", Some(coin.other), 11);
    for s in [&denied_first, &denied_later] {
        let at_15 = s.validate_at(15, std::slice::from_ref(&transfer)).await;
        assert_eq!(at_15.dropped, coin.address_denied(&transfer, coin.other));
    }
}

/// The coin type is paused for every address at commit 11, on one validator
/// only. A transfer of a regulated coin by an address that is not denied is
/// kept at commit 14 (horizon 10) on both validators, and dropped for the
/// pause at commit 15 (horizon 11) once both have executed it.
#[tokio::test]
async fn global_pause_at_a_commit_counts_from_the_horizon_distance_after_it() {
    let (paused_first, paused_later, coin) =
        setup_validators_with_a_shared_regulated_coin(RegulatedCoinOptions::default()).await;
    let transfer = coin.transfer_owned_coin(
        &paused_first,
        coin.other,
        &coin.other_key,
        &coin.other_gas,
        &coin.other_coin,
    );
    coin.call_deny_list(&paused_first, "deny_list_v1_enable_global_pause", None, 11);

    let on_paused = paused_first
        .validate_at(14, std::slice::from_ref(&transfer))
        .await;
    let on_not_paused = paused_later
        .validate_at(14, std::slice::from_ref(&transfer))
        .await;
    assert_eq!(on_not_paused.kept, vec![*transfer.digest()]);
    assert_eq!(on_paused, on_not_paused);

    coin.call_deny_list(&paused_later, "deny_list_v1_enable_global_pause", None, 11);
    let paused: Vec<(TransactionDigest, IotaError)> = vec![(
        *transfer.digest(),
        UserInputError::CoinTypeGlobalPause {
            coin_type: coin.coin_type.to_canonical_string(false),
        }
        .into(),
    )];
    for s in [&paused_first, &paused_later] {
        let at_15 = s.validate_at(15, std::slice::from_ref(&transfer)).await;
        assert_eq!(at_15.dropped, paused);
    }
}

/// An address is denied at commit 11 and its entry removed at commit 12 on
/// one validator, which creates the entry's dynamic field and deletes it
/// again. Rows above the horizon from several commits, as a replay after a
/// restart can find them, must not change the result: a transfer of the
/// address's regulated coin is kept at commit 14 (horizon 10) on both
/// validators. At commit 15 (horizon 11) the denial counts, and at commit 16
/// (horizon 12) the deleted entry reads as not denied again.
#[tokio::test]
async fn deny_entry_added_and_removed_above_the_horizon_gets_one_verdict_on_every_validator() {
    let (changed, unchanged, coin) =
        setup_validators_with_a_shared_regulated_coin(RegulatedCoinOptions::default()).await;
    let transfer = coin.transfer_owned_coin(
        &changed,
        coin.other,
        &coin.other_key,
        &coin.other_gas,
        &coin.other_coin,
    );
    coin.call_deny_list(&changed, "deny_list_v1_add", Some(coin.other), 11);
    coin.call_deny_list(&changed, "deny_list_v1_remove", Some(coin.other), 12);
    let config = get_per_type_coin_deny_list_v1(
        &coin.coin_type.to_canonical_string(false),
        &changed.authority.get_object_store(),
    )
    .expect("the deny list holds a config for the coin type");
    assert!(
        matches!(
            get_dynamic_field_object_from_store(
                &changed.authority.get_object_store(),
                *config.id.object_id(),
                &AddressKey(coin.other),
            ),
            Err(IotaError::DynamicFieldNotExists { .. })
        ),
        "the removal must delete the entry's dynamic field"
    );

    let on_changed = changed
        .validate_at(14, std::slice::from_ref(&transfer))
        .await;
    let on_unchanged = unchanged
        .validate_at(14, std::slice::from_ref(&transfer))
        .await;
    assert_eq!(on_unchanged.kept, vec![*transfer.digest()]);
    assert_eq!(on_changed, on_unchanged);

    let at_15 = changed
        .validate_at(15, std::slice::from_ref(&transfer))
        .await;
    assert_eq!(at_15.dropped, coin.address_denied(&transfer, coin.other));
    let at_16 = changed
        .validate_at(16, std::slice::from_ref(&transfer))
        .await;
    assert_eq!(at_16.kept, vec![*transfer.digest()]);
}

/// An address is denied at commit 9 on both validators. On one of them its
/// entry is then removed at commit 11, which deletes the entry's dynamic
/// field, and added again at commit 12, which creates it anew under the same
/// id. At commit 14 (horizon 10) the read must walk past the new field and
/// the deletion to the entry of commit 9, so a transfer of the address's
/// regulated coin is dropped on both validators. At commit 15 (horizon 11)
/// the deleted entry reads as not denied, and at commit 16 (horizon 12) the
/// new one denies again.
#[tokio::test]
async fn deny_entry_deleted_and_created_again_above_the_horizon_gets_one_verdict() {
    run_deny_entry_deleted_and_created_again(false).await;
}

/// As [`deny_entry_deleted_and_created_again_above_the_horizon_gets_one_verdict`],
/// with every row written to the table and none left in memory, so the walk
/// reads the table.
#[tokio::test]
async fn deny_entry_deleted_and_created_again_gets_one_verdict_from_durable_rows() {
    run_deny_entry_deleted_and_created_again(true).await;
}

/// The scenario of
/// [`deny_entry_deleted_and_created_again_above_the_horizon_gets_one_verdict`].
/// With `durable`, the rows of the deny-list changes are written to the
/// table and removed from memory before any validation.
async fn run_deny_entry_deleted_and_created_again(durable: bool) {
    let (changed, unchanged, coin) =
        setup_validators_with_a_shared_regulated_coin(RegulatedCoinOptions::default()).await;
    let transfer = coin.transfer_owned_coin(
        &changed,
        coin.other,
        &coin.other_key,
        &coin.other_gas,
        &coin.other_coin,
    );
    let mut changes = Vec::new();
    for s in [&changed, &unchanged] {
        changes.push(vec![coin.call_deny_list(
            s,
            "deny_list_v1_add",
            Some(coin.other),
            9,
        )]);
    }
    changes[0].push(coin.call_deny_list(&changed, "deny_list_v1_remove", Some(coin.other), 11));
    changes[0].push(coin.call_deny_list(&changed, "deny_list_v1_add", Some(coin.other), 12));
    if durable {
        for (s, effects) in [&changed, &unchanged].into_iter().zip(&changes) {
            s.epoch_store
                .persist_checkpoint_bookkeeping(effects)
                .unwrap();
            assert_eq!(
                s.epoch_store
                    .handler_object_state_for_testing()
                    .overlay_size_for_testing(),
                0
            );
        }
    }

    let on_changed = changed
        .validate_at(14, std::slice::from_ref(&transfer))
        .await;
    let on_unchanged = unchanged
        .validate_at(14, std::slice::from_ref(&transfer))
        .await;
    assert_eq!(
        on_unchanged.dropped,
        coin.address_denied(&transfer, coin.other)
    );
    assert_eq!(on_changed, on_unchanged);

    let at_15 = changed
        .validate_at(15, std::slice::from_ref(&transfer))
        .await;
    assert_eq!(at_15.kept, vec![*transfer.digest()]);
    let at_16 = changed
        .validate_at(16, std::slice::from_ref(&transfer))
        .await;
    assert_eq!(at_16.dropped, coin.address_denied(&transfer, coin.other));
}

/// A regulated coin frozen at commit 5 is an immutable input of a call that
/// reads its value. At commit 12 the deny check at the commit sees its type
/// from the bytes the owned reader loads: the denied address's call is
/// dropped, and the same call by an address that is not denied is kept.
#[tokio::test]
async fn frozen_regulated_coin_input_is_checked_at_the_commit() {
    let (s, _, coin) =
        setup_validators_with_a_shared_regulated_coin(RegulatedCoinOptions::default()).await;
    let coin_object_type = TypeTag::Struct(Box::new(StructTag::new(
        ObjectId::FRAMEWORK,
        Identifier::from_static("coin"),
        Identifier::from_static("Coin"),
        vec![coin.coin_type.clone()],
    )));
    let freeze = TestTransactionBuilder::new(coin.other, s.latest_ref(&coin.other_gas), s.rgp)
        .move_call(
            ObjectId::FRAMEWORK,
            "transfer",
            "public_freeze_object",
            vec![CallArg::ImmutableOrOwned(s.latest_ref(&coin.other_coin))],
        )
        .with_type_args(vec![coin_object_type])
        .build();
    let freeze = s
        .epoch_store
        .verify_transaction(to_sender_signed_transaction(freeze, &coin.other_key))
        .unwrap();
    let freeze_effects = s.execute_with_assigned_commit(vec![freeze], 5).remove(0);
    assert!(
        freeze_effects.status().is_success(),
        "{:?}",
        freeze_effects.status()
    );
    let frozen = s.latest_ref(&coin.other_coin);

    let read_value = |sender: Address, sender_key: &AccountPrivateKey, gas_id: &ObjectId| {
        let tx = TestTransactionBuilder::new(sender, s.latest_ref(gas_id), s.rgp)
            .move_call(
                ObjectId::FRAMEWORK,
                "coin",
                "value",
                vec![CallArg::ImmutableOrOwned(frozen)],
            )
            .with_type_args(vec![coin.coin_type.clone()])
            .build();
        s.epoch_store
            .verify_transaction(to_sender_signed_transaction(tx, sender_key))
            .unwrap()
    };
    let by_denied = read_value(coin.denied, &coin.denied_key, &coin.denied_gas);
    let by_other = read_value(coin.other, &coin.other_key, &coin.other_gas);

    let result = s
        .validate_at(12, &[by_denied.clone(), by_other.clone()])
        .await;
    assert_eq!(result.dropped, coin.address_denied(&by_denied, coin.denied));
    assert_eq!(result.kept, vec![*by_other.digest()]);
}

/// The read of an object as of the horizon, which the deny check at a commit
/// uses for the deny list's dynamic fields, on a dynamic-field child created
/// at commit 5 and deleted at commit 6. Below the horizon of the creation it
/// is absent, at it the created version, and from the deletion on absent
/// again. Once the deletion's checkpoint commits, the created version is read
/// from this epoch's bucket. Once its bytes are removed, the read that needs it
/// is a storage error, never an answer, and after the epoch ended it abandons
/// the commit.
#[tokio::test]
async fn read_at_the_horizon_follows_a_dynamic_field_child_and_needs_its_bytes() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;
    let (field_ref, committed, remove_effects) =
        consume_a_dynamic_field_child_without_a_lock(&s, &gas_id, sender, &sender_key);
    let read = |commit| match s
        .reader_at(commit)
        .read_dynamic_field(field_ref.object_id)
        .unwrap()
    {
        DynamicFieldAtHorizon::Live(object) => Some(object.object_ref()),
        DynamicFieldAtHorizon::Absent => None,
    };
    assert_eq!(read(8), None, "created above horizon 4");
    assert_eq!(read(9), Some(field_ref), "created at horizon 5");
    assert_eq!(read(10), None, "deleted at horizon 6");

    // The deletion's checkpoint commits: the created version leaves the live
    // state for this epoch's bucket, and the read finds it there.
    s.commit_outputs(&committed);
    s.authority.clear_execution_caches_for_testing();
    assert!(
        s.authority
            .get_object_cache_reader()
            .get_object_by_key(&field_ref.object_id, field_ref.version)
            .is_none()
    );
    assert_eq!(read(9), Some(field_ref), "read from this epoch's bucket");

    s.prune_versions_consumed_by(&committed, &remove_effects)
        .await;
    let result = s.reader_at(9).read_dynamic_field(field_ref.object_id);
    assert!(
        matches!(result, Err(IotaError::Storage(_))),
        "unexpected result: {result:?}"
    );

    // Once the epoch has ended, the same miss abandons the commit instead.
    s.epoch_store.epoch_terminated().await;
    let result = s.reader_at(9).read_dynamic_field(field_ref.object_id);
    assert!(
        matches!(result, Err(IotaError::EpochEnded(epoch)) if epoch == s.epoch_store.epoch()),
        "unexpected result: {result:?}"
    );
}

/// A reader keeps its first read of a dynamic field for the rest of its
/// commit, live or absent. The second read does not load the bytes again:
/// once the bytes are removed the same reader still answers, and a new reader
/// at the same commit fails.
#[tokio::test]
async fn reader_keeps_its_first_dynamic_field_read_for_the_rest_of_its_commit() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;
    let (field_ref, committed, remove_effects) =
        consume_a_dynamic_field_child_without_a_lock(&s, &gas_id, sender, &sender_key);
    let reused = || s.reused_reads("dynamic_field");
    let live_ref = |field| match field {
        DynamicFieldAtHorizon::Live(object) => Some(object.object_ref()),
        DynamicFieldAtHorizon::Absent => None,
    };

    // Commit 8, horizon 4: created at 5, so absent.
    let absent_reader = s.reader_at(8);
    for _ in 0..2 {
        assert_eq!(
            absent_reader
                .read_dynamic_field(field_ref.object_id)
                .unwrap(),
            DynamicFieldAtHorizon::Absent
        );
    }
    assert_eq!(reused(), 1);

    // Commit 9, horizon 5: live, past the deletion's row above the horizon.
    let reader = s.reader_at(9);
    assert_eq!(
        live_ref(reader.read_dynamic_field(field_ref.object_id).unwrap()),
        Some(field_ref)
    );
    assert_eq!(reused(), 1);

    s.commit_outputs(&committed);
    s.authority.clear_execution_caches_for_testing();
    s.prune_versions_consumed_by(&committed, &remove_effects)
        .await;
    assert_eq!(
        live_ref(reader.read_dynamic_field(field_ref.object_id).unwrap()),
        Some(field_ref)
    );
    assert_eq!(reused(), 2);
    let result = s.reader_at(9).read_dynamic_field(field_ref.object_id);
    assert!(
        matches!(result, Err(IotaError::Storage(_))),
        "unexpected result: {result:?}"
    );
    assert_eq!(reused(), 2);
}

/// Callers on several threads that read one dynamic field through one reader
/// share one read: one of them reads and the others wait for its result.
#[tokio::test]
async fn threads_reading_one_dynamic_field_share_one_read() {
    const THREADS: u64 = 8;

    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![Object::with_id_owner_for_testing(gas_id, sender)],
        true,
    )
    .await;
    let (field_ref, ..) =
        consume_a_dynamic_field_child_without_a_lock(&s, &gas_id, sender, &sender_key);

    // Commit 9, horizon 5: live, past the deletion's row above the horizon.
    let reader = s.reader_at(9);
    std::thread::scope(|scope| {
        let callers: Vec<_> = (0..THREADS)
            .map(|_| scope.spawn(|| reader.read_dynamic_field(field_ref.object_id).unwrap()))
            .collect();
        for caller in callers {
            match caller.join().unwrap() {
                DynamicFieldAtHorizon::Live(object) => assert_eq!(object.object_ref(), field_ref),
                DynamicFieldAtHorizon::Absent => panic!("the field is live at the horizon"),
            }
        }
    });
    assert_eq!(s.reused_reads("dynamic_field"), THREADS - 1);
}

/// A commit with two transactions sent by one account reads the account's
/// authenticator function field once and answers the second transaction from
/// that read.
#[tokio::test]
async fn commit_with_two_transactions_of_one_account_reads_its_field_once() {
    let (sponsor, sponsor_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_ids = [ObjectId::random(), ObjectId::random()];
    let account_id = ObjectId::random();
    // The plain-signature rule also probes the sponsor's address, a read of
    // its own; it is off so that only the account's field read is counted.
    let s = setup_handler_object_state_with_config_guard(
        vec![
            Object::with_id_owner_for_testing(gas_ids[0], sponsor),
            Object::with_id_owner_for_testing(gas_ids[1], sponsor),
            shared_object_with_id(account_id),
        ],
        Some(ProtocolConfig::apply_overrides_for_testing(
            |_, mut config| {
                config.set_reject_plain_signatures_for_explicit_accounts_for_testing(false);
                config
            },
        )),
    )
    .await;
    s.authority
        .insert_genesis_object(authenticator_function_ref_field(
            account_id,
            s.package_id,
            Version::OBJECT_START,
        ));
    let transactions = gas_ids.map(|gas_id| {
        s.build_account_authenticated_transfer(
            account_authenticator(account_id),
            &gas_id,
            sponsor,
            &sponsor_key,
        )
    });

    let result = s.validate_at(12, &transactions).await;

    assert!(result.dropped.is_empty(), "{:?}", result.dropped);
    assert_eq!(result.kept.len(), 2);
    assert_eq!(s.reused_reads("dynamic_field"), 1);
}

/// A commit with two transfers of one regulated coin type, by a denied
/// address and by another one, reads the type's deny list config and its
/// global pause setting once each and answers the second transfer from those
/// reads. The denial still drops only the denied address's transfer.
#[tokio::test]
async fn commit_with_two_transfers_of_one_regulated_coin_type_reads_its_deny_list_once() {
    let (s, _, coin) =
        setup_validators_with_a_shared_regulated_coin(RegulatedCoinOptions::default()).await;
    let denied_transfer = coin.transfer_owned_coin(
        &s,
        coin.denied,
        &coin.denied_key,
        &coin.denied_gas,
        &coin.denied_coin,
    );
    let other_transfer = coin.transfer_owned_coin(
        &s,
        coin.other,
        &coin.other_key,
        &coin.other_gas,
        &coin.other_coin,
    );
    let reused_before = s.reused_reads("dynamic_field");

    let result = s
        .validate_at(14, &[denied_transfer.clone(), other_transfer.clone()])
        .await;

    assert_eq!(
        result.dropped,
        coin.address_denied(&denied_transfer, coin.denied)
    );
    assert_eq!(result.kept, vec![*other_transfer.digest()]);
    assert_eq!(s.reused_reads("dynamic_field") - reused_before, 2);
}

/// A reader keeps the coin type of a shared input for the rest of its commit.
/// The second read does not read the store: once the coin is deleted above
/// the horizon and the version the deletion replaced is removed, the same
/// reader still answers, and a new reader at the same commit fails.
#[tokio::test]
async fn reader_keeps_the_coin_type_of_a_shared_input_for_the_rest_of_its_commit() {
    let (s, _, coin) =
        setup_validators_with_a_shared_regulated_coin(RegulatedCoinOptions::default()).await;
    let initial = initial_shared_version(&s.authority.get_object(&coin.coin_id).unwrap().owner);
    let coin_type = Some(coin.coin_type.to_canonical_string(false));
    let reused_before = s.reused_reads("shared_coin_type");
    let reused = || s.reused_reads("shared_coin_type") - reused_before;

    // Commit 14, horizon 10. The type read is for an input the reader kept.
    let reader = s.reader_at(14);
    assert_eq!(
        reader.read_shared(coin.coin_id, initial).unwrap(),
        SharedVerdict::Exists
    );
    assert_eq!(
        reader.read_shared_coin_type(coin.coin_id, initial).unwrap(),
        coin_type
    );
    assert_eq!(reused(), 0);

    let deletion_effects = coin.delete_shared_coin(&s, 11);
    s.commit_all_executed_and_clear_caches();
    s.prune_versions_consumed_by(&[], &deletion_effects).await;
    assert_eq!(
        reader.read_shared_coin_type(coin.coin_id, initial).unwrap(),
        coin_type
    );
    assert_eq!(reused(), 1);
    // A new reader still keeps the input, deleted above the horizon, and
    // then fails on its type.
    let new_reader = s.reader_at(14);
    assert_deleted_by(
        new_reader.read_shared(coin.coin_id, initial).unwrap(),
        &deletion_effects,
    );
    let result = new_reader.read_shared_coin_type(coin.coin_id, initial);
    assert!(
        matches!(result, Err(IotaError::Storage(_))),
        "unexpected result: {result:?}"
    );
    assert_eq!(reused(), 1);
}

/// A commit with two transactions that name one shared coin reads the coin's
/// type once and answers the second transaction from that read. The deny list
/// still drops only the denied sender's transaction.
#[tokio::test]
async fn commit_with_two_transactions_on_one_shared_coin_reads_its_type_once() {
    let (s, _, coin) =
        setup_validators_with_a_shared_regulated_coin(RegulatedCoinOptions::default()).await;
    let by_denied = coin.destroy_shared_coin(&s, coin.denied, &coin.denied_key, &coin.denied_gas);
    let by_other = coin.destroy_shared_coin(&s, coin.other, &coin.other_key, &coin.other_gas);
    let reused_before = s.reused_reads("shared_coin_type");

    let result = s
        .validate_at(14, &[by_denied.clone(), by_other.clone()])
        .await;

    assert_eq!(result.dropped, coin.address_denied(&by_denied, coin.denied));
    assert_eq!(result.kept, vec![*by_other.digest()]);
    assert_eq!(s.reused_reads("shared_coin_type") - reused_before, 1);
}

/// On a validator that lost the bytes of a dynamic-field child consumed
/// without a lock, removed by hand, commit 9 holds a
/// transfer of the child paid with a gas coin named at a wrong digest. Every
/// read of the transaction's inputs at commit 9 must end the same way: each
/// validator reads them once, and a storage error saves no decision, so the
/// commit is validated afresh after the restart that follows the handler's
/// panic.
#[tokio::test]
async fn reads_of_a_transaction_with_a_pruned_child_and_a_dropping_input_end_alike() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let consumer_gas = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(gas_id, sender),
            Object::with_id_owner_for_testing(consumer_gas, sender),
        ],
        true,
    )
    .await;
    let (field_ref, committed, remove_effects) =
        consume_a_dynamic_field_child_without_a_lock(&s, &gas_id, sender, &sender_key);
    s.prune_versions_consumed_by(&committed, &remove_effects)
        .await;

    let mut wrong_gas = s.latest_ref(&consumer_gas);
    wrong_gas.digest = ObjectDigest::random();
    let consumer = s
        .epoch_store
        .verify_transaction(make_transfer_object_transaction(
            field_ref,
            wrong_gas,
            sender,
            &sender_key,
            Address::random(),
            s.rgp,
        ))
        .unwrap();
    let loader = TransactionInputLoader::new(s.authority.get_object_cache_reader().clone());
    let outcomes: std::collections::BTreeSet<&str> = (0..32)
        .map(|_| {
            let kinds = consumer
                .collect_all_input_object_kind_for_reading(
                    s.epoch_store
                        .protocol_config()
                        .move_authenticator_type_arg_packages_as_inputs(),
                )
                .unwrap();
            match loader.read_objects_at_commit(&s.reader_at(9), &kinds) {
                Ok(InputResolution::Drop(..)) => "drop",
                Ok(InputResolution::Missing(..)) => "missing",
                Ok(InputResolution::Loaded(_)) => "loaded",
                Err(_) => "error",
            }
        })
        .collect();
    assert_eq!(outcomes.len(), 1, "reads ended differently: {outcomes:?}");
}

/// Commits the outputs of the transactions `executed` names, oldest first,
/// and executes them as one certified checkpoint, then expires every
/// historic objects bucket but the running epoch's, as a node with
/// aggressive retention does.
async fn execute_checkpoint_and_prune(s: &HandlerObjectStateSetup, executed: &[TransactionDigest]) {
    use iota_types::{
        base_types::ExecutionData,
        messages_checkpoint::{FullCheckpointContents, VerifiedCheckpointContents},
    };

    let epoch = s.epoch_store.epoch();
    let cache = s.authority.get_transaction_cache_reader();
    let executed_data: Vec<_> = executed
        .iter()
        .map(|digest| {
            let tx = cache
                .get_transaction_block(digest)
                .expect("the executed transaction is stored");
            let effects = cache
                .get_executed_effects(digest)
                .expect("the executed transaction has effects");
            ((*tx).clone(), effects)
        })
        .collect();
    s.commit_outputs(executed);

    let consumed: Vec<ObjectId> = executed_data
        .iter()
        .flat_map(|(_, effects)| effects.modified_at_versions())
        .map(|modified| *modified.object_id())
        .collect();
    let contents = VerifiedCheckpointContents::new_unchecked(
        FullCheckpointContents::new_with_causally_ordered_transactions(
            executed_data
                .into_iter()
                .map(|(tx, effects)| ExecutionData::new(tx.into_inner(), effects)),
        ),
    );
    let checkpoint_store = s.authority.get_checkpoint_store();
    let fixture = iota_swarm_config::test_utils::CommitteeFixture::generate(
        rand::rand_core::UnwrapErr(rand::rngs::SysRng),
        epoch,
        4,
    );
    // The fixture's first checkpoint is its own genesis; checkpoint 1 holds
    // the transactions.
    let (checkpoints, ..) = fixture.make_checkpoints(2, None, || contents.clone());
    for checkpoint in checkpoints.iter().skip(1) {
        checkpoint_store
            .insert_certified_checkpoint(checkpoint)
            .unwrap();
        checkpoint_store
            .insert_checkpoint_contents(checkpoint, contents.clone().into_checkpoint_contents())
            .unwrap();
        checkpoint_store
            .update_highest_executed_checkpoint(checkpoint)
            .unwrap();
    }

    s.authority
        .database_for_testing()
        .expire_historic_objects_and_compact_for_testing();
    for id in consumed {
        s.authority.evict_committed_object_for_testing(&id);
    }
}

/// A version that a commit above the horizon consumed stays readable after
/// the consumer's checkpoint commits and the earlier buckets expire. A validation at a
/// commit less than the horizon distance above the consumer, or a replay of
/// any commit after a restart before the consumer's output flushed, keeps the
/// version, as on a validator that has not executed the consumer, and needs
/// its bytes for that.
#[tokio::test]
async fn pruning_keeps_a_version_a_later_validation_can_still_keep() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let obj_id = ObjectId::random();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(obj_id, sender),
            Object::with_id_owner_for_testing(gas_id, sender),
        ],
        true,
    )
    .await;

    let produce = s.build_transfer(&obj_id, &gas_id, sender, &sender_key, sender);
    let produce_digest = *produce.digest();
    s.execute_with_assigned_commit(vec![produce], 3);
    let named = s.latest_ref(&obj_id);
    let consume = s.build_transfer(&obj_id, &gas_id, sender, &sender_key, Address::random());
    let consume_digest = *consume.digest();
    s.execute_with_assigned_commit(vec![consume], 5);
    // Commit 8 reads at horizon 4: the row of commit 3 keeps the version, the
    // consumer's row at commit 5 is above the horizon.
    assert_keeps(s.read_owned(8, named), named);

    execute_checkpoint_and_prune(&s, &[produce_digest, consume_digest]).await;
    let key = ObjectKey(named.object_id, named.version);
    assert!(
        s.authority
            .get_object_cache_reader()
            .get_object_by_key(&key.0, key.1)
            .is_none(),
        "the commit moves the consumed version out of the live state"
    );
    assert!(
        s.authority
            .get_historic_objects()
            .get_in_epoch(s.epoch_store.epoch(), &key)
            .unwrap()
            .is_some(),
        "into this epoch's bucket"
    );
    assert_keeps(s.read_owned(8, named), named);
}

/// A dynamic-field child is consumed through its parent at commit 6 without a
/// lock, and commit 9 (horizon 5) holds a transaction naming the consumed
/// version as an owned input. On one validator the checkpoint of those
/// commits executes and the earlier buckets expire, the other has not
/// executed it. The child's bytes stay readable, and both reach the same
/// result: they drop the
/// transaction, since a child cannot be an owned input.
#[tokio::test]
async fn pruning_keeps_a_dynamic_field_child_a_later_validation_can_still_keep() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let consumer_gas = ObjectId::random();
    let genesis = vec![
        Object::with_id_owner_for_testing(gas_id, sender),
        Object::with_id_owner_for_testing(consumer_gas, sender),
    ];
    let pruning = setup_handler_object_state(genesis.clone(), true).await;
    let keeping = setup_handler_object_state(genesis, true).await;

    let (field_ref, committed, _) =
        consume_a_dynamic_field_child_without_a_lock(&pruning, &gas_id, sender, &sender_key);
    assert_eq!(
        consume_a_dynamic_field_child_without_a_lock(&keeping, &gas_id, sender, &sender_key).0,
        field_ref
    );
    execute_checkpoint_and_prune(&pruning, &committed).await;
    assert!(
        pruning
            .store_object(&field_ref.object_id, field_ref.version)
            .is_some()
    );

    let consumer = pruning
        .epoch_store
        .verify_transaction(make_transfer_object_transaction(
            field_ref,
            pruning.latest_ref(&consumer_gas),
            sender,
            &sender_key,
            Address::random(),
            pruning.rgp,
        ))
        .unwrap();
    let validate = |s: &HandlerObjectStateSetup| {
        let authority = s.authority.clone();
        let epoch_store = s.epoch_store.clone();
        let consumer = consumer.clone();
        async move {
            let mut transactions = vec![make_user_tx_v1_verified(consumer)];
            // Only the decision must agree across validators; the error a
            // drop carries is local.
            post_consensus_validation::validate_and_resolve_conflicts(
                &authority,
                &epoch_store,
                9,
                &Default::default(),
                &mut transactions,
            )
            .await
            .map(|(dropped, locks, _)| {
                let kept: Vec<_> = transactions
                    .iter()
                    .map(|tx| tx.0.transaction.user_transaction_digest().unwrap())
                    .collect();
                let dropped: Vec<_> = dropped.into_iter().map(|(digest, _, _)| digest).collect();
                (kept, dropped, locks)
            })
        }
    };
    let on_keeping = validate(&keeping).await.unwrap();
    assert_eq!(on_keeping.1.len(), 1, "{on_keeping:?}");
    assert_eq!(validate(&pruning).await, Ok(on_keeping));
}

// ---------------------------------------------------------------------------
// P-COOL deterministic-validation checks at a commit: an account at the
// horizon
// ---------------------------------------------------------------------------

/// A [`HandlerObjectStateSetup`] with the flags on and the `move_authenticator` test
/// package published at commit 1, so a test makes its accounts with
/// transactions at the commits it picks. `sender` pays for those transactions
/// with `setup_gas_id`, and sponsors the account's own transactions with
/// `sponsor_gas_id`, which nothing else uses, so it is visible at every
/// horizon.
struct AccountSetup {
    s: HandlerObjectStateSetup,
    package_id: ObjectId,
    sender: Address,
    sender_key: AccountPrivateKey,
    setup_gas_id: ObjectId,
    sponsor_gas_id: ObjectId,
}

impl AccountSetup {
    async fn new() -> Self {
        let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
        let (setup_gas_id, sponsor_gas_id) = (ObjectId::random(), ObjectId::random());
        let s = setup_handler_object_state(
            vec![
                Object::with_id_owner_for_testing(setup_gas_id, sender),
                Object::with_id_owner_for_testing(sponsor_gas_id, sender),
            ],
            true,
        )
        .await;
        let mut setup = Self {
            s,
            package_id: ObjectId::ZERO,
            sender,
            sender_key,
            setup_gas_id,
            sponsor_gas_id,
        };

        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/unit_tests/data/move_authenticator");
        let package = iota_move_build::BuildConfig::new_for_testing()
            .build(&path)
            .unwrap();
        let mut builder = ProgrammableTransactionBuilder::new();
        let upgrade_cap = builder.publish_upgradeable(
            package.get_package_bytes(false),
            package.get_dependency_storage_package_ids(),
        );
        builder.transfer_arg(sender, upgrade_cap);
        let effects = setup.execute(builder.finish(), TEST_ONLY_GAS_UNIT_FOR_PUBLISH, 1);
        setup.package_id = effects
            .created()
            .into_iter()
            .map(|created| *created.reference().object_id())
            .find(|id| setup.s.authority.get_object(id).unwrap().is_package())
            .expect("the publish must create the package");
        setup
    }

    /// Executes `pt`, sent and paid for by `sender` with `gas_units`, as the
    /// only transaction of commit `index`, with its shared inputs at versions
    /// assigned directly. It must succeed.
    fn execute(
        &self,
        pt: ProgrammableTransaction,
        gas_units: u64,
        index: CommitIndex,
    ) -> TransactionEffects {
        let has_shared_input = pt
            .inputs
            .iter()
            .any(|input| matches!(input, CallArg::Shared(_)));
        let tx = Transaction::new_programmable(
            self.sender,
            vec![self.s.latest_ref(&self.setup_gas_id)],
            pt,
            gas_units * self.s.rgp,
            self.s.rgp,
        );
        let tx = self
            .s
            .epoch_store
            .verify_transaction(to_sender_signed_transaction(tx, &self.sender_key))
            .unwrap();
        if has_shared_input {
            self.s
                .epoch_store
                .assign_commit_to_transactions(index, vec![TransactionKey::Digest(*tx.digest())]);
            self.s.execute_with_assigned_shared_versions(tx)
        } else {
            self.s
                .execute_with_assigned_commit(vec![tx], index)
                .remove(0)
        }
    }

    /// A call of `test_account::function` with `type_arguments` and
    /// `arguments`, executed as [`Self::execute`] does.
    fn call(
        &self,
        function: &'static str,
        type_arguments: Vec<TypeTag>,
        arguments: Vec<CallArg>,
        index: CommitIndex,
    ) -> TransactionEffects {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder
            .move_call(
                self.package_id,
                Identifier::from_static("test_account"),
                Identifier::from_static(function),
                type_arguments,
                arguments,
            )
            .unwrap();
        self.execute(
            builder.finish(),
            TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS * 10,
            index,
        )
    }

    /// Shares a `TestAccount` that is not an account yet at commit `index`,
    /// and returns its id.
    fn create_delayed(&self, index: CommitIndex) -> ObjectId {
        let effects = self.call("create_delayed", vec![], vec![], index);
        *effects
            .created()
            .into_iter()
            .find(|created| matches!(created.owner(), Owner::Shared(_)))
            .expect("the call must share the object")
            .reference()
            .object_id()
    }

    /// The argument naming `id` as a shared input.
    fn shared(&self, id: &ObjectId, mutable: bool) -> CallArg {
        let initial_shared_version =
            initial_shared_version(&self.s.authority.get_object(id).unwrap().owner);
        CallArg::Shared(SharedObjectReference::new(
            *id,
            initial_shared_version,
            mutable,
        ))
    }

    /// Sends a new coin to `account`'s address at commit `index`, and returns
    /// its reference.
    fn send_coin(&self, account: &ObjectId, index: CommitIndex) -> ObjectReference {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.transfer_iota((*account).into(), Some(1_000));
        let effects = self.execute(
            builder.finish(),
            TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS,
            index,
        );
        *effects
            .created()
            .into_iter()
            .find(|created| created.owner() == &Owner::Address((*account).into()))
            .expect("the transfer must create the coin")
            .reference()
    }

    /// Receives `coin` through `account` at commit `index` and sends it back
    /// to the account's address, which moves it to a new version.
    fn send_back(&self, account: &ObjectId, coin: ObjectReference, index: CommitIndex) {
        self.call(
            "send_back",
            vec![TypeTag::from_str("0x2::coin::Coin<0x2::iota::IOTA>").unwrap()],
            vec![self.shared(account, true), CallArg::Receiving(coin)],
            index,
        );
    }

    /// Adds to `builder` the authenticator function reference of a
    /// `TestAccount` whose authenticator always passes.
    fn passing_function_ref(&self, builder: &mut ProgrammableTransactionBuilder) -> Argument {
        let metadata_ref = self
            .s
            .latest_ref(&move_package::derive_package_metadata_id(self.package_id));
        let account_type =
            TypeTag::from_str(&format!("{}::test_account::TestAccount", self.package_id)).unwrap();
        let arguments = vec![
            builder
                .obj(CallArg::ImmutableOrOwned(metadata_ref))
                .unwrap(),
            builder.pure("test_account").unwrap(),
            builder.pure("authenticate_pass").unwrap(),
        ];
        builder.programmable_move_call(
            IOTA_FRAMEWORK_PACKAGE_ID,
            Identifier::from_static("authenticator_function"),
            Identifier::from_static("create_auth_function_ref_v1"),
            vec![account_type],
            arguments,
        )
    }

    /// Creates an account whose authenticator always passes, with its
    /// authenticator function field in the same transaction, at commit
    /// `index`, and returns its id.
    fn create_account(&self, index: CommitIndex) -> ObjectId {
        let mut builder = ProgrammableTransactionBuilder::new();
        let function_ref = self.passing_function_ref(&mut builder);
        builder.programmable_move_call(
            self.package_id,
            Identifier::from_static("test_account"),
            Identifier::from_static("create"),
            vec![],
            vec![function_ref],
        );
        let effects = self.execute(
            builder.finish(),
            TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS * 10,
            index,
        );
        *effects
            .created()
            .into_iter()
            .find(|created| matches!(created.owner(), Owner::Shared(_)))
            .expect("the call must share the account")
            .reference()
            .object_id()
    }

    /// Makes `account` an account whose authenticator always passes, at
    /// commit `index`.
    fn make_account(&self, account: &ObjectId, index: CommitIndex) {
        let mut builder = ProgrammableTransactionBuilder::new();
        let function_ref = self.passing_function_ref(&mut builder);
        let account = builder.obj(self.shared(account, true)).unwrap();
        builder.programmable_move_call(
            self.package_id,
            Identifier::from_static("test_account"),
            Identifier::from_static("make_account"),
            vec![],
            vec![account, function_ref],
        );
        self.execute(
            builder.finish(),
            TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS * 10,
            index,
        );
    }

    /// A transaction of `account` that names no owned input of its own: it
    /// pays 1 nano out of the gas coin `sender` sponsors with
    /// `sponsor_gas_id`.
    fn account_payment(&self, account: &ObjectId) -> VerifiedTransaction {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.transfer_iota(Address::ZERO, Some(1));
        self.sponsored_account_call(account, builder.finish())
    }

    /// The account's transfer of `object`, authenticated by `account` and
    /// sponsored by `sender` with `sponsor_gas_id`.
    fn account_transfer(&self, account: &ObjectId, object: ObjectReference) -> VerifiedTransaction {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.transfer_object(Address::ZERO, object).unwrap();
        self.sponsored_account_call(account, builder.finish())
    }

    /// `pt` sent by `account`, authenticated by its Move authenticator and
    /// sponsored by `sender` with `sponsor_gas_id`.
    fn sponsored_account_call(
        &self,
        account: &ObjectId,
        pt: ProgrammableTransaction,
    ) -> VerifiedTransaction {
        let tx = Transaction::new_programmable_allow_sponsor(
            (*account).into(),
            vec![self.s.latest_ref(&self.sponsor_gas_id)],
            pt,
            TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS * self.s.rgp,
            self.s.rgp,
            self.sender,
        );
        let CallArg::Shared(account) = self.shared(account, false) else {
            unreachable!()
        };
        let tx =
            to_sender_signed_transaction_with_optional_sponsor(
                tx,
                UserSignature::MoveAuthenticator(MoveAuthenticator::from(
                    MoveAuthenticatorV1::new(vec![], vec![], account),
                )),
                Some(&self.sender_key),
            );
        self.s.epoch_store.verify_transaction(tx).unwrap()
    }

    /// Post-consensus validation of `tx` at commit `index`.
    fn validate(&self, tx: &VerifiedTransaction, index: CommitIndex) -> ValidationAtCommit {
        self.s
            .authority
            .handle_transaction_validation_checks_at_commit(
                &self.s.reader_at(index),
                tx,
                &self.s.epoch_store,
                &self.s.authority.config.transaction_deny_config,
                VerifierLimitsSource::ProtocolConfig,
                false,
            )
            .unwrap()
    }
}

/// An object made an account at a commit is reported missing as the account
/// of a transaction until the horizon reaches that commit, and the
/// transaction is kept from then on. Every validator has executed the commits
/// up to the horizon, so all of them reach the same result.
#[tokio::test]
async fn validation_at_commit_reports_an_account_missing_until_it_is_one_at_the_horizon() {
    let a = AccountSetup::new().await;
    let distance = a.s.horizon_distance();
    let account = a.create_delayed(2);
    let coin = a.send_coin(&account, 3);
    let made_account_at = 5;
    a.make_account(&account, made_account_at);
    let tx = a.account_transfer(&account, coin);

    match a.validate(&tx, made_account_at + distance - 1) {
        ValidationAtCommit::Missing(InputObjectKind::SharedMoveObject { id, .. }, reason) => {
            assert_eq!(id, account);
            assert_eq!(reason.kind(), MissingKind::AuthenticatorFunctionNotFound);
        }
        other => panic!("expected the account missing, got {other:?}"),
    }
    match a.validate(&tx, made_account_at + distance) {
        ValidationAtCommit::Keep(mut owned) => {
            owned.sort();
            let mut expected = vec![coin, a.s.latest_ref(&a.sponsor_gas_id)];
            expected.sort();
            assert_eq!(owned, expected);
        }
        other => panic!("expected keep, got {other:?}"),
    }
}

/// An account created with its authenticator function field in one
/// transaction waits for nothing more than any shared object does: it is
/// missing while its creation is above the horizon, for that reason, and
/// kept from the commit whose horizon reaches it.
#[tokio::test]
async fn validation_at_commit_keeps_a_new_account_as_soon_as_its_creation_is_at_the_horizon() {
    let a = AccountSetup::new().await;
    let distance = a.s.horizon_distance();
    let created_at = 3;
    let account = a.create_account(created_at);
    let tx = a.account_payment(&account);

    match a.validate(&tx, created_at + distance - 1) {
        ValidationAtCommit::Missing(InputObjectKind::SharedMoveObject { id, .. }, reason) => {
            assert_eq!(id, account);
            assert_eq!(reason.kind(), MissingKind::SharedCreationAboveHorizon);
        }
        other => panic!("expected the account's creation missing, got {other:?}"),
    }
    match a.validate(&tx, created_at + distance) {
        ValidationAtCommit::Keep(owned) => {
            assert_eq!(owned, vec![a.s.latest_ref(&a.sponsor_gas_id)])
        }
        other => panic!("expected keep, got {other:?}"),
    }
}

/// A receive through an object moves a coin sent to it to a new version
/// without a lock, and the object can only receive while it is not an
/// account. Once the object is an account, its transfer naming the coin's
/// received version is never kept: it is missing until the horizon reaches
/// the commit that made the object an account, and from then on the
/// receive's commit is at or below the horizon, so the received version drops
/// on every validator. Execution never meets the stale version.
#[tokio::test]
async fn validation_at_commit_never_keeps_an_account_input_a_receive_moved() {
    let a = AccountSetup::new().await;
    let distance = a.s.horizon_distance();
    let account = a.create_delayed(2);
    let coin = a.send_coin(&account, 3);
    let received_at = 4;
    a.send_back(&account, coin, received_at);
    let made_account_at = 6;
    a.make_account(&account, made_account_at);
    let tx = a.account_transfer(&account, coin);

    for index in received_at..made_account_at + distance {
        let verdict = a.validate(&tx, index);
        assert!(
            !matches!(verdict, ValidationAtCommit::Keep(_)),
            "the transfer must not be kept at commit {index}"
        );
    }
    match a.validate(&tx, made_account_at + distance) {
        ValidationAtCommit::Drop(InputObjectKind::ImmOrOwnedMoveObject(dropped), _) => {
            assert_eq!(dropped, coin)
        }
        other => panic!("expected the received version to drop, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Inputs read ahead of the validation loop
// ---------------------------------------------------------------------------

/// How many transactions `authority` read the inputs of so far, `ahead` of
/// the loop, `inline` in it, or `unused`.
fn input_reads(authority: &AuthorityState, when: &str) -> u64 {
    authority
        .metrics
        .consensus_handler_validation_input_reads
        .with_label_values(&[when])
        .get()
}

/// An authority validating with `readers` readers, holding one object owned
/// by `sender` for each of `ids`.
async fn authority_with_readers(
    readers: usize,
    sender: Address,
    ids: &[ObjectId],
) -> Arc<AuthorityState> {
    let authority = TestAuthorityBuilder::new()
        .with_post_consensus_validation_readers(readers.try_into().unwrap())
        .build()
        .await;
    for id in ids {
        authority.insert_genesis_object(Object::with_id_owner_for_testing(*id, sender));
    }
    publish_object_basics(authority).await.0
}

/// What validating `transactions` at commit 1 returns and keeps, in a form
/// two authorities can be compared by.
async fn validation_outcome(
    authority: &Arc<AuthorityState>,
    transactions: &[TransactionEnvelope],
) -> (
    Vec<(TransactionDigest, String, PostConsensusDecision)>,
    BTreeSet<(ObjectReference, TransactionDigest)>,
    Vec<TransactionDigest>,
    Vec<TransactionDigest>,
) {
    let epoch_store = authority.epoch_store_for_testing();
    let mut sequenced: Vec<_> = transactions
        .iter()
        .map(|transaction| {
            make_user_tx_v1_verified(epoch_store.verify_transaction(transaction.clone()).unwrap())
        })
        .collect();
    let (dropped, locks, user_tx_digests) =
        post_consensus_validation::validate_and_resolve_conflicts(
            authority,
            &epoch_store,
            1,
            &Default::default(),
            &mut sequenced,
        )
        .await
        .unwrap();
    (
        dropped
            .into_iter()
            .map(|(digest, error, decision)| (digest, format!("{error:?}"), decision))
            .collect(),
        locks.into_iter().collect(),
        user_tx_digests,
        sequenced
            .iter()
            .filter_map(|tx| tx.0.transaction.user_transaction_digest())
            .collect(),
    )
}

/// A commit with valid transfers, a transfer losing its object to an earlier
/// one, a duplicate, inputs the reader drops or does not see, and a transfer
/// whose gas coin an earlier dropped one names. Validated with four readers
/// and with one, it gives the same drops with the same errors and decisions,
/// the same locks and the same kept transactions in the same order.
#[tokio::test]
async fn reading_inputs_ahead_gives_the_decisions_of_reading_them_inline() {
    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let recipient = Address::random();
    let objects: Vec<_> = (0..8).map(|_| ObjectId::random()).collect();
    let gas: Vec<_> = (0..12).map(|_| ObjectId::random()).collect();
    let ids: Vec<_> = objects.iter().chain(&gas).copied().collect();

    let ahead = authority_with_readers(4, sender, &ids).await;
    let inline = authority_with_readers(1, sender, &ids).await;
    let reference = |id: &ObjectId| {
        let reference = ahead.get_object(id).unwrap().object_ref();
        assert_eq!(reference, inline.get_object(id).unwrap().object_ref());
        reference
    };
    let rgp = ahead.reference_gas_price_for_testing().unwrap();
    assert_eq!(rgp, inline.reference_gas_price_for_testing().unwrap());
    let transfer = |object: ObjectReference, gas: &ObjectId| {
        make_transfer_object_transaction(
            object,
            reference(gas),
            sender,
            &sender_key,
            recipient,
            rgp,
        )
    };

    let mut transactions: Vec<_> = (0..6)
        .map(|i| transfer(reference(&objects[i]), &gas[i]))
        .collect();
    // Loses object 0 to the first transfer.
    transactions.push(transfer(reference(&objects[0]), &gas[6]));
    // The same copy as the second transfer.
    transactions.push(transactions[1].clone());
    // A version object 6 never had, and an object that does not exist.
    let fresh = reference(&objects[6]);
    transactions.push(transfer(
        ObjectReference::new(fresh.object_id, Version::from_u64(1_000), fresh.digest),
        &gas[8],
    ));
    transactions.push(transfer(random_object_ref(), &gas[9]));
    // Object 6 at its real version, and a transfer on the gas coin of the
    // transaction dropped for the version above.
    transactions.push(transfer(fresh, &gas[10]));
    transactions.push(transfer(reference(&objects[7]), &gas[8]));

    let with_readers = validation_outcome(&ahead, &transactions).await;
    let with_one = validation_outcome(&inline, &transactions).await;
    assert_eq!(with_readers, with_one);
    let (dropped, _, user_tx_digests, kept) = with_readers;
    assert_eq!(user_tx_digests.len(), 11);
    assert_eq!(dropped.len(), 3);
    assert_eq!(kept.len(), 8);

    // Every first copy is read ahead. The loop takes all but the read of the
    // loser of object 0, which it drops at the lock check.
    assert_eq!(input_reads(&ahead, "ahead"), 10);
    assert_eq!(input_reads(&ahead, "inline"), 0);
    assert_eq!(input_reads(&ahead, "unused"), 1);
    assert_eq!(input_reads(&inline, "ahead"), 0);
    assert_eq!(input_reads(&inline, "inline"), 10);
}

/// A transaction whose owned input a transaction of an earlier commit
/// locked is read ahead, then dropped by the lock check. Its read is counted
/// unused and the drop carries the lock conflict, not the read's result.
#[tokio::test]
async fn a_read_ahead_the_loop_does_not_take_is_dropped_unused() {
    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let object = ObjectId::random();
    let gas = [ObjectId::random(), ObjectId::random()];
    let authority = authority_with_readers(4, sender, &[object, gas[0], gas[1]]).await;
    let epoch_store = authority.epoch_store_for_testing();
    let reference = |id: &ObjectId| authority.get_object(id).unwrap().object_ref();
    let rgp = authority.reference_gas_price_for_testing().unwrap();
    let transfer = |gas: &ObjectId| {
        epoch_store
            .verify_transaction(make_transfer_object_transaction(
                reference(&object),
                reference(gas),
                sender,
                &sender_key,
                Address::random(),
                rgp,
            ))
            .unwrap()
    };
    let winner = transfer(&gas[0]);
    let loser = transfer(&gas[1]);
    // The winner's lock, as a commit before this one left it.
    seed_quarantined_lock(&epoch_store, reference(&object), *winner.digest());

    let mut transactions = vec![make_user_tx_v1_verified(loser.clone())];
    let (dropped, _, _) = post_consensus_validation::validate_and_resolve_conflicts(
        &authority,
        &epoch_store,
        2,
        &Default::default(),
        &mut transactions,
    )
    .await
    .unwrap();

    assert!(transactions.is_empty());
    assert_eq!(dropped.len(), 1);
    assert!(matches!(dropped[0].1, IotaError::ObjectLockConflict { .. }));
    assert_eq!(input_reads(&authority, "ahead"), 0);
    assert_eq!(input_reads(&authority, "inline"), 0);
    assert_eq!(input_reads(&authority, "unused"), 1);
}

/// A replayed commit and a transaction deferred at an earlier commit read
/// nothing ahead of the loop.
#[tokio::test]
async fn a_replayed_commit_and_a_deferred_transaction_read_nothing_ahead() {
    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let objects = [ObjectId::random(), ObjectId::random()];
    let gas = [ObjectId::random(), ObjectId::random()];
    let authority =
        authority_with_readers(4, sender, &[objects[0], objects[1], gas[0], gas[1]]).await;
    let epoch_store = authority.epoch_store_for_testing();
    let reference = |id: &ObjectId| authority.get_object(id).unwrap().object_ref();
    let rgp = authority.reference_gas_price_for_testing().unwrap();
    let transfer = |i: usize| {
        make_transfer_object_transaction(
            reference(&objects[i]),
            reference(&gas[i]),
            sender,
            &sender_key,
            Address::random(),
            rgp,
        )
    };

    let first = [transfer(0)];
    let first_run = validation_outcome(&authority, &first).await;
    assert_eq!(input_reads(&authority, "ahead"), 1);
    // The same commit again repeats its saved decisions.
    assert_eq!(validation_outcome(&authority, &first).await, first_run);
    assert_eq!(input_reads(&authority, "ahead"), 1);
    assert_eq!(input_reads(&authority, "unused"), 0);

    let deferred = epoch_store.verify_transaction(transfer(1)).unwrap();
    let previously_deferred = std::collections::HashMap::from([(
        *deferred.digest(),
        (
            crate::authority::transaction_deferral::DeferralKey::new_for_consensus_round(2, 1),
            None,
        ),
    )]);
    let inline_before = input_reads(&authority, "inline");
    let mut transactions = vec![make_user_tx_v1_verified(deferred)];
    post_consensus_validation::validate_and_resolve_conflicts(
        &authority,
        &epoch_store,
        2,
        &previously_deferred,
        &mut transactions,
    )
    .await
    .unwrap();
    assert_eq!(transactions.len(), 1);
    assert_eq!(input_reads(&authority, "ahead"), 1);
    assert_eq!(input_reads(&authority, "inline"), inline_before);
    assert_eq!(input_reads(&authority, "unused"), 0);
}

/// With every thread of the blocking pool busy, no other reader starts. The
/// commit is validated by the handler's own reader and does not wait for a
/// free thread. Not under the simulator: it has no blocking pool to fill and
/// cannot build a runtime of its own.
#[cfg(not(msim))]
#[test]
fn validation_does_not_wait_for_a_reader_the_blocking_pool_has_not_started() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
        let objects = [ObjectId::random(), ObjectId::random()];
        let gas = [ObjectId::random(), ObjectId::random()];
        let authority =
            authority_with_readers(4, sender, &[objects[0], objects[1], gas[0], gas[1]]).await;
        let reference = |id: &ObjectId| authority.get_object(id).unwrap().object_ref();
        let rgp = authority.reference_gas_price_for_testing().unwrap();
        let transfers: Vec<_> = (0..2)
            .map(|i| {
                make_transfer_object_transaction(
                    reference(&objects[i]),
                    reference(&gas[i]),
                    sender,
                    &sender_key,
                    Address::random(),
                    rgp,
                )
            })
            .collect();

        // Hold the pool's only thread until the commit is validated.
        let (running, is_running) = tokio::sync::oneshot::channel();
        let (release, released) = std::sync::mpsc::channel::<()>();
        let holder = tokio::task::spawn_blocking(move || {
            running.send(()).unwrap();
            released.recv().unwrap();
        });
        is_running.await.unwrap();

        let outcome = tokio::time::timeout(
            Duration::from_secs(60),
            validation_outcome(&authority, &transfers),
        )
        .await
        .expect("validation waited for a thread of the blocking pool");
        let (dropped, _, _, kept) = outcome;
        assert!(dropped.is_empty());
        assert_eq!(kept.len(), 2);
        assert_eq!(input_reads(&authority, "ahead"), 2);

        release.send(()).unwrap();
        holder.await.unwrap();
    });
}

#[test]
fn validation_readers_are_a_third_of_the_cores_between_one_and_sixteen() {
    use super::validation_readers_for;

    for (cores, readers) in [
        (1, 1),
        (4, 1),
        (8, 2),
        (16, 5),
        (32, 10),
        (48, 16),
        (96, 16),
    ] {
        assert_eq!(validation_readers_for(cores), readers, "{cores} cores");
    }
}

/// Transactions that `submit_tx` rejects for what they contain are also
/// dropped at a commit, with the same error, when another validator
/// sequences them.
#[tokio::test]
async fn invalid_transactions_are_dropped_at_the_commit_with_the_exact_error() {
    telemetry_subscribers::init_for_testing();
    let s = setup_handler_object_state(vec![], true).await;

    let cases: Vec<_> = crate::rejection_cases::rejection_cases(&s.authority)
        .into_iter()
        .filter(|case| case.at_commit.is_some())
        .collect();
    let transactions: Vec<_> = cases
        .iter()
        .map(|case| VerifiedTransaction::new_unchecked(case.tx.clone()))
        .collect();

    let result = s.validate_at(1, &transactions).await;

    assert!(
        result.kept.is_empty(),
        "no invalid transaction may be kept: {:?}",
        result.kept
    );
    let dropped: std::collections::HashMap<_, _> = result.dropped.into_iter().collect();
    for case in &cases {
        eprintln!("case: {}", case.name);
        let error = dropped
            .get(case.tx.digest())
            .unwrap_or_else(|| panic!("the transaction must be dropped: {}", case.name));
        case.at_commit.as_ref().unwrap()(error);
    }
}

/// A transaction paid for by a sponsor, signed by both, is kept.
#[tokio::test]
async fn sponsored_transaction_is_kept_at_the_commit() {
    telemetry_subscribers::init_for_testing();
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let (sponsor, sponsor_key): (Address, AccountPrivateKey) = get_key_pair();
    let object_id = ObjectId::random();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state(
        vec![
            Object::with_id_owner_for_testing(object_id, sender),
            Object::with_id_owner_for_testing(gas_id, sponsor),
        ],
        true,
    )
    .await;

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder
            .transfer_object(Address::random(), s.latest_ref(&object_id))
            .unwrap();
        builder.finish()
    };
    let tx = Transaction::new_with_gas_data(
        iota_sdk_types::TransactionKind::new_programmable(pt),
        sender,
        iota_sdk_types::GasPayment {
            objects: vec![s.latest_ref(&gas_id)],
            owner: sponsor,
            price: s.rgp,
            budget: iota_types::transaction::TEST_ONLY_GAS_UNIT_FOR_TRANSFER * s.rgp,
        },
    );
    let tx = iota_types::utils::to_sender_signed_transaction_with_multi_signers(
        tx,
        vec![&sender_key, &sponsor_key],
    );
    let digest = *tx.digest();

    let result = s
        .validate_at(1, &[VerifiedTransaction::new_unchecked(tx)])
        .await;

    assert_eq!(result.kept, vec![digest]);
    assert!(result.dropped.is_empty(), "{:?}", result.dropped);
}

/// A transaction the validator's local deny config rejects at admission is
/// kept at the commit, where only the deny rules from governance apply: a
/// local config differs between validators and would fork their decisions.
#[tokio::test]
async fn local_deny_config_is_not_applied_at_the_commit() {
    telemetry_subscribers::init_for_testing();
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let object_id = ObjectId::random();
    let gas_id = ObjectId::random();
    let s = setup_handler_object_state_with_deny_config(
        vec![
            Object::with_id_owner_for_testing(object_id, sender),
            Object::with_id_owner_for_testing(gas_id, sender),
        ],
        iota_config::transaction_deny_config::TransactionDenyConfigBuilder::new()
            .disable_user_transaction()
            .add_denied_address(sender)
            .add_denied_object(object_id)
            .build(),
    )
    .await;
    let tx = s.build_transfer(&object_id, &gas_id, sender, &sender_key, Address::random());
    let digest = *tx.digest();

    let denied = s
        .authority
        .handle_transaction_validation_checks(
            &tx,
            &s.epoch_store,
            &s.authority.config.transaction_deny_config,
        )
        .await
        .unwrap_err();
    assert!(
        matches!(
            denied,
            IotaError::UserInput {
                error: UserInputError::TransactionDenied { .. }
            }
        ),
        "admission must deny the transaction, got {denied:?}"
    );

    let result = s.validate_at(1, &[tx]).await;

    assert_eq!(result.kept, vec![digest]);
    assert!(result.dropped.is_empty(), "{:?}", result.dropped);
}

// ---------------------------------------------------------------------------
// ClaimAccount: at most one claim of an address is kept
// ---------------------------------------------------------------------------

impl HandlerObjectStateSetup {
    /// A verified `ClaimAccount` of `sender`, the address of `sender_key`'s
    /// Ed25519 key, paid with its gas coin `gas_id`.
    fn build_claim(
        &self,
        sender: Address,
        sender_key: &AccountPrivateKey,
        gas_id: &ObjectId,
    ) -> VerifiedTransaction {
        let claim = iota_sdk_types::SmartAccountClaim {
            public_key_scheme: iota_sdk_types::SignatureScheme::Ed25519.to_u8(),
            public_key_raw_bytes: AsRef::<[u8]>::as_ref(&sender_key.public_key()).to_vec(),
        };
        let tx = Transaction::new(
            iota_sdk_types::TransactionKind::new_claim_account(
                iota_sdk_types::ClaimAccountTransaction::new_smart_account(claim),
            ),
            sender,
            self.latest_ref(gas_id),
            iota_types::transaction::TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE * self.rgp,
            self.rgp,
        );
        let tx = to_sender_signed_transaction(tx, sender_key);
        self.epoch_store.verify_transaction(tx).unwrap()
    }

    /// Executes `claim` as the only root of commit `index` and checks that it
    /// created the account object at its sender's address.
    fn execute_claim_with_assigned_commit(
        &self,
        claim: VerifiedTransaction,
        index: CommitIndex,
    ) -> TransactionEffects {
        let account_id = ObjectId::from(claim.data().transaction().sender());
        self.epoch_store
            .assign_commit_to_transactions(index, vec![TransactionKey::Digest(*claim.digest())]);
        let (effects, error) = self
            .authority
            .try_execute_immediately(
                &self.executable(claim),
                ExecutionEnv::new(),
                &self.epoch_store,
            )
            .unwrap();
        assert!(
            effects.status().is_success(),
            "{:?}, {error:?}",
            effects.status()
        );
        assert!(
            effects
                .created()
                .iter()
                .any(|created| created.reference().object_id == account_id),
            "the claim must create the account object"
        );
        effects
    }
}

/// [`setup_handler_object_state`] with the flags on and ClaimAccount accepted,
/// which no protocol version enables, with the bounds on a claim's cost that it
/// needs.
async fn setup_claim_state(genesis_objects: Vec<Object>) -> HandlerObjectStateSetup {
    let guard =
        ProtocolConfig::apply_overrides_for_testing(|_, config| enable_claim_account(config));
    setup_handler_object_state_with_config_guard(genesis_objects, Some(guard)).await
}

/// Turns the `ClaimAccount` kind on, with the bounds on a claim's cost that it
/// needs.
fn enable_claim_account(mut config: ProtocolConfig) -> ProtocolConfig {
    config.set_claim_account_max_computation_units_for_testing(5_000);
    config.set_claim_account_storage_bytes_bound_for_testing(2_000);
    config.set_enable_claim_account_transaction_for_testing(true);
    config
}

/// The error a second claim of `address` is dropped with.
fn already_explicit(address: Address) -> IotaError {
    UserInputError::AccountAlreadyExplicit { address }.into()
}

/// A claim kept at a commit takes the lock on its address.
fn claim_lock(address: Address) -> ObjectReference {
    ObjectReference::new(
        ObjectId::from(address),
        Version::from_u64(0),
        ObjectDigest::MIN,
    )
}

#[tokio::test]
async fn claim_account_executes_and_creates_the_account() {
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let gas = ObjectId::random();
    let s = setup_claim_state(vec![Object::with_id_owner_for_testing(gas, sender)]).await;
    let claim = s.build_claim(sender, &key, &gas);
    s.execute_claim_with_assigned_commit(claim, 1);
}

/// After execution, a claim is reported when its address was claimed before
/// it ran, whatever its status, and when it did not create its account,
/// unless this node's own certificate deny list denied it.
#[test]
fn claim_execution_fault_reports_a_repeated_or_failed_claim() {
    let failed = ExecutionStatus::Failure {
        error: iota_sdk_types::ExecutionError::InsufficientGas,
        command: None,
    };
    let denied = ExecutionStatus::Failure {
        error: iota_sdk_types::ExecutionError::CertificateDenied,
        command: None,
    };

    assert_eq!(
        crate::authority::claim_execution_fault(false, &ExecutionStatus::Success),
        None
    );
    assert_eq!(
        crate::authority::claim_execution_fault(false, &denied),
        None
    );
    assert!(
        crate::authority::claim_execution_fault(false, &failed)
            .is_some_and(|fault| fault.contains("did not create its account"))
    );
    for status in [ExecutionStatus::Success, failed, denied] {
        assert!(
            crate::authority::claim_execution_fault(true, &status)
                .is_some_and(|fault| fault.contains("already claimed")),
            "{status:?}"
        );
    }
}

/// A second claim of an address that reaches execution, which validation
/// never lets happen, is reported after it runs.
#[tokio::test]
#[should_panic(expected = "executed although its address was already claimed")]
async fn second_claim_executed_without_validation_is_reported() {
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let gas = [ObjectId::random(), ObjectId::random()];
    let s = setup_claim_state(
        gas.iter()
            .map(|id| Object::with_id_owner_for_testing(*id, sender))
            .collect(),
    )
    .await;
    let first = s.build_claim(sender, &key, &gas[0]);
    let second = s.build_claim(sender, &key, &gas[1]);
    s.execute_claim_with_assigned_commit(first, 1);

    s.epoch_store
        .assign_commit_to_transactions(2, vec![TransactionKey::Digest(*second.digest())]);
    let _ = s.authority.try_execute_immediately(
        &s.executable(second),
        ExecutionEnv::new(),
        &s.epoch_store,
    );
}

/// Two claims of one address in one commit: the first is kept with the lock
/// on the address, the second is dropped for good.
#[tokio::test]
async fn second_claim_in_the_same_commit_is_dropped() {
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let gas = [ObjectId::random(), ObjectId::random()];
    let s = setup_claim_state(
        gas.iter()
            .map(|id| Object::with_id_owner_for_testing(*id, sender))
            .collect(),
    )
    .await;
    let first = s.build_claim(sender, &key, &gas[0]);
    let second = s.build_claim(sender, &key, &gas[1]);

    let result = s.validate_at(1, &[first.clone(), second.clone()]).await;

    assert_eq!(result.kept, vec![*first.digest()]);
    assert_eq!(
        result.dropped,
        vec![(*second.digest(), already_explicit(sender))]
    );
    assert_eq!(result.locks.get(&claim_lock(sender)), Some(first.digest()));
}

/// A claim kept at commit 1 holds the lock in the quarantine, so a claim of
/// the same address at commit 2 is dropped, although the horizon of commit 2
/// does not see the first claim yet.
#[tokio::test]
async fn second_claim_within_the_horizon_is_dropped_by_the_lock() {
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let gas = [ObjectId::random(), ObjectId::random()];
    let s = setup_claim_state(
        gas.iter()
            .map(|id| Object::with_id_owner_for_testing(*id, sender))
            .collect(),
    )
    .await;
    let first = s.build_claim(sender, &key, &gas[0]);
    let second = s.build_claim(sender, &key, &gas[1]);

    let scheduled = s.process_at(1, std::slice::from_ref(&first)).await;
    assert_eq!(scheduled.len(), 1);
    assert_eq!(
        s.epoch_store
            .get_quarantined_owned_object_lock(&claim_lock(sender)),
        Some(*first.digest())
    );

    let result = s.validate_at(2, std::slice::from_ref(&second)).await;
    assert!(result.kept.is_empty());
    assert_eq!(
        result.dropped,
        vec![(*second.digest(), already_explicit(sender))]
    );
}

/// The reader answers from the account's creation row: not claimed while the
/// row is above the horizon, as on a validator that has not executed it, and
/// claimed once the row is at or below it. Without a lock, as when the claim
/// was not kept by this commit processing, the horizon read alone decides.
#[tokio::test]
async fn claim_is_dropped_once_the_account_row_is_at_the_horizon() {
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let gas = [ObjectId::random(), ObjectId::random()];
    let s = setup_claim_state(
        gas.iter()
            .map(|id| Object::with_id_owner_for_testing(*id, sender))
            .collect(),
    )
    .await;
    let first = s.build_claim(sender, &key, &gas[0]);
    let first_effects = s.execute_claim_with_assigned_commit(first, 1);
    s.complete_commit(1, &[&first_effects]);
    let k = s.horizon_distance();

    let read_at = |index: CommitIndex| {
        CommitIndexedReader::new(
            s.authority.get_object_cache_reader().clone(),
            s.epoch_store.clone(),
            index,
        )
        .read_claimed_address(sender)
        .unwrap()
    };
    assert_eq!(read_at(k), ClaimedAddressAtHorizon::NotClaimed);
    assert_eq!(read_at(k + 1), ClaimedAddressAtHorizon::Claimed);

    let second = s.build_claim(sender, &key, &gas[1]);
    let result = s.validate_at(k + 1, std::slice::from_ref(&second)).await;
    assert!(result.kept.is_empty());
    assert_eq!(
        result.dropped,
        vec![(*second.digest(), already_explicit(sender))]
    );
}

/// An address the latest state holds no field for is answered without
/// reading the rows, before and after the horizon passes another address's
/// claim and in the next epoch, and still once the rows can no longer be
/// read; the answer the rows give is checked against it in debug builds.
#[tokio::test]
async fn never_claimed_address_reads_as_not_claimed() {
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let never_claimed = Address::random();
    let gas = ObjectId::random();
    let mut s = setup_claim_state(vec![Object::with_id_owner_for_testing(gas, sender)]).await;
    let claim = s.build_claim(sender, &key, &gas);
    let effects = s.execute_claim_with_assigned_commit(claim, 1);
    s.complete_commit(1, &[&effects]);
    let k = s.horizon_distance();

    for index in [1, k, k + 1] {
        assert_eq!(
            s.reader_at(index)
                .read_claimed_address(never_claimed)
                .unwrap(),
            ClaimedAddressAtHorizon::NotClaimed,
            "commit {index}"
        );
    }
    assert_eq!(
        s.reader_at(k + 1).read_claimed_address(sender).unwrap(),
        ClaimedAddressAtHorizon::Claimed
    );

    s.advance_epoch().await;
    assert_eq!(
        s.reader_at(1).read_claimed_address(never_claimed).unwrap(),
        ClaimedAddressAtHorizon::NotClaimed
    );
    assert_eq!(
        s.reader_at(1).read_claimed_address(sender).unwrap(),
        ClaimedAddressAtHorizon::Claimed
    );

    s.epoch_store.release_db_handles();
    let reader = s.reader_at(1);
    assert!(reader.read_account(ObjectId::from(never_claimed)).is_err());
    assert_eq!(
        reader.read_claimed_address(never_claimed).unwrap(),
        ClaimedAddressAtHorizon::NotClaimed
    );
}

/// An address whose field the latest state holds deleted is answered from the
/// rows: claimed while the deletion is above the horizon, not claimed once it
/// is at the horizon. No transaction can delete the field, so the test writes
/// the deletion to the store.
#[tokio::test]
async fn claimed_address_reads_as_claimed_while_its_field_deletion_is_above_the_horizon() {
    use crate::transaction_outputs::TransactionOutputs;

    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let (deleter, deleter_key): (Address, AccountPrivateKey) = get_key_pair();
    let gas = ObjectId::random();
    let deleter_gas = ObjectId::random();
    let s = setup_claim_state(vec![
        Object::with_id_owner_for_testing(gas, sender),
        Object::with_id_owner_for_testing(deleter_gas, deleter),
    ])
    .await;
    let claim = s.build_claim(sender, &key, &gas);
    let claim_effects = s.execute_claim_with_assigned_commit(claim, 1);
    s.complete_commit(1, &[&claim_effects]);
    let k = s.horizon_distance();

    let field_id = derive_authenticator_function_ref_v1_dynamic_field_id(sender).unwrap();
    let field = s.authority.get_object(&field_id).unwrap();
    let deletion = s.build_move_call(
        "object_basics",
        "create",
        create_object_args(deleter),
        &deleter_gas,
        deleter,
        &deleter_key,
    );
    let deletion_effects = TestEffectsBuilder::new(deletion.data())
        .with_deleted_objects_owned_by([(field_id, field.version(), field.owner)])
        .build();
    let deletion_key = TransactionKey::Digest(*deletion.digest());
    s.epoch_store
        .assign_commit_to_transactions(3, vec![deletion_key]);
    s.epoch_store
        .record_executed_transaction(&deletion_key, &deletion_effects);
    s.authority.get_cache_writer().write_transaction_outputs(
        s.epoch_store.epoch(),
        Arc::new(TransactionOutputs {
            transaction: Arc::new(deletion),
            effects: deletion_effects.clone(),
            events: Default::default(),
            superseded: vec![],
            markers: vec![],
            wrapped: vec![],
            deleted: vec![ObjectKey(field_id, deletion_effects.lamport_version())],
            live_object_markers_to_delete: vec![],
            new_live_object_markers_to_init: vec![],
            written: Default::default(),
            attestation_record: None,
        }),
    );
    s.complete_commit(3, &[&deletion_effects]);

    let cache = s.authority.get_object_cache_reader();
    assert!(cache.try_get_object(&field_id).unwrap().is_none());
    assert!(
        cache
            .try_get_latest_object_ref_or_tombstone(field_id)
            .unwrap()
            .is_some(),
        "the latest state must hold the field deleted"
    );

    // Horizon 2: the field is live there and its deletion at 3 is above.
    assert_eq!(
        s.reader_at(k + 2).read_claimed_address(sender).unwrap(),
        ClaimedAddressAtHorizon::Claimed
    );
    // Horizon 3: the deletion is at the horizon.
    assert_eq!(
        s.reader_at(k + 3).read_claimed_address(sender).unwrap(),
        ClaimedAddressAtHorizon::NotClaimed
    );
}

/// An account claimed in an earlier epoch is in the store with no row this
/// epoch, so a new claim of its address is dropped at the first commit.
#[tokio::test]
async fn claim_of_an_account_from_an_earlier_epoch_is_dropped() {
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let gas = [ObjectId::random(), ObjectId::random()];
    let mut s = setup_claim_state(
        gas.iter()
            .map(|id| Object::with_id_owner_for_testing(*id, sender))
            .collect(),
    )
    .await;
    let first = s.build_claim(sender, &key, &gas[0]);
    let first_effects = s.execute_claim_with_assigned_commit(first, 1);
    s.complete_commit(1, &[&first_effects]);
    s.advance_epoch().await;

    let second = s.build_claim(sender, &key, &gas[1]);
    let result = s.validate_at(1, std::slice::from_ref(&second)).await;
    assert!(result.kept.is_empty());
    assert_eq!(
        result.dropped,
        vec![(*second.digest(), already_explicit(sender))]
    );
}

/// After a restart, the replay of the commit that kept an executed claim
/// takes the claim's lock again from the transaction, so a later claim of
/// the address is still dropped within the horizon.
#[tokio::test]
async fn replayed_executed_claim_takes_its_lock_again() {
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let gas = [ObjectId::random(), ObjectId::random()];
    let mut s = setup_claim_state(
        gas.iter()
            .map(|id| Object::with_id_owner_for_testing(*id, sender))
            .collect(),
    )
    .await;
    let first = s.build_claim(sender, &key, &gas[0]);
    let second = s.build_claim(sender, &key, &gas[1]);
    let scheduled = s.process_at(1, std::slice::from_ref(&first)).await;
    assert_eq!(scheduled.len(), 1);
    s.execute_executable(&scheduled[0], ExecutionEnv::new());

    s.restart_epoch_store().await;
    assert_eq!(
        s.epoch_store
            .get_quarantined_owned_object_lock(&claim_lock(sender)),
        None,
        "the restart must lose the quarantined lock"
    );
    let replayed = s.process_at(1, std::slice::from_ref(&first)).await;
    assert_eq!(replayed.len(), 1);
    assert_eq!(
        s.epoch_store
            .get_quarantined_owned_object_lock(&claim_lock(sender)),
        Some(*first.digest())
    );

    let result = s.validate_at(2, std::slice::from_ref(&second)).await;
    assert!(result.kept.is_empty());
    assert_eq!(
        result.dropped,
        vec![(*second.digest(), already_explicit(sender))]
    );
}

/// Admission answers early from the validator's latest state.
#[tokio::test]
async fn admission_rejects_a_claim_of_an_existing_account() {
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let gas = [ObjectId::random(), ObjectId::random()];
    let s = setup_claim_state(
        gas.iter()
            .map(|id| Object::with_id_owner_for_testing(*id, sender))
            .collect(),
    )
    .await;
    let first = s.build_claim(sender, &key, &gas[0]);
    s.execute_claim_with_assigned_commit(first, 1);

    let second = s.build_claim(sender, &key, &gas[1]);
    let error = s
        .authority
        .handle_transaction_validation_checks(
            &second,
            &s.epoch_store,
            &s.authority.config.transaction_deny_config,
        )
        .await
        .unwrap_err();
    assert_eq!(error, already_explicit(sender));
}

/// The error a plain signature for the claimed address `address` is dropped
/// with.
fn plain_signature_for(address: Address) -> IotaError {
    UserInputError::PlainSignatureForExplicitAccount { address }.into()
}

/// A setup where `sender` owns an object and two gas coins: one for its
/// claim, one for a plain-signed transfer of the object.
async fn setup_claim_and_transfer() -> (
    HandlerObjectStateSetup,
    Address,
    AccountPrivateKey,
    [ObjectId; 3],
) {
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let ids = [ObjectId::random(), ObjectId::random(), ObjectId::random()];
    let s = setup_claim_state(
        ids.iter()
            .map(|id| Object::with_id_owner_for_testing(*id, sender))
            .collect(),
    )
    .await;
    (s, sender, key, ids)
}

/// A plain-signed transaction of the claimed address in the same commit,
/// after the claim, is dropped as a copy: a copy where the address signs with
/// a Move authenticator could still be kept.
#[tokio::test]
async fn plain_signature_after_a_claim_in_the_same_commit_is_dropped() {
    let (s, sender, key, [claim_gas, object, gas]) = setup_claim_and_transfer().await;
    let claim = s.build_claim(sender, &key, &claim_gas);
    let transfer = s.build_transfer(&object, &gas, sender, &key, Address::random());

    let result = s.validate_at(1, &[claim.clone(), transfer.clone()]).await;

    assert_eq!(result.kept, vec![*claim.digest()]);
    assert_eq!(
        result.dropped,
        vec![(*transfer.digest(), plain_signature_for(sender))]
    );
    assert_eq!(
        s.epoch_store
            .post_consensus_decision(1, *transfer.digest())
            .unwrap()
            .map(|saved| saved.decision),
        Some(PostConsensusDecision::DroppedCopy)
    );
}

/// A plain-signed transaction sequenced before the claim proceeds as
/// implicit.
#[tokio::test]
async fn plain_signature_before_a_claim_in_the_same_commit_is_kept() {
    let (s, sender, key, [claim_gas, object, gas]) = setup_claim_and_transfer().await;
    let claim = s.build_claim(sender, &key, &claim_gas);
    let transfer = s.build_transfer(&object, &gas, sender, &key, Address::random());

    let result = s.validate_at(1, &[transfer.clone(), claim.clone()]).await;

    assert_eq!(result.kept, vec![*transfer.digest(), *claim.digest()]);
    assert!(result.dropped.is_empty(), "{:?}", result.dropped);
}

/// A claim kept at commit 1 holds the lock, so a plain-signed transaction of
/// the address at commit 2 is dropped before the horizon sees the account.
#[tokio::test]
async fn plain_signature_within_the_horizon_is_dropped_by_the_lock() {
    let (s, sender, key, [claim_gas, object, gas]) = setup_claim_and_transfer().await;
    let claim = s.build_claim(sender, &key, &claim_gas);
    assert_eq!(s.process_at(1, std::slice::from_ref(&claim)).await.len(), 1);

    let transfer = s.build_transfer(&object, &gas, sender, &key, Address::random());
    let result = s.validate_at(2, std::slice::from_ref(&transfer)).await;

    assert!(result.kept.is_empty());
    assert_eq!(
        result.dropped,
        vec![(*transfer.digest(), plain_signature_for(sender))]
    );
}

/// In a later epoch no lock remains, and the horizon read finds the account
/// the epoch began with.
#[tokio::test]
async fn plain_signature_for_an_account_from_an_earlier_epoch_is_dropped() {
    let (mut s, sender, key, [claim_gas, object, gas]) = setup_claim_and_transfer().await;
    let claim = s.build_claim(sender, &key, &claim_gas);
    let claim_effects = s.execute_claim_with_assigned_commit(claim, 1);
    s.complete_commit(1, &[&claim_effects]);
    s.advance_epoch().await;

    let transfer = s.build_transfer(&object, &gas, sender, &key, Address::random());
    let result = s.validate_at(1, std::slice::from_ref(&transfer)).await;

    assert!(result.kept.is_empty());
    assert_eq!(
        result.dropped,
        vec![(*transfer.digest(), plain_signature_for(sender))]
    );
}

/// The rule covers the gas owner too: a claimed address cannot sponsor with a
/// plain signature.
#[tokio::test]
async fn plain_signed_sponsor_of_a_claimed_address_is_dropped() {
    let (sponsor, sponsor_key): (Address, AccountPrivateKey) = get_key_pair();
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let [claim_gas, gas, object] = [ObjectId::random(), ObjectId::random(), ObjectId::random()];
    let s = setup_claim_state(vec![
        Object::with_id_owner_for_testing(claim_gas, sponsor),
        Object::with_id_owner_for_testing(gas, sponsor),
        Object::with_id_owner_for_testing(object, sender),
    ])
    .await;
    let claim = s.build_claim(sponsor, &sponsor_key, &claim_gas);
    let mut tx = TestTransactionBuilder::new(sender, s.latest_ref(&gas), s.rgp)
        .transfer(s.latest_ref(&object), Address::random())
        .build();
    tx.gas_data_mut().owner = sponsor;
    let sponsored = s
        .epoch_store
        .verify_transaction(
            iota_types::utils::to_sender_signed_transaction_with_multi_signers(
                tx,
                vec![&sender_key, &sponsor_key],
            ),
        )
        .unwrap();

    let result = s.validate_at(1, &[claim.clone(), sponsored.clone()]).await;

    assert_eq!(result.kept, vec![*claim.digest()]);
    assert_eq!(
        result.dropped,
        vec![(*sponsored.digest(), plain_signature_for(sponsor))]
    );
}

/// Admission answers early for a plain signature too.
#[tokio::test]
async fn admission_rejects_a_plain_signature_for_a_claimed_address() {
    let (s, sender, key, [claim_gas, object, gas]) = setup_claim_and_transfer().await;
    let claim = s.build_claim(sender, &key, &claim_gas);
    s.execute_claim_with_assigned_commit(claim, 1);

    let transfer = s.build_transfer(&object, &gas, sender, &key, Address::random());
    let error = s
        .authority
        .handle_transaction_validation_checks(
            &transfer,
            &s.epoch_store,
            &s.authority.config.transaction_deny_config,
        )
        .await
        .unwrap_err();
    assert_eq!(error, plain_signature_for(sender));
}

/// The claim is recorded by the account's authenticator function field, not
/// by the account object. No claim leaves the field without the object today,
/// but if one did, the address would stay claimed, so a new claim and a plain
/// signature are both dropped.
#[tokio::test]
async fn orphaned_authenticator_field_keeps_the_address_claimed() {
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let [claim_gas, object, gas] = [ObjectId::random(), ObjectId::random(), ObjectId::random()];
    let account_id = ObjectId::from(sender);
    let s = setup_claim_state(vec![
        Object::with_id_owner_for_testing(claim_gas, sender),
        Object::with_id_owner_for_testing(object, sender),
        Object::with_id_owner_for_testing(gas, sender),
        authenticator_function_ref_field(account_id, ObjectId::random(), Version::from(1)),
    ])
    .await;
    assert!(s.authority.get_object(&account_id).is_none());

    let claim = s.build_claim(sender, &key, &claim_gas);
    let transfer = s.build_transfer(&object, &gas, sender, &key, Address::random());
    let result = s.validate_at(1, &[claim.clone(), transfer.clone()]).await;

    assert!(result.kept.is_empty(), "{:?}", result.kept);
    assert_eq!(
        result.dropped,
        vec![
            (*claim.digest(), already_explicit(sender)),
            (*transfer.digest(), plain_signature_for(sender)),
        ]
    );
}

/// The plain-signature rule follows its own protocol flag, so a claimed
/// address keeps rejecting plain signatures in a version that no longer
/// accepts claims.
#[tokio::test]
async fn plain_signature_rule_outlives_the_claim_flag() {
    let guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_claim_account_transaction_for_testing(false);
        config.set_reject_plain_signatures_for_explicit_accounts_for_testing(true);
        config
    });
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let [object, gas] = [ObjectId::random(), ObjectId::random()];
    let s = setup_handler_object_state_with_config_guard(
        vec![
            Object::with_id_owner_for_testing(object, sender),
            Object::with_id_owner_for_testing(gas, sender),
        ],
        Some(guard),
    )
    .await;
    // A claim of the sender kept earlier in the epoch holds the lock.
    seed_quarantined_lock(
        &s.epoch_store,
        claim_lock(sender),
        TransactionDigest::new([7; 32]),
    );
    let transfer = s.build_transfer(&object, &gas, sender, &key, Address::random());

    let result = s.validate_at(1, std::slice::from_ref(&transfer)).await;

    assert_eq!(
        result.dropped,
        vec![(*transfer.digest(), plain_signature_for(sender))]
    );
}

/// A claim whose plain-signed sponsor is explicit is dropped for good: a
/// claim cannot carry a Move authenticator, so no other copy of it can be
/// kept, and the claimant is told.
#[tokio::test]
async fn claim_with_an_explicit_sponsor_is_dropped_for_good() {
    let (sponsor, sponsor_key): (Address, AccountPrivateKey) = get_key_pair();
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let [sponsor_claim_gas, gas] = [ObjectId::random(), ObjectId::random()];
    let s = setup_claim_state(vec![
        Object::with_id_owner_for_testing(sponsor_claim_gas, sponsor),
        Object::with_id_owner_for_testing(gas, sponsor),
    ])
    .await;
    let sponsor_claim = s.build_claim(sponsor, &sponsor_key, &sponsor_claim_gas);
    let mut tx = Transaction::new(
        iota_sdk_types::TransactionKind::new_claim_account(
            iota_sdk_types::ClaimAccountTransaction::new_smart_account(
                iota_sdk_types::SmartAccountClaim {
                    public_key_scheme: iota_sdk_types::SignatureScheme::Ed25519.to_u8(),
                    public_key_raw_bytes: AsRef::<[u8]>::as_ref(&sender_key.public_key()).to_vec(),
                },
            ),
        ),
        sender,
        s.latest_ref(&gas),
        iota_types::transaction::TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE * s.rgp,
        s.rgp,
    );
    tx.gas_data_mut().owner = sponsor;
    let sponsored = s
        .epoch_store
        .verify_transaction(
            iota_types::utils::to_sender_signed_transaction_with_multi_signers(
                tx,
                vec![&sender_key, &sponsor_key],
            ),
        )
        .unwrap();

    let result = s
        .validate_at(1, &[sponsor_claim.clone(), sponsored.clone()])
        .await;

    assert_eq!(result.kept, vec![*sponsor_claim.digest()]);
    assert_eq!(
        result.dropped,
        vec![(*sponsored.digest(), plain_signature_for(sponsor))]
    );
    let saved = s
        .epoch_store
        .post_consensus_decision(1, *sponsored.digest())
        .unwrap()
        .expect("the decision is saved");
    assert_eq!(saved.decision, PostConsensusDecision::Dropped);
}

/// A claim kept at commit 1 moves its lock to the lock table when commit 1
/// flushes, where it still drops a second claim of the address that the
/// horizon cannot see yet.
#[tokio::test]
async fn claim_lock_in_the_lock_table_drops_a_second_claim() {
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let gas = [ObjectId::random(), ObjectId::random()];
    let s = setup_claim_state(
        gas.iter()
            .map(|id| Object::with_id_owner_for_testing(*id, sender))
            .collect(),
    )
    .await;
    let first = s.build_claim(sender, &key, &gas[0]);
    let scheduled = s.process_at(1, std::slice::from_ref(&first)).await;
    assert_eq!(scheduled.len(), 1);
    let effects = s.execute(first.clone());
    let height = s
        .checkpoint_roots()
        .into_iter()
        .map(|(height, _)| height)
        .max()
        .expect("commit 1 has a pending checkpoint");
    s.epoch_store
        .flush_pushed_commits_for_testing(height, 1, vec![*effects.transaction_digest()])
        .unwrap();
    assert_eq!(
        s.epoch_store
            .get_quarantined_owned_object_lock(&claim_lock(sender)),
        None
    );
    assert_eq!(
        s.epoch_store
            .tables()
            .unwrap()
            .get_locked_transaction(&claim_lock(sender))
            .unwrap(),
        Some(*first.digest())
    );

    let second = s.build_claim(sender, &key, &gas[1]);
    let result = s.validate_at(2, std::slice::from_ref(&second)).await;
    assert!(result.kept.is_empty(), "{:?}", result.kept);
    assert_eq!(
        result.dropped,
        vec![(*second.digest(), already_explicit(sender))]
    );
}

/// A kept claim that has not executed keeps its lock however far the horizon
/// moves, so a second claim of the address is dropped. The lock ends with the
/// epoch: in the next one, the address, never claimed, can be claimed.
#[tokio::test]
async fn claim_lock_counts_until_the_epoch_ends() {
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let gas = [ObjectId::random(), ObjectId::random()];
    let mut s = setup_claim_state(
        gas.iter()
            .map(|id| Object::with_id_owner_for_testing(*id, sender))
            .collect(),
    )
    .await;
    let first = s.build_claim(sender, &key, &gas[0]);
    let scheduled = s.process_at(1, std::slice::from_ref(&first)).await;
    assert_eq!(scheduled.len(), 1);

    let second = s.build_claim(sender, &key, &gas[1]);
    let result = s
        .validate_at(s.horizon_distance() + 10, std::slice::from_ref(&second))
        .await;
    assert!(result.kept.is_empty(), "{:?}", result.kept);
    assert_eq!(
        result.dropped,
        vec![(*second.digest(), already_explicit(sender))]
    );

    s.advance_epoch().await;
    let retried = s.build_claim(sender, &key, &gas[1]);
    let result = s.validate_at(1, std::slice::from_ref(&retried)).await;
    assert_eq!(result.kept, vec![*retried.digest()]);
}

/// With an execution worker limit and no deferral allowance, congestion
/// control cancels one of two claims kept at a commit. Its lock still holds
/// until the epoch ends, so its address cannot be claimed again in that
/// epoch, which is why no version accepts claims together with that limit
/// (see `every_kept_claim_account_can_execute` in `iota-protocol-config`).
#[tokio::test]
async fn claim_cancelled_by_worker_congestion_keeps_its_lock() {
    let guard = ProtocolConfig::apply_overrides_for_testing(|_, config| {
        let mut config = enable_claim_account(config);
        config.set_per_object_congestion_control_mode_for_testing(
            iota_protocol_config::PerObjectCongestionControlMode::TotalTxCount,
        );
        config.set_max_accumulated_txn_cost_per_object_in_mysticeti_commit_for_testing(1);
        config.set_max_congestion_limit_overshoot_per_commit_for_testing(0);
        config.set_max_concurrent_execution_workers_for_testing(1);
        config.set_separate_gas_price_feedback_mechanism_for_randomness_for_testing(false);
        config.set_max_deferral_rounds_for_congestion_control_for_testing(0);
        config
    });
    let accounts: Vec<(Address, AccountPrivateKey, ObjectId, ObjectId)> = (0..2)
        .map(|_| {
            let (address, key): (Address, AccountPrivateKey) = get_key_pair();
            (address, key, ObjectId::random(), ObjectId::random())
        })
        .collect();
    let s = setup_handler_object_state_with_config_guard(
        accounts
            .iter()
            .flat_map(|(address, _, gas, retry_gas)| {
                [
                    Object::with_id_owner_for_testing(*gas, *address),
                    Object::with_id_owner_for_testing(*retry_gas, *address),
                ]
            })
            .collect(),
        Some(guard),
    )
    .await;
    let claims: Vec<_> = accounts
        .iter()
        .map(|(address, key, gas, _)| s.build_claim(*address, key, gas))
        .collect();

    let (scheduled, assigned_versions) = s.process_at_with_versions(1, &claims).await;
    assert_eq!(
        scheduled.len(),
        2,
        "both claims are scheduled, one cancelled"
    );
    let assigned_versions = assigned_versions.into_map();
    let cancelled = claims
        .iter()
        .filter(|claim| {
            assigned_versions
                .get(&TransactionKey::Digest(*claim.digest()))
                .is_some_and(|versions| {
                    versions
                        .iter()
                        .any(|assignment| assignment.version.is_congested())
                })
        })
        .count();
    assert_eq!(cancelled, 1, "one claim is cancelled");
    for ((address, ..), claim) in accounts.iter().zip(&claims) {
        assert_eq!(
            s.epoch_store
                .get_quarantined_owned_object_lock(&claim_lock(*address)),
            Some(*claim.digest())
        );
    }

    let retries: Vec<_> = accounts
        .iter()
        .map(|(address, key, _, retry_gas)| s.build_claim(*address, key, retry_gas))
        .collect();
    let result = s.validate_at(s.horizon_distance() + 10, &retries).await;
    assert!(result.kept.is_empty(), "{:?}", result.kept);
    assert_eq!(
        result.dropped,
        accounts
            .iter()
            .zip(&retries)
            .map(|((address, ..), retried)| (*retried.digest(), already_explicit(*address)))
            .collect::<Vec<_>>()
    );
}

/// No object has a claim lock's reference, so a transaction that names one as
/// an owned input is never kept and never holds the lock. With no claim of
/// the address, the input is reported as not visible yet. Before a claim in
/// the same commit, it is dropped and the claim still takes the lock. Once a
/// claim holds the lock, it is dropped on it.
#[tokio::test]
async fn transaction_naming_a_claim_lock_as_an_owned_input_never_takes_it() {
    let (victim, victim_key): (Address, AccountPrivateKey) = get_key_pair();
    let (attacker, attacker_key): (Address, AccountPrivateKey) = get_key_pair();
    let [claim_gas, attacker_gas] = [ObjectId::random(), ObjectId::random()];
    let s = setup_claim_state(vec![
        Object::with_id_owner_for_testing(claim_gas, victim),
        Object::with_id_owner_for_testing(attacker_gas, attacker),
    ])
    .await;
    let naming_the_lock = s
        .epoch_store
        .verify_transaction(make_transfer_object_transaction(
            claim_lock(victim),
            s.latest_ref(&attacker_gas),
            attacker,
            &attacker_key,
            Address::random(),
            s.rgp,
        ))
        .unwrap();
    let claim = s.build_claim(victim, &victim_key, &claim_gas);

    let result = s
        .validate_at(1, std::slice::from_ref(&naming_the_lock))
        .await;
    assert!(result.kept.is_empty(), "{:?}", result.kept);
    assert!(result.locks.is_empty(), "{:?}", result.locks);
    assert_eq!(
        s.epoch_store
            .post_consensus_decision(1, *naming_the_lock.digest())
            .unwrap()
            .map(|saved| saved.decision),
        Some(PostConsensusDecision::DroppedInputNotVisibleYet)
    );

    let scheduled = s
        .process_at(2, &[naming_the_lock.clone(), claim.clone()])
        .await;
    assert_eq!(
        scheduled.iter().map(|tx| *tx.digest()).collect::<Vec<_>>(),
        vec![*claim.digest()]
    );
    assert_eq!(
        s.epoch_store
            .get_quarantined_owned_object_lock(&claim_lock(victim)),
        Some(*claim.digest())
    );

    let result = s
        .validate_at(3, std::slice::from_ref(&naming_the_lock))
        .await;
    assert!(result.kept.is_empty(), "{:?}", result.kept);
    assert_eq!(
        result.dropped,
        vec![(
            *naming_the_lock.digest(),
            IotaError::ObjectLockConflict {
                obj_ref: claim_lock(victim),
                pending_transaction: *claim.digest(),
            }
        )]
    );
    assert!(result.locks.is_empty(), "{:?}", result.locks);
}

/// A plain-signed transaction kept before a claim of its sender and deferred
/// is kept again after the claim without being validated again: the rule
/// against plain signatures holds only for decisions made once the claim is
/// kept. The same transaction validated anew after the claim is dropped. In a
/// version without an execution worker limit only a transaction with a shared
/// input can be deferred; the test hands the transfer to validation as
/// previously deferred, which is all the skip looks at.
#[tokio::test]
async fn deferred_plain_signed_transaction_is_kept_after_a_claim() {
    let (s, sender, key, [claim_gas, object, gas]) = setup_claim_and_transfer().await;
    let claim = s.build_claim(sender, &key, &claim_gas);
    let transfer = s.build_transfer(&object, &gas, sender, &key, Address::random());
    let scheduled = s.process_at(1, std::slice::from_ref(&claim)).await;
    assert_eq!(scheduled.len(), 1);

    let previously_deferred = std::collections::HashMap::from([(
        *transfer.digest(),
        (
            crate::authority::transaction_deferral::DeferralKey::new_for_consensus_round(2, 1),
            None,
        ),
    )]);
    let mut transactions = vec![make_user_tx_v1_verified(transfer.clone())];
    let (dropped, locks, _) = post_consensus_validation::validate_and_resolve_conflicts(
        &s.authority,
        &s.epoch_store,
        2,
        &previously_deferred,
        &mut transactions,
    )
    .await
    .unwrap();
    assert!(dropped.is_empty(), "{dropped:?}");
    assert_eq!(transactions.len(), 1);
    assert!(locks.is_empty());

    let result = s.validate_at(3, std::slice::from_ref(&transfer)).await;
    assert!(result.kept.is_empty(), "{:?}", result.kept);
    assert_eq!(
        result.dropped,
        vec![(*transfer.digest(), plain_signature_for(sender))]
    );
}

/// The costliest claim the validity check accepts at its smallest budget:
/// the largest MultiSig key, of Secp256r1 members, paid with the most gas
/// coins a transaction may name, none with a storage rebate, at the highest
/// gas price. It is kept and creates its account within the computation
/// bound the budget assumes.
#[tokio::test]
async fn claim_at_the_gas_floor_with_the_most_gas_coins_succeeds() {
    use iota_sdk_crypto::{Signer, secp256r1::Secp256r1PrivateKey, simple::SimpleKeypair};
    use iota_sdk_types::{
        SignatureScheme, SmartAccountClaim,
        crypto::{
            Intent, IntentMessage, MULTISIG_COMMITTEE_SIZE_MAX, MultisigAggregatedSignature,
            MultisigCommittee, MultisigMember, SimpleSignature,
        },
    };
    use iota_types::move_public_key::MovePublicKey;
    use rand::{SeedableRng, rngs::StdRng};

    let mut rng = StdRng::from_seed([7; 32]);
    let keys: Vec<Secp256r1PrivateKey> = (0..MULTISIG_COMMITTEE_SIZE_MAX)
        .map(|_| Secp256r1PrivateKey::random_with(&mut rng))
        .collect();
    let committee = MultisigCommittee::new(
        keys.iter()
            .map(|key| MultisigMember::new(key.public_key(), 1))
            .collect(),
        1,
    )
    .unwrap();
    let claim = SmartAccountClaim::new_multisig(&committee);
    let sender = MovePublicKey::new(
        SignatureScheme::Multisig,
        claim.public_key_raw_bytes.clone(),
    )
    .unwrap()
    .address()
    .unwrap();
    let max_gas_payment_objects =
        ProtocolConfig::get_for_max_version_UNSAFE().max_gas_payment_objects() as usize;
    let gas: Vec<ObjectId> = (0..max_gas_payment_objects - 1)
        .map(|_| ObjectId::random())
        .collect();
    let s = setup_claim_state(
        gas.iter()
            .map(|id| Object::with_id_owner_for_testing(*id, sender))
            .collect(),
    )
    .await;
    let config = s.epoch_store.protocol_config();
    let gas_price = config.max_gas_price();
    let budget = iota_types::transaction::claim_account_min_gas_budget(
        config,
        gas_price,
        claim.public_key_raw_bytes.len() as u64,
    );
    let tx = Transaction::new_with_gas_coins(
        iota_sdk_types::TransactionKind::new_claim_account(
            iota_sdk_types::ClaimAccountTransaction::new_smart_account(claim),
        ),
        sender,
        gas.iter().map(|id| s.latest_ref(id)).collect(),
        budget,
        gas_price,
    );
    let signature: SimpleSignature = SimpleKeypair::from(keys[0].clone())
        .sign(&IntentMessage::new(Intent::iota_transaction(), tx.clone()).signing_digest());
    let multisig = MultisigAggregatedSignature::new(vec![signature.into()], committee).unwrap();
    let claim = s
        .epoch_store
        .verify_transaction(TransactionEnvelope::from_user_sig_data(
            tx,
            vec![UserSignature::Multisig(multisig)],
        ))
        .unwrap();

    let result = s.validate_at(1, std::slice::from_ref(&claim)).await;
    assert_eq!(result.kept, vec![*claim.digest()]);
    let effects = s.execute_claim_with_assigned_commit(claim, 1);
    assert!(
        effects.gas_cost_summary().computation_cost
            <= config.claim_account_max_computation_units() * gas_price,
        "{:?}",
        effects.gas_cost_summary()
    );
}

// ---------------------------------------------------------------------------
// ClaimAccount: every signature scheme
// ---------------------------------------------------------------------------

/// A key that claims the address it derives and signs for it, of one of the
/// schemes a claim accepts.
enum KeyHolder {
    /// An Ed25519, Secp256k1 or Secp256r1 key.
    Keypair(iota_sdk_crypto::simple::SimpleKeypair),
    /// A Secp256r1 key that signs as a passkey.
    Passkey(iota_sdk_crypto::simple::SimpleKeypair),
    /// A MultiSig committee, and keys of members of it, in committee order,
    /// whose weights reach its threshold.
    Multisig {
        committee: iota_sdk_types::crypto::MultisigCommittee,
        signers: Vec<KeyHolder>,
    },
}

impl KeyHolder {
    /// One key of each scheme a claim accepts, drawn from `seed`. The MultiSig
    /// committee has a Secp256k1, a Secp256r1 and a second Secp256k1 member
    /// with a threshold of two, and its first two members sign.
    fn every_scheme(seed: u8) -> Vec<Self> {
        use iota_sdk_crypto::{
            ed25519::Ed25519PrivateKey, secp256k1::Secp256k1PrivateKey,
            secp256r1::Secp256r1PrivateKey, simple::SimpleKeypair,
        };
        use rand::{SeedableRng, rngs::StdRng};

        let rng = |index: u8| StdRng::from_seed([seed.wrapping_mul(16).wrapping_add(index); 32]);
        let k1 = |index| SimpleKeypair::from(Secp256k1PrivateKey::random_with(rng(index)));
        let r1 = |index| SimpleKeypair::from(Secp256r1PrivateKey::random_with(rng(index)));
        vec![
            Self::Keypair(SimpleKeypair::from(Ed25519PrivateKey::random_with(rng(0)))),
            Self::Keypair(k1(1)),
            Self::Keypair(r1(2)),
            Self::Passkey(r1(3)),
            Self::multisig(Self::members(seed), &[1, 1, 1], 2, 2),
        ]
    }

    /// Three keys drawn from `seed`, to be members of a MultiSig committee: a
    /// Secp256k1, a Secp256r1 and a second Secp256k1 key.
    fn members(seed: u8) -> Vec<Self> {
        use iota_sdk_crypto::{
            secp256k1::Secp256k1PrivateKey, secp256r1::Secp256r1PrivateKey, simple::SimpleKeypair,
        };
        use rand::{SeedableRng, rngs::StdRng};

        let rng = |index: u8| StdRng::from_seed([seed.wrapping_mul(16).wrapping_add(index); 32]);
        vec![
            Self::Keypair(SimpleKeypair::from(Secp256k1PrivateKey::random_with(rng(
                4,
            )))),
            Self::Keypair(SimpleKeypair::from(Secp256r1PrivateKey::random_with(rng(
                5,
            )))),
            Self::Keypair(SimpleKeypair::from(Secp256k1PrivateKey::random_with(rng(
                6,
            )))),
        ]
    }

    /// A MultiSig committee of `members` with `weights` and `threshold`, for
    /// which the members at `signing`, in committee order, sign.
    fn multisig_signed_by(
        members: Vec<Self>,
        weights: &[u8],
        threshold: u16,
        signing: &[usize],
    ) -> Self {
        let Self::Multisig { committee, .. } = Self::multisig(
            members.iter().map(Self::same_key).collect(),
            weights,
            threshold,
            0,
        ) else {
            unreachable!()
        };
        Self::Multisig {
            committee,
            signers: members
                .into_iter()
                .enumerate()
                .filter(|(index, _)| signing.contains(index))
                .map(|(_, member)| member)
                .collect(),
        }
    }

    /// Another holder of the same key, which is not a MultiSig key.
    fn same_key(&self) -> Self {
        match self {
            Self::Keypair(keypair) => Self::Keypair(keypair.clone()),
            Self::Passkey(keypair) => Self::Passkey(keypair.clone()),
            Self::Multisig { .. } => panic!("a MultiSig key is no committee member"),
        }
    }

    /// A MultiSig committee of `members`, which are not MultiSig keys, with
    /// `weights` and `threshold`, whose first `signing` members sign.
    fn multisig(members: Vec<Self>, weights: &[u8], threshold: u16, signing: usize) -> Self {
        use iota_sdk_types::crypto::{MultisigCommittee, MultisigMember};

        let committee = MultisigCommittee::new(
            members
                .iter()
                .zip(weights)
                .map(|(member, weight)| MultisigMember::new(member.member_public_key(), *weight))
                .collect(),
            threshold,
        )
        .unwrap();
        Self::Multisig {
            committee,
            signers: members.into_iter().take(signing).collect(),
        }
    }

    /// The key this key has as a member of a MultiSig committee.
    fn member_public_key(&self) -> iota_sdk_types::crypto::PublicKey {
        use iota_sdk_types::{PasskeyPublicKey, crypto::PublicKey};

        match self {
            Self::Keypair(keypair) => keypair.public_key(),
            Self::Passkey(keypair) => match keypair.public_key() {
                PublicKey::Secp256r1(key) => PublicKey::Passkey(PasskeyPublicKey::new(key)),
                other => panic!("a passkey is a Secp256r1 key, got {other:?}"),
            },
            Self::Multisig { .. } => panic!("a MultiSig key is no committee member"),
        }
    }

    fn scheme(&self) -> iota_sdk_types::SignatureScheme {
        match self {
            Self::Keypair(keypair) => keypair.public_key().scheme(),
            Self::Passkey(_) => iota_sdk_types::SignatureScheme::PasskeyAuthenticator,
            Self::Multisig { .. } => iota_sdk_types::SignatureScheme::Multisig,
        }
    }

    /// The public key without the scheme flag.
    fn raw_public_key(&self) -> Vec<u8> {
        match self {
            Self::Keypair(keypair) | Self::Passkey(keypair) => {
                keypair.public_key().as_ref().to_vec()
            }
            Self::Multisig { committee, .. } => bcs::to_bytes(committee).unwrap(),
        }
    }

    fn public_key(&self) -> iota_types::move_public_key::MovePublicKey {
        iota_types::move_public_key::MovePublicKey::new(self.scheme(), self.raw_public_key())
            .unwrap()
    }

    /// The address this key derives, which its claim claims.
    fn address(&self) -> Address {
        self.public_key().address().unwrap()
    }

    fn claim(&self) -> iota_sdk_types::SmartAccountClaim {
        iota_sdk_types::SmartAccountClaim {
            public_key_scheme: self.scheme().to_u8(),
            public_key_raw_bytes: self.raw_public_key(),
        }
    }

    /// This key's signature of `tx`.
    fn sign(&self, tx: &Transaction) -> UserSignature {
        use iota_sdk_crypto::Signer;
        use iota_sdk_types::{SimpleSignature, crypto::MultisigAggregatedSignature};

        let digest = tx.signing_digest();
        match self {
            Self::Keypair(keypair) => {
                let signature: SimpleSignature = keypair.sign(&digest);
                UserSignature::Simple(signature)
            }
            Self::Passkey(keypair) => passkey_signature(keypair, &digest),
            Self::Multisig { committee, signers } => {
                let signatures = signers.iter().map(|signer| signer.sign(tx)).collect();
                UserSignature::Multisig(
                    MultisigAggregatedSignature::new(signatures, committee.clone()).unwrap(),
                )
            }
        }
    }
}

/// The passkey signature, by the Secp256r1 `keypair`, of a WebAuthn assertion
/// whose challenge is `challenge`.
fn passkey_signature(
    keypair: &iota_sdk_crypto::simple::SimpleKeypair,
    challenge: &[u8],
) -> UserSignature {
    use fastcrypto::{
        hash::{HashFunction, Sha256},
        rsa::{Base64UrlUnpadded, Encoding as _},
    };
    use iota_sdk_crypto::Signer;
    use iota_sdk_types::{SimpleSignature, crypto::PasskeyAuthenticator};

    let client_data_json = format!(
        r#"{{"type":"webauthn.get","challenge":"{}","origin":"https://iota.org","crossOrigin":false}}"#,
        Base64UrlUnpadded::encode_string(challenge)
    );
    let authenticator_data = vec![0; 37];
    let mut message = authenticator_data.clone();
    message.extend_from_slice(&Sha256::digest(client_data_json.as_bytes()).digest);
    let signature: SimpleSignature = keypair.sign(&message);
    UserSignature::PasskeyAuthenticator(
        PasskeyAuthenticator::new(authenticator_data, client_data_json, signature).unwrap(),
    )
}

/// The digests of `transactions`, in order.
fn digests_of(transactions: &[VerifiedTransaction]) -> Vec<TransactionDigest> {
    transactions.iter().map(|tx| *tx.digest()).collect()
}

/// A gas coin at genesis for each of `owners`, and their ids.
fn gas_coins_for(owners: impl IntoIterator<Item = Address>) -> (Vec<ObjectId>, Vec<Object>) {
    owners
        .into_iter()
        .map(|owner| {
            let id = ObjectId::random();
            (id, Object::with_id_owner_for_testing(id, owner))
        })
        .unzip()
}

impl HandlerObjectStateSetup {
    /// `tx` signed by `signers`, in the order of the transaction's signers,
    /// and verified.
    fn sign_with(&self, tx: Transaction, signers: &[&KeyHolder]) -> VerifiedTransaction {
        let signatures = signers.iter().map(|signer| signer.sign(&tx)).collect();
        self.epoch_store
            .verify_transaction(TransactionEnvelope::from_user_sig_data(tx, signatures))
            .unwrap()
    }

    /// A `ClaimAccount` of `key`'s address, paid with its gas coin `gas_id`
    /// at its latest version, unsigned. Its budget covers the claim's gas
    /// floor for `key` at the reference gas price.
    fn claim_transaction(&self, key: &KeyHolder, gas_id: &ObjectId) -> Transaction {
        let floor = iota_types::transaction::claim_account_min_gas_budget(
            self.epoch_store.protocol_config(),
            self.rgp,
            key.raw_public_key().len() as u64,
        );
        self.claim_transaction_with_gas(
            key,
            gas_id,
            floor.max(
                iota_types::transaction::TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE
                    * self.rgp,
            ),
            self.rgp,
        )
    }

    /// [`Self::claim_transaction`] with `budget` at `gas_price`.
    fn claim_transaction_with_gas(
        &self,
        key: &KeyHolder,
        gas_id: &ObjectId,
        budget: u64,
        gas_price: u64,
    ) -> Transaction {
        self.claim_transaction_from(key.address(), key.claim(), gas_id, budget, gas_price)
    }

    /// A `ClaimAccount` carrying `claim`, sent by `sender` and paid with its
    /// gas coin `gas_id` at its latest version, unsigned. Nothing checks that
    /// `claim` is a valid claim of `sender`.
    fn claim_transaction_from(
        &self,
        sender: Address,
        claim: iota_sdk_types::SmartAccountClaim,
        gas_id: &ObjectId,
        budget: u64,
        gas_price: u64,
    ) -> Transaction {
        Transaction::new(
            iota_sdk_types::TransactionKind::new_claim_account(
                iota_sdk_types::ClaimAccountTransaction::new_smart_account(claim),
            ),
            sender,
            self.latest_ref(gas_id),
            budget,
            gas_price,
        )
    }

    /// A verified `ClaimAccount` of `key`'s address, signed by `key` and paid
    /// with its gas coin `gas_id`.
    fn build_claim_of(&self, key: &KeyHolder, gas_id: &ObjectId) -> VerifiedTransaction {
        self.sign_with(self.claim_transaction(key, gas_id), &[key])
    }

    /// An `object_basics::create` call of `value` sent by `sender` and paid
    /// with its gas coin `gas_id`, unsigned. Calls that differ only in `value`
    /// have different digests.
    fn object_basics_create(&self, sender: Address, gas_id: &ObjectId, value: u64) -> Transaction {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder
            .move_call(
                self.package_id,
                Identifier::from_static("object_basics"),
                Identifier::from_static("create"),
                vec![],
                vec![
                    CallArg::Pure(bcs::to_bytes(&value).unwrap()),
                    CallArg::Pure(bcs::to_bytes(&sender).unwrap()),
                ],
            )
            .unwrap();
        Transaction::new_programmable(
            sender,
            vec![self.latest_ref(gas_id)],
            builder.finish(),
            iota_types::transaction::TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE * self.rgp,
            self.rgp,
        )
    }

    /// A verified `object_basics::create` call of `value` sent by the claimed
    /// account at `account`, created at `initial_shared_version`, and paid with
    /// its gas coin `gas_id`. It is signed through the account's built-in
    /// authenticator with `signer`'s signature of the transaction.
    fn build_account_call(
        &self,
        account: Address,
        initial_shared_version: Version,
        signer: &KeyHolder,
        gas_id: &ObjectId,
        value: u64,
    ) -> VerifiedTransaction {
        let tx = self.object_basics_create(account, gas_id, value);
        let authenticator =
            builtin_authenticator(account, initial_shared_version, &signer.sign(&tx));
        self.epoch_store
            .verify_transaction(TransactionEnvelope::from_user_sig_data(
                tx,
                vec![UserSignature::MoveAuthenticator(MoveAuthenticator::from(
                    authenticator,
                ))],
            ))
            .unwrap()
    }

    /// The decision commit `index` saved for `digest`, if any.
    fn saved_decision(
        &self,
        index: CommitIndex,
        digest: &TransactionDigest,
    ) -> Option<PostConsensusDecision> {
        self.epoch_store
            .post_consensus_decision(index, *digest)
            .unwrap()
            .map(|saved| saved.decision)
    }

    /// The transaction holding the lock on `address` in the quarantine or in
    /// the epoch's lock table.
    fn claim_lock_holder(&self, address: Address) -> Option<TransactionDigest> {
        self.epoch_store
            .get_quarantined_owned_object_lock(&claim_lock(address))
            .or_else(|| {
                self.epoch_store
                    .tables()
                    .unwrap()
                    .get_locked_transaction(&claim_lock(address))
                    .unwrap()
            })
    }

    /// Assigns commit `index` to `txs` and executes them with their shared
    /// versions assigned directly, without asserting success.
    fn execute_at_commit_with_shared_versions(
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
        txs.into_iter()
            .map(|tx| self.execute_with_assigned_shared_versions_unchecked(tx))
            .collect()
    }

    /// Checks what `effects`, those of `key`'s claim, created: the shared
    /// account at `key`'s address, its authenticator function field naming the
    /// built-in authenticator, and its public key field holding `key`'s key.
    /// Returns the account's initial shared version.
    #[track_caller]
    fn assert_claimed_account(&self, key: &KeyHolder, effects: &TransactionEffects) -> Version {
        use iota_types::account_abstraction::authenticator_function::AuthenticatorFunctionRefV1;

        let scheme = key.scheme();
        let account_id = ObjectId::from(key.address());
        assert!(
            effects.status().is_success(),
            "{scheme:?}: {:?}",
            effects.status()
        );
        assert_eq!(
            effects.created().len(),
            3,
            "{scheme:?}: the account and its two fields, got {:?}",
            effects.created()
        );
        let account = self.authority.get_object(&account_id).unwrap();
        let account_type = account.data.as_opt_struct().unwrap().struct_tag();
        assert_eq!(
            (account_type.module().as_str(), account_type.name().as_str()),
            ("smart_account", "SmartAccount"),
            "{scheme:?}"
        );
        let initial_shared_version = initial_shared_version(&account.owner);

        let field_id = derive_authenticator_function_ref_v1_dynamic_field_id(account_id).unwrap();
        let field = self.authority.get_object(&field_id).unwrap();
        assert_eq!(field.owner, Owner::Object(account_id), "{scheme:?}");
        let (_, _, authenticator): (ObjectId, bool, AuthenticatorFunctionRefV1) =
            bcs::from_bytes(field.data.as_opt_struct().unwrap().contents()).unwrap();
        assert_eq!(
            authenticator,
            AuthenticatorFunctionRefV1 {
                package: ObjectId::FRAMEWORK,
                module: "builtin_authenticator_functions".to_string(),
                function: "builtin_authenticator_v1".to_string(),
            },
            "{scheme:?}"
        );

        let public_key_field = effects
            .created()
            .iter()
            .map(|created| created.reference().object_id)
            .filter(|id| *id != account_id && *id != field_id)
            .map(|id| self.authority.get_object(&id).unwrap())
            .next()
            .unwrap_or_else(|| panic!("{scheme:?}: the claim must create the public key field"));
        assert_eq!(
            public_key_field.owner,
            Owner::Object(account_id),
            "{scheme:?}"
        );
        let (_, _, public_key): (ObjectId, bool, iota_types::move_public_key::MovePublicKey) =
            bcs::from_bytes(public_key_field.data.as_opt_struct().unwrap().contents()).unwrap();
        assert_eq!(public_key, key.public_key(), "{scheme:?}");

        initial_shared_version
    }
}

/// The `MoveAuthenticator` for the account at `account`, created at
/// `initial_shared_version`, that passes `signature` to its built-in
/// authenticator.
fn builtin_authenticator(
    account: Address,
    initial_shared_version: Version,
    signature: &UserSignature,
) -> MoveAuthenticatorV1 {
    MoveAuthenticatorV1::new(
        vec![CallArg::Pure(bcs::to_bytes(&signature.to_bytes()).unwrap())],
        vec![],
        SharedObjectReference::new(ObjectId::from(account), initial_shared_version, false),
    )
}

/// A claim of each scheme is kept with the lock on its address, and creates a
/// shared account at that address whose authenticator function field names
/// the built-in authenticator and whose public key field holds the claimed
/// key.
#[tokio::test]
async fn claim_of_every_scheme_is_kept_and_creates_its_account() {
    let keys = KeyHolder::every_scheme(1);
    let (gas, coins) = gas_coins_for(keys.iter().map(KeyHolder::address));
    let s = setup_claim_state(coins).await;
    let claims: Vec<_> = keys
        .iter()
        .zip(&gas)
        .map(|(key, gas)| s.build_claim_of(key, gas))
        .collect();

    let result = s.validate_at(1, &claims).await;

    assert_eq!(result.kept, digests_of(&claims), "{:?}", result.dropped);
    for (key, claim) in keys.iter().zip(&claims) {
        assert_eq!(
            result.locks.get(&claim_lock(key.address())),
            Some(claim.digest()),
            "{:?}",
            key.scheme()
        );
    }
    let effects = s.execute_with_assigned_commit(claims, 1);
    for (key, effects) in keys.iter().zip(&effects) {
        s.assert_claimed_account(key, effects);
    }
}

/// A claimed account of each scheme sends through its built-in authenticator,
/// signed with the claimed key. Until the claim's commit is at the horizon,
/// the account's transaction is reported missing, so it can be sent again;
/// from then on it is kept and authenticates at execution, and in the next
/// epoch it is kept at the first commit. One signed with another key of the
/// same scheme is kept too, since the authenticator runs only at execution,
/// and fails there. One naming another initial shared version of the account
/// is dropped as a copy: the version comes from its authenticator, not from
/// the transaction data.
#[tokio::test]
async fn claimed_account_of_every_scheme_authenticates_once_its_claim_is_at_the_horizon() {
    let keys = KeyHolder::every_scheme(2);
    let others = KeyHolder::every_scheme(3);
    let (claim_gas, claim_coins) = gas_coins_for(keys.iter().map(KeyHolder::address));
    let (call_gas, call_coins) = gas_coins_for(keys.iter().map(KeyHolder::address));
    let mut s = setup_claim_state(claim_coins.into_iter().chain(call_coins).collect()).await;
    let claims: Vec<_> = keys
        .iter()
        .zip(&claim_gas)
        .map(|(key, gas)| s.build_claim_of(key, gas))
        .collect();
    let claim_effects = s.execute_with_assigned_commit(claims, 1);
    s.complete_commit(1, &claim_effects.iter().collect::<Vec<_>>());
    let versions: Vec<Version> = keys
        .iter()
        .zip(&claim_effects)
        .map(|(key, effects)| s.assert_claimed_account(key, effects))
        .collect();
    let k = s.horizon_distance();
    let calls_signed_by = |s: &HandlerObjectStateSetup,
                           signers: &[KeyHolder],
                           value: u64|
     -> Vec<VerifiedTransaction> {
        keys.iter()
            .zip(signers)
            .zip(&versions)
            .zip(&call_gas)
            .map(|(((key, signer), version), gas)| {
                s.build_account_call(key.address(), *version, signer, gas, value)
            })
            .collect()
    };

    let calls = calls_signed_by(&s, &keys, 1);
    let result = s.validate_at(k, &calls).await;
    assert!(result.kept.is_empty(), "{:?}", result.kept);
    for (key, call) in keys.iter().zip(&calls) {
        assert_eq!(
            s.saved_decision(k, call.digest()),
            Some(PostConsensusDecision::DroppedInputNotVisibleYet),
            "{:?}",
            key.scheme()
        );
    }

    // Each later commit validates at a horizon that has the coins the
    // previous one moved.
    let mut commit = k + 1;
    let result = s.validate_at(commit, &calls).await;
    assert_eq!(result.kept, digests_of(&calls), "{:?}", result.dropped);
    let effects = s.execute_at_commit_with_shared_versions(calls, commit);
    for (key, effects) in keys.iter().zip(&effects) {
        assert!(
            effects.status().is_success(),
            "{:?}: {:?}",
            key.scheme(),
            effects.status()
        );
        assert_eq!(effects.created().len(), 1, "{:?}", key.scheme());
    }
    s.complete_commit(commit, &effects.iter().collect::<Vec<_>>());

    commit += k;
    let forged = calls_signed_by(&s, &others, 2);
    let result = s.validate_at(commit, &forged).await;
    assert_eq!(result.kept, digests_of(&forged), "{:?}", result.dropped);
    let effects = s.execute_at_commit_with_shared_versions(forged, commit);
    for (key, effects) in keys.iter().zip(&effects) {
        assert!(
            matches!(
                effects.status(),
                ExecutionStatus::Failure {
                    error: iota_sdk_types::ExecutionError::MoveAuthentication { .. },
                    ..
                }
            ),
            "{:?}: a transaction signed with another key must fail its authentication, got {:?}",
            key.scheme(),
            effects.status()
        );
        assert!(effects.created().is_empty(), "{:?}", key.scheme());
    }
    s.complete_commit(commit, &effects.iter().collect::<Vec<_>>());

    commit += k;
    let stale: Vec<_> = keys
        .iter()
        .zip(&versions)
        .zip(&call_gas)
        .map(|((key, version), gas)| {
            s.build_account_call(
                key.address(),
                Version::from_u64(version.as_u64() + 1),
                key,
                gas,
                3,
            )
        })
        .collect();
    let result = s.validate_at(commit, &stale).await;
    assert!(result.kept.is_empty(), "{:?}", result.kept);
    for (key, stale) in keys.iter().zip(&stale) {
        assert_eq!(
            s.saved_decision(commit, stale.digest()),
            Some(PostConsensusDecision::DroppedCopy),
            "{:?}: {:?}",
            key.scheme(),
            result.dropped
        );
    }

    s.advance_epoch().await;
    let calls = calls_signed_by(&s, &keys, 4);
    let result = s.validate_at(1, &calls).await;
    assert_eq!(result.kept, digests_of(&calls), "{:?}", result.dropped);
}

/// Every claimed key the validity check accepts creates its account: a key of
/// each scheme, and MultiSig committees of one member of each scheme, of the
/// most members of one scheme (ten Passkey members make the longest key a
/// claim can carry), of mixed schemes, of the highest weights, and with a
/// member whose bytes are no curve point, which neither the validity check
/// nor Move checks. A key the validity check accepted and Move refused would
/// be kept and fail, and its address would stay locked until the epoch ends.
/// The claims reach validation unverified: the test is about the keys.
#[tokio::test]
async fn every_key_the_validity_check_accepts_creates_its_account() {
    use iota_sdk_crypto::{
        ed25519::Ed25519PrivateKey, secp256k1::Secp256k1PrivateKey, secp256r1::Secp256r1PrivateKey,
        simple::SimpleKeypair,
    };
    use iota_sdk_types::crypto::{
        MULTISIG_COMMITTEE_SIZE_MAX, MultisigCommittee, MultisigMember, PublicKey,
        Secp256k1PublicKey,
    };

    let ed25519 = || KeyHolder::Keypair(SimpleKeypair::from(Ed25519PrivateKey::random()));
    let k1 = || KeyHolder::Keypair(SimpleKeypair::from(Secp256k1PrivateKey::random()));
    let r1 = || KeyHolder::Keypair(SimpleKeypair::from(Secp256r1PrivateKey::random()));
    let passkey = || KeyHolder::Passkey(SimpleKeypair::from(Secp256r1PrivateKey::random()));
    let one_member = |member: KeyHolder| KeyHolder::multisig(vec![member], &[1], 1, 1);
    let most_members = |member: &dyn Fn() -> KeyHolder| {
        KeyHolder::multisig(
            (0..MULTISIG_COMMITTEE_SIZE_MAX).map(|_| member()).collect(),
            &[1; MULTISIG_COMMITTEE_SIZE_MAX],
            1,
            1,
        )
    };
    let signer = r1();
    let off_curve = MultisigCommittee::new(
        vec![
            MultisigMember::new(PublicKey::Secp256k1(Secp256k1PublicKey::new([0x05; 33])), 1),
            MultisigMember::new(signer.member_public_key(), 1),
        ],
        1,
    )
    .unwrap();
    let keys = vec![
        ed25519(),
        k1(),
        r1(),
        passkey(),
        one_member(ed25519()),
        one_member(k1()),
        one_member(r1()),
        one_member(passkey()),
        most_members(&ed25519),
        most_members(&k1),
        most_members(&r1),
        most_members(&passkey),
        KeyHolder::multisig(vec![k1(), r1(), passkey()], &[1, 2, 3], 6, 3),
        KeyHolder::multisig(
            vec![passkey(), k1()],
            &[u8::MAX, u8::MAX],
            2 * u16::from(u8::MAX),
            2,
        ),
        KeyHolder::Multisig {
            committee: off_curve,
            signers: vec![signer],
        },
    ];
    let longest = keys
        .iter()
        .map(|key| key.raw_public_key().len())
        .max()
        .unwrap();
    assert_eq!(longest, 353, "ten Passkey members make the longest key");
    let (gas, coins) = gas_coins_for(keys.iter().map(KeyHolder::address));
    let s = setup_claim_state(coins).await;
    let claims: Vec<_> = keys
        .iter()
        .zip(&gas)
        .map(|(key, gas)| {
            let tx = s.claim_transaction(key, gas);
            let claim = TransactionEnvelope::from_user_sig_data(tx.clone(), vec![key.sign(&tx)]);
            claim
                .validity_check(&s.epoch_store.tx_validity_check_context())
                .unwrap_or_else(|error| {
                    panic!(
                        "{:?} key {:?}: {error:?}",
                        key.scheme(),
                        key.raw_public_key()
                    )
                });
            VerifiedTransaction::new_unchecked(claim)
        })
        .collect();

    let result = s.validate_at(1, &claims).await;

    assert_eq!(result.kept, digests_of(&claims), "{:?}", result.dropped);
    let effects = s.execute_with_assigned_commit(claims, 1);
    for (key, effects) in keys.iter().zip(&effects) {
        s.assert_claimed_account(key, effects);
    }
}

/// A claim paid by another address is kept and creates the sender's account,
/// and the sponsor's coin pays for it. The sponsor signs with a plain
/// signature and is not claimed: its address takes no claim lock, and its own
/// plain-signed transactions are kept in the claim's commit and once the
/// horizon has the claim.
#[tokio::test]
async fn sponsored_claim_is_kept_and_creates_the_senders_account() {
    let keys = KeyHolder::every_scheme(20);
    let sponsor = KeyHolder::every_scheme(21).swap_remove(0);
    let (sponsor_gas, sponsor_coins) = gas_coins_for(keys.iter().map(|_| sponsor.address()));
    let (own_gas, own_coins) = gas_coins_for([sponsor.address(), sponsor.address()]);
    let s = setup_claim_state(sponsor_coins.into_iter().chain(own_coins).collect()).await;
    let mut commit_one: Vec<_> = keys
        .iter()
        .zip(&sponsor_gas)
        .map(|(key, gas)| {
            let mut tx = s.claim_transaction(key, gas);
            tx.gas_data_mut().owner = sponsor.address();
            s.sign_with(tx, &[key, &sponsor])
        })
        .collect();
    commit_one.push(s.sign_with(
        s.object_basics_create(sponsor.address(), &own_gas[0], 1),
        &[&sponsor],
    ));

    let result = s.validate_at(1, &commit_one).await;

    assert_eq!(result.kept, digests_of(&commit_one), "{:?}", result.dropped);
    assert_eq!(result.locks.get(&claim_lock(sponsor.address())), None);
    for (key, claim) in keys.iter().zip(&commit_one) {
        assert_eq!(
            result.locks.get(&claim_lock(key.address())),
            Some(claim.digest()),
            "{:?}",
            key.scheme()
        );
    }
    let effects = s.execute_with_assigned_commit(commit_one, 1);
    s.complete_commit(1, &effects.iter().collect::<Vec<_>>());
    for ((key, gas), effects) in keys.iter().zip(&sponsor_gas).zip(&effects) {
        s.assert_claimed_account(key, effects);
        assert_eq!(
            effects.gas_object().reference().object_id,
            *gas,
            "{:?}: the sponsor's coin must pay",
            key.scheme()
        );
    }

    let own = s.sign_with(
        s.object_basics_create(sponsor.address(), &own_gas[1], 2),
        &[&sponsor],
    );
    let result = s
        .validate_at(1 + s.horizon_distance(), std::slice::from_ref(&own))
        .await;
    assert_eq!(result.kept, vec![*own.digest()], "{:?}", result.dropped);
}

/// A claim of each scheme at exactly its gas floor, paid with one coin, at the
/// reference gas price and at the highest one, is kept and creates its
/// account within the computation the floor assumes. The floor is checked
/// with the costliest key elsewhere; this covers the key of each scheme.
#[tokio::test]
async fn claim_at_its_gas_floor_succeeds_for_every_scheme() {
    let keys: Vec<_> = KeyHolder::every_scheme(30)
        .into_iter()
        .chain(KeyHolder::every_scheme(31))
        .collect();
    let (gas, coins) = gas_coins_for(keys.iter().map(KeyHolder::address));
    let s = setup_claim_state(coins).await;
    let config = s.epoch_store.protocol_config();
    let schemes = keys.len() / 2;
    let prices: Vec<u64> = (0..keys.len())
        .map(|index| {
            if index < schemes {
                s.rgp
            } else {
                config.max_gas_price()
            }
        })
        .collect();
    let claims: Vec<_> = keys
        .iter()
        .zip(&gas)
        .zip(&prices)
        .map(|((key, gas), price)| {
            let floor = iota_types::transaction::claim_account_min_gas_budget(
                config,
                *price,
                key.raw_public_key().len() as u64,
            );
            s.sign_with(
                s.claim_transaction_with_gas(key, gas, floor, *price),
                &[key],
            )
        })
        .collect();

    let result = s.validate_at(1, &claims).await;

    assert_eq!(result.kept, digests_of(&claims), "{:?}", result.dropped);
    let effects = s.execute_with_assigned_commit(claims, 1);
    for ((key, effects), price) in keys.iter().zip(&effects).zip(&prices) {
        s.assert_claimed_account(key, effects);
        assert!(
            effects.gas_cost_summary().computation_cost
                <= config.claim_account_max_computation_units() * price,
            "{:?} at {price}: {:?}",
            key.scheme(),
            effects.gas_cost_summary()
        );
    }
}

// ---------------------------------------------------------------------------
// ClaimAccount: a key that does not derive the sender, and other invalid
// claims
// ---------------------------------------------------------------------------

/// A claim the validity check refuses is dropped at the commit for good, with
/// the error admission gives, and takes no claim lock, on its sender or on the
/// address its key derives. The claims are signed by their senders, as a block
/// author can sequence them. The rightful claims of those addresses, later in
/// the same commit, are kept.
#[tokio::test]
async fn invalid_claims_are_dropped_at_the_commit_and_take_no_lock() {
    use iota_sdk_types::{SignatureScheme, SmartAccountClaim};

    let keys = KeyHolder::every_scheme(40);
    let [ed25519, k1, r1, passkey, multisig] = &keys[..] else {
        panic!("one key of each scheme");
    };
    let KeyHolder::Multisig { signers, .. } = multisig else {
        panic!("the last key is a MultiSig key");
    };
    let member = &signers[0];
    let raw = |scheme: u8, raw_public_key: Vec<u8>| SmartAccountClaim {
        public_key_scheme: scheme,
        public_key_raw_bytes: raw_public_key,
    };
    let mut uncompressed = vec![0x04];
    uncompressed.extend([0x11; 64]);
    // (case, sender, claim, whether the budget is one below the floor)
    let cases: Vec<(&str, &KeyHolder, SmartAccountClaim, bool)> = vec![
        ("key of another address", ed25519, k1.claim(), false),
        (
            "a member's key for the committee's address",
            multisig,
            member.claim(),
            false,
        ),
        (
            "Secp256r1 key under the Passkey flag",
            r1,
            raw(
                SignatureScheme::PasskeyAuthenticator.to_u8(),
                r1.raw_public_key(),
            ),
            false,
        ),
        (
            "BLS flag",
            ed25519,
            raw(SignatureScheme::Bls12381.to_u8(), ed25519.raw_public_key()),
            false,
        ),
        (
            "zkLogin flag",
            ed25519,
            raw(0x05, ed25519.raw_public_key()),
            false,
        ),
        (
            "MoveAuthenticator flag",
            ed25519,
            raw(
                SignatureScheme::MoveAuthenticator.to_u8(),
                ed25519.raw_public_key(),
            ),
            false,
        ),
        (
            "unknown flag",
            ed25519,
            raw(0xff, ed25519.raw_public_key()),
            false,
        ),
        (
            "uncompressed key",
            k1,
            raw(SignatureScheme::Secp256k1.to_u8(), uncompressed),
            false,
        ),
        ("budget below the gas floor", passkey, passkey.claim(), true),
    ];
    let (invalid_gas, invalid_coins) =
        gas_coins_for(cases.iter().map(|(_, sender, ..)| sender.address()));
    let (rightful_gas, rightful_coins) = gas_coins_for(keys.iter().map(KeyHolder::address));
    let s = setup_claim_state(invalid_coins.into_iter().chain(rightful_coins).collect()).await;
    let invalid: Vec<_> = cases
        .iter()
        .zip(&invalid_gas)
        .map(|((_, sender, claim, below_floor), gas)| {
            let floor = iota_types::transaction::claim_account_min_gas_budget(
                s.epoch_store.protocol_config(),
                s.rgp,
                claim.public_key_raw_bytes.len() as u64,
            );
            let budget = if *below_floor {
                floor - 1
            } else {
                floor.max(
                    iota_types::transaction::TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE
                        * s.rgp,
                )
            };
            s.sign_with(
                s.claim_transaction_from(sender.address(), claim.clone(), gas, budget, s.rgp),
                &[*sender],
            )
        })
        .collect();
    let admission_errors: Vec<IotaError> = cases
        .iter()
        .zip(&invalid)
        .map(|((case, ..), claim)| {
            claim
                .validity_check(&s.epoch_store.tx_validity_check_context())
                .expect_err(case)
        })
        .collect();
    let rightful: Vec<_> = keys
        .iter()
        .zip(&rightful_gas)
        .map(|(key, gas)| s.build_claim_of(key, gas))
        .collect();

    let result = s
        .validate_at(1, &[invalid.clone(), rightful.clone()].concat())
        .await;

    assert_eq!(
        result.dropped,
        digests_of(&invalid)
            .into_iter()
            .zip(admission_errors)
            .collect::<Vec<_>>()
    );
    for ((case, ..), claim) in cases.iter().zip(&invalid) {
        assert_eq!(
            s.saved_decision(1, claim.digest()),
            Some(PostConsensusDecision::Dropped),
            "{case}"
        );
        assert!(
            !result.locks.values().any(|holder| holder == claim.digest()),
            "{case}: a dropped claim takes no lock"
        );
    }
    assert_eq!(result.kept, digests_of(&rightful));
    for (key, claim) in keys.iter().zip(&rightful) {
        assert_eq!(
            result.locks.get(&claim_lock(key.address())),
            Some(claim.digest()),
            "{:?}",
            key.scheme()
        );
    }
    assert_eq!(result.locks.get(&claim_lock(member.address())), None);
}

// ---------------------------------------------------------------------------
// ClaimAccount: where a claim is recorded when a later transaction of its
// address is validated
// ---------------------------------------------------------------------------

/// Where a kept claim of an address is recorded when a later transaction of
/// the address is validated.
#[derive(Debug, Clone, Copy)]
enum ClaimRecord {
    /// The claim is earlier in the same commit: the commit's own lock map.
    SameCommit,
    /// The claim was kept at an earlier commit whose output is in the
    /// quarantine.
    Quarantine,
    /// As `Quarantine`, and the claim executed: its field is in the latest
    /// state, above the horizon.
    ExecutedAboveTheHorizon,
    /// The claim was kept, executed and flushed: its lock is in the epoch's
    /// lock table.
    LockTable,
    /// As `LockTable`, after a restart.
    LockTableAfterARestart,
    /// The claim executed at a commit at the horizon, without this commit
    /// processing keeping it, so only the horizon read sees it.
    AtTheHorizon,
    /// The claim executed in the previous epoch.
    EarlierEpoch,
}

impl ClaimRecord {
    const ALL: [Self; 7] = [
        Self::SameCommit,
        Self::Quarantine,
        Self::ExecutedAboveTheHorizon,
        Self::LockTable,
        Self::LockTableAfterARestart,
        Self::AtTheHorizon,
        Self::EarlierEpoch,
    ];

    /// Whether the claim's lock is held when the later transaction is
    /// validated.
    fn holds_the_lock(self) -> bool {
        !matches!(self, Self::AtTheHorizon | Self::EarlierEpoch)
    }
}

/// Waiters, registered before a commit is processed, of the drop of one
/// transaction: as the transaction's status, which clients wait for, and as
/// the drop of any copy of it, which the consensus adapter waits for.
struct DropWaiters {
    status: tokio::task::JoinHandle<IotaError>,
    any_copy: tokio::task::JoinHandle<IotaError>,
}

impl DropWaiters {
    /// What each waiter was told, if anything, once the commit is processed.
    async fn told(self) -> (Option<IotaError>, Option<IotaError>) {
        // A waiter that was not told is stopped, so that it does not keep the
        // epoch store, and its DB, open.
        let told = |mut handle: tokio::task::JoinHandle<IotaError>| async move {
            match tokio::time::timeout(Duration::from_millis(300), &mut handle).await {
                Ok(told) => Some(told.unwrap()),
                Err(_) => {
                    handle.abort();
                    let _ = handle.await;
                    None
                }
            }
        };
        (told(self.status).await, told(self.any_copy).await)
    }
}

impl HandlerObjectStateSetup {
    /// [`Self::process_at`], failing the test if the commit waits for its
    /// horizon instead of being processed at once.
    async fn process_at_promptly(
        &self,
        index: CommitIndex,
        transactions: &[VerifiedTransaction],
    ) -> Vec<TransactionDigest> {
        tokio::time::timeout(
            Duration::from_secs(10),
            self.process_at(index, transactions),
        )
        .await
        .unwrap_or_else(|_| panic!("commit {index} must not wait"))
        .iter()
        .map(|tx| *tx.digest())
        .collect()
    }

    /// [`Self::process_at_with_versions`], failing the test if the commit
    /// waits for its horizon instead of being processed at once.
    async fn process_at_with_versions_promptly(
        &self,
        index: CommitIndex,
        transactions: &[VerifiedTransaction],
    ) -> (Vec<VerifiedExecutableTransaction>, AssignedTxAndVersions) {
        tokio::time::timeout(
            Duration::from_secs(10),
            self.process_at_with_versions(index, transactions),
        )
        .await
        .unwrap_or_else(|_| panic!("commit {index} must not wait"))
    }

    /// The signatures of the copies the handler keeps when it processes
    /// `transactions` at commit `index`.
    async fn kept_signatures_at(
        &self,
        index: CommitIndex,
        transactions: &[VerifiedTransaction],
    ) -> Vec<Vec<UserSignature>> {
        self.process_at_with_versions_promptly(index, transactions)
            .await
            .0
            .iter()
            .map(|tx| tx.data().signatures().to_vec())
            .collect()
    }

    /// Registers the waiters of the drop of `digest`.
    async fn drop_waiters(&self, digest: TransactionDigest) -> DropWaiters {
        let status = {
            let epoch_store = self.epoch_store.clone();
            tokio::spawn(async move { epoch_store.notify_read_dropped_digests(digest).await })
        };
        let any_copy = {
            let epoch_store = self.epoch_store.clone();
            tokio::spawn(async move { epoch_store.notify_read_any_drop(digest).await })
        };
        // Let both register before the commit is processed.
        tokio::time::sleep(Duration::from_millis(20)).await;
        DropWaiters { status, any_copy }
    }

    /// Brings `claim`, a valid claim, to the state `record` names, and returns
    /// the commit at which a later transaction of its address is processed,
    /// with the transactions that commit has before it: the claim for
    /// `SameCommit`, none otherwise.
    async fn record_claim(
        &mut self,
        claim: &VerifiedTransaction,
        record: ClaimRecord,
    ) -> (CommitIndex, Vec<VerifiedTransaction>) {
        let keep = |scheduled: Vec<TransactionDigest>| {
            assert_eq!(scheduled, vec![*claim.digest()], "the claim must be kept");
        };
        match record {
            ClaimRecord::SameCommit => (1, vec![claim.clone()]),
            ClaimRecord::Quarantine => {
                keep(
                    self.process_at_promptly(1, std::slice::from_ref(claim))
                        .await,
                );
                (2, vec![])
            }
            ClaimRecord::ExecutedAboveTheHorizon => {
                keep(
                    self.process_at_promptly(1, std::slice::from_ref(claim))
                        .await,
                );
                let effects = self.execute(claim.clone());
                self.complete_commit(1, &[&effects]);
                (2, vec![])
            }
            ClaimRecord::LockTable | ClaimRecord::LockTableAfterARestart => {
                keep(
                    self.process_at_promptly(1, std::slice::from_ref(claim))
                        .await,
                );
                let effects = self.execute(claim.clone());
                let height = self
                    .checkpoint_roots()
                    .into_iter()
                    .map(|(height, _)| height)
                    .max()
                    .expect("commit 1 has a pending checkpoint");
                self.epoch_store
                    .flush_pushed_commits_for_testing(
                        height,
                        1,
                        vec![*effects.transaction_digest()],
                    )
                    .unwrap();
                if matches!(record, ClaimRecord::LockTableAfterARestart) {
                    self.epoch_store
                        .persist_checkpoint_bookkeeping([&effects])
                        .unwrap();
                    self.restart_epoch_store().await;
                }
                assert_eq!(
                    self.epoch_store
                        .tables()
                        .unwrap()
                        .get_locked_transaction(&claim_lock(claim.data().transaction().sender()))
                        .unwrap(),
                    Some(*claim.digest())
                );
                (2, vec![])
            }
            ClaimRecord::AtTheHorizon => {
                let effects = self.execute_claim_with_assigned_commit(claim.clone(), 1);
                self.complete_commit(1, &[&effects]);
                (1 + self.horizon_distance(), vec![])
            }
            ClaimRecord::EarlierEpoch => {
                let effects = self.execute_claim_with_assigned_commit(claim.clone(), 1);
                self.complete_commit(1, &[&effects]);
                self.advance_epoch().await;
                (1, vec![])
            }
        }
    }
}

/// A second claim of an address gets the same answer wherever the first is
/// recorded: in the same commit, in the quarantine, executed above the
/// horizon, in the lock table, in the lock table after a restart, only at the
/// horizon, or in the previous epoch. It is dropped for good with
/// `AccountAlreadyExplicit`, and its submitter is told. It locks neither the
/// address nor its gas coin.
#[tokio::test]
async fn second_claim_gets_the_same_answer_wherever_the_first_is_recorded() {
    for record in ClaimRecord::ALL {
        let key = KeyHolder::every_scheme(50).swap_remove(1);
        let address = key.address();
        let (gas, coins) = gas_coins_for([address, address]);
        let mut s = setup_claim_state(coins).await;
        let first = s.build_claim_of(&key, &gas[0]);
        let second = s.build_claim_of(&key, &gas[1]);
        let second_gas = s.latest_ref(&gas[1]);
        let (commit, before) = s.record_claim(&first, record).await;
        let waiters = s.drop_waiters(*second.digest()).await;

        let scheduled = s
            .process_at_promptly(commit, &[before.clone(), vec![second.clone()]].concat())
            .await;

        assert_eq!(scheduled, digests_of(&before), "{record:?}");
        assert_eq!(
            s.saved_decision(commit, second.digest()),
            Some(PostConsensusDecision::Dropped),
            "{record:?}"
        );
        assert_eq!(
            waiters.told().await,
            (
                Some(already_explicit(address)),
                Some(already_explicit(address))
            ),
            "{record:?}"
        );
        assert_eq!(
            s.epoch_store.get_quarantined_owned_object_lock(&second_gas),
            None,
            "{record:?}"
        );
        assert_eq!(
            s.claim_lock_holder(address),
            record.holds_the_lock().then_some(*first.digest()),
            "{record:?}"
        );
    }
}

/// A second claim paying with the first claim's gas coin is dropped on the
/// coin's lock, which is checked before the claim's: `ObjectLockConflict`,
/// for good.
#[tokio::test]
async fn second_claim_with_the_first_claims_gas_coin_is_dropped_on_the_coin() {
    let key = KeyHolder::every_scheme(51).swap_remove(0);
    let (gas, coins) = gas_coins_for([key.address()]);
    let s = setup_claim_state(coins).await;
    let first = s.build_claim_of(&key, &gas[0]);
    let mut tx = s.claim_transaction(&key, &gas[0]);
    tx.gas_data_mut().budget += 1;
    let second = s.sign_with(tx, &[&key]);

    let result = s.validate_at(1, &[first.clone(), second.clone()]).await;

    assert_eq!(result.kept, vec![*first.digest()]);
    assert_eq!(
        result.dropped,
        vec![(
            *second.digest(),
            IotaError::ObjectLockConflict {
                obj_ref: s.latest_ref(&gas[0]),
                pending_transaction: *first.digest(),
            }
        )]
    );
    assert_eq!(
        s.saved_decision(1, second.digest()),
        Some(PostConsensusDecision::Dropped)
    );
}

/// What happened to `bad`, a claim dropped at `commit`, and checks that its
/// address can still be claimed: `good`, a valid claim of the address paying
/// with another coin, is kept after `bad` in that commit; `bad` takes no claim
/// lock when the handler processes it at the next commit; and `good` is kept
/// at the commit after that. Returns `bad`'s error and saved decision.
async fn dropped_claim_leaves_the_address_claimable(
    s: &HandlerObjectStateSetup,
    bad: &VerifiedTransaction,
    good: &VerifiedTransaction,
    commit: CommitIndex,
) -> (IotaError, PostConsensusDecision) {
    let address = good.data().transaction().sender();
    let result = s.validate_at(commit, &[bad.clone(), good.clone()]).await;
    assert_eq!(result.kept, vec![*good.digest()], "{:?}", result.dropped);
    assert_eq!(result.locks.get(&claim_lock(address)), Some(good.digest()));
    let [(digest, error)] = &result.dropped[..] else {
        panic!("only the bad claim drops: {:?}", result.dropped);
    };
    assert_eq!(digest, bad.digest());
    let decision = s.saved_decision(commit, bad.digest()).unwrap();

    let scheduled = s
        .process_at_promptly(commit + 1, std::slice::from_ref(bad))
        .await;
    assert!(scheduled.is_empty(), "{scheduled:?}");
    assert_eq!(s.claim_lock_holder(address), None);

    let result = s.validate_at(commit + 2, std::slice::from_ref(good)).await;
    assert_eq!(result.kept, vec![*good.digest()], "{:?}", result.dropped);
    (error.clone(), decision)
}

/// A claim dropped for a reason other than its address being claimed leaves
/// the address claimable, in the same commit and later: when its gas coin is
/// locked by an earlier transaction, consumed, not visible yet, owned by
/// another address or short of the budget, and when its sponsor is explicit.
/// A claim dropped because its coin is not visible yet is kept once the coin
/// is.
#[tokio::test]
async fn dropped_claim_leaves_its_address_claimable() {
    let keys = KeyHolder::every_scheme(52);
    let key = &keys[0];
    let address = key.address();
    let k = 4;

    // Its gas coin is locked by an earlier transaction of the address.
    let (gas, coins) = gas_coins_for([address, address]);
    let s = setup_claim_state(coins).await;
    let earlier = s.sign_with(s.object_basics_create(address, &gas[0], 1), &[key]);
    assert_eq!(
        s.process_at_promptly(1, std::slice::from_ref(&earlier))
            .await,
        vec![*earlier.digest()]
    );
    let bad = s.build_claim_of(key, &gas[0]);
    let good = s.build_claim_of(key, &gas[1]);
    let (error, decision) = dropped_claim_leaves_the_address_claimable(&s, &bad, &good, 2).await;
    assert_eq!(
        error,
        IotaError::ObjectLockConflict {
            obj_ref: s.latest_ref(&gas[0]),
            pending_transaction: *earlier.digest(),
        }
    );
    assert_eq!(decision, PostConsensusDecision::Dropped);

    // Each setup holds the protocol config override until it is dropped.
    drop(s);

    // Its gas coin was consumed at a commit at the horizon.
    let (gas, coins) = gas_coins_for([address, address]);
    let s = setup_claim_state(coins).await;
    assert_eq!(s.horizon_distance(), k);
    let bad = s.build_claim_of(key, &gas[0]);
    let good = s.build_claim_of(key, &gas[1]);
    let spend = s.sign_with(s.object_basics_create(address, &gas[0], 1), &[key]);
    let spent = s.execute_with_assigned_commit(vec![spend], 1);
    s.complete_commit(1, &[&spent[0]]);
    s.complete_empty_commit(2);
    let (error, decision) =
        dropped_claim_leaves_the_address_claimable(&s, &bad, &good, k + 1).await;
    assert!(
        matches!(
            error,
            IotaError::UserInput {
                error: UserInputError::InputObjectNotAvailableAtCommit { .. }
            }
        ),
        "{error:?}"
    );
    assert_eq!(decision, PostConsensusDecision::Dropped);

    drop(s);

    // Its gas coin is at a version produced above the horizon; once the
    // horizon has it, the same claim is kept.
    let (gas, coins) = gas_coins_for([address, address]);
    let s = setup_claim_state(coins).await;
    let spend = s.sign_with(s.object_basics_create(address, &gas[0], 1), &[key]);
    let spent = s.execute_with_assigned_commit(vec![spend], 1);
    s.complete_commit(1, &[&spent[0]]);
    let bad = s.build_claim_of(key, &gas[0]);
    let good = s.build_claim_of(key, &gas[1]);
    let (error, decision) = dropped_claim_leaves_the_address_claimable(&s, &bad, &good, 2).await;
    assert!(
        matches!(
            error,
            IotaError::UserInput {
                error: UserInputError::ObjectNotFound { .. }
            }
        ),
        "{error:?}"
    );
    assert_eq!(decision, PostConsensusDecision::DroppedInputNotVisibleYet);
    let result = s.validate_at(k + 1, std::slice::from_ref(&bad)).await;
    assert_eq!(result.kept, vec![*bad.digest()], "{:?}", result.dropped);

    drop(s);

    // Its gas coin belongs to another address.
    let other = &keys[1];
    let (gas, coins) = gas_coins_for([other.address(), address]);
    let s = setup_claim_state(coins).await;
    let bad = s.build_claim_of(key, &gas[0]);
    let good = s.build_claim_of(key, &gas[1]);
    let (error, decision) = dropped_claim_leaves_the_address_claimable(&s, &bad, &good, 1).await;
    assert!(matches!(error, IotaError::UserInput { .. }), "{error:?}");
    assert_eq!(decision, PostConsensusDecision::Dropped);

    drop(s);

    // Its gas coin holds less than the budget.
    let short_id = ObjectId::random();
    let (gas, coins) = gas_coins_for([address]);
    let s = setup_claim_state(
        coins
            .into_iter()
            .chain([Object::with_id_owner_gas_for_testing(
                short_id, address, 1_000,
            )])
            .collect(),
    )
    .await;
    let bad = s.build_claim_of(key, &short_id);
    let good = s.build_claim_of(key, &gas[0]);
    let (error, decision) = dropped_claim_leaves_the_address_claimable(&s, &bad, &good, 1).await;
    assert!(
        matches!(
            error,
            IotaError::UserInput {
                error: UserInputError::GasBalanceTooLow { .. }
            }
        ),
        "{error:?}"
    );
    assert_eq!(decision, PostConsensusDecision::Dropped);

    drop(s);

    // Its sponsor is explicit.
    let sponsor = &keys[1];
    let (gas, coins) = gas_coins_for([sponsor.address(), sponsor.address(), address]);
    let s = setup_claim_state(coins).await;
    let sponsor_claim = s.build_claim_of(sponsor, &gas[0]);
    assert_eq!(
        s.process_at_promptly(1, std::slice::from_ref(&sponsor_claim))
            .await,
        vec![*sponsor_claim.digest()]
    );
    let mut tx = s.claim_transaction(key, &gas[1]);
    tx.gas_data_mut().owner = sponsor.address();
    let bad = s.sign_with(tx, &[key, sponsor]);
    let good = s.build_claim_of(key, &gas[2]);
    let (error, decision) = dropped_claim_leaves_the_address_claimable(&s, &bad, &good, 2).await;
    assert_eq!(error, plain_signature_for(sponsor.address()));
    assert_eq!(decision, PostConsensusDecision::Dropped);
}

/// Copies of a kept claim are not second claims. In the claim's commit, an
/// identical copy and a MultiSig copy signed by other members are removed
/// without a drop; at later commits, before and after the claim executes, a
/// copy is not processed again. The claim keeps its lock, runs once, and no
/// copy is reported dropped.
#[tokio::test]
async fn copy_of_a_kept_claim_is_not_a_second_claim() {
    let signed_by = |signing: &[usize]| {
        KeyHolder::multisig_signed_by(KeyHolder::members(53), &[1, 1, 1], 2, signing)
    };
    let key = signed_by(&[0, 1]);
    let address = key.address();
    assert_eq!(signed_by(&[1, 2]).address(), address);
    let (gas, coins) = gas_coins_for([address]);
    let s = setup_claim_state(coins).await;
    let tx = s.claim_transaction(&key, &gas[0]);
    let claim = s.sign_with(tx.clone(), &[&key]);
    let other_copy = s.sign_with(tx, &[&signed_by(&[1, 2])]);
    assert_eq!(other_copy.digest(), claim.digest());
    assert_ne!(other_copy.data().signatures(), claim.data().signatures());
    let waiters = s.drop_waiters(*claim.digest()).await;

    let scheduled = s
        .process_at_promptly(1, &[claim.clone(), claim.clone(), other_copy.clone()])
        .await;

    assert_eq!(scheduled, vec![*claim.digest()]);
    assert_eq!(s.claim_lock_holder(address), Some(*claim.digest()));
    assert!(
        s.process_at_promptly(2, std::slice::from_ref(&other_copy))
            .await
            .is_empty()
    );
    let effects = s.execute(claim.clone());
    s.complete_commit(1, &[&effects]);
    assert!(
        s.process_at_promptly(3, std::slice::from_ref(&claim))
            .await
            .is_empty()
    );
    assert_eq!(waiters.told().await, (None, None));
    assert_eq!(s.claim_lock_holder(address), Some(*claim.digest()));
    s.assert_claimed_account(&key, &effects);
}

/// A copy of a claim carrying a made-up `MoveAuthenticator`, as a block author
/// can put in its block, fails the validity check for that copy only: it is
/// dropped as a copy, and the claim's real copy is kept and takes the lock
/// once. After the real copy, the made-up one is removed without a check. No
/// copy is reported dropped.
#[tokio::test]
async fn claim_copy_with_a_move_authenticator_does_not_decide_the_claim() {
    let keys = KeyHolder::every_scheme(54);
    let (gas, coins) = gas_coins_for(keys.iter().take(2).map(KeyHolder::address));
    let s = setup_claim_state(coins).await;
    let copies = |key: &KeyHolder, gas: &ObjectId| {
        let tx = s.claim_transaction(key, gas);
        let made_up = VerifiedTransaction::new_unchecked(TransactionEnvelope::from_user_sig_data(
            tx.clone(),
            vec![UserSignature::MoveAuthenticator(MoveAuthenticator::from(
                builtin_authenticator(key.address(), Version::OBJECT_START, &key.sign(&tx)),
            ))],
        ));
        (made_up, s.sign_with(tx, &[key]))
    };
    let (made_up_first, real_second) = copies(&keys[0], &gas[0]);
    let (made_up_second, real_first) = copies(&keys[1], &gas[1]);
    let waiters = [
        s.drop_waiters(*real_second.digest()).await,
        s.drop_waiters(*real_first.digest()).await,
    ];
    let (copies, _) = s
        .process_at_with_versions(
            1,
            &[
                made_up_first,
                real_second.clone(),
                real_first.clone(),
                made_up_second,
            ],
        )
        .await;

    assert_eq!(
        copies
            .iter()
            .map(|copy| (*copy.digest(), copy.data().signatures().to_vec()))
            .collect::<Vec<_>>(),
        [&real_second, &real_first]
            .map(|real| (*real.digest(), real.data().signatures().to_vec()))
            .to_vec()
    );
    for (key, real) in keys.iter().zip([&real_second, &real_first]) {
        assert_eq!(s.claim_lock_holder(key.address()), Some(*real.digest()));
        assert_eq!(
            s.saved_decision(1, real.digest()),
            Some(PostConsensusDecision::Kept)
        );
    }
    for waiters in waiters {
        assert_eq!(waiters.told().await, (None, None));
    }
}

/// A claim executed in an epoch and sequenced again in the next is validated
/// anew, since nothing at the commit remembers a digest executed in an earlier
/// epoch. The horizon read finds its own account: it is dropped for good with
/// `AccountAlreadyExplicit`, takes no lock, and the account is left as it was.
#[tokio::test]
async fn executed_claim_sequenced_again_in_the_next_epoch_is_dropped() {
    let key = KeyHolder::every_scheme(55).swap_remove(2);
    let address = key.address();
    let (gas, coins) = gas_coins_for([address]);
    let mut s = setup_claim_state(coins).await;
    let claim = s.build_claim_of(&key, &gas[0]);
    assert_eq!(
        s.process_at_promptly(1, std::slice::from_ref(&claim)).await,
        vec![*claim.digest()]
    );
    let effects = s.execute(claim.clone());
    s.complete_commit(1, &[&effects]);
    let account = s.latest_ref(&ObjectId::from(address));
    s.advance_epoch().await;
    let waiters = s.drop_waiters(*claim.digest()).await;

    let scheduled = s.process_at_promptly(1, std::slice::from_ref(&claim)).await;

    assert!(scheduled.is_empty(), "{scheduled:?}");
    assert_eq!(
        s.saved_decision(1, claim.digest()),
        Some(PostConsensusDecision::Dropped)
    );
    assert_eq!(
        waiters.told().await,
        (
            Some(already_explicit(address)),
            Some(already_explicit(address))
        )
    );
    assert_eq!(s.claim_lock_holder(address), None);
    assert_eq!(s.latest_ref(&ObjectId::from(address)), account);
}

/// A claim sequenced after this validator stopped accepting user transactions
/// for the epoch is not validated: no decision, no lock. A claim of the
/// address is kept in the next epoch.
#[tokio::test]
async fn claim_sequenced_after_the_final_round_takes_no_lock() {
    let key = KeyHolder::every_scheme(56).swap_remove(3);
    let address = key.address();
    let (gas, coins) = gas_coins_for([address]);
    let mut s = setup_claim_state(coins).await;
    let claim = s.build_claim_of(&key, &gas[0]);
    s.epoch_store
        .get_reconfig_state_write_lock_guard()
        .close_all_tx();

    let scheduled = s.process_at_promptly(1, std::slice::from_ref(&claim)).await;

    assert!(scheduled.is_empty(), "{scheduled:?}");
    assert_eq!(s.saved_decision(1, claim.digest()), None);
    assert_eq!(s.claim_lock_holder(address), None);

    s.advance_epoch().await;
    assert_eq!(
        s.process_at_promptly(1, std::slice::from_ref(&claim)).await,
        vec![*claim.digest()]
    );
    assert_eq!(s.claim_lock_holder(address), Some(*claim.digest()));
}

/// A claim cancelled by the execution worker limit, which no version sets
/// where claims are accepted, creates no account when it executes, and the
/// check after execution reports it.
#[tokio::test]
#[should_panic(expected = "did not create its account")]
async fn cancelled_claim_is_reported_when_it_executes() {
    let guard = ProtocolConfig::apply_overrides_for_testing(|_, config| {
        let mut config = enable_claim_account(config);
        config.set_per_object_congestion_control_mode_for_testing(
            iota_protocol_config::PerObjectCongestionControlMode::TotalTxCount,
        );
        config.set_max_accumulated_txn_cost_per_object_in_mysticeti_commit_for_testing(1);
        config.set_max_congestion_limit_overshoot_per_commit_for_testing(0);
        config.set_max_concurrent_execution_workers_for_testing(1);
        config.set_separate_gas_price_feedback_mechanism_for_randomness_for_testing(false);
        config.set_max_deferral_rounds_for_congestion_control_for_testing(0);
        config
    });
    let keys: Vec<_> = KeyHolder::every_scheme(57).into_iter().take(2).collect();
    let (gas, coins) = gas_coins_for(keys.iter().map(KeyHolder::address));
    let s = setup_handler_object_state_with_config_guard(coins, Some(guard)).await;
    let claims: Vec<_> = keys
        .iter()
        .zip(&gas)
        .map(|(key, gas)| s.build_claim_of(key, gas))
        .collect();
    let (scheduled, assigned_versions) = s.process_at_with_versions(1, &claims).await;
    assert_eq!(
        scheduled.len(),
        2,
        "both claims are scheduled, one cancelled"
    );
    let mut assigned = assigned_versions.into_map();
    let (cancelled, versions) = claims
        .into_iter()
        .find_map(|claim| {
            let versions = assigned.remove(&TransactionKey::Digest(*claim.digest()))?;
            versions
                .iter()
                .any(|assignment| assignment.version.is_congested())
                .then_some((claim, versions))
        })
        .expect("one claim is cancelled");

    s.execute_executable(
        &s.executable(cancelled),
        ExecutionEnv::new().with_assigned_versions(versions),
    );
}

// ---------------------------------------------------------------------------
// ClaimAccount: plain signatures of a claimed address
// ---------------------------------------------------------------------------

/// A plain signature of a claimed address gets the same answer wherever the
/// claim is recorded (see [`ClaimRecord`]), as the sender of a transaction
/// and as its sponsor: dropped with `PlainSignatureForExplicitAccount`, saved
/// as a dropped copy, since a copy signed with the account's authenticator
/// can still be kept. Only the consensus adapter is told, not the client
/// waiting for the transaction's status. The transaction locks none of its
/// inputs.
#[tokio::test]
async fn plain_signature_gets_the_same_answer_wherever_the_claim_is_recorded() {
    for record in ClaimRecord::ALL {
        let keys = KeyHolder::every_scheme(60);
        let (key, other) = (&keys[0], &keys[1]);
        let address = key.address();
        let (gas, coins) = gas_coins_for([address, address, address, other.address()]);
        let mut s = setup_claim_state(coins).await;
        let claim = s.build_claim_of(key, &gas[0]);
        let (commit, before) = s.record_claim(&claim, record).await;
        let sent = s.sign_with(s.object_basics_create(address, &gas[1], 1), &[key]);
        let mut tx = s.object_basics_create(other.address(), &gas[2], 2);
        tx.gas_data_mut().owner = address;
        let sponsored = s.sign_with(tx, &[other, key]);
        let inputs = [s.latest_ref(&gas[1]), s.latest_ref(&gas[2])];
        let waiters = [
            s.drop_waiters(*sent.digest()).await,
            s.drop_waiters(*sponsored.digest()).await,
        ];

        let scheduled = s
            .process_at_promptly(
                commit,
                &[before.clone(), vec![sent.clone(), sponsored.clone()]].concat(),
            )
            .await;

        assert_eq!(scheduled, digests_of(&before), "{record:?}");
        for (tx, waiters) in [&sent, &sponsored].into_iter().zip(waiters) {
            assert_eq!(
                s.saved_decision(commit, tx.digest()),
                Some(PostConsensusDecision::DroppedCopy),
                "{record:?}"
            );
            assert_eq!(
                waiters.told().await,
                (None, Some(plain_signature_for(address))),
                "{record:?}"
            );
        }
        for input in inputs {
            assert_eq!(
                s.epoch_store.get_quarantined_owned_object_lock(&input),
                None,
                "{record:?}"
            );
        }
    }
}

impl HandlerObjectStateSetup {
    /// `tx`, sent by the claimed account at `account` created at
    /// `initial_shared_version`, signed through its built-in authenticator by
    /// `signer`, and by `sponsor` with a plain signature if it pays.
    fn sign_through_account(
        &self,
        tx: Transaction,
        account: Address,
        initial_shared_version: Version,
        signer: &KeyHolder,
        sponsor: Option<&KeyHolder>,
    ) -> VerifiedTransaction {
        let mut signatures = vec![UserSignature::MoveAuthenticator(MoveAuthenticator::from(
            builtin_authenticator(account, initial_shared_version, &signer.sign(&tx)),
        ))];
        signatures.extend(sponsor.map(|sponsor| sponsor.sign(&tx)));
        self.epoch_store
            .verify_transaction(TransactionEnvelope::from_user_sig_data(tx, signatures))
            .unwrap()
    }

    /// Keeps the claims of `keys` at commit 1, paid with `gas`, executes them
    /// and completes the commit. Returns the accounts' initial shared versions.
    async fn claim_at_commit_one(&self, keys: &[&KeyHolder], gas: &[ObjectId]) -> Vec<Version> {
        let claims: Vec<_> = keys
            .iter()
            .zip(gas)
            .map(|(key, gas)| self.build_claim_of(key, gas))
            .collect();
        assert_eq!(
            self.process_at_promptly(1, &claims).await,
            digests_of(&claims)
        );
        let effects: Vec<_> = claims
            .into_iter()
            .map(|claim| self.execute(claim))
            .collect();
        self.complete_commit(1, &effects.iter().collect::<Vec<_>>());
        keys.iter()
            .zip(&effects)
            .map(|(key, effects)| self.assert_claimed_account(key, effects))
            .collect()
    }
}

/// One transaction of a claimed account, in two copies with the same digest:
/// signed with the claimed key, and through the account's authenticator. The
/// plain copy is dropped as a copy, and the authenticated copy after it is
/// kept and executes, in the same commit and in a later one. Before the plain
/// copy, the authenticated copy is kept and the plain one is removed without
/// a check. In the next epoch, where only the account's field records the
/// claim, the answer is the same, and a restart replays the commit to the
/// same result.
#[tokio::test]
async fn move_authenticated_copy_is_kept_after_its_plain_signed_copy_is_dropped() {
    let key = KeyHolder::every_scheme(61).swap_remove(1);
    let address = key.address();
    let (gas, coins) = gas_coins_for([address; 5]);
    let mut s = setup_claim_state(coins).await;
    let [version] = s.claim_at_commit_one(&[&key], &gas[..1]).await[..] else {
        unreachable!()
    };
    let copies = |s: &HandlerObjectStateSetup, gas: &ObjectId, value: u64| {
        let tx = s.object_basics_create(address, gas, value);
        let plain = s.sign_with(tx.clone(), &[&key]);
        let authenticated = s.sign_through_account(tx, address, version, &key, None);
        assert_eq!(plain.digest(), authenticated.digest());
        (plain, authenticated)
    };
    // Commit `k + 1` reads the claim at its horizon. The commits up to `k`
    // are empty, so that each later commit's horizon is fully executed.
    let k = s.horizon_distance();
    for commit in 2..=k {
        s.complete_empty_commit(commit);
    }

    // In one commit, the plain copy first. The kept copy executes.
    let (plain, authenticated) = copies(&s, &gas[1], 1);
    let (scheduled, assigned_versions) = s
        .process_at_with_versions_promptly(k + 1, &[plain.clone(), authenticated.clone()])
        .await;
    assert_eq!(
        scheduled
            .iter()
            .map(|tx| tx.data().signatures().to_vec())
            .collect::<Vec<_>>(),
        vec![authenticated.data().signatures().to_vec()]
    );
    let effects = s
        .schedule_kept_transaction(scheduled[0].clone(), assigned_versions)
        .await
        .expect("the authenticated copy must execute");
    assert!(effects.status().is_success(), "{:?}", effects.status());
    s.complete_commit(k + 1, &[&effects]);

    // In one commit, the authenticated copy first.
    let (plain, authenticated) = copies(&s, &gas[2], 2);
    assert_eq!(
        s.kept_signatures_at(k + 2, &[authenticated.clone(), plain.clone()])
            .await,
        vec![authenticated.data().signatures().to_vec()]
    );

    // In two commits.
    let (plain, authenticated) = copies(&s, &gas[3], 3);
    assert!(
        s.kept_signatures_at(k + 3, std::slice::from_ref(&plain))
            .await
            .is_empty()
    );
    assert_eq!(
        s.saved_decision(k + 3, plain.digest()),
        Some(PostConsensusDecision::DroppedCopy)
    );
    assert_eq!(
        s.kept_signatures_at(k + 4, std::slice::from_ref(&authenticated))
            .await,
        vec![authenticated.data().signatures().to_vec()]
    );

    // In the next epoch, and replayed after a restart. The replay is of the
    // first commit, whose horizon needs no execution to wait for.
    s.advance_epoch().await;
    let (plain, authenticated) = copies(&s, &gas[4], 4);
    let both = [plain, authenticated.clone()];
    let first_run = s.kept_signatures_at(1, &both).await;
    assert_eq!(first_run, vec![authenticated.data().signatures().to_vec()]);
    s.restart_epoch_store().await;
    assert_eq!(s.kept_signatures_at(1, &both).await, first_run);
}

/// In the K commits after a claim, before the horizon has the account, a
/// transaction of the account is dropped in both of its copies: the plain one
/// for the plain signature, the authenticated one because the account is not
/// visible yet. The decision saved, and what the client hears, is that of the
/// last copy in the commit: told that an input is not visible yet, and can
/// send again, when the authenticated copy is last; told nothing when the
/// plain copy is last.
#[tokio::test]
async fn copies_of_a_just_claimed_accounts_transaction_take_the_decision_of_the_last_copy() {
    let key = KeyHolder::every_scheme(62).swap_remove(0);
    let address = key.address();
    let (gas, coins) = gas_coins_for([address; 3]);
    let s = setup_claim_state(coins).await;
    let [version] = s.claim_at_commit_one(&[&key], &gas[..1]).await[..] else {
        unreachable!()
    };
    let copies = |gas: &ObjectId, value: u64| {
        let tx = s.object_basics_create(address, gas, value);
        (
            s.sign_with(tx.clone(), &[&key]),
            s.sign_through_account(tx, address, version, &key, None),
        )
    };

    let (plain, authenticated) = copies(&gas[1], 1);
    let waiters = s.drop_waiters(*plain.digest()).await;
    assert!(
        s.process_at_promptly(2, &[plain.clone(), authenticated])
            .await
            .is_empty()
    );
    assert_eq!(
        s.saved_decision(2, plain.digest()),
        Some(PostConsensusDecision::DroppedInputNotVisibleYet)
    );
    let (status, any_copy) = waiters.told().await;
    assert!(
        matches!(
            status,
            Some(IotaError::UserInput {
                error: UserInputError::ObjectNotFound { .. }
            })
        ),
        "{status:?}"
    );
    assert_eq!(status, any_copy);

    let (plain, authenticated) = copies(&gas[2], 2);
    let waiters = s.drop_waiters(*plain.digest()).await;
    assert!(
        s.process_at_promptly(3, &[authenticated, plain.clone()])
            .await
            .is_empty()
    );
    assert_eq!(
        s.saved_decision(3, plain.digest()),
        Some(PostConsensusDecision::DroppedCopy)
    );
    assert_eq!(
        waiters.told().await,
        (None, Some(plain_signature_for(address)))
    );
}

/// The rule looks at each signer on its own: a claimed sender signing through
/// its authenticator with a claimed sponsor signing with a plain signature is
/// dropped for the sponsor, and an implicit sender with a plain signature with
/// a claimed sponsor signing through its authenticator is kept.
#[tokio::test]
async fn signers_of_one_transaction_are_judged_one_by_one() {
    let keys = KeyHolder::every_scheme(63);
    let (sender, sponsor, implicit) = (&keys[0], &keys[1], &keys[2]);
    let (gas, coins) = gas_coins_for([
        sender.address(),
        sponsor.address(),
        sponsor.address(),
        sponsor.address(),
    ]);
    let s = setup_claim_state(coins).await;
    let versions = s.claim_at_commit_one(&[sender, sponsor], &gas[..2]).await;
    let commit = 1 + s.horizon_distance();

    let mut tx = s.object_basics_create(sender.address(), &gas[2], 1);
    tx.gas_data_mut().owner = sponsor.address();
    let plain_sponsor =
        s.sign_through_account(tx, sender.address(), versions[0], sender, Some(sponsor));
    let mut tx = s.object_basics_create(implicit.address(), &gas[3], 2);
    tx.gas_data_mut().owner = sponsor.address();
    let mut signatures = vec![implicit.sign(&tx)];
    signatures.push(UserSignature::MoveAuthenticator(MoveAuthenticator::from(
        builtin_authenticator(sponsor.address(), versions[1], &sponsor.sign(&tx)),
    )));
    let authenticated_sponsor = s
        .epoch_store
        .verify_transaction(TransactionEnvelope::from_user_sig_data(tx, signatures))
        .unwrap();

    let result = s
        .validate_at(
            commit,
            &[plain_sponsor.clone(), authenticated_sponsor.clone()],
        )
        .await;

    assert_eq!(result.kept, vec![*authenticated_sponsor.digest()]);
    assert_eq!(
        result.dropped,
        vec![(
            *plain_sponsor.digest(),
            plain_signature_for(sponsor.address())
        )]
    );
    assert_eq!(
        s.saved_decision(commit, plain_sponsor.digest()),
        Some(PostConsensusDecision::DroppedCopy)
    );
}

/// A sponsor's plain signature for its claimed address is dropped as a copy
/// whether or not the protocol lets a sponsor sign with a `MoveAuthenticator`.
/// Without that, no copy of the transaction can be kept, but the drop is
/// still not the transaction's status, so its client is not told.
#[tokio::test]
async fn plain_signed_sponsor_drop_is_saved_as_a_copy_whatever_the_sponsor_can_sign_with() {
    for sponsor_can_authenticate in [true, false] {
        let guard = ProtocolConfig::apply_overrides_for_testing(move |_, config| {
            let mut config = enable_claim_account(config);
            config.set_enable_move_authentication_for_sponsor_for_testing(sponsor_can_authenticate);
            config
        });
        let keys = KeyHolder::every_scheme(64);
        let (sponsor, sender) = (&keys[0], &keys[1]);
        let (gas, coins) = gas_coins_for([sponsor.address(), sponsor.address()]);
        let s = setup_handler_object_state_with_config_guard(coins, Some(guard)).await;
        let claim = s.build_claim_of(sponsor, &gas[0]);
        assert_eq!(
            s.process_at_promptly(1, std::slice::from_ref(&claim)).await,
            vec![*claim.digest()]
        );
        let mut tx = s.object_basics_create(sender.address(), &gas[1], 1);
        tx.gas_data_mut().owner = sponsor.address();
        let sponsored = s.sign_with(tx, &[sender, sponsor]);
        let waiters = s.drop_waiters(*sponsored.digest()).await;

        assert!(
            s.process_at_promptly(2, std::slice::from_ref(&sponsored))
                .await
                .is_empty()
        );

        assert_eq!(
            s.saved_decision(2, sponsored.digest()),
            Some(PostConsensusDecision::DroppedCopy),
            "sponsor can authenticate: {sponsor_can_authenticate}"
        );
        assert_eq!(
            waiters.told().await,
            (None, Some(plain_signature_for(sponsor.address()))),
            "sponsor can authenticate: {sponsor_can_authenticate}"
        );
    }
}

/// A claim paid by an address that a claim later in the same commit makes
/// explicit is kept: its sponsor was implicit at its position. Both claims
/// execute. The other order is
/// `claim_with_an_explicit_sponsor_is_dropped_for_good`.
#[tokio::test]
async fn claim_sponsored_by_an_address_claimed_later_in_the_commit_is_kept() {
    let keys = KeyHolder::every_scheme(65);
    let (sender, sponsor) = (&keys[0], &keys[4]);
    let (gas, coins) = gas_coins_for([sponsor.address(), sponsor.address()]);
    let s = setup_claim_state(coins).await;
    let mut tx = s.claim_transaction(sender, &gas[0]);
    tx.gas_data_mut().owner = sponsor.address();
    let sponsored_claim = s.sign_with(tx, &[sender, sponsor]);
    let sponsor_claim = s.build_claim_of(sponsor, &gas[1]);

    let scheduled = s
        .process_at_promptly(1, &[sponsored_claim.clone(), sponsor_claim.clone()])
        .await;

    assert_eq!(
        scheduled,
        vec![*sponsored_claim.digest(), *sponsor_claim.digest()]
    );
    s.assert_claimed_account(sender, &s.execute(sponsored_claim));
    s.assert_claimed_account(sponsor, &s.execute(sponsor_claim));
}

/// A plain-signed transaction kept before a claim of its address in the same
/// commit executes after the claim: execution does not check the rule, which
/// holds for decisions made once the claim is kept.
#[tokio::test]
async fn plain_signed_transaction_kept_before_a_claim_executes_after_it() {
    let key = KeyHolder::every_scheme(66).swap_remove(3);
    let address = key.address();
    let (gas, coins) = gas_coins_for([address, address]);
    let s = setup_claim_state(coins).await;
    let plain = s.sign_with(s.object_basics_create(address, &gas[0], 1), &[&key]);
    let claim = s.build_claim_of(&key, &gas[1]);

    let (scheduled, _) = s
        .process_at_with_versions(1, &[plain.clone(), claim.clone()])
        .await;

    assert_eq!(
        scheduled.iter().map(|tx| *tx.digest()).collect::<Vec<_>>(),
        vec![*plain.digest(), *claim.digest()]
    );
    let claim_effects = s.execute_executable(&scheduled[1], ExecutionEnv::new());
    s.assert_claimed_account(&key, &claim_effects);
    let effects = s.execute_executable(&scheduled[0], ExecutionEnv::new());
    assert!(effects.status().is_success(), "{:?}", effects.status());
    assert_eq!(effects.created().len(), 1);
}

/// With `reject_plain_signatures_for_explicit_accounts` off and claims on, a
/// claimed address keeps signing with plain signatures: the lock probe, the
/// horizon read and admission accept them.
#[tokio::test]
async fn plain_signatures_are_kept_while_the_rule_is_off() {
    let guard = ProtocolConfig::apply_overrides_for_testing(|_, config| {
        let mut config = enable_claim_account(config);
        config.set_reject_plain_signatures_for_explicit_accounts_for_testing(false);
        config
    });
    let key = KeyHolder::every_scheme(67).swap_remove(0);
    let address = key.address();
    let (gas, coins) = gas_coins_for([address; 4]);
    let s = setup_handler_object_state_with_config_guard(coins, Some(guard)).await;
    let claim = s.build_claim_of(&key, &gas[0]);
    assert_eq!(
        s.process_at_promptly(1, std::slice::from_ref(&claim)).await,
        vec![*claim.digest()]
    );

    let within_the_horizon = s.sign_with(s.object_basics_create(address, &gas[1], 1), &[&key]);
    let result = s
        .validate_at(2, std::slice::from_ref(&within_the_horizon))
        .await;
    assert_eq!(
        result.kept,
        vec![*within_the_horizon.digest()],
        "{:?}",
        result.dropped
    );

    let effects = s.execute(claim);
    s.complete_commit(1, &[&effects]);
    let at_the_horizon = s.sign_with(s.object_basics_create(address, &gas[2], 2), &[&key]);
    let result = s
        .validate_at(
            1 + s.horizon_distance(),
            std::slice::from_ref(&at_the_horizon),
        )
        .await;
    assert_eq!(
        result.kept,
        vec![*at_the_horizon.digest()],
        "{:?}",
        result.dropped
    );

    let admitted = s.sign_with(s.object_basics_create(address, &gas[3], 3), &[&key]);
    s.authority
        .handle_transaction_validation_checks(
            &admitted,
            &s.epoch_store,
            &s.authority.config.transaction_deny_config,
        )
        .await
        .unwrap();
}

// ---------------------------------------------------------------------------
// ClaimAccount: order of the checks, attested copies, admission and reads
// ahead
// ---------------------------------------------------------------------------

/// The error and the saved decision of each of `transactions` validated
/// together at commit `index`; `None` for a transaction that is kept.
async fn outcomes_at(
    s: &HandlerObjectStateSetup,
    index: CommitIndex,
    transactions: &[VerifiedTransaction],
) -> Vec<Option<(IotaError, PostConsensusDecision)>> {
    let result = s.validate_at(index, transactions).await;
    transactions
        .iter()
        .map(|tx| {
            result
                .dropped
                .iter()
                .find(|(digest, _)| digest == tx.digest())
                .map(|(_, error)| (error.clone(), s.saved_decision(index, tx.digest()).unwrap()))
        })
        .collect()
}

/// A transaction that fails another check besides the claim rules is dropped
/// for the first check in the loop that fails it, so where the claim is
/// recorded can change the answer. In the claim's epoch the claim's lock is
/// probed with the owned inputs, before the transaction dependency check; in
/// later epochs only the horizon read records the claim, and it runs after
/// that check and before the inputs are read. A plain-signed transaction of a
/// claimed address:
/// - sharing the claim's gas coin is dropped on the coin's lock in the claim's epoch, for good, and
///   for its plain signature later, as a copy;
/// - naming an object that does not exist is dropped for its plain signature in both;
/// - without a gas coin fails the validity check, which runs first, in both;
/// - depending on a transaction that is not sequenced is dropped for its plain signature in the
///   claim's epoch, as a copy that its client is not told about, and for the dependency later,
///   which its client is told about.
///
/// A claim whose sender is claimed only at the horizon and whose sponsor's
/// claim holds a lock is dropped for the sponsor's plain signature; once both
/// are at the horizon, for the sender's claim.
#[tokio::test]
async fn transaction_failing_two_checks_is_dropped_for_the_first_one_in_the_loop() {
    let guard = ProtocolConfig::apply_overrides_for_testing(|_, config| {
        let mut config = enable_claim_account(config);
        config.set_enable_transaction_dependencies_for_testing(true);
        config
    });
    let keys = KeyHolder::every_scheme(70);
    let (key, sponsor) = (&keys[0], &keys[1]);
    let address = key.address();
    let (gas, coins) = gas_coins_for([address, address, address, address, address]);
    let (sponsor_gas, sponsor_coins) = gas_coins_for([sponsor.address(), sponsor.address()]);
    let mut s = setup_handler_object_state_with_config_guard(
        coins.into_iter().chain(sponsor_coins).collect(),
        Some(guard),
    )
    .await;
    let claim = s.build_claim_of(key, &gas[0]);
    let plain = |s: &HandlerObjectStateSetup, tx: Transaction| s.sign_with(tx, &[key]);
    let shares_the_claims_coin = plain(&s, s.object_basics_create(address, &gas[0], 1));
    let missing_object = plain(
        &s,
        TestTransactionBuilder::new(address, s.latest_ref(&gas[1]), s.rgp)
            .transfer(random_object_ref(), Address::random())
            .build(),
    );
    let mut tx = s.object_basics_create(address, &gas[2], 3);
    tx.gas_data_mut().objects.clear();
    let without_gas = plain(&s, tx);
    let without_gas_error = without_gas
        .validity_check(&s.epoch_store.tx_validity_check_context())
        .unwrap_err();
    let dependency = TransactionDigest::random();
    let with_dependency = |s: &HandlerObjectStateSetup| {
        plain(
            s,
            TestTransactionBuilder::new(address, s.latest_ref(&gas[3]), s.rgp)
                .transfer_iota(Some(1), Address::random())
                .with_transaction_dependency(dependency, s.epoch_store.epoch())
                .build(),
        )
    };
    let in_the_claims_epoch = with_dependency(&s);
    assert_eq!(
        s.process_at_promptly(1, std::slice::from_ref(&claim)).await,
        vec![*claim.digest()]
    );

    let plain_signature = Some((
        plain_signature_for(address),
        PostConsensusDecision::DroppedCopy,
    ));
    assert_eq!(
        outcomes_at(
            &s,
            2,
            &[
                shares_the_claims_coin.clone(),
                missing_object.clone(),
                without_gas.clone(),
                in_the_claims_epoch,
            ]
        )
        .await,
        vec![
            Some((
                IotaError::ObjectLockConflict {
                    obj_ref: s.latest_ref(&gas[0]),
                    pending_transaction: *claim.digest(),
                },
                PostConsensusDecision::Dropped
            )),
            plain_signature.clone(),
            Some((without_gas_error.clone(), PostConsensusDecision::Dropped)),
            plain_signature.clone(),
        ]
    );

    let effects = s.execute(claim);
    s.complete_commit(1, &[&effects]);
    s.advance_epoch().await;
    let sponsor_claim = s.build_claim_of(sponsor, &sponsor_gas[0]);
    assert_eq!(
        s.process_at_promptly(1, std::slice::from_ref(&sponsor_claim))
            .await,
        vec![*sponsor_claim.digest()]
    );
    let mut tx = s.claim_transaction(key, &gas[4]);
    tx.gas_data_mut().owner = sponsor.address();
    tx.gas_data_mut().objects = vec![s.latest_ref(&sponsor_gas[1])];
    let sponsored_claim = s.sign_with(tx, &[key, sponsor]);
    assert_eq!(
        outcomes_at(
            &s,
            2,
            &[
                shares_the_claims_coin,
                missing_object,
                without_gas,
                with_dependency(&s),
                sponsored_claim.clone(),
            ]
        )
        .await,
        vec![
            plain_signature.clone(),
            plain_signature,
            Some((without_gas_error, PostConsensusDecision::Dropped)),
            Some((
                UserInputError::TransactionDependencyNotFound { digest: dependency }.into(),
                PostConsensusDecision::DroppedInputNotVisibleYet
            )),
            Some((
                plain_signature_for(sponsor.address()),
                PostConsensusDecision::Dropped
            )),
        ]
    );

    let effects = s.execute(sponsor_claim);
    s.complete_commit(1, &[&effects]);
    s.advance_epoch().await;
    assert_eq!(
        outcomes_at(&s, 1, std::slice::from_ref(&sponsored_claim)).await,
        vec![Some((
            already_explicit(address),
            PostConsensusDecision::Dropped
        ))]
    );
}

/// Attested copies get the decisions of unattested ones: a claim is kept, a
/// second claim of its address is dropped for good, and a plain-signed
/// transaction of the address is dropped as a copy. A claim attested by a
/// validator cut off from attesting is dropped as a copy, so another
/// validator's copy of it is kept.
#[tokio::test]
async fn attested_claims_and_plain_copies_get_the_decisions_of_unattested_ones() {
    let guard = ProtocolConfig::apply_overrides_for_testing(|_, config| {
        let mut config = enable_claim_account(config);
        config.set_enable_validator_attestation_for_testing(true);
        config.set_checkpoint_summary_version_specific_data_for_testing(2);
        config.set_attestation_max_refuted_units_squared_for_testing(100);
        config
    });
    let keys = KeyHolder::every_scheme(71);
    let (key, other) = (&keys[2], &keys[3]);
    let (refuted_sender, refuted_key): (Address, AccountPrivateKey) = get_key_pair();
    let (refuted_object, refuted_gas) = (ObjectId::random(), ObjectId::random());
    let (gas, coins) =
        gas_coins_for([key.address(), key.address(), key.address(), other.address()]);
    let s = setup_handler_object_state_with_config_guard(
        coins
            .into_iter()
            .chain([
                Object::with_id_owner_for_testing(refuted_object, refuted_sender),
                Object::with_id_owner_for_testing(refuted_gas, refuted_sender),
            ])
            .collect(),
        Some(guard),
    )
    .await;
    let claim = s.build_claim_of(key, &gas[0]);
    let second = s.build_claim_of(key, &gas[1]);
    let plain = s.sign_with(s.object_basics_create(key.address(), &gas[2], 1), &[key]);

    assert_eq!(
        s.validate_copies_at(
            1,
            vec![
                s.attested_copy(&claim, 0),
                s.attested_copy(&second, 1),
                s.attested_copy(&plain, 2),
            ]
        )
        .await,
        ValidatedCopies {
            kept: vec![(*claim.digest(), 0)],
            dropped: vec![
                (
                    *second.digest(),
                    already_explicit(key.address()),
                    PostConsensusDecision::Dropped
                ),
                (
                    *plain.digest(),
                    plain_signature_for(key.address()),
                    PostConsensusDecision::DroppedCopy
                ),
            ],
        }
    );

    // Commit 1 refutes attestor 0 with 10 units, 100 squared, its whole
    // score. From the commit whose horizon is commit 1, its copies drop.
    let refuted = s.build_transfer(
        &refuted_object,
        &refuted_gas,
        refuted_sender,
        &refuted_key,
        Address::random(),
    );
    let _watcher = s.start_execution_watcher();
    s.epoch_store
        .assign_commit_to_transactions(2, vec![TransactionKey::Digest(*refuted.digest())]);
    s.execute_refuted(refuted, 0, 10);
    s.wait_for_fully_executed_commit(2).await;
    let other_claim = s.build_claim_of(other, &gas[3]);
    assert_eq!(
        s.validate_copies_at(
            2 + s.horizon_distance(),
            vec![
                s.attested_copy(&other_claim, 0),
                s.attested_copy(&other_claim, 1),
            ]
        )
        .await,
        ValidatedCopies {
            kept: vec![(*other_claim.digest(), 1)],
            dropped: vec![],
        }
    );
}

/// Admission answers early from the validator's latest state, and the commit
/// decides. For a second claim and a plain-signed transaction of an address,
/// in every state of its claim, admission never rejects what the commit
/// keeps, and where it rejects, the commit drops with the same error. It
/// accepts what the commit drops while the claim is kept but has not executed
/// on this validator, and while the address's field is deleted above the
/// horizon, which no transaction can do today.
#[tokio::test]
async fn admission_never_rejects_what_the_commit_keeps() {
    use crate::transaction_outputs::TransactionOutputs;

    #[derive(Debug, Clone, Copy, PartialEq)]
    enum State {
        Unclaimed,
        KeptNotExecuted,
        ExecutedAboveTheHorizon,
        AtTheHorizon,
        NextEpoch,
        OrphanedField,
        FieldDeletedAboveTheHorizon,
    }
    let mut accepted_and_dropped = vec![];
    for state in [
        State::Unclaimed,
        State::KeptNotExecuted,
        State::ExecutedAboveTheHorizon,
        State::AtTheHorizon,
        State::NextEpoch,
        State::OrphanedField,
        State::FieldDeletedAboveTheHorizon,
    ] {
        let key = KeyHolder::every_scheme(72).swap_remove(0);
        let address = key.address();
        let (deleter, deleter_key): (Address, AccountPrivateKey) = get_key_pair();
        let (gas, mut coins) = gas_coins_for([address, address, address]);
        let deleter_gas = ObjectId::random();
        coins.push(Object::with_id_owner_for_testing(deleter_gas, deleter));
        if state == State::OrphanedField {
            coins.push(authenticator_function_ref_field(
                ObjectId::from(address),
                ObjectId::random(),
                Version::from(1),
            ));
        }
        let mut s = setup_claim_state(coins).await;
        let claim = s.build_claim_of(&key, &gas[0]);
        let k = s.horizon_distance();
        let first_commit = match state {
            State::Unclaimed | State::OrphanedField => 1,
            State::KeptNotExecuted => {
                s.process_at_promptly(1, std::slice::from_ref(&claim)).await;
                2
            }
            State::ExecutedAboveTheHorizon | State::AtTheHorizon | State::NextEpoch => {
                s.process_at_promptly(1, std::slice::from_ref(&claim)).await;
                let effects = s.execute(claim.clone());
                s.complete_commit(1, &[&effects]);
                match state {
                    State::ExecutedAboveTheHorizon => 2,
                    State::AtTheHorizon => k + 1,
                    _ => {
                        s.advance_epoch().await;
                        1
                    }
                }
            }
            State::FieldDeletedAboveTheHorizon => {
                let effects = s.execute_claim_with_assigned_commit(claim.clone(), 1);
                s.complete_commit(1, &[&effects]);
                // No transaction can delete the field, so the deletion is
                // written to the store, at commit 3.
                let field_id =
                    derive_authenticator_function_ref_v1_dynamic_field_id(address).unwrap();
                let field = s.authority.get_object(&field_id).unwrap();
                let deletion = s.build_move_call(
                    "object_basics",
                    "create",
                    create_object_args(deleter),
                    &deleter_gas,
                    deleter,
                    &deleter_key,
                );
                let deletion_effects = TestEffectsBuilder::new(deletion.data())
                    .with_deleted_objects_owned_by([(field_id, field.version(), field.owner)])
                    .build();
                let deletion_key = TransactionKey::Digest(*deletion.digest());
                s.epoch_store
                    .assign_commit_to_transactions(3, vec![deletion_key]);
                s.epoch_store
                    .record_executed_transaction(&deletion_key, &deletion_effects);
                s.authority.get_cache_writer().write_transaction_outputs(
                    s.epoch_store.epoch(),
                    Arc::new(TransactionOutputs {
                        transaction: Arc::new(deletion),
                        effects: deletion_effects.clone(),
                        events: Default::default(),
                        superseded: vec![],
                        markers: vec![],
                        wrapped: vec![],
                        deleted: vec![ObjectKey(field_id, deletion_effects.lamport_version())],
                        live_object_markers_to_delete: vec![],
                        new_live_object_markers_to_init: vec![],
                        written: Default::default(),
                        attestation_record: None,
                    }),
                );
                s.complete_commit(3, &[&deletion_effects]);
                k + 1
            }
        };
        let second = s.build_claim_of(&key, &gas[1]);
        let plain = s.sign_with(s.object_basics_create(address, &gas[2], 1), &[&key]);
        for (kind, tx, commit) in [
            ("second claim", &second, first_commit),
            ("plain signature", &plain, first_commit + 1),
        ] {
            let admitted = s
                .authority
                .handle_transaction_validation_checks(
                    tx,
                    &s.epoch_store,
                    &s.authority.config.transaction_deny_config,
                )
                .await;
            let result = s.validate_at(commit, std::slice::from_ref(tx)).await;
            match (&admitted, result.dropped.first()) {
                (Ok(_), None) => assert_eq!(result.kept, vec![*tx.digest()]),
                (Err(rejected), Some((_, dropped))) => {
                    assert_eq!(rejected, dropped, "{state:?}: {kind}")
                }
                (Ok(_), Some((_, dropped))) => {
                    accepted_and_dropped.push((state, kind, dropped.clone()))
                }
                (Err(rejected), None) => {
                    panic!("{state:?}: {kind} rejected with {rejected:?} but kept")
                }
            }
        }
    }

    let address = KeyHolder::every_scheme(72).swap_remove(0).address();
    assert_eq!(
        accepted_and_dropped,
        [State::KeptNotExecuted, State::FieldDeletedAboveTheHorizon]
            .into_iter()
            .flat_map(|state| [
                (state, "second claim", already_explicit(address)),
                (state, "plain signature", plain_signature_for(address)),
            ])
            .collect::<Vec<_>>()
    );
}

/// A commit with a claim, a second claim of its address, a plain-signed
/// transaction of that address, a claim and a plain-signed transaction of an
/// address claimed at the horizon, a plain-signed transaction of an unclaimed
/// address and a Move account's transaction gives the same decisions, locks
/// and kept transactions validated with four readers, which read inputs
/// ahead of the loop, and with one.
#[tokio::test]
async fn claim_decisions_do_not_depend_on_the_number_of_readers() {
    let _guard =
        ProtocolConfig::apply_overrides_for_testing(|_, config| enable_claim_account(config));
    let keys = KeyHolder::every_scheme(73);
    let (claimed_here, claimed_before, unclaimed) = (&keys[0], &keys[1], &keys[2]);
    let account_id = ObjectId::random();
    let account = shared_object_with_id(account_id);
    let mut genesis = vec![
        authenticator_field(&account, "missing_function"),
        account,
        authenticator_function_ref_field(
            ObjectId::from(claimed_before.address()),
            ObjectId::random(),
            Version::from(1),
        ),
    ];
    let mut coin_of = |owner: Address| {
        let coin = Object::with_id_owner_for_testing(ObjectId::random(), owner);
        let reference = coin.object_ref();
        genesis.push(coin);
        reference
    };
    let here = [(); 3].map(|_| coin_of(claimed_here.address()));
    let before = [(); 2].map(|_| coin_of(claimed_before.address()));
    let unclaimed_coin = coin_of(unclaimed.address());
    let account_coin = coin_of(account_id.into());

    let authority_with = |readers: usize| {
        let genesis = genesis.clone();
        async move {
            let authority = TestAuthorityBuilder::new()
                .with_post_consensus_validation_readers(readers.try_into().unwrap())
                .build()
                .await;
            for object in genesis {
                authority.insert_genesis_object(object);
            }
            authority
        }
    };
    let ahead = authority_with(4).await;
    let inline = authority_with(1).await;
    let rgp = ahead.reference_gas_price_for_testing().unwrap();
    let sign = |tx: Transaction, key: &KeyHolder| {
        TransactionEnvelope::from_user_sig_data(tx.clone(), vec![key.sign(&tx)])
    };
    let claim = |key: &KeyHolder, gas: ObjectReference| {
        sign(
            Transaction::new(
                iota_sdk_types::TransactionKind::new_claim_account(
                    iota_sdk_types::ClaimAccountTransaction::new_smart_account(key.claim()),
                ),
                key.address(),
                gas,
                iota_types::transaction::TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE * rgp,
                rgp,
            ),
            key,
        )
    };
    let transfer = |key: &KeyHolder, gas: ObjectReference| {
        sign(
            TestTransactionBuilder::new(key.address(), gas, rgp)
                .transfer_iota(Some(1), Address::random())
                .build(),
            key,
        )
    };
    let account_call = TransactionEnvelope::from_user_sig_data(
        TestTransactionBuilder::new(account_id.into(), account_coin, rgp)
            .transfer_iota(Some(1), Address::random())
            .build(),
        vec![UserSignature::MoveAuthenticator(MoveAuthenticator::from(
            account_authenticator(account_id),
        ))],
    );
    let transactions = vec![
        claim(claimed_here, here[0]),
        claim(claimed_here, here[1]),
        transfer(claimed_here, here[2]),
        claim(claimed_before, before[0]),
        transfer(claimed_before, before[1]),
        transfer(unclaimed, unclaimed_coin),
        account_call,
    ];

    let with_readers = validation_outcome(&ahead, &transactions).await;
    let with_one = validation_outcome(&inline, &transactions).await;

    assert_eq!(with_readers, with_one);
    let (dropped, _, _, kept) = with_readers;
    let digest = |index: usize| *transactions[index].digest();
    assert_eq!(kept, vec![digest(0), digest(5), digest(6)]);
    assert_eq!(
        dropped
            .into_iter()
            .map(|(digest, _, decision)| (digest, decision))
            .collect::<Vec<_>>(),
        vec![
            (digest(1), PostConsensusDecision::Dropped),
            (digest(2), PostConsensusDecision::DroppedCopy),
            (digest(3), PostConsensusDecision::Dropped),
            (digest(4), PostConsensusDecision::DroppedCopy),
        ]
    );
}

// ---------------------------------------------------------------------------
// ClaimAccount: replay after a restart
// ---------------------------------------------------------------------------

/// A kept claim that has not executed when the validator restarts is
/// validated again at the replay of its commit and takes its lock again, so a
/// later claim of the address is still dropped. The existing test covers an
/// executed claim.
#[tokio::test]
async fn replayed_kept_claim_that_has_not_executed_takes_its_lock_again() {
    let key = KeyHolder::every_scheme(80).swap_remove(4);
    let address = key.address();
    let (gas, coins) = gas_coins_for([address, address]);
    let mut s = setup_claim_state(coins).await;
    let claim = s.build_claim_of(&key, &gas[0]);
    let second = s.build_claim_of(&key, &gas[1]);
    assert_eq!(
        s.process_at_promptly(1, std::slice::from_ref(&claim)).await,
        vec![*claim.digest()]
    );

    s.restart_epoch_store().await;
    assert_eq!(
        s.claim_lock_holder(address),
        None,
        "the restart loses the lock"
    );
    assert_eq!(
        s.process_at_promptly(1, std::slice::from_ref(&claim)).await,
        vec![*claim.digest()]
    );

    assert_eq!(s.claim_lock_holder(address), Some(*claim.digest()));
    let result = s.validate_at(2, std::slice::from_ref(&second)).await;
    assert_eq!(
        result.dropped,
        vec![(*second.digest(), already_explicit(address))]
    );
}

/// What a commit decided for a plain-signed transaction kept before a claim
/// of its address, the claim, a second claim and a plain-signed transaction
/// after the claim, is repeated when the commit is replayed after a restart:
/// before anything executed, after the claim executed, and after both kept
/// transactions executed. The kept plain-signed transaction is validated
/// again while the claim's account is in the latest state: the account is
/// above the horizon, as its rows show, so it is kept again. The rows are
/// written before the restart, as a node writes them before the outputs.
#[tokio::test]
async fn replayed_commit_repeats_its_claim_and_plain_signature_decisions() {
    for executed in 0..3 {
        let key = KeyHolder::every_scheme(81).swap_remove(1);
        let address = key.address();
        let (gas, coins) = gas_coins_for([address; 4]);
        let mut s = setup_claim_state(coins).await;
        let before = s.sign_with(s.object_basics_create(address, &gas[0], 1), &[&key]);
        let claim = s.build_claim_of(&key, &gas[1]);
        let second = s.build_claim_of(&key, &gas[2]);
        let after = s.sign_with(s.object_basics_create(address, &gas[3], 2), &[&key]);
        let commit = [before.clone(), claim.clone(), second.clone(), after.clone()];
        let decisions = |s: &HandlerObjectStateSetup| {
            commit
                .iter()
                .map(|tx| s.saved_decision(1, tx.digest()))
                .collect::<Vec<_>>()
        };

        let first_run = s.process_at_promptly(1, &commit).await;
        assert_eq!(first_run, vec![*before.digest(), *claim.digest()]);
        let saved = decisions(&s);
        assert_eq!(
            saved,
            vec![
                Some(PostConsensusDecision::Kept),
                Some(PostConsensusDecision::Kept),
                Some(PostConsensusDecision::Dropped),
                Some(PostConsensusDecision::DroppedCopy),
            ]
        );
        let effects: Vec<_> = [&claim, &before]
            .into_iter()
            .take(executed)
            .map(|tx| s.execute(tx.clone()))
            .collect();
        s.epoch_store
            .persist_checkpoint_bookkeeping(&effects)
            .unwrap();
        s.restart_epoch_store().await;

        assert_eq!(
            s.process_at_promptly(1, &commit).await,
            first_run,
            "{executed} executed"
        );
        assert_eq!(decisions(&s), saved, "{executed} executed");
        assert_eq!(
            s.claim_lock_holder(address),
            Some(*claim.digest()),
            "{executed} executed"
        );
    }
}

/// A claim executed by this validator, as state sync can execute it, before
/// its commit is validated for the first time, has no saved decision there:
/// it is validated like on a validator that has not executed it, kept, and
/// takes its lock. Its account is above the horizon, as its rows show.
#[tokio::test]
async fn already_executed_claim_without_a_saved_decision_is_validated_like_any_other() {
    let key = KeyHolder::every_scheme(82).swap_remove(3);
    let address = key.address();
    let (gas, coins) = gas_coins_for([address]);
    let s = setup_claim_state(coins).await;
    let claim = s.build_claim_of(&key, &gas[0]);
    s.execute_claim_with_assigned_commit(claim.clone(), 1);

    let result = s.validate_at(1, std::slice::from_ref(&claim)).await;

    assert_eq!(result.kept, vec![*claim.digest()], "{:?}", result.dropped);
    assert_eq!(result.locks.get(&claim_lock(address)), Some(claim.digest()));
}

/// Once the switch to synced checkpoints after a claim's commit is saved, the
/// replay of that commit repeats the claim's saved keep without reading its
/// inputs and without taking its lock, as for the owned inputs of any kept
/// transaction: no commit after the switch is validated in the epoch.
#[tokio::test]
async fn replay_up_to_the_switch_repeats_a_kept_claim_without_its_lock() {
    let key = KeyHolder::every_scheme(83).swap_remove(2);
    let address = key.address();
    let (gas, coins) = gas_coins_for([address]);
    let mut s = setup_claim_state(coins).await;
    let claim = s.build_claim_of(&key, &gas[0]);
    let first = s.validate_at(1, std::slice::from_ref(&claim)).await;
    assert_eq!(first.kept, vec![*claim.digest()]);
    assert_eq!(first.locks.get(&claim_lock(address)), Some(claim.digest()));

    // The switch is saved, a synced checkpoint executes the claim, which
    // leaves no row, and the validator restarts.
    s.epoch_store.record_handled_commit(1);
    s.epoch_store.start_executing_synced_checkpoints().await;
    let effects = s.execute(claim.clone());
    s.assert_claimed_account(&key, &effects);
    s.epoch_store = reopen(&s.authority, &s.epoch_store);

    assert_repeats_the_saved_keep(&first, s.validate_at(1, std::slice::from_ref(&claim)).await);
}

// ---------------------------------------------------------------------------
// ClaimAccount: the authenticator function field through a rotation
// ---------------------------------------------------------------------------

/// A claimed account that rotates to an authenticator of its own removes its
/// authenticator function field and adds it back in one transaction, which
/// writes the field at a new version. The address stays claimed at every
/// horizon, before, at and after the rotation's commit, and in the next
/// epoch: a second claim and a plain signature of it are dropped at each.
#[tokio::test]
async fn address_stays_claimed_through_a_rotation_of_its_authenticator() {
    let key = KeyHolder::every_scheme(84).swap_remove(0);
    let address = key.address();
    let account_id = ObjectId::from(address);
    let (publisher, publisher_key): (Address, AccountPrivateKey) = get_key_pair();
    let (gas, mut coins) = gas_coins_for([address; 4]);
    let publisher_gas = ObjectId::random();
    coins.push(Object::with_id_owner_for_testing(publisher_gas, publisher));
    let mut s = setup_claim_state(coins).await;
    let k = s.horizon_distance();

    // Commit 1 publishes the package with an authenticator for a
    // `SmartAccount`, and claims the address.
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/unit_tests/data/move_authenticator");
    let package = iota_move_build::BuildConfig::new_for_testing()
        .build(&path)
        .unwrap();
    let mut builder = ProgrammableTransactionBuilder::new();
    let upgrade_cap = builder.publish_upgradeable(
        package.get_package_bytes(false),
        package.get_dependency_storage_package_ids(),
    );
    builder.transfer_arg(publisher, upgrade_cap);
    let publish = s
        .epoch_store
        .verify_transaction(to_sender_signed_transaction(
            Transaction::new_programmable(
                publisher,
                vec![s.latest_ref(&publisher_gas)],
                builder.finish(),
                TEST_ONLY_GAS_UNIT_FOR_PUBLISH * s.rgp,
                s.rgp,
            ),
            &publisher_key,
        ))
        .unwrap();
    let claim = s.build_claim_of(&key, &gas[0]);
    let effects = s.execute_with_assigned_commit(vec![publish, claim], 1);
    s.complete_commit(1, &effects.iter().collect::<Vec<_>>());
    let package_id = effects[0]
        .created()
        .into_iter()
        .map(|created| created.reference().object_id)
        .find(|id| s.authority.get_object(id).unwrap().is_package())
        .expect("the publish creates the package");
    let version = s.assert_claimed_account(&key, &effects[1]);
    s.complete_empty_commit(2);

    // Commit 3 rotates the account to the package's authenticator.
    let field_id = derive_authenticator_function_ref_v1_dynamic_field_id(account_id).unwrap();
    let field_before = s.latest_ref(&field_id);
    let mut builder = ProgrammableTransactionBuilder::new();
    let metadata = builder
        .obj(CallArg::ImmutableOrOwned(s.latest_ref(
            &move_package::derive_package_metadata_id(package_id),
        )))
        .unwrap();
    let module = builder.pure("test_account").unwrap();
    let function = builder.pure("authenticate_smart_account").unwrap();
    let authenticator = builder.programmable_move_call(
        ObjectId::FRAMEWORK,
        Identifier::from_static("authenticator_function"),
        Identifier::from_static("create_auth_function_ref_v1"),
        vec![TypeTag::from_str("0x2::smart_account::SmartAccount").unwrap()],
        vec![metadata, module, function],
    );
    let account = builder
        .obj(CallArg::Shared(SharedObjectReference::new(
            account_id, version, true,
        )))
        .unwrap();
    builder.programmable_move_call(
        ObjectId::FRAMEWORK,
        Identifier::from_static("smart_account_builtin_authenticator"),
        Identifier::from_static("rotate_to_custom_authenticator_v1"),
        vec![],
        vec![account, authenticator],
    );
    let rotation = s.sign_through_account(
        Transaction::new_programmable(
            address,
            vec![s.latest_ref(&gas[1])],
            builder.finish(),
            iota_types::transaction::TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE * s.rgp,
            s.rgp,
        ),
        address,
        version,
        &key,
        None,
    );
    let rotated = s
        .execute_at_commit_with_shared_versions(vec![rotation], 3)
        .remove(0);
    assert!(rotated.status().is_success(), "{:?}", rotated.status());
    s.complete_commit(3, &[&rotated]);
    let field_after = s.latest_ref(&field_id);
    assert!(field_after.version > field_before.version);
    assert!(
        rotated
            .mutated()
            .iter()
            .any(|mutated| mutated.reference().object_id == field_id),
        "the rotation writes the field at a new version: {rotated:?}"
    );
    assert!(
        !rotated
            .deleted()
            .iter()
            .any(|deleted| deleted.object_id == field_id),
        "the rotation does not delete the field"
    );

    let second = s.build_claim_of(&key, &gas[2]);
    let plain = s.sign_with(s.object_basics_create(address, &gas[3], 1), &[&key]);
    // Horizons 2, 3 and 4: before, at and after the rotation's commit.
    for horizon in 2..=4 {
        let commit = horizon + k;
        assert_eq!(
            s.reader_at(commit).read_claimed_address(address).unwrap(),
            ClaimedAddressAtHorizon::Claimed,
            "horizon {horizon}"
        );
        assert_eq!(
            s.validate_at(commit, &[second.clone(), plain.clone()])
                .await
                .dropped,
            vec![
                (*second.digest(), already_explicit(address)),
                (*plain.digest(), plain_signature_for(address)),
            ],
            "horizon {horizon}"
        );
    }

    s.advance_epoch().await;
    assert_eq!(
        s.reader_at(1).read_claimed_address(address).unwrap(),
        ClaimedAddressAtHorizon::Claimed
    );
    assert_eq!(
        s.validate_at(1, &[second.clone(), plain.clone()])
            .await
            .dropped,
        vec![
            (*second.digest(), already_explicit(address)),
            (*plain.digest(), plain_signature_for(address)),
        ]
    );
}
