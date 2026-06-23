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
//! When account-claim invalidation is enabled (issue #11900), the
//! `0x2::smart_account::build_v1` claimers are validated first (recording their
//! senders), then every other transaction whose sender or sponsor matches a
//! claim is dropped. `build_v1` transactions themselves are never dropped by
//! this rule.
//!
//! # Per-transaction order within the loop
//!
//! 1. Non-`UserTransactionV1` — pass through unchanged.
//! 2. Dedup by `ConsensusTransactionKey` — silent drop.
//! 3. Already executed — **retained** as a committee-agreed winner (registers
//!    its locks, skips re-validation); not dropped. See issue #11649.
//! 4. `validity_check()` — drop with error.
//! 5. Three-tier lock conflict check (local HashMap → quarantine → DB) — drop
//!    with error, except a lock held by the same transaction (a deferred tx's
//!    own prior-round lock), which is exempt. Cheap; performed before expensive
//!    checks.
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
use iota_sdk_types::Address;
use iota_types::{
    base_types::{ObjectRef, TransactionDigest},
    error::{IotaError, IotaResult},
    messages_consensus::{ConsensusTransaction, ConsensusTransactionKind},
    transaction::{InputObjectKind, TransactionDataAPI, VerifiedTransaction},
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

/// Mutable per-commit state threaded through [`transaction_validation_logic`]
/// and the surrounding loop.
struct CommitValidationState {
    /// Seen `ConsensusTransactionKey`s, for deduplication.
    seen_keys: HashSet<SequencedConsensusTransactionKey>,
    /// Index-parallel keep flags: `true` = keep, `false` = remove.
    keep: Vec<bool>,
    /// `(digest, error)` for every dropped transaction (silent dedup drops are
    /// not recorded here).
    dropped: Vec<(TransactionDigest, IotaError)>,
    /// Owned-object locks acquired within this commit. Populated for every
    /// transaction that passes all checks; used by subsequent transactions'
    /// conflict checks.
    current_commit_locks: HashMap<ObjectRef, LockDetails>,
    /// Claimed sender address -> one claiming `build_v1` transaction digest.
    claim_senders: HashMap<Address, TransactionDigest>,
}

impl CommitValidationState {
    fn new(num_transactions: usize) -> Self {
        Self {
            seen_keys: HashSet::new(),
            keep: vec![true; num_transactions],
            dropped: Vec::new(),
            current_commit_locks: HashMap::new(),
            claim_senders: HashMap::new(),
        }
    }
}

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
    let mut state = CommitValidationState::new(transactions.len());

    // When enabled, all account build and claim transactions are recorded and
    // validated first, then every other transaction whose sender or sponsor
    // matches a claim is dropped.
    if epoch_store
        .protocol_config()
        .enable_account_claim_conflict_invalidation()
    {
        // Split the commit into the claimers and everything else,
        // preserving consensus order within each group.
        let (build_account_txs, remaining_txs): (Vec<_>, Vec<_>) = transactions
            .iter()
            .enumerate()
            .partition(|(_, tx)| is_smart_account_build_user_transaction(tx));

        // Validate the claimers first so their claims are recorded before any
        // other transaction is judged against them.
        for (i, tx) in build_account_txs {
            transaction_validation_logic(authority_state, epoch_store, tx, i, true, &mut state)
                .await?;
        }
        for (j, tx) in remaining_txs {
            transaction_validation_logic(authority_state, epoch_store, tx, j, false, &mut state)
                .await?;
        }
    } else {
        // Single pass when the feature is disabled — unchanged validation
        // behavior and consensus ordering.
        for (i, tx) in transactions.iter().enumerate() {
            transaction_validation_logic(authority_state, epoch_store, tx, i, false, &mut state)
                .await?;
        }
    }

    if !state.dropped.is_empty() {
        warn!(
            num_dropped = state.dropped.len(),
            num_retained = transactions
                .iter()
                .enumerate()
                .filter(|(i, _)| state.keep[*i])
                .count(),
            "Post-consensus validation dropped transactions"
        );
    }

    // Remove invalid/duplicate/conflicting entries using the parallel keep
    // flags.
    let mut iter = state.keep.into_iter();
    transactions.retain(|_| iter.next().unwrap_or(true));

    Ok((state.dropped, state.current_commit_locks))
}

