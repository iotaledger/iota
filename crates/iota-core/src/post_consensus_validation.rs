// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Post-consensus validation and owned-object conflict resolution for
//! `UserTransactionV1` and `UserTransactionV2` transactions.
//!
//! This module merges two formerly separate pipeline stages into a single pass:
//!
//! 1. **Semantic validation** — deduplication, already-executed check,
//!    structural validity, attestor verification, and deny checks (deny lists,
//!    gas, ownership, coin deny list, Move authenticator).
//! 2. **Owned-object conflict resolution** (white-flag) — three-tier lock check
//!    and lock acquisition.
//!
//! Processing both in one loop avoids iterating over every transaction twice
//! and skips expensive validation for transactions that can't acquire object
//! locks anyway.
//!
//! # Per-transaction order within the loop
//!
//! - Non-user transaction — pass through unchanged.
//! - Check #0: Dedup by `ConsensusTransactionKey` — silent drop.
//! - Check #1: Already executed — **retained** as a committee-agreed winner
//!   (registers its locks, skips re-validation); not dropped. See issue #11649.
//! - Check #2: `validity_check()` — drop with error.
//! - Check #3: Attestor verification (`UserTransactionV2` only) — verifies that
//!   the claimed attestor matches the block author and that the attested
//!   computation units fall within the valid range (cost floor and ceiling).
//!   Drop with error on mismatch, out-of-range cost, or unsupported attestation
//!   variant.
//! - Check #4: Extract owned input objects (needed for lock conflict
//!   detection).
//! - Check #5: Three-tier lock conflict check (local HashMap → quarantine → DB)
//!   — drop with error, except a lock held by the same transaction (a deferred
//!   tx's own prior-round lock), which is exempt. Cheap; performed before
//!   expensive checks.
//! - Check #6: `handle_transaction_validation_checks()` for
//!   `UserTransactionV1`, or the deny-list and coin deny-list re-checks for
//!   attested `UserTransactionV2`
//!   (`check_transaction_deny_list_for_attested_tx()` then
//!   `check_coin_deny_list_for_attested_tx()`). Drop with error. Only reached
//!   when all locks are free.
//! - All passed — acquire locks in the local tracking map, keep transaction.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use iota_common::fatal;
use iota_types::{
    attestation::Attestation,
    base_types::{ObjectRef, TransactionDigest},
    error::{IotaError, IotaResult, UserInputError},
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

/// Validates `UserTransactionV1/V2` transactions and resolves owned-object
/// conflicts in a single pass.
///
/// For each `UserTransactionV1` or `UserTransactionV2` in consensus order:
/// - Runs deduplication, already-executed check, structural validity, attestor
///   verification (V2 only), lock conflict check, and deny checks (deny list,
///   gas, ownership, coin deny list, Move authenticator).
/// - If all checks pass, acquires owned-object locks in a local tracking map.
/// - Drops the transaction (with an error) on any failure.
/// - An already-executed transaction is **retained** (not dropped): it
///   registers its owned-object locks and skips re-validation. See issue
///   #11649.
///
/// Non-`UserTransactionV1`/`UserTransactionV2` transactions pass through
/// unchanged.
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
/// `Ok((dropped, locks, all_user_tx_digests))` where:
/// - `dropped` — `(digest, error)` for every semantically-invalid or
///   lock-conflicting transaction. Silent drops (duplicates) are **not**
///   included.
/// - `locks` — Owned-object locks acquired in this commit, to be stored in the
///   consensus quarantine so subsequent commits can see them.
/// - `all_user_tx_digests` — Every `UserTransactionV1`/`UserTransactionV2`
///   digest that passed dedup (both kept and dropped). Used by the caller to
///   release pre-consensus soft locks.
pub async fn validate_and_resolve_conflicts(
    authority_state: &AuthorityState,
    epoch_store: &Arc<AuthorityPerEpochStore>,
    transactions: &mut Vec<VerifiedSequencedConsensusTransaction>,
) -> IotaResult<(
    Vec<(TransactionDigest, IotaError)>,
    HashMap<ObjectRef, LockDetails>,
    Vec<TransactionDigest>,
)> {
    let mut dropped: Vec<(TransactionDigest, IotaError)> = Vec::new();
    let mut seen_keys: HashSet<SequencedConsensusTransactionKey> = HashSet::new();
    // Locks acquired within this commit. Populated for every transaction that
    // passes all checks; used by subsequent transactions' conflict checks.
    let mut current_commit_locks: HashMap<ObjectRef, LockDetails> = HashMap::new();
    // Index-parallel keep flags: true = keep, false = remove.
    let mut keep = vec![true; transactions.len()];
    // All UserTransactionV1/V2 digests seen in this commit (both kept and dropped),
    // used by the caller to release pre-consensus soft locks.
    let mut all_user_tx_digests = Vec::with_capacity(transactions.len());

    for (i, tx) in transactions.iter().enumerate() {
        // Check #0: Dedup by ConsensusTransactionKey.
        // The same UserTransactionV1 or UserTransactionV2 may appear in DAG
        // blocks from multiple validators within the same consensus commit.
        // Only the first occurrence is kept. Silent drop — not added to `dropped`.
        if !seen_keys.insert(tx.0.key()) {
            keep[i] = false;
            continue;
        }

        // Only validate UserTransactionV1/V2; pass everything else through
        // unchanged.
        let (transaction, attestation) = match &tx.0.transaction {
            SequencedConsensusTransactionKind::External(ConsensusTransaction {
                kind: ConsensusTransactionKind::UserTransactionV1(t),
                ..
            }) => (t.as_ref(), None),
            SequencedConsensusTransactionKind::External(ConsensusTransaction {
                kind: ConsensusTransactionKind::UserTransactionV2(a),
                ..
            }) => (&a.transaction, Some(&a.attestation)),
            _ => continue,
        };

        let digest = *transaction.digest();
        all_user_tx_digests.push(digest);

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
        if let Err(e) = transaction.validity_check(&epoch_store.tx_validity_check_context()) {
            warn!(
                ?digest,
                kind = if attestation.is_some() { "UserTransactionV2" } else { "UserTransactionV1" },
                error = ?e,
                "user transaction failed validity_check post-consensus, dropping"
            );
            dropped.push((digest, e));
            keep[i] = false;
            continue;
        }

        // Check #3: Attestor verification (UserTransactionV2 only).
        // The block signature transitively authenticates the attestation;
        // verify the claimed attestor matches the actual block author and that
        // the payload is not malformed.
        if let Some(attestation) = attestation {
            let block_author =
                starfish_config::AuthorityIndex::from(tx.0.certificate_author_index as u8);
            let protocol_config = epoch_store.protocol_config();
            let min_attested_units = protocol_config
                .base_tx_cost_fixed()
                .min(protocol_config.gas_rounding_step());
            let attested_units = attestation.computation_units();
            let tx_data = transaction.data().transaction_data();
            let max_attested_units = tx_data
                .gas_budget()
                .checked_div(tx_data.gas_price())
                .unwrap_or(u64::MAX);
            let error = match attestation {
                Attestation::Validator { attestor_index, .. } => {
                    if *attestor_index != block_author {
                        Some(IotaError::AttestationAuthorMismatch {
                            expected: *attestor_index,
                            actual: block_author,
                        })
                    } else if attested_units < min_attested_units {
                        Some(IotaError::AttestationUnitsBelowMinimum {
                            actual: attested_units,
                            minimum: min_attested_units,
                        })
                    } else if attested_units > max_attested_units {
                        Some(IotaError::AttestationUnitsAboveBudget {
                            actual: attested_units,
                            maximum: max_attested_units,
                        })
                    } else {
                        None
                    }
                }
                // Reject Explicit variant as not yet implemented.
                Attestation::Explicit { .. } => Some(IotaError::UnsupportedFeature {
                    error: "Explicit attestation not yet supported".into(),
                }),
            };
            if let Some(e) = error {
                warn!(
                    ?digest,
                    error = ?e,
                    "UserTransactionV2 failed attestation verification, dropping"
                );
                dropped.push((digest, e));
                keep[i] = false;
                continue;
            }
        }

        // Check #4: Extract owned input objects for lock conflict detection.
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

        // Check #5: Three-tier lock conflict check.
        // Cheap (HashMap + quarantine + DB lookups); performed before the
        // expensive deny checks so conflicting transactions are filtered first.
        //
        // Locks are keyed by full ObjectRef (id + version + digest), not just
        // ObjectID. Two transactions referencing the same object at different
        // versions will NOT conflict here — version freshness is validated
        // later in Check #6 (deny checks load objects from DB and verify
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
            dropped.push((digest, e));
            keep[i] = false;
            continue;
        }

        // Check #6: Deny list, gas, ownership, coin deny list, Move
        // authenticator. Only reached if all locks are free — skips the
        // expensive object loading for transactions that would be dropped
        // by the lock conflict check.
        //
        // `UserTransactionV1` runs the full
        // `handle_transaction_validation_checks` (which includes the
        // `TransactionDenyConfig` deny-list check). For `UserTransactionV2`
        // (attested transactions) two checks are re-run individually — the
        // deny-list check and the coin deny-list check (see below). The rest
        // of `handle_transaction_validation_checks` is skipped for V2 because
        // it is either re-applied during execution or is not safety-critical
        // to run post-consensus:
        //   - Receiving-object validity: the Move runtime fails the `receive()` call
        //     when the ref doesn't match current state.
        //   - Move bytecode verifier on publish: the Move VM re-verifies every newly
        //     published package; the signing-time variant only adds a stricter meter as
        //     a DoS gate.
        //   - Gas, ownership, `MoveAuthenticator` execution: re-applied in the
        //     execution pipeline (`check_certificate_input` and
        //     `authenticate_then_execute_transaction_to_effects`).
        //
        // The user signature is verified pre-consensus in the block verifier
        // (`IotaTxValidator::validate_transactions`) for both `UserTransactionV1`
        // and `UserTransactionV2`, and is not re-checked here.
        //
        // Deny-list check (`TransactionDenyConfig`: sender/object/package deny
        // lists, feature kill-switches): this is a LOCAL check, sourced from
        // each validator's `NodeConfig`. TODO: source the deny config from
        // consensus-agreed state instead of the local `NodeConfig`.
        //
        // Coin deny list v1 MUST be re-checked here for attested
        // transactions: the attestor's view may be stale if a deny-list
        // update tx was sequenced between attestation and consensus, and
        // running this check at execution time would crash the validator.
        if attestation.is_none() {
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
                dropped.push((digest, e));
                keep[i] = false;
                continue;
            }
        } else {
            let verified_tx = VerifiedTransaction::new_from_verified(transaction.clone());
            // Deny-list check (placeholder using the local deny config — see
            // the `TransactionDenyConfig` note in the Check #6 doc above).
            if let Err(e) =
                authority_state.check_transaction_deny_list_for_attested_tx(&verified_tx)
            {
                if e.is_storage_or_epoch_error() {
                    return Err(e);
                }
                warn!(
                    ?digest,
                    error = ?e,
                    "UserTransactionV2 failed post-consensus deny-list check, dropping"
                );
                dropped.push((digest, e));
                keep[i] = false;
                continue;
            }
            if let Err(e) = authority_state
                .check_coin_deny_list_for_attested_tx(&verified_tx, epoch_store.epoch())
            {
                if e.is_storage_or_epoch_error() {
                    return Err(e);
                }
                // The helper performs two distinct steps; surface which one
                // failed so triage doesn't mistake a stale-attestation input
                // for an actual deny-list violation.
                let reason = match &e {
                    IotaError::UserInput {
                        error:
                            UserInputError::CoinTypeGlobalPause { .. }
                            | UserInputError::AddressDeniedForCoin { .. },
                    } => "coin deny-list re-check",
                    _ => "input load (likely stale attestation)",
                };
                warn!(
                    ?digest,
                    error = ?e,
                    "UserTransactionV2 failed post-consensus {reason}, dropping"
                );
                dropped.push((digest, e));
                keep[i] = false;
                continue;
            }
        }

        // All checks passed — acquire owned-object locks in local tracking.
        let num_owned_inputs = owned_inputs.len();
        for obj_ref in &owned_inputs {
            current_commit_locks.insert(*obj_ref, digest);
        }
        // Log the acquired refs, not just their count, so the winner's locks
        // are attributable per (object_id, version).
        debug!(
            ?digest,
            num_owned_inputs,
            owned_inputs = ?owned_inputs,
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

    Ok((dropped, current_commit_locks, all_user_tx_digests))
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

/// Extracts owned input object references from a `UserTransactionV1` or
/// `UserTransactionV2` consensus transaction.
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
        SequencedConsensusTransactionKind::External(ConsensusTransaction {
            kind: ConsensusTransactionKind::UserTransactionV2(a),
            ..
        }) => a.transaction.data(),
        _ => {
            return Err(IotaError::GenericAuthority {
                error:
                    "Expected UserTransactionV1 or UserTransactionV2 in extract_owned_input_objects"
                        .to_string(),
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
