// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Post-consensus validation and owned-object conflict resolution for
//! `UserTransactionV1` transactions.
//!
//! This module merges two formerly separate pipeline stages into a single pass:
//!
//! 1. **Semantic validation** — deduplication, already-executed check, structural validity, and
//!    deny checks (deny lists, gas, ownership, coin deny list, Move authenticator).
//! 2. **Owned-object conflict resolution** (white-flag) — three-tier lock check and lock
//!    acquisition.
//!
//! Processing both in one loop avoids iterating over every transaction twice
//! and skips expensive validation for transactions that can't acquire object
//! locks anyway.
//!
//! # Per-transaction order within the loop
//!
//! 1. Non-`UserTransactionV1` — pass through unchanged.
//! 2. Dedup by `ConsensusTransactionKey` — silent drop. A later copy of a user transaction with
//!    other signatures is validated when every earlier copy was dropped for a reason that may come
//!    from its own signatures (see `drop_may_come_from_signatures`).
//! 3. Saved decisions, already-executed and deferred transactions:
//!    - Replayed commit: a commit replayed after a restart repeats the decisions saved on its first
//!      run. A saved drop is repeated without checks. Once the switch to executing synced
//!      checkpoints after commit `L` is saved, a commit at or below `L` repeats a saved keep
//!      without checks and takes no locks for it. Otherwise a saved keep is validated again unless
//!      the transaction has executed or was deferred at an earlier commit.
//!    - Already executed (typically by state sync): the transaction is **retained** as a
//!      committee-agreed winner, not dropped: dropping it would make this validator's checkpoints
//!      differ from those of the other validators. It registers the locks its own effects report
//!      and skips re-validation. This applies only if the first run of this commit saved a keep for
//!      it; otherwise it goes through the checks below like any other transaction, unless it was
//!      deferred.
//!    - Deferred at an earlier commit: the transaction is kept without checks and takes no new
//!      locks. It was validated at the commit that kept it and still holds the locks it took there.
//! 4. `validity_check()` — drop with error.
//! 5. Three-tier lock conflict check (local HashMap → quarantine → DB) — drop with error, except a
//!    lock held by the same transaction, which is exempt. Cheap; performed before expensive checks.
//! 6. `handle_transaction_validation_checks_at_commit()` — drop with error. Only reached when all
//!    locks are free.
//! 7. All passed — acquire locks in the local tracking map, keep transaction.
//!
//! Non-`UserTransactionV1` transactions pass through unchanged.
//!
//! # Which references get locked
//!
//! Step 5 probes every `ImmOrOwnedMoveObject` reference, but only genuinely
//! owned references are locked — an immutable input never takes a new
//! version, so its lock would hold until the epoch ends. Step 7 filters via
//! the loaded objects, step 3 via the executed transaction's effects.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use iota_common::fatal;
use iota_macros::fail_point;
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{ObjectReference, SenderSignedTransaction, TransactionDigest, UserSignature};
use iota_transaction_checks::VerifierLimitsSource;
use iota_types::{
    deny_rule_governance::DenyRuleConfig,
    effects::TransactionEffectsAPI,
    error::{IotaError, IotaResult},
    transaction::{
        InputObjectKind, SenderSignedTransactionAPI, TransactionAPI, VerifiedTransaction,
    },
};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};
use typed_store::Map;

use crate::{
    authority::{
        AuthorityState,
        authority_per_epoch_store::{
            AuthorityPerEpochStore, LockDetails, PreviouslyDeferredTransactions,
            handler_object_state::CommitIndex,
        },
    },
    consensus_handler::{
        SequencedConsensusTransactionKey, SequencedConsensusTransactionKind,
        VerifiedSequencedConsensusTransaction,
    },
    post_consensus_input_reader::{ValidationAtCommit, reader::CommitIndexedReader},
};

