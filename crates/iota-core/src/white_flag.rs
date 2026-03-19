// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! White flag conflict resolution for post-consensus owned object locking.
//!
//! This module implements the white flag flow, where transactions bypass
//! pre-consensus certification and owned object locking. Instead, conflicts are
//! resolved deterministically post-consensus using persistent locks.
//!
//! # Key Invariants
//!
//! - A transaction is issued through either white-flag flow OR certificate
//!   flow, never both
//! - Conflict resolution happens in consensus order
//! - Uses the same persistent lock mechanism
//!   (`owned_object_locked_transactions` table) as cert flow
//! - Deferred transactions retain their locks across commits, giving them
//!   natural precedence
//! - If ANY owned input object is locked, the ENTIRE transaction is dropped
//!   (atomic conflict detection)

use std::collections::HashMap;

use iota_types::{
    base_types::{ObjectRef, TransactionDigest},
    error::{IotaError, IotaResult},
    messages_consensus::{ConsensusTransaction, ConsensusTransactionKind},
    transaction::{InputObjectKind, TransactionDataAPI},
};
use tracing::{debug, warn};

use crate::{
    authority::authority_per_epoch_store::{AuthorityPerEpochStore, LockDetails},
    consensus_handler::{SequencedConsensusTransactionKind, VerifiedSequencedConsensusTransaction},
};

