// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Post-consensus validation and owned-object conflict resolution for
//! `UserTransactionV1` transactions.
//!
//! This module merges two formerly separate pipeline stages into a single pass:
//!
//! 1. **Semantic validation** — deduplication, already-executed check,
//!    structural validity, and deny checks (deny lists, gas, ownership, coin
//!    deny list, Move authenticator).
//! 2. **Owned-object conflict resolution** (white-flag) — three-tier lock check
//!    and lock acquisition.
//!
//! Processing both in one loop avoids iterating over every transaction twice
//! and skips expensive validation for transactions that can't acquire object
//! locks anyway.
//!
//! # Per-transaction order within the loop
//!
//! 1. Non-`UserTransactionV1` — pass through unchanged.
//! 2. Dedup by `ConsensusTransactionKey` — silent drop.
//! 3. Already executed — **retained** as a committee-agreed winner (registers
//!    its locks, skips re-validation); not dropped. See issue #11649.
//! 4. `validity_check()` — drop with error.
//! 5. Three-tier lock conflict check (local HashMap → quarantine → DB) — drop
//!    with error. Cheap; performed before expensive checks.
//! 6. `handle_transaction_validation_checks()` — drop with error. Only reached
//!    when all locks are free.
//! 7. All passed — acquire locks in the local tracking map, keep transaction.
//!
//! Non-`UserTransactionV1` transactions pass through unchanged.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use iota_common::fatal;
use iota_types::{
    base_types::{ObjectRef, TransactionDigest},
    error::{IotaError, IotaResult},
    messages_consensus::{ConsensusTransaction, ConsensusTransactionKind},
    transaction::{InputObjectKind, VerifiedTransaction},
};
use tracing::{debug, warn};

use crate::{
    authority::{
        AuthorityState,
        authority_per_epoch_store::{AuthorityPerEpochStore, LockDetails},
    },
    consensus_handler::{
        SequencedConsensusTransactionKey, SequencedConsensusTransactionKind,
        VerifiedSequencedConsensusTransaction,
    },
};