/// The post-consensus validation decision for one transaction at one
/// consensus commit.
///
/// Saved in the epoch tables, so its encoding must stay readable by other
/// binaries within the epoch: variants carry no data and new ones go at the
/// end.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PostConsensusDecision {
    /// The transaction stayed in the commit.
    Kept,
    /// The transaction was dropped.
    Dropped,
    /// The transaction was dropped because an input is not visible at the
    /// commit's [horizon](crate::post_consensus_input_reader::reader::horizon)
    /// yet. The digest can be kept when sequenced again, so the drop is not
    /// its final status.
    DroppedInputNotVisibleYet,
    /// The copy of the transaction validated last was dropped for a reason
    /// that may come from its own signatures, which other copies of the
    /// digest do not share. Another copy can be kept when sequenced, so the
    /// drop is not the final status of the digest.
    DroppedCopy,
}

/// A post-consensus validation decision as saved, so a replay of the commit
/// after a restart reaches the same decision.
///
/// The error of a drop is kept as BCS bytes and decoded on its own, so a
/// binary that encodes `IotaError` differently still reads the decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SavedPostConsensusDecision {
    pub(crate) decision: PostConsensusDecision,
    /// The BCS bytes of the drop's error; empty for a keep.
    pub(crate) error: Vec<u8>,
}

impl SavedPostConsensusDecision {
    pub(crate) fn kept() -> Self {
        Self {
            decision: PostConsensusDecision::Kept,
            error: Vec::new(),
        }
    }

    pub(crate) fn dropped(decision: PostConsensusDecision, error: &IotaError) -> Self {
        Self {
            decision,
            error: bcs::to_bytes(error).expect("an IotaError should encode to BCS"),
        }
    }

    /// The error of a saved drop, or `IotaError::Unknown` if this binary
    /// cannot decode it.
    pub(crate) fn error(&self) -> IotaError {
        bcs::from_bytes(&self.error).unwrap_or_else(|e| {
            IotaError::Unknown(format!(
                "the saved error of a post-consensus drop cannot be decoded: {e}"
            ))
        })
    }
}