/// Resolves owned object conflicts for white-flag transactions using persistent
/// object locks. Runs BEFORE PostConsensusTxReorder.
///
/// For each UserTransactionV1 in consensus order:
/// 1. Extract all owned input objects
/// 2. Check if ALL owned inputs are unlocked (both in persistent storage and in
///    local tracking)
/// 3. If any owned input is already locked → drop the entire transaction
/// 4. If all owned inputs are free → acquire persistent locks, add to local
///    tracking, and keep the tx
///
/// Deferred transactions from previous commits already have their locks in
/// persistent storage from when they won conflict resolution in their original
/// commit. This gives them natural precedence over new transactions.
///
/// # Arguments
///
/// * `epoch_store` - The epoch store for accessing persistent lock storage
/// * `transactions` - Mutable vector of sequenced consensus transactions
///   (modified in-place)
///
/// # Returns
///
/// * `Ok((Vec<TransactionDigest>, HashMap<ObjectRef, LockDetails>))` - Tuple
///   of:
///   - Vector of dropped transaction digests
///   - HashMap of accumulated locks acquired in this commit
/// * `Err(IotaError)` - If lock acquisition or object loading fails
///
/// # Invariant
///
/// INVARIANT: A transaction is issued through either white-flag OR cert flow,
/// never both. This is enforced by the feature flag and submission endpoint.
pub fn resolve_owned_object_conflicts(
    epoch_store: &AuthorityPerEpochStore,
    transactions: &mut Vec<VerifiedSequencedConsensusTransaction>,
) -> IotaResult<(Vec<TransactionDigest>, HashMap<ObjectRef, LockDetails>)> {
    let mut dropped = Vec::new();
    let mut current_commit_locks: HashMap<ObjectRef, LockDetails> = HashMap::new();

    transactions.retain(|tx| {
        // Only process UserTransactionV1 variants
        let is_user_tx = matches!(
            &tx.0.transaction,
            SequencedConsensusTransactionKind::External(ConsensusTransaction {
                kind: ConsensusTransactionKind::UserTransactionV1(_),
                ..
            })
        );

        if !is_user_tx {
            return true; // keep non-white-flag txs unchanged
        }

        // Extract transaction digest
        let tx_digest = match tx.0.transaction.executable_transaction_digest() {
            Some(digest) => digest,
            None => {
                warn!("UserTransactionV1 missing digest, dropping");
                return false;
            }
        };

        // Extract owned input objects
        let owned_inputs = match extract_owned_input_objects(tx) {
            Ok(inputs) => inputs,
            Err(e) => {
                warn!(
                    ?tx_digest,
                    error = ?e,
                    "Failed to extract owned input objects for white flag transaction, dropping"
                );
                dropped.push(tx_digest);
                return false; // drop tx on error
            }
        };

        // Check for conflicts using three-tier lookup:
        // Tier 1: Local HashMap (current commit)
        // Tier 2: Consensus quarantine (previous uncommitted commits)
        // Tier 3: DB (committed data)
        for obj_ref in &owned_inputs {
            // Tier 1: Check local tracking (within this commit)
            if current_commit_locks.contains_key(obj_ref) {
                debug!(
                    ?tx_digest,
                    ?obj_ref,
                    "White flag transaction conflicts with earlier tx in same commit, dropping"
                );
                dropped.push(tx_digest);
                return false;
            }

            // Tier 2: Check quarantine (previous uncommitted commits)
            if let Some(locked_by) = epoch_store.get_quarantined_owned_object_lock(obj_ref) {
                debug!(
                    ?tx_digest,
                    ?obj_ref,
                    ?locked_by,
                    "White flag transaction conflicts with quarantined lock, dropping"
                );
                dropped.push(tx_digest);
                return false;
            }

            // Tier 3: Check DB (committed data)
            match epoch_store.tables() {
                Ok(tables) => {
                    match tables.get_locked_transaction(obj_ref) {
                        Ok(Some(locked_by)) => {
                            debug!(
                                ?tx_digest,
                                ?obj_ref,
                                ?locked_by,
                                "White flag transaction conflicts with persistent lock, dropping"
                            );
                            dropped.push(tx_digest);
                            return false;
                        }
                        Ok(None) => {} // object is unlocked, continue checking
                        Err(e) => {
                            warn!(
                                ?tx_digest,
                                ?obj_ref,
                                error = ?e,
                                "Failed to check persistent lock for white flag transaction, dropping"
                            );
                            dropped.push(tx_digest);
                            return false;
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        ?tx_digest,
                        error = ?e,
                        "Failed to access epoch store tables for white flag transaction, dropping"
                    );
                    dropped.push(tx_digest);
                    return false;
                }
            }
        }

        // No conflicts: add locks to current commit tracking
        let num_owned_inputs = owned_inputs.len();
        for obj_ref in owned_inputs {
            current_commit_locks.insert(obj_ref, tx_digest);
        }

        debug!(
            ?tx_digest,
            num_owned_inputs,
            "White flag transaction successfully acquired all object locks"
        );
        true // all objects locked successfully, keep tx
    });

    if !dropped.is_empty() {
        warn!(
            num_dropped = dropped.len(),
            num_retained = transactions.len(),
            "White flag conflict resolution dropped transactions"
        );
    }

    Ok((dropped, current_commit_locks))
}

/// Extracts owned input object references from a sequenced consensus
/// transaction.
///
/// Returns only the owned objects (not shared, not immutable). These are the
/// objects that need to be locked for conflict resolution.
fn extract_owned_input_objects(
    tx: &VerifiedSequencedConsensusTransaction,
) -> IotaResult<Vec<ObjectRef>> {
    // Extract transaction data from UserTransactionV1
    let transaction_data = match &tx.0.transaction {
        SequencedConsensusTransactionKind::External(ConsensusTransaction {
            kind: ConsensusTransactionKind::UserTransactionV1(transaction),
            ..
        }) => transaction.data().transaction_data(),
        _ => {
            // TODO: add new error variant
            return Err(IotaError::GenericAuthority {
                error: "Expected UserTransactionV1 in white flag conflict resolution".to_string(),
            });
        }
    };

    let input_objects = transaction_data.input_objects()?;

    let owned_objects: Vec<ObjectRef> = input_objects
        .into_iter()
        .filter_map(|input| match input {
            InputObjectKind::ImmOrOwnedMoveObject(obj_ref) => {
                // Note: This includes both owned and immutable objects.
                // In the full implementation, we'd check the object's ownership
                // status from the cache. For the PoC, we treat all as potentially owned.
                Some(obj_ref)
            }
            InputObjectKind::SharedMoveObject { .. } => None, // shared objects handled separately
            InputObjectKind::MovePackage(_) => None,          // packages don't need locks
        })
        .collect();

    Ok(owned_objects)
}