/// Validates `UserTransactionV1` transactions and resolves owned-object
/// conflicts in a single pass.
///
/// For each `UserTransactionV1` in consensus order:
/// - Runs deduplication, structural validity, lock conflict check, and deny
///   checks (deny list, gas, ownership, coin deny list, Move authenticator).
/// - If all checks pass, acquires owned-object locks in a local tracking map.
/// - Drops the transaction (with an error) on any failure.
/// - An already-executed transaction is **retained** (not dropped): it
///   registers its owned-object locks and skips re-validation. See issue
///   #11649.
///
/// Non-`UserTransactionV1` transactions pass through unchanged.
///
/// # Arguments
///
/// * `authority_state` — Used for cache reads and deny checks.
/// * `epoch_store` — Current epoch store (protocol config, lock storage).
/// * `transactions` — All sequenced transactions for this consensus commit;
///   modified in-place.
///
/// # Returns
///
/// `Ok((dropped, locks))` where:
/// - `dropped` — `(digest, error)` for every semantically-invalid or
///   lock-conflicting transaction. Silent drops (duplicates) are **not**
///   included.
/// - `locks` — Owned-object locks acquired in this commit, to be stored in the
///   consensus quarantine so subsequent commits can see them.
pub async fn validate_and_resolve_conflicts(
    authority_state: &AuthorityState,
    epoch_store: &Arc<AuthorityPerEpochStore>,
    transactions: &mut Vec<VerifiedSequencedConsensusTransaction>,
) -> IotaResult<(
    Vec<(TransactionDigest, IotaError)>,
    HashMap<ObjectRef, LockDetails>,
)> {
    let mut dropped: Vec<(TransactionDigest, IotaError)> = Vec::new();
    let mut seen_keys: HashSet<SequencedConsensusTransactionKey> = HashSet::new();
    // Locks acquired within this commit. Populated for every transaction that
    // passes all checks; used by subsequent transactions' conflict checks.
    let mut current_commit_locks: HashMap<ObjectRef, LockDetails> = HashMap::new();
    // Index-parallel keep flags: true = keep, false = remove.
    let mut keep = vec![true; transactions.len()];

    for (i, tx) in transactions.iter().enumerate() {
        // Check #0: Dedup by ConsensusTransactionKey.
        // The same UserTransactionV1 may appear in DAG blocks from multiple
        // validators within the same consensus commit. Only the first occurrence
        // is kept. Silent drop — not added to `dropped`.
        if !seen_keys.insert(tx.0.key()) {
            keep[i] = false;
            continue;
        }

        // Only validate UserTransactionV1; pass everything else through
        // unchanged.
        let transaction = match &tx.0.transaction {
            SequencedConsensusTransactionKind::External(ConsensusTransaction {
                kind: ConsensusTransactionKind::UserTransactionV1(t),
                ..
            }) => t,
            _ => continue,
        };

        let digest = *transaction.digest();

        // Check #1: Already executed (typically by state-sync before this node's
        // consensus handler reached the commit). It is a committee-agreed winner, so
        // keep it in the sequence to flow into checkpoint roots like on every other
        // validator (dropping it forks — issue #11649). Register its owned-object
        // locks so double-spend siblings still lose, then skip re-validation (#2/#5);
        // `TransactionManager::enqueue` suppresses the re-execution.
        if authority_state
            .get_transaction_cache_reader()
            .try_is_tx_already_executed(&digest)?
        {
            // Byte-based, so safe even though the inputs are already consumed.
            let owned_inputs = extract_owned_input_objects(tx)?;
            for obj_ref in &owned_inputs {
                // A winner cannot be out-locked: an executed tx owns its inputs. A lock
                // held by a different tx is a consistency violation, not a conflict.
                if let Some(other) =
                    find_existing_lock(obj_ref, &current_commit_locks, epoch_store)?
                {
                    if other != digest {
                        fatal!(
                            "already-executed transaction {:?} has owned input {:?} \
                             locked by a different transaction {:?}",
                            digest,
                            obj_ref,
                            other,
                        );
                    }
                }
                current_commit_locks.insert(*obj_ref, digest);
            }
            debug!(
                ?digest,
                num_owned_inputs = owned_inputs.len(),
                "Transaction already executed; retained as checkpoint root, skipping re-validation"
            );
            // keep[i] stays true so the transaction remains in the sequence.
            continue;
        }

        // Check #2: Structural validity.
        if let Err(e) =
            transaction.validity_check(epoch_store.protocol_config(), epoch_store.epoch())
        {
            warn!(
                ?digest,
                error = ?e,
                "UserTransactionV1 failed validity_check post-consensus, dropping"
            );
            dropped.push((digest, e));
            keep[i] = false;
            continue;
        }

        // Check #3: Extract owned input objects for lock conflict detection.
        let owned_inputs = match extract_owned_input_objects(tx) {
            Ok(inputs) => inputs,
            Err(e) => {
                warn!(
                    ?digest,
                    error = ?e,
                    "Failed to extract owned input objects post-consensus, dropping"
                );
                dropped.push((digest, e));
                keep[i] = false;
                continue;
            }
        };

        // Check #4: Three-tier lock conflict check.
        // Cheap (HashMap + quarantine + DB lookups); performed before the
        // expensive deny checks so conflicting transactions are filtered first.
        //
        // Locks are keyed by full ObjectRef (id + version + digest), not just
        // ObjectID. Two transactions referencing the same object at different
        // versions will NOT conflict here — version freshness is validated
        // later in Check #5 (deny checks load objects from DB and verify
        // that the transaction's input refs match the current state).
        //
        // Tier 1: Local HashMap (current commit).
        // Tier 2: Consensus quarantine (previous uncommitted commits).
        // Tier 3: Persistent DB (committed data).
        let mut conflict: Option<IotaError> = None;
        for obj_ref in &owned_inputs {
            if let Some(locked_by) =
                find_existing_lock(obj_ref, &current_commit_locks, epoch_store)?
            {
                debug!(
                    ?digest,
                    ?obj_ref,
                    ?locked_by,
                    "Transaction conflicts with existing owned-object lock, dropping"
                );
                conflict = Some(IotaError::ObjectLockConflict {
                    obj_ref: *obj_ref,
                    pending_transaction: locked_by,
                });
                break;
            }
        }
        if let Some(e) = conflict {
            dropped.push((digest, e));
            keep[i] = false;
            continue;
        }

        // Check #5: Deny list, gas, ownership, coin deny list, Move
        // authenticator. Only reached if all locks are free — skips the
        // expensive object loading for transactions that would be dropped
        // by the lock conflict check.
        //
        // Safe to skip signature re-verification: the consensus block verifier
        // (`IotaTxValidator::validate_transactions`) already called
        // `verify_tx()` on every `UserTransactionV1` before accepting the
        // block. Re-verifying here would not add safety — if a quorum
        // committed a bad signature it indicates a protocol-level failure
        // (2f+1 Byzantine/buggy validators), not something we can recover from
        // by rejecting the transaction post-consensus. Doing so would also risk
        // diverging from other honest validators.
        let verified_tx = VerifiedTransaction::new_from_verified((**transaction).clone());
        if let Err(e) = authority_state
            .handle_transaction_validation_checks(&verified_tx, epoch_store)
            .await
        {
            if e.is_storage_or_epoch_error() {
                return Err(e);
            }
            warn!(
                ?digest,
                error = ?e,
                "UserTransactionV1 failed post-consensus deny checks, dropping"
            );
            dropped.push((digest, e));
            keep[i] = false;
            continue;
        }

        // All checks passed — acquire owned-object locks in local tracking.
        let num_owned_inputs = owned_inputs.len();
        for obj_ref in owned_inputs {
            current_commit_locks.insert(obj_ref, digest);
        }
        debug!(
            ?digest,
            num_owned_inputs,
            "Transaction passed post-consensus validation, acquired all object locks"
        );
    }

    if !dropped.is_empty() {
        warn!(
            num_dropped = dropped.len(),
            num_retained = transactions
                .iter()
                .enumerate()
                .filter(|(i, _)| keep[*i])
                .count(),
            "Post-consensus validation dropped transactions"
        );
    }

    // Remove invalid/duplicate/conflicting entries using the parallel keep
    // flags.
    let mut iter = keep.into_iter();
    transactions.retain(|_| iter.next().unwrap_or(true));

    Ok((dropped, current_commit_locks))
}