/// Runs Checks #0–#5 for a single sequenced transaction, updating `state`'s
/// `seen_keys`, `keep`, `dropped` and `current_commit_locks`.
///
/// `is_claim_tx` drives account claims checks: when `true`, the tx sender is
/// recorded in `claim_senders`; when `false`, a transaction whose sender or
/// sponsor is already in `claim_senders` is dropped with
/// `AccountClaimConflict`.
async fn transaction_validation_logic(
    authority_state: &AuthorityState,
    epoch_store: &Arc<AuthorityPerEpochStore>,
    tx: &VerifiedSequencedConsensusTransaction,
    i: usize,
    is_claim_tx: bool,
    state: &mut CommitValidationState,
) -> IotaResult<()> {
    // Check #0: Dedup by ConsensusTransactionKey.
    // The same UserTransactionV1 may appear in DAG blocks from multiple
    // validators within the same consensus commit. Only the first occurrence
    // is kept. Silent drop — not added to `dropped`.
    if !state.seen_keys.insert(tx.0.key()) {
        state.keep[i] = false;
        return Ok(());
    }

    // Only validate UserTransactionV1; pass everything else through
    // unchanged.
    let transaction = match &tx.0.transaction {
        SequencedConsensusTransactionKind::External(ConsensusTransaction {
            kind: ConsensusTransactionKind::UserTransactionV1(t),
            ..
        }) => &**t,
        _ => return Ok(()),
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
                find_existing_lock(obj_ref, &state.current_commit_locks, epoch_store)?
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
            state.current_commit_locks.insert(*obj_ref, digest);
        }
        // An already-executed build smart account tx is a committee-agreed account
        // claim, so it still claims its sender. It is retained, never dropped.
        if is_claim_tx {
            state
                .claim_senders
                .entry(transaction.data().transaction_data().sender())
                .or_insert(digest);
        }
        debug!(
            ?digest,
            num_owned_inputs = owned_inputs.len(),
            "Transaction already executed; retained as checkpoint root, skipping re-validation"
        );
        // keep[i] stays true so the transaction remains in the sequence.
        return Ok(());
    }

    // Check Account-claim: a non-claiming transaction whose sender or sponsor
    // was claimed by a build/claim transaction in this commit is dropped before
    // its own validation. Placed after Check #1 so an already-executed transaction
    // is never dropped here
    if !is_claim_tx && !state.claim_senders.is_empty() {
        let transaction_data = transaction.data().transaction_data();
        let sender = transaction_data.sender();
        let gas_owner = transaction_data.gas_owner();
        let hit = state
            .claim_senders
            .get(&sender)
            .map(|claimer| (sender, *claimer))
            .or_else(|| {
                (gas_owner != sender)
                    .then(|| {
                        state
                            .claim_senders
                            .get(&gas_owner)
                            .map(|claimer| (gas_owner, *claimer))
                    })
                    .flatten()
            });
        if let Some((address, claiming_transaction)) = hit {
            debug!(
                ?digest,
                ?address,
                ?claiming_transaction,
                "Transaction conflicts with a concurrent account claim, dropping"
            );
            state.dropped.push((
                digest,
                IotaError::AccountClaimConflict {
                    address,
                    claiming_transaction,
                },
            ));
            state.keep[i] = false;
            return Ok(());
        }
    }

    // Check #2: Structural validity.
    if let Err(e) = transaction.validity_check(epoch_store.protocol_config(), epoch_store.epoch()) {
        warn!(
            ?digest,
            error = ?e,
            "UserTransactionV1 failed validity_check post-consensus, dropping"
        );
        state.dropped.push((digest, e));
        state.keep[i] = false;
        return Ok(());
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
            state.dropped.push((digest, e));
            state.keep[i] = false;
            return Ok(());
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
            find_existing_lock(obj_ref, &state.current_commit_locks, epoch_store)?
        {
            // A lock held by this same transaction is its own lock from a
            // prior round: it acquired owned-object locks, was then deferred,
            // and is reloaded this round. A self-held lock is NOT a conflict
            // - without this guard, the transaction will be dropped with
            // `ObjectLockConflict` and never executed. Issue #11649 mirrors
            // this guard for the already-executed branch in Check #1.
            //
            // Note that same-commit duplicates cannot slip through here since
            // Check #0 already dedups by digest, so a same-digest lock here
            // is always this transaction's own prior-round lock.
            if locked_by == digest {
                continue;
            }

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
        state.dropped.push((digest, e));
        state.keep[i] = false;
        return Ok(());
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
    let verified_tx = VerifiedTransaction::new_from_verified(transaction.clone());
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
        state.dropped.push((digest, e));
        state.keep[i] = false;
        return Ok(());
    }

    // All checks passed — acquire owned-object locks in local tracking.
    let num_owned_inputs = owned_inputs.len();
    for obj_ref in &owned_inputs {
        state.current_commit_locks.insert(*obj_ref, digest);
    }
    debug!(
        ?digest,
        num_owned_inputs, "Transaction passed post-consensus validation, acquired all object locks"
    );

    // A surviving claimer records its sender so later transactions in the commit
    // can be checked against it.
    if is_claim_tx {
        state
            .claim_senders
            .entry(transaction.data().transaction_data().sender())
            .or_insert(digest);
    }

    Ok(())
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

/// Returns `true` if `tx` is a `UserTransactionV1` that calls
/// `0x2::smart_account::build_v1`.
fn is_smart_account_build_user_transaction(tx: &VerifiedSequencedConsensusTransaction) -> bool {
    matches!(
        &tx.0.transaction,
        SequencedConsensusTransactionKind::External(ConsensusTransaction {
            kind: ConsensusTransactionKind::UserTransactionV1(t),
            ..
        }) if t.data().transaction_data().calls_smart_account_build_v1()
    )
}

#[cfg(test)]
#[path = "unit_tests/post_consensus_validation_tests.rs"]
mod post_consensus_validation_tests;