/// Validates `UserTransactionV1` transactions and resolves owned-object
/// conflicts in a single pass.
///
/// For each `UserTransactionV1` in consensus order:
/// - Runs deduplication, structural validity, lock conflict check, and deny checks (deny list, gas,
///   ownership, coin deny list, Move authenticator).
/// - If all checks pass, acquires owned-object locks in a local tracking map; immutable inputs are
///   not locked, as an immutable input never takes a new version and its lock would hold until the
///   epoch ends.
/// - Drops the transaction (with an error) on any failure.
/// - An already-executed transaction is **retained** (not dropped): it registers the locks its
///   effects report and skips re-validation. Dropping it would make this validator's checkpoints
///   differ from those of the other validators.
/// - A transaction in `previously_deferred` is kept without checks and takes no new locks: the
///   locks it took at the commit that kept it stay in place.
///
/// Copies of one digest with other signatures can come from different authors in one commit. The
/// first copy is validated; a later one only while every earlier copy was dropped for a reason that
/// may come from its own signatures. The digest's decision is that of the last copy validated, and
/// only a kept copy stays in `transactions`.
///
/// The decision for every candidate is saved before returning,
/// including keeps that the caller then defers, and a commit replayed after a restart repeats the
/// saved decisions; step 3 of the module docs says which transactions are validated again. Panics
/// if the saved decisions are incomplete, the candidates differ from the saved ones, or a saved
/// keep is now dropped. Once the switch to executing synced checkpoints after commit `L` is saved,
/// also panics for a commit above `L`, or for a commit at or below `L` without saved decisions.
///
/// Non-`UserTransactionV1` transactions pass through unchanged.
///
/// # Arguments
///
/// * `authority_state` — Used for cache reads and deny checks.
/// * `epoch_store` — Current epoch store (protocol config, lock storage, governance deny rules).
/// * `commit_index` — The consensus commit being validated. Inputs are read as of this commit.
/// * `previously_deferred` — The transactions deferred at earlier commits and reloaded into
///   `transactions`, randomness-using ones included.
/// * `transactions` — All sequenced transactions for this consensus commit; modified in-place.
///
/// # Returns
///
/// `Ok((dropped, locks, all_user_tx_digests))` where:
/// - `dropped` — `(digest, error, decision)` for every semantically-invalid or lock-conflicting
///   transaction, where `decision` is never `Kept`. Silent drops (duplicates) are **not** included.
/// - `locks` — Owned-object locks acquired in this commit, to be stored in the consensus quarantine
///   so subsequent commits can see them.
/// - `all_user_tx_digests` — Every `UserTransactionV1` digest that passed dedup (both kept and
///   dropped), once each. Used by the caller to release pre-consensus soft locks.
pub async fn validate_and_resolve_conflicts(
    authority_state: &AuthorityState,
    epoch_store: &Arc<AuthorityPerEpochStore>,
    commit_index: CommitIndex,
    previously_deferred: &PreviouslyDeferredTransactions,
    transactions: &mut Vec<VerifiedSequencedConsensusTransaction>,
) -> IotaResult<(
    Vec<(TransactionDigest, IotaError, PostConsensusDecision)>,
    HashMap<ObjectReference, LockDetails>,
    Vec<TransactionDigest>,
)> {
    // Keys of the transactions other than user transactions seen so far.
    let mut seen_keys: HashSet<SequencedConsensusTransactionKey> = HashSet::new();
    // The signatures of each copy of a digest validated at this commit, in
    // consensus order.
    let mut validated_copies: HashMap<TransactionDigest, Vec<&[UserSignature]>> = HashMap::new();
    let mut decisions = CommitDecisions::default();
    // Every validated copy's digest, in consensus order: a digest appears once
    // per copy validated.
    let mut candidates = Vec::with_capacity(transactions.len());
    // Locks acquired within this commit. Populated for every transaction that
    // passes all checks; used by subsequent transactions' conflict checks.
    let mut current_commit_locks: HashMap<ObjectReference, LockDetails> = HashMap::new();
    // Index-parallel keep flags: true = keep, false = remove.
    let mut keep = vec![true; transactions.len()];
    // All UserTransactionV1 digests seen in this commit (both kept and dropped),
    // used by the caller to release pre-consensus soft locks.
    let mut all_user_tx_digests = Vec::with_capacity(transactions.len());

    // One deny-rule snapshot for the whole commit, so every transaction in it
    // is judged by the same set. This must be the consensus-derived set — local
    // config can differ between validators and would fork the post-consensus
    // decisions.
    let governance_deny_rules = epoch_store.get_active_transaction_deny_rules();
    let deny_config: &dyn DenyRuleConfig = governance_deny_rules.as_ref();

    // The decisions saved when this commit was first validated, present only
    // when it is replayed after a restart. A saved candidate list means every
    // candidate has a saved decision.
    let saved_candidates = epoch_store
        .tables()?
        .post_consensus_decision_candidates
        .get(&commit_index)?;
    // A node that switched to executing synced checkpoints after commit
    // `last` replays only the commits up to it after a restart. Synced
    // checkpoints have since written outputs without handler rows, which the
    // reader cannot place, so these commits repeat their saved decisions.
    let replaying_up_to_the_switch =
        match epoch_store.last_commit_before_synced_checkpoint_execution()? {
            Some(last) if commit_index > last => fatal!(
                "commit {commit_index} validated after the switch to synced checkpoints \
                 after commit {last}"
            ),
            Some(last) if saved_candidates.is_none() => fatal!(
                "commit {commit_index} has no saved decisions although the switch to synced \
                 checkpoints after commit {last} saved every decision up to it"
            ),
            Some(_) => true,
            None => false,
        };
    // How many copies of each digest the first run validated. The decision
    // of the last one is the digest's; the ones before it were dropped for
    // their own signatures.
    let mut saved_copies: HashMap<TransactionDigest, usize> = HashMap::new();
    for digest in saved_candidates.iter().flatten() {
        *saved_copies.entry(*digest).or_default() += 1;
    }
    let mut saved_decisions = HashMap::new();

    // One reader for the whole commit, so every transaction in it is read as
    // of the same commit index.
    let reader = CommitIndexedReader::new(
        authority_state.get_object_cache_reader().clone(),
        epoch_store.clone(),
        commit_index,
    );

    for (i, tx) in transactions.iter().enumerate() {
        // Only validate UserTransactionV1; pass everything else through
        // unchanged, once per key.
        let Some(transaction) = (match &tx.0.transaction {
            SequencedConsensusTransactionKind::External(ext) => ext.kind.as_user_transaction(),
            SequencedConsensusTransactionKind::System(_) => None,
        }) else {
            if !seen_keys.insert(tx.0.key()) {
                keep[i] = false;
            }
            continue;
        };

        let digest = *transaction.digest();
        let signatures = transaction.data().signatures();

        // Check #0: Dedup by digest.
        // The same UserTransactionV1 may appear in DAG blocks from multiple
        // validators within the same consensus commit. Silent drop — not added
        // to `dropped`. A copy with other signatures is validated while no
        // earlier copy has decided the digest. A copy with the same signatures
        // as one already validated would get the same decision.
        let copies = validated_copies.entry(digest).or_default();
        if copies.is_empty() {
            all_user_tx_digests.push(digest);
        } else if decisions.is_decided(&digest) || copies.contains(&signatures) {
            keep[i] = false;
            continue;
        }
        copies.push(signatures);
        let copy_number = copies.len();
        candidates.push(digest);

        let saved = match &saved_candidates {
            Some(_) => {
                if copy_number < saved_copies.get(&digest).copied().unwrap_or_default() {
                    keep[i] = false;
                    continue;
                }
                match epoch_store.post_consensus_decision(commit_index, digest)? {
                    Some(saved) => Some(saved),
                    None => fatal!(
                        "commit {commit_index} has a saved candidate list but no saved \
                         validation decision for {digest:?}"
                    ),
                }
            }
            None => None,
        };
        let saved_decision = saved.as_ref().map(|saved| saved.decision);
        if let Some(decision) = saved_decision {
            saved_decisions.insert(digest, decision);
        }
        // A saved drop is replayed as is: the inputs it was decided on may
        // have changed since.
        if let Some(saved) = saved.filter(|saved| saved.decision != PostConsensusDecision::Kept) {
            decisions.decide(digest, saved.decision, Some(saved.error()));
            keep[i] = false;
            continue;
        }
        // Its locks are not registered: nothing in this epoch reads them, as
        // every transaction up to the switch repeats its saved decision and no
        // commit after it is validated.
        if replaying_up_to_the_switch && saved_decision == Some(PostConsensusDecision::Kept) {
            debug!(
                ?digest,
                "Repeating the saved keep up to the synced checkpoint switch"
            );
            decisions.decide(digest, PostConsensusDecision::Kept, None);
            continue;
        }
        // Validating it again could drop it, and the locks it took at the
        // commit that kept it would then stay until the epoch ends. Later
        // contenders find those locks in the quarantine or the lock table.
        if previously_deferred.contains_key(&digest) {
            debug!(
                ?digest,
                "Keeping a transaction deferred at an earlier commit without validating it again"
            );
            decisions.decide(digest, PostConsensusDecision::Kept, None);
            continue;
        }
        // Whether a drop of this copy may come from its own signatures, so a
        // later copy of the digest is validated.
        let from_signatures = |check: FailedCheck<'_>| {
            drop_may_come_from_signatures(transaction.data(), check, epoch_store.protocol_config())
        };

        // Check #1: Already executed (typically by state-sync before this node's
        // consensus handler reached the commit). It is a committee-agreed winner, so
        // keep it in the sequence to flow into checkpoint roots like on every other
        // validator (dropping it would make this validator's checkpoint differ from
        // those of the others). Register its owned-object locks so double-spend
        // siblings still lose, then skip re-validation (#2/#5);
        // the active scheduler's enqueue filter suppresses the re-execution.
        //
        // Effects alone do not show at which commit the transaction was kept,
        // so only a saved keep at this commit skips validation; its consumed
        // inputs may be pruned by now. Without one, the commit-indexed reader
        // decides, as on every other validator.
        if saved_decision == Some(PostConsensusDecision::Kept)
            && authority_state
                .get_transaction_cache_reader()
                .try_is_tx_already_executed(&digest)?
        {
            // Byte-based, so safe even though the inputs are already consumed.
            let owned_inputs = filter_locked_inputs_by_effects(
                authority_state,
                &digest,
                extract_owned_input_objects(tx)?,
            )?;
            for obj_ref in &owned_inputs {
                // A winner cannot be out-locked: after the effects filter,
                // every reference here was consumed by this transaction, so a
                // lock held by a different tx is a consistency violation, not
                // a conflict. (Flag off, immutable references, which are not
                // consumed, stay in the set and weaken this invariant.)
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
            decisions.decide(digest, PostConsensusDecision::Kept, None);
            continue;
        }

        // Check #2: Structural validity.
        if let Err(e) = transaction.validity_check(&epoch_store.tx_validity_check_context()) {
            warn!(
                ?digest,
                commit_index,
                error = ?e,
                "UserTransactionV1 failed validity_check post-consensus, dropping"
            );
            let from_signatures = from_signatures(FailedCheck::Validity(&e));
            decisions.drop_copy(digest, e, false, from_signatures);
            keep[i] = false;
            continue;
        }

        // Check #3: Extract owned input objects for lock conflict detection.
        let owned_inputs = match extract_owned_input_objects(tx) {
            Ok(inputs) => inputs,
            Err(e) => {
                warn!(
                    ?digest,
                    commit_index,
                    error = ?e,
                    "Failed to extract owned input objects post-consensus, dropping"
                );
                decisions.drop_copy(digest, e, false, from_signatures(FailedCheck::OwnedInputs));
                keep[i] = false;
                continue;
            }
        };

        // Check #4: Three-tier lock conflict check.
        // Cheap (HashMap + quarantine + DB lookups); performed before the
        // expensive deny checks so conflicting transactions are filtered first.
        //
        // Locks are keyed by full ObjectReference (id + version + digest), not just
        // ObjectID. Two transactions referencing the same object at different
        // versions will NOT conflict here — version freshness is validated
        // later in Check #5 (deny checks load objects from DB and verify
        // that the transaction's input refs match the current state).
        //
        // Probing the raw set is safe though immutable inputs are never
        // locked: acquisition never inserts one, and the lock tables are
        // epoch-scoped while the protocol config is fixed per epoch, so no
        // tier can hold a stale immutable lock.
        //
        // Tier 1: Local HashMap (current commit).
        // Tier 2: Consensus quarantine (previous uncommitted commits).
        // Tier 3: Persistent DB (committed data).
        let mut conflict: Option<(ObjectReference, IotaError)> = None;
        for obj_ref in &owned_inputs {
            if let Some(locked_by) =
                find_existing_lock(obj_ref, &current_commit_locks, epoch_store)?
            {
                // A lock held by this same transaction is its own lock from an
                // earlier commit, not a conflict; Check #1 accepts one too.
                // Same-commit duplicates cannot slip through here, since Check #0
                // lets a later copy through only while no copy of the digest is
                // kept.
                if locked_by == digest {
                    continue;
                }

                debug!(
                    ?digest,
                    ?obj_ref,
                    ?locked_by,
                    "Transaction conflicts with existing owned-object lock, dropping"
                );
                conflict = Some((
                    *obj_ref,
                    IotaError::ObjectLockConflict {
                        obj_ref: *obj_ref,
                        pending_transaction: locked_by,
                    },
                ));
                break;
            }
        }
        if let Some((obj_ref, e)) = conflict {
            decisions.drop_copy(
                digest,
                e,
                false,
                from_signatures(FailedCheck::LockConflict(&obj_ref)),
            );
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
        let verified_tx = VerifiedTransaction::new_from_verified((*transaction).clone());
        // The input the reader dropped or did not see, and whether it did
        // not see it.
        let mut input_verdict: Option<(InputObjectKind, bool)> = None;
        let mut denied_for_every_copy = false;
        // Inputs read as of the commit. A missing input is reported with
        // the error admission gives a missing input, which the transaction
        // and quorum drivers retry: it drops the transaction from this
        // commit only, the decision is saved as `Missing`, and a
        // resubmission can be kept at a later commit. A dropped input is
        // final for the digest, so it has its own error, which the drivers
        // do not retry (a drop that may come from the copy's signatures is
        // not sent to the waiters). The reason goes to the log. The Move
        // authenticator runs at execution instead. Here only whether each
        // account is one is checked, and its authenticator function field
        // is read for the deny check of its package, both as of the
        // horizon. The coin deny list is read as of the horizon too.
        let checked = authority_state
            .handle_transaction_validation_checks_at_commit(
                &reader,
                &verified_tx,
                epoch_store,
                deny_config,
                VerifierLimitsSource::ProtocolConfig,
            )
            .and_then(|verdict| {
                let verdicts = &authority_state
                    .metrics
                    .consensus_handler_validation_reader_verdicts;
                match verdict {
                    ValidationAtCommit::Keep(owned) => {
                        verdicts.with_label_values(&["keep", "none"]).inc();
                        Ok(owned)
                    }
                    ValidationAtCommit::Drop(kind, reason) => {
                        verdicts
                            .with_label_values(&["drop", &format!("{:?}", reason.kind())])
                            .inc();
                        warn!(
                            ?digest,
                            commit_index,
                            ?kind,
                            ?reason,
                            "input dropped by the commit-indexed reader"
                        );
                        input_verdict = Some((kind, false));
                        Err(kind.not_available_at_commit_error().into())
                    }
                    ValidationAtCommit::Missing(kind, reason) => {
                        verdicts
                            .with_label_values(&["missing", &format!("{:?}", reason.kind())])
                            .inc();
                        warn!(
                            ?digest,
                            commit_index,
                            ?kind,
                            ?reason,
                            "input not visible at this commit"
                        );
                        input_verdict = Some((kind, true));
                        Err(kind.object_not_found_error().into())
                    }
                    ValidationAtCommit::AuthenticatorPackageDenied(error) => {
                        denied_for_every_copy = true;
                        Err(error)
                    }
                }
            });
        let validated_owned_objects = match checked {
            Ok(owned) => owned,
            Err(e) => {
                if e.is_storage_or_epoch_error() {
                    return Err(e);
                }
                warn!(
                    ?digest,
                    commit_index,
                    error = ?e,
                    "UserTransactionV1 failed post-consensus deny checks, dropping"
                );
                let (missing, check) = match &input_verdict {
                    Some((kind, missing)) => (*missing, FailedCheck::Input(kind)),
                    None if denied_for_every_copy => (false, FailedCheck::Data),
                    None => (false, FailedCheck::Other),
                };
                let from_signatures = from_signatures(check);
                decisions.drop_copy(digest, e, missing, from_signatures);
                keep[i] = false;
                continue;
            }
        };

        // All checks passed — acquire owned-object locks in local tracking.
        // The validated set comes from loaded objects, so unlike the
        // byte-derived `owned_inputs` it excludes immutable inputs, which must
        // never be locked (see the module docs). It is a subset of `owned_inputs`,
        // so every reference locked here was probed by Check #4 first.
        for obj_ref in &validated_owned_objects {
            current_commit_locks.insert(*obj_ref, digest);
        }
        // Log the acquired refs, not just their count, so the winner's locks
        // are attributable per (object_id, version).
        debug!(
            ?digest,
            num_owned_inputs = validated_owned_objects.len(),
            owned_inputs = ?validated_owned_objects,
            "Transaction passed post-consensus validation, acquired all object locks"
        );
        decisions.decide(digest, PostConsensusDecision::Kept, None);
    }

    let dropped: Vec<(TransactionDigest, IotaError, PostConsensusDecision)> = all_user_tx_digests
        .iter()
        .filter_map(|digest| match decisions.verdicts.get(digest) {
            Some((decision, Some(error))) => Some((*digest, error.clone(), *decision)),
            _ => None,
        })
        .collect();

    match &saved_candidates {
        Some(saved_candidates) => {
            if *saved_candidates != candidates {
                fatal!(
                    "validation candidates of commit {commit_index} differ from the saved \
                     ones: saved {saved_candidates:?}, now {candidates:?}"
                );
            }
            for digest in &all_user_tx_digests {
                let decision = decisions.verdicts.get(digest).map(|(decision, _)| decision);
                let saved = saved_decisions.get(digest);
                if saved != decision {
                    fatal!(
                        "validation decision for {digest:?} at commit {commit_index} \
                         differs from the saved one: saved {saved:?}, now {decision:?}"
                    );
                }
            }
        }
        None => {
            // Every copy's entry carries the digest's decision.
            let saved: Vec<_> = candidates
                .iter()
                .map(|digest| {
                    let saved = match decisions
                        .verdicts
                        .get(digest)
                        .expect("every validated copy decides its digest")
                    {
                        (decision, Some(error)) => {
                            SavedPostConsensusDecision::dropped(*decision, error)
                        }
                        (_, None) => SavedPostConsensusDecision::kept(),
                    };
                    (*digest, saved)
                })
                .collect();
            epoch_store.persist_post_consensus_decisions(commit_index, &saved)?;
            if !saved.is_empty() {
                fail_point!("crash-after-post-consensus-decisions");
            }
        }
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

/// The decision so far for each user transaction digest of a commit.
#[derive(Default)]
struct CommitDecisions {
    /// The decision of the last copy of each digest validated, with the error
    /// of a drop.
    verdicts: HashMap<TransactionDigest, (PostConsensusDecision, Option<IotaError>)>,
    /// The digests no later copy can change the decision of: a copy was
    /// kept, or dropped for a reason every copy shares.
    decided: HashSet<TransactionDigest>,
}

impl CommitDecisions {
    fn is_decided(&self, digest: &TransactionDigest) -> bool {
        self.decided.contains(digest)
    }

    /// Records the decision of `digest`, with the error of a drop.
    fn decide(
        &mut self,
        digest: TransactionDigest,
        decision: PostConsensusDecision,
        error: Option<IotaError>,
    ) {
        self.verdicts.insert(digest, (decision, error));
        self.decided.insert(digest);
    }

    /// Records the drop of one copy of `digest` with `error`. `missing` is
    /// whether an input was not visible at the horizon. A drop that may come
    /// from the copy's own signatures leaves the digest open for a later copy;
    /// it is saved as [`PostConsensusDecision::DroppedCopy`] unless an input
    /// was not visible, which is not final either.
    fn drop_copy(
        &mut self,
        digest: TransactionDigest,
        error: IotaError,
        missing: bool,
        from_signatures: bool,
    ) {
        if from_signatures && missing {
            self.verdicts.insert(
                digest,
                (
                    PostConsensusDecision::DroppedInputNotVisibleYet,
                    Some(error),
                ),
            );
        } else if from_signatures {
            self.verdicts
                .insert(digest, (PostConsensusDecision::DroppedCopy, Some(error)));
        } else if missing {
            self.decide(
                digest,
                PostConsensusDecision::DroppedInputNotVisibleYet,
                Some(error),
            );
        } else {
            self.decide(digest, PostConsensusDecision::Dropped, Some(error));
        }
    }
}

/// The check a copy of a transaction failed, for
/// [`drop_may_come_from_signatures`].
enum FailedCheck<'a> {
    /// `validity_check`, with its error.
    Validity(&'a IotaError),
    /// Listing the owned inputs.
    OwnedInputs,
    /// Another transaction holds a lock on this input.
    LockConflict(&'a ObjectReference),
    /// The commit-indexed reader dropped this input or did not see it.
    Input(&'a InputObjectKind),
    /// A check whose outcome is the same for every copy.
    Data,
    /// Any other check of the inputs, deny rules and gas.
    Other,
}

/// Whether a drop of this copy of a transaction may come from its signatures
/// rather than from the transaction data, which every copy of the digest
/// shares.
///
/// Block verification checks every signature against the data, except a Move
/// authenticator, which it checks only for the address it claims. So a copy
/// without one fails like every other copy, while a faulty validator can put
/// in its block a copy with a made-up authenticator that fails here. For a
/// copy with a Move authenticator, a drop counts as the data's only when the
/// data alone explains it: the data fails its own validity check or has
/// expired, its inputs cannot be listed, or the conflicting lock or the
/// dropped or unseen input is one the data names, or the package holding a
/// signer account's authenticate function is denied. Any other drop of such
/// a copy, including the other checks of the inputs, deny rules and gas, may
/// come from the authenticator.
fn drop_may_come_from_signatures(
    transaction: &SenderSignedTransaction,
    check: FailedCheck<'_>,
    config: &ProtocolConfig,
) -> bool {
    if transaction.move_authenticators().is_empty() {
        return false;
    }
    let data = transaction.transaction();
    let data_names = |kind: &InputObjectKind| {
        data.input_objects()
            .is_ok_and(|inputs| inputs.contains(kind))
    };
    match check {
        FailedCheck::Validity(error) => {
            !matches!(error, IotaError::TransactionExpired)
                && !data.is_system_tx()
                && data.validity_check(config).is_ok()
        }
        FailedCheck::OwnedInputs => data.input_objects().is_ok(),
        FailedCheck::LockConflict(obj_ref) => {
            !data_names(&InputObjectKind::ImmOrOwnedMoveObject(*obj_ref))
        }
        FailedCheck::Input(kind) => {
            !data_names(kind)
                && !matches!(
                    kind,
                    InputObjectKind::ImmOrOwnedMoveObject(obj_ref)
                        if data.receiving_objects().contains(obj_ref)
                )
        }
        FailedCheck::Data => false,
        FailedCheck::Other => true,
    }
}

/// Narrows the byte-derived owned inputs of an already-executed transaction to
/// the references it consumed — the same set the main path locks — by
/// intersecting them with the pre-execution references in its own effects;
/// immutable inputs never appear there. A missing effects body
/// is a storage error, never a "not executed" verdict, which would drop a
/// committee-agreed winner other validators keep.
fn filter_locked_inputs_by_effects(
    authority_state: &AuthorityState,
    digest: &TransactionDigest,
    owned_inputs: Vec<ObjectReference>,
) -> IotaResult<Vec<ObjectReference>> {
    let effects = authority_state
        .get_transaction_cache_reader()
        .try_get_executed_effects(digest)?
        .ok_or_else(|| IotaError::GenericAuthority {
            error: format!(
                "effects of already-executed transaction {digest:?} are unavailable in \
                 post-consensus validation"
            ),
        })?;
    // Owned inputs always appear in effects and immutable ones never do
    // (`TemporaryStore::ensure_active_inputs_mutated` force-mutates every
    // non-immutable owned input); intersecting on the full `ObjectReference`
    // — the lock table's own key — keeps the sets identical by construction.
    let consumed: HashSet<ObjectReference> = effects
        .old_object_metadata()
        .into_iter()
        .map(|old| *old.reference())
        .collect();
    Ok(owned_inputs
        .into_iter()
        .filter(|obj_ref| consumed.contains(obj_ref))
        .collect())
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
    obj_ref: &ObjectReference,
    current_commit_locks: &HashMap<ObjectReference, LockDetails>,
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

/// Extracts the `ImmOrOwnedMoveObject` input references of a
/// `UserTransactionV1` consensus transaction (excludes shared objects and
/// packages).
///
/// This is the set probed for lock conflicts, not the set that gets locked.
/// It is derived from the transaction bytes alone, where owned and immutable
/// inputs share one kind, so it cannot tell them apart; the callers that
/// acquire locks narrow it down first. Probing immutable references is
/// harmless, since no tier ever holds a lock on one.
fn extract_owned_input_objects(
    tx: &VerifiedSequencedConsensusTransaction,
) -> IotaResult<Vec<ObjectReference>> {
    let Some(transaction_data) = (match &tx.0.transaction {
        SequencedConsensusTransactionKind::External(ext) => {
            ext.kind.as_user_transaction().map(|t| t.data())
        }
        SequencedConsensusTransactionKind::System(_) => None,
    }) else {
        return Err(IotaError::GenericAuthority {
            error: "Expected UserTransactionV1 in extract_owned_input_objects".to_string(),
        });
    };

    // Use SenderSignedTransaction::input_objects() rather than
    // Transaction::input_objects() to also include objects coming from
    // MoveAuthenticator signatures. Those can only be immutable or shared,
    // so they widen the probe set but never the locked set.
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