/// Finds an existing owned-object lock on `obj_ref`, walking three tiers in
/// order:
/// 1. `current_commit_locks` — locks acquired earlier in the same commit.
/// 2. Consensus quarantine — locks from previous uncommitted commits.
/// 3. Persistent DB — committed locks.
///
/// Returns `Ok(Some(locker))` if any tier holds a lock, `Ok(None)` if the
/// input is free. The caller decides what to do with the result (drop with
/// `ObjectLockConflict` for a contender, or `fatal!` if a winner is
/// out-locked).
fn find_existing_lock(
    obj_ref: &ObjectRef,
    current_commit_locks: &HashMap<ObjectRef, LockDetails>,
    epoch_store: &Arc<AuthorityPerEpochStore>,
) -> IotaResult<Option<LockDetails>> {
    if let Some(&locker) = current_commit_locks.get(obj_ref) {
        return Ok(Some(locker));
    }
    if let Some(locker) = epoch_store.get_quarantined_owned_object_lock(obj_ref) {
        return Ok(Some(locker));
    }
    epoch_store.tables()?.get_locked_transaction(obj_ref)
}

/// Extracts owned input object references from a `UserTransactionV1`
/// consensus transaction.
///
/// Returns only `ImmOrOwnedMoveObject` inputs (excludes shared objects and
/// packages) — these are the objects that need lock conflict detection.
fn extract_owned_input_objects(
    tx: &VerifiedSequencedConsensusTransaction,
) -> IotaResult<Vec<ObjectRef>> {
    let transaction_data = match &tx.0.transaction {
        SequencedConsensusTransactionKind::External(ConsensusTransaction {
            kind: ConsensusTransactionKind::UserTransactionV1(transaction),
            ..
        }) => transaction.data(),
        _ => {
            return Err(IotaError::GenericAuthority {
                error: "Expected UserTransactionV1 in extract_owned_input_objects".to_string(),
            });
        }
    };

    // Use SenderSignedData::input_objects() rather than
    // TransactionData::input_objects() to also include any owned objects
    // that may come from MoveAuthenticator signatures in the future.
    let owned_objects = transaction_data
        .input_objects()?
        .into_iter()
        .filter_map(|input| match input {
            InputObjectKind::ImmOrOwnedMoveObject(obj_ref) => Some(obj_ref),
            InputObjectKind::SharedMoveObject { .. } => None,
            InputObjectKind::MovePackage(_) => None,
        })
        .collect();

    Ok(owned_objects)
}

#[cfg(test)]
#[path = "unit_tests/post_consensus_validation_tests.rs"]
mod post_consensus_validation_tests;
