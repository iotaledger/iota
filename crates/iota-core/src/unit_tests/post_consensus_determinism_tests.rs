// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Property-based check of P-COOL deterministic validation against the
//! oracle in `post_consensus_determinism_oracle.rs`.
//!
//! A random consensus history (objects, commits, candidates) is judged by
//! the oracle once. Then one validator with a random local schedule -
//! execution order among the commits its handler assigned, watcher
//! completion, the wait on the execution frontier, checkpoint bookkeeping,
//! commit flushes, pruning, the switch to executing synced checkpoints at the
//! end of the epoch, crashes at every durable write - runs the real bookkeeping
//! ([`HandlerObjectState`] through the epoch store), the real reader
//! ([`CommitIndexedReader`] and its input machines) and the real saved
//! validation decisions over a RocksDB epoch store, with a fake object store
//! standing in for the writeback cache. Every verdict, every input read, the
//! lock set of every commit and the bookkeeping invariant are compared with
//! the oracle after every step.
//!
//! The executor runs a transaction only once the handler assigned its commit,
//! until the switch; from then on it runs every commit, with no handler rows,
//! and the handler validates no further commit. As on the node, pruning
//! deletes no consumed version a validation above the resume point can still
//! keep, and a node that crashes after the switch replays the commits up to
//! the switch on restart and switches again at the next one; the scenario
//! `restart_after_the_switch_replays_only_up_to_the_switch` covers the
//! restart.
//!
//! What is real: the bookkeeping state and its durable tables, the reader
//! machines, the horizon distance from the protocol config, the saved
//! decisions and their retirement, the owned-object lock table, the
//! processed-message table and the resume point. What is modeled: the object
//! store (in memory, with dirty and durable tiers, tombstones, deletion
//! markers and pruning), the consensus quarantine (locks and processed keys
//! of unflushed commits, and its lock against the checkpoint bookkeeping and
//! the watcher), the consensus handler's duplicate filter and the
//! transaction loop of `validate_and_resolve_conflicts`, whose per candidate
//! rules are transcribed here over the real storage tiers, and execution
//! itself, whose effects come from the oracle's transitions.
//!
//! The random property runs `PROPTEST_CASES` cases (64 when unset) from the
//! seed in `PROPTEST_RNG_SEED` (1 when unset), and prints both. Running it
//! again with the same seed and case count replays a failure.
//!
//! [`HandlerObjectState`]: crate::authority::authority_per_epoch_store::handler_object_state::HandlerObjectState

use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fmt::Write as _,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use futures::{FutureExt, future::BoxFuture};
use iota_protocol_config::{OverrideGuard, ProtocolConfig};
use iota_sdk_types::{
    ExecutionError, ExecutionStatus, GasCostSummary, ObjectDigest, ObjectId, ObjectReference,
    Owner, TransactionDigest, TransactionEffects, Version,
    effects::{ChangedObject, IdOperation, ObjectIn, ObjectOut},
};
use iota_types::{
    base_types::EpochId,
    effects::{TransactionEffectsAPI, TransactionEffectsExt},
    error::{IotaError, IotaResult},
    execution::SharedInput,
    iota_system_state::IotaSystemState,
    messages_checkpoint::CheckpointSequenceNumber,
    messages_consensus::ConsensusTransactionKey,
    object::Object,
    storage::{InputKey, MarkerValue, ObjectKey, ObjectOrTombstone, PackageObject},
    transaction::{InputObjectKind, TransactionKey},
};
use proptest::{
    arbitrary::any,
    prop_oneof,
    strategy::{BoxedStrategy, Just, Strategy, Union},
    test_runner::{Config, RngSeed, TestCaseError, TestRunner},
};
use typed_store::Map;

use super::{
    DropKind, DropReason, KeptObject, OwnedVerdict, PackageVerdict, SharedVerdict, owned, package,
    reader::CommitIndexedReader, shared,
};
use crate::{
    authority::{
        AuthorityState,
        authority_per_epoch_store::{
            AuthorityPerEpochStore, ExecutionIndices, ExecutionIndicesWithStats,
            authority_per_epoch_store_tests::{clear_pcool_validation_tables, reopen},
            consensus_quarantine::ConsensusCommitOutput,
            handler_object_state::{CommitIndex, HandlerProcessedObject, handler_rows_for_commit},
        },
        authority_store::IotaLockResult,
        test_authority_builder::TestAuthorityBuilder,
    },
    consensus_handler::SequencedConsensusTransactionKey,
    execution_cache::ObjectCacheRead,
    post_consensus_validation::{PostConsensusDecision, SavedPostConsensusDecision},
};

#[path = "post_consensus_determinism_oracle.rs"]
mod oracle;

use oracle::{
    Candidate, ChangeKind, Execution, Input, InputClass, Oracle, PACKAGE_ID, SENDER, Slot, State,
    Verdict, Write, coin, package, wrong_digest,
};

type Outcome<T> = Result<T, TestCaseError>;

// ---------------------------------------------------------------------------
// History generation
// ---------------------------------------------------------------------------

/// Where a candidate's owned input reference is drawn from, resolved against
/// the oracle's state when the history is built.
#[derive(Clone, Copy, Debug)]
enum Source {
    /// The object's version in `S_{C-K}`.
    LiveAtHorizon,
    /// The object's version in `S_{C-1}`.
    LiveNow,
    /// A version produced at a commit in `(C-K, C-1]`, else `LiveNow`.
    AboveHorizon,
    /// A version consumed at a commit `< C`, else `LiveAtHorizon`.
    ConsumedEarlier,
    /// A version consumed at a commit in `(C-K, C)`, by a lock or a receive,
    /// else `LiveAtHorizon`.
    ConsumedAboveHorizon,
    /// The retained version the object had before the epoch, else
    /// `UnknownId`.
    PreEpochStale,
    /// A version above anything the object has.
    NeverExisting,
    /// An id no state has ever held.
    UnknownId,
    /// `LiveAtHorizon` with another digest.
    WrongDigest,
}

#[derive(Clone, Copy, Debug)]
struct InputSpec {
    object: u8,
    source: Source,
}

#[derive(Clone, Copy, Debug)]
enum ObjectWriteSpec {
    Mutate,
    Delete,
    Wrap,
}

#[derive(Clone, Copy, Debug)]
enum ExtraSpec {
    None,
    CreateOwned,
    CreateShared,
    Unwrap,
    Publish,
    DeleteShared,
}

#[derive(Clone, Copy, Debug)]
enum SharedDeclared {
    Correct,
    Wrong,
}

/// A shared input: which shared object of the history, by index over every
/// shared id known when the candidate is built (pre-epoch, created, deleted),
/// and whether the declared initial shared version is right.
#[derive(Clone, Copy, Debug)]
struct SharedSpec {
    object: u8,
    declared: SharedDeclared,
    read_only: bool,
}

#[derive(Clone, Debug)]
struct FreshSpec {
    gas: InputSpec,
    object: Option<InputSpec>,
    shared: Vec<SharedSpec>,
    package: bool,
    /// A receive, over the receivable objects only, so that a gas coin is
    /// never received.
    receive: Option<InputSpec>,
    object_write: ObjectWriteSpec,
    extra: ExtraSpec,
    defer: bool,
    fails: bool,
}

#[derive(Clone, Debug)]
enum CandidateSpec {
    Fresh(FreshSpec),
    /// A network copy of an earlier candidate, by index over every
    /// candidate of earlier commits.
    Retry(u8),
    /// A copy of an earlier candidate of the same commit.
    SameCommitDuplicate(u8),
}

#[derive(Clone, Debug)]
struct HistorySpec {
    k: u8,
    owned: u8,
    /// Owned objects (by bit) whose previous version is still in the store.
    stale_mask: u8,
    /// Owned objects (by bit) deleted before the epoch, their tombstone
    /// latest and the previous version retained.
    deleted_mask: u8,
    /// Shared objects that exist at the epoch start.
    shared_pre_epoch: u8,
    package_pre_epoch: bool,
    /// Owned objects only receives consume, and only the second input of a
    /// candidate names, never its gas coin.
    receivable: u8,
    /// Receivable objects (by bit) whose previous version is still in the
    /// store.
    receivable_stale_mask: u8,
    /// Receivable objects (by bit) deleted before the epoch, their tombstone
    /// latest and the previous version retained.
    receivable_deleted_mask: u8,
    commits: Vec<Vec<CandidateSpec>>,
}

fn source() -> impl Strategy<Value = Source> {
    prop_oneof![
        5 => Just(Source::LiveAtHorizon),
        2 => Just(Source::LiveNow),
        2 => Just(Source::AboveHorizon),
        2 => Just(Source::ConsumedEarlier),
        2 => Just(Source::ConsumedAboveHorizon),
        1 => Just(Source::PreEpochStale),
        1 => Just(Source::NeverExisting),
        1 => Just(Source::UnknownId),
        1 => Just(Source::WrongDigest),
    ]
}

fn input_spec() -> impl Strategy<Value = InputSpec> {
    (any::<u8>(), source()).prop_map(|(object, source)| InputSpec { object, source })
}

fn shared_spec() -> impl Strategy<Value = SharedSpec> {
    (
        any::<u8>(),
        prop_oneof![3 => Just(SharedDeclared::Correct), 1 => Just(SharedDeclared::Wrong)],
        proptest::bool::weighted(0.3),
    )
        .prop_map(|(object, declared, read_only)| SharedSpec {
            object,
            declared,
            read_only,
        })
}

fn fresh_spec() -> impl Strategy<Value = FreshSpec> {
    (
        input_spec(),
        proptest::option::weighted(0.7, input_spec()),
        proptest::option::weighted(0.4, proptest::collection::vec(shared_spec(), 1..=2)),
        proptest::bool::weighted(0.2),
        proptest::option::weighted(0.35, input_spec()),
        prop_oneof![
            6 => Just(ObjectWriteSpec::Mutate),
            1 => Just(ObjectWriteSpec::Delete),
            1 => Just(ObjectWriteSpec::Wrap),
        ],
        prop_oneof![
            8 => Just(ExtraSpec::None),
            1 => Just(ExtraSpec::CreateOwned),
            1 => Just(ExtraSpec::CreateShared),
            1 => Just(ExtraSpec::Unwrap),
            1 => Just(ExtraSpec::Publish),
            2 => Just(ExtraSpec::DeleteShared),
        ],
        proptest::bool::weighted(0.15),
        proptest::bool::weighted(0.1),
    )
        .prop_map(
            |(gas, object, shared, package, receive, object_write, extra, defer, fails)| {
                FreshSpec {
                    gas,
                    object,
                    shared: shared.unwrap_or_default(),
                    package,
                    receive,
                    object_write,
                    extra,
                    defer,
                    fails,
                }
            },
        )
}

fn candidate_spec() -> impl Strategy<Value = CandidateSpec> {
    prop_oneof![
        8 => fresh_spec().prop_map(CandidateSpec::Fresh),
        1 => any::<u8>().prop_map(CandidateSpec::Retry),
        1 => any::<u8>().prop_map(CandidateSpec::SameCommitDuplicate),
    ]
}

fn history_spec() -> impl Strategy<Value = HistorySpec> {
    (
        1..=3u8,
        2..=4u8,
        any::<u8>(),
        prop_oneof![3 => Just(0u8), 1 => 1..=3u8],
        0..=2u8,
        any::<bool>(),
        prop_oneof![1 => Just(0u8), 2 => 1..=2u8],
        any::<u8>(),
        prop_oneof![3 => Just(0u8), 1 => 1..=3u8],
        proptest::collection::vec(proptest::collection::vec(candidate_spec(), 1..=4), 3..=6),
    )
        .prop_map(
            |(
                k,
                owned,
                stale_mask,
                deleted_mask,
                shared_pre_epoch,
                package_pre_epoch,
                receivable,
                receivable_stale_mask,
                receivable_deleted_mask,
                commits,
            )| {
                HistorySpec {
                    k,
                    owned,
                    stale_mask,
                    deleted_mask,
                    shared_pre_epoch,
                    package_pre_epoch,
                    receivable,
                    receivable_stale_mask,
                    receivable_deleted_mask,
                    commits,
                }
            },
        )
}

fn object_id(tag: u8, index: u8) -> ObjectId {
    let mut bytes = [0u8; 32];
    bytes[0] = tag;
    bytes[1] = index;
    ObjectId::new(bytes)
}

fn transaction_digest(commit: CommitIndex, index: usize) -> TransactionDigest {
    let mut bytes = [0u8; 32];
    bytes[0] = 0x7C;
    bytes[1..9].copy_from_slice(&commit.to_be_bytes());
    bytes[9..17].copy_from_slice(&(index as u64).to_be_bytes());
    TransactionDigest::new(bytes)
}

const EPOCH_START_VERSION: u64 = 3;
const RETAINED_VERSION: u64 = 2;

/// A shared object no history ever creates, named when a candidate asks for
/// a shared input before any shared object exists.
fn unknown_shared_id() -> ObjectId {
    object_id(0xA5, 0xFF)
}

/// A consensus history the oracle has judged, ready to be replayed on a
/// validator.
#[derive(Clone, Debug)]
struct History {
    epoch_start: State,
    /// Versions the store holds from before the epoch beyond the latest of
    /// each object, superseded but not pruned.
    retained_pre_epoch: Vec<Object>,
    commits: Vec<Vec<Candidate>>,
    oracle: Oracle,
}

impl History {
    /// Judges `commits` in order, extending them with the reload of every
    /// deferred candidate at the next commit.
    fn judge(
        k: CommitIndex,
        epoch_start: State,
        retained_pre_epoch: Vec<Object>,
        commits: Vec<Vec<Candidate>>,
    ) -> Self {
        let mut oracle = Oracle::new(epoch_start.clone(), k);
        let mut judged = Vec::new();
        let mut pending_reloads: Vec<Candidate> = Vec::new();
        let mut commits = commits.into_iter();
        loop {
            let next = commits.next();
            if next.is_none() && pending_reloads.is_empty() {
                break;
            }
            let mut candidates = std::mem::take(&mut pending_reloads);
            candidates.extend(next.into_iter().flatten());
            let outcome = oracle.judge_commit(&candidates);
            pending_reloads = reloads_of(&candidates, outcome);
            judged.push(candidates);
        }
        History {
            epoch_start,
            retained_pre_epoch,
            commits: judged,
            oracle,
        }
    }

    fn k(&self) -> CommitIndex {
        self.oracle.k
    }

    fn last_commit(&self) -> CommitIndex {
        self.commits.len() as CommitIndex
    }

    /// Every execution in consensus order, with its commit.
    fn executions(&self) -> impl Iterator<Item = (CommitIndex, &Execution)> {
        self.oracle
            .outcomes
            .iter()
            .enumerate()
            .flat_map(|(i, outcome)| {
                outcome
                    .executions
                    .iter()
                    .map(move |execution| (i as CommitIndex + 1, execution))
            })
    }

    fn execution(&self, digest: &TransactionDigest) -> Option<(CommitIndex, &Execution)> {
        self.executions()
            .find(|(_, execution)| execution.digest == *digest)
    }

    fn describe(&self) -> String {
        let mut out = format!("K = {}\nepoch start:\n", self.oracle.k);
        for (id, slot) in &self.epoch_start.slots {
            let _ = writeln!(out, "  {} = {}", short_id(id), describe_slot(slot));
        }
        for object in &self.retained_pre_epoch {
            let _ = writeln!(
                out,
                "  retained {}@{}",
                short_id(&object.id()),
                object.version().as_u64()
            );
        }
        for (i, candidates) in self.commits.iter().enumerate() {
            let commit = i as CommitIndex + 1;
            let outcome = self.oracle.outcome(commit);
            let _ = writeln!(out, "commit {commit}:");
            for candidate in candidates {
                let _ = writeln!(
                    out,
                    "  {} {:?} -> {:?}",
                    describe_candidate(candidate),
                    self.oracle.input_classes(commit, candidate),
                    outcome.verdict(&candidate.digest)
                );
            }
        }
        out
    }
}

/// The reloads of the kept candidates `candidates` deferred, which the next
/// commit starts with.
fn reloads_of(candidates: &[Candidate], outcome: &oracle::CommitOutcome) -> Vec<Candidate> {
    candidates
        .iter()
        .filter(|candidate| {
            candidate.defer && outcome.verdict(&candidate.digest) == Some(Verdict::Kept)
        })
        .map(|candidate| Candidate {
            defer: false,
            deferral_reload: true,
            ..candidate.clone()
        })
        .collect()
}

fn short_id(id: &ObjectId) -> String {
    let bytes: &[u8] = id.as_ref();
    format!("{:02x}{:02x}", bytes[0], bytes[1])
}

fn describe_slot(slot: &Slot) -> String {
    match slot {
        Slot::Live(object) => format!(
            "live@{} {}",
            object.version().as_u64(),
            match object.owner {
                Owner::Shared(initial) => format!("shared({})", initial.as_u64()),
                Owner::Immutable => "package".to_string(),
                _ => "owned".to_string(),
            }
        ),
        Slot::Deleted(version) => format!("deleted@{}", version.as_u64()),
        Slot::Wrapped(version) => format!("wrapped@{}", version.as_u64()),
    }
}

fn describe_candidate(candidate: &Candidate) -> String {
    let digest: &[u8] = candidate.digest.as_ref();
    let mut out = format!("tx{:02x}{:02x}", digest[8], digest[16]);
    for reference in &candidate.owned_inputs {
        let _ = write!(
            out,
            " {}@{}",
            short_id(&reference.object_id),
            reference.version.as_u64()
        );
    }
    for (id, initial) in &candidate.shared_inputs {
        let access = if candidate.read_only_shared.contains(id) {
            "read"
        } else {
            "shared"
        };
        let _ = write!(out, " {access}:{}#{}", short_id(id), initial.as_u64());
    }
    for id in &candidate.packages {
        let _ = write!(out, " pkg:{}", short_id(id));
    }
    for reference in &candidate.receiving {
        let _ = write!(
            out,
            " recv:{}@{}",
            short_id(&reference.object_id),
            reference.version.as_u64()
        );
    }
    if !candidate.writes.is_empty() {
        let _ = write!(out, " {:?}", candidate.writes);
    }
    if candidate.fails {
        out.push_str(" fails");
    }
    if candidate.defer {
        out.push_str(" defer");
    }
    if candidate.deferral_reload {
        out.push_str(" reload");
    }
    out
}

/// Resolves a history spec against the oracle, commit by commit, so every
/// input choice refers to versions the judged history has.
fn build_history(spec: &HistorySpec) -> History {
    let owned_ids: Vec<ObjectId> = (0..spec.owned).map(|i| object_id(0xA0, i)).collect();
    let mut epoch_start = State::default();
    let mut retained = Vec::new();
    for (i, id) in owned_ids.iter().enumerate() {
        let deleted = spec.deleted_mask & (1 << i) != 0;
        let stale = spec.stale_mask & (1 << i) != 0;
        let owner = Owner::Address(SENDER);
        if deleted {
            epoch_start
                .slots
                .insert(*id, Slot::Deleted(Version::from_u64(EPOCH_START_VERSION)));
        } else {
            epoch_start.slots.insert(
                *id,
                Slot::Live(coin(*id, Version::from_u64(EPOCH_START_VERSION), owner)),
            );
        }
        if deleted || stale {
            retained.push(coin(*id, Version::from_u64(RETAINED_VERSION), owner));
        }
    }
    let receivable_ids: Vec<ObjectId> = (0..spec.receivable).map(|i| object_id(0xA2, i)).collect();
    for (i, id) in receivable_ids.iter().enumerate() {
        let deleted = spec.receivable_deleted_mask & (1 << i) != 0;
        let stale = spec.receivable_stale_mask & (1 << i) != 0;
        let owner = Owner::Address(SENDER);
        if deleted {
            epoch_start
                .slots
                .insert(*id, Slot::Deleted(Version::from_u64(EPOCH_START_VERSION)));
        } else {
            epoch_start.slots.insert(
                *id,
                Slot::Live(coin(*id, Version::from_u64(EPOCH_START_VERSION), owner)),
            );
        }
        if deleted || stale {
            retained.push(coin(*id, Version::from_u64(RETAINED_VERSION), owner));
        }
    }
    let mut shared_ids = Vec::new();
    for index in 0..spec.shared_pre_epoch {
        let id = object_id(0xA5, index);
        epoch_start.slots.insert(
            id,
            Slot::Live(coin(
                id,
                Version::from_u64(RETAINED_VERSION),
                Owner::Shared(Version::from_u64(1)),
            )),
        );
        shared_ids.push(id);
    }
    if spec.package_pre_epoch {
        epoch_start
            .slots
            .insert(PACKAGE_ID, Slot::Live(package(Version::from_u64(1))));
    }

    let mut oracle = Oracle::new(epoch_start.clone(), spec.k as CommitIndex);
    let mut commits: Vec<Vec<Candidate>> = Vec::new();
    let mut earlier: Vec<Candidate> = Vec::new();
    let mut pending_reloads: Vec<Candidate> = Vec::new();
    let mut created_owned: Vec<ObjectId> = Vec::new();
    let mut next_created = 0u8;
    let mut next_unknown = 0u8;
    let mut commit_specs = spec.commits.iter();
    loop {
        let commit = oracle.next_commit();
        let specs = commit_specs.next();
        if specs.is_none() && pending_reloads.is_empty() {
            break;
        }
        let mut candidates = std::mem::take(&mut pending_reloads);
        for (index, candidate_spec) in specs.into_iter().flatten().enumerate() {
            let fresh = match candidate_spec {
                CandidateSpec::Retry(n) if !earlier.is_empty() => {
                    let copy = &earlier[*n as usize % earlier.len()];
                    candidates.push(Candidate {
                        defer: false,
                        deferral_reload: false,
                        ..copy.clone()
                    });
                    continue;
                }
                CandidateSpec::SameCommitDuplicate(n) if !candidates.is_empty() => {
                    let copy = candidates[*n as usize % candidates.len()].clone();
                    candidates.push(copy);
                    continue;
                }
                CandidateSpec::Fresh(fresh) => fresh.clone(),
                CandidateSpec::Retry(_) | CandidateSpec::SameCommitDuplicate(_) => FreshSpec {
                    gas: InputSpec {
                        object: 0,
                        source: Source::LiveAtHorizon,
                    },
                    object: None,
                    shared: Vec::new(),
                    package: false,
                    receive: None,
                    object_write: ObjectWriteSpec::Mutate,
                    extra: ExtraSpec::None,
                    defer: false,
                    fails: false,
                },
            };
            let gas_pool: Vec<ObjectId> = owned_ids.iter().chain(&created_owned).copied().collect();
            let object_pool: Vec<ObjectId> =
                gas_pool.iter().chain(&receivable_ids).copied().collect();
            let resolve = |pool: &[ObjectId], input: &InputSpec, next_unknown: &mut u8| {
                resolve_input(&oracle, commit, pool, &retained, input, next_unknown)
            };
            let gas = resolve(&gas_pool, &fresh.gas, &mut next_unknown);
            let object = fresh
                .object
                .as_ref()
                .map(|input| resolve(&object_pool, input, &mut next_unknown))
                .filter(|reference| reference.object_id != gas.object_id);
            let receiving: Vec<ObjectReference> = match &fresh.receive {
                Some(input) if !receivable_ids.is_empty() => {
                    vec![resolve(&receivable_ids, input, &mut next_unknown)]
                }
                _ => Vec::new(),
            };
            let now = oracle.state(commit - 1);
            let mut shared_inputs: Vec<(ObjectId, Version)> = Vec::new();
            let mut read_only_shared = Vec::new();
            for shared in &fresh.shared {
                let (id, initial) = if shared_ids.is_empty() {
                    (unknown_shared_id(), Version::from_u64(1))
                } else {
                    let id = shared_ids[shared.object as usize % shared_ids.len()];
                    (id, shared_initial_version(&oracle, &id))
                };
                let declared = match shared.declared {
                    SharedDeclared::Correct => initial,
                    SharedDeclared::Wrong => Version::from_u64(initial.as_u64() + 1),
                };
                if shared_inputs.iter().all(|(named, _)| *named != id) {
                    shared_inputs.push((id, declared));
                    if shared.read_only {
                        read_only_shared.push(id);
                    }
                }
            }
            let mut writes = Vec::new();
            if let Some(object) = &object {
                match fresh.object_write {
                    ObjectWriteSpec::Mutate => {}
                    ObjectWriteSpec::Delete => writes.push(Write::Delete(object.object_id)),
                    ObjectWriteSpec::Wrap => writes.push(Write::Wrap(object.object_id)),
                }
            }
            match fresh.extra {
                ExtraSpec::None => {}
                ExtraSpec::CreateOwned => {
                    let id = object_id(0xA1, next_created);
                    next_created += 1;
                    created_owned.push(id);
                    writes.push(Write::CreateOwned(id));
                }
                ExtraSpec::CreateShared => {
                    let id = object_id(0xA6, next_created);
                    next_created += 1;
                    shared_ids.push(id);
                    writes.push(Write::CreateShared(id));
                }
                ExtraSpec::Unwrap => {
                    if let Some((id, _)) = now
                        .slots
                        .iter()
                        .find(|(_, slot)| matches!(slot, Slot::Wrapped(_)))
                    {
                        writes.push(Write::Unwrap(*id));
                    }
                }
                ExtraSpec::Publish => writes.push(Write::Publish(PACKAGE_ID)),
                ExtraSpec::DeleteShared => {
                    if let Some((id, _)) = shared_inputs.first() {
                        writes.push(Write::DeleteShared(*id));
                    }
                }
            }
            candidates.push(Candidate {
                digest: transaction_digest(commit, index),
                owned_inputs: std::iter::once(gas).chain(object).collect(),
                shared_inputs,
                read_only_shared,
                packages: if fresh.package {
                    vec![PACKAGE_ID]
                } else {
                    Vec::new()
                },
                receiving,
                writes,
                fails: fresh.fails,
                defer: fresh.defer,
                deferral_reload: false,
            });
        }
        let outcome = oracle.judge_commit(&candidates);
        pending_reloads = reloads_of(&candidates, outcome);
        earlier.extend(candidates.iter().cloned());
        commits.push(candidates);
    }
    History {
        epoch_start,
        retained_pre_epoch: retained,
        commits,
        oracle,
    }
}

/// The initial shared version of `id`, from the latest state it is live in,
/// or 1 for an id no state has held yet.
fn shared_initial_version(oracle: &Oracle, id: &ObjectId) -> Version {
    let from_state = (0..oracle.next_commit()).rev().find_map(|commit| {
        match oracle.state(commit).live(id)?.owner {
            Owner::Shared(initial) => Some(initial),
            _ => None,
        }
    });
    from_state.unwrap_or_else(|| Version::from_u64(1))
}

fn slot_reference(state: &State, id: &ObjectId) -> Option<ObjectReference> {
    Some(match state.slots.get(id)? {
        Slot::Live(object) => object.object_ref(),
        Slot::Deleted(version) => ObjectReference::new(*id, *version, ObjectDigest::OBJECT_DELETED),
        Slot::Wrapped(version) => ObjectReference::new(*id, *version, ObjectDigest::OBJECT_WRAPPED),
    })
}

fn resolve_input(
    oracle: &Oracle,
    commit: CommitIndex,
    pool: &[ObjectId],
    retained: &[Object],
    input: &InputSpec,
    next_unknown: &mut u8,
) -> ObjectReference {
    let id = pool[input.object as usize % pool.len()];
    let horizon = oracle.horizon_state(commit);
    let now = oracle.state(commit - 1);
    let never = |id: ObjectId| {
        let above = now
            .slots
            .get(&id)
            .map(|slot| slot.version().as_u64())
            .unwrap_or(0)
            + 3;
        ObjectReference::new(id, Version::from_u64(above), wrong_digest(0xEE))
    };
    let unknown = |next_unknown: &mut u8| {
        let id = object_id(0xAF, *next_unknown);
        *next_unknown += 1;
        ObjectReference::new(id, Version::from_u64(1), wrong_digest(0xEF))
    };
    match input.source {
        Source::LiveAtHorizon => slot_reference(horizon, &id).unwrap_or_else(|| never(id)),
        Source::LiveNow => slot_reference(now, &id).unwrap_or_else(|| never(id)),
        Source::AboveHorizon => {
            let low = commit.saturating_sub(oracle.k);
            let produced = (low + 1..commit)
                .rev()
                .flat_map(|c| oracle.outcome(c).executions.iter().rev())
                .flat_map(|execution| execution.changes.iter())
                .find_map(|change| match (change.id == id, &change.after) {
                    (true, Slot::Live(object)) => Some(object.object_ref()),
                    _ => None,
                });
            produced.unwrap_or_else(|| slot_reference(now, &id).unwrap_or_else(|| never(id)))
        }
        Source::ConsumedEarlier => {
            let consumed = (1..commit)
                .flat_map(|c| oracle.outcome(c).executions.iter())
                .flat_map(|execution| execution.consumed())
                .find(|object| object.id() == id)
                .map(|object| object.object_ref());
            consumed.unwrap_or_else(|| slot_reference(horizon, &id).unwrap_or_else(|| never(id)))
        }
        Source::ConsumedAboveHorizon => {
            let low = commit.saturating_sub(oracle.k);
            let consumed = (low + 1..commit)
                .flat_map(|c| oracle.outcome(c).executions.iter())
                .flat_map(|execution| execution.consumed())
                .find(|object| object.id() == id)
                .map(|object| object.object_ref());
            consumed.unwrap_or_else(|| slot_reference(horizon, &id).unwrap_or_else(|| never(id)))
        }
        Source::PreEpochStale => retained
            .iter()
            .find(|object| object.id() == id)
            .map(|object| object.object_ref())
            .unwrap_or_else(|| unknown(next_unknown)),
        Source::NeverExisting => never(id),
        Source::UnknownId => unknown(next_unknown),
        Source::WrongDigest => {
            let mut reference = slot_reference(horizon, &id).unwrap_or_else(|| never(id));
            reference.digest = wrong_digest(0xED);
            reference
        }
    }
}

// ---------------------------------------------------------------------------
// Fake object store
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum StoredVersion {
    Live { object: Object, pruned: bool },
    Tombstone(ObjectDigest),
}

#[derive(Clone, Debug)]
struct StoredEntry {
    version: StoredVersion,
    /// Written by an execution whose outputs are not durable yet; lost on a
    /// crash.
    dirty: bool,
    /// Produced by an execution of a commit the handler assigned, and covered
    /// by the bookkeeping: a handler row must be readable whenever the entry
    /// is.
    bookkept: bool,
}

#[derive(Default)]
struct StoreState {
    objects: BTreeMap<ObjectId, BTreeMap<Version, StoredEntry>>,
    /// Deletion markers by id and version, with whether they are dirty.
    markers: BTreeMap<ObjectId, BTreeMap<Version, (MarkerValue, bool)>>,
}

impl StoreState {
    fn latest(&self, id: &ObjectId) -> Option<(Version, &StoredEntry)> {
        self.objects
            .get(id)?
            .iter()
            .next_back()
            .map(|(version, entry)| (*version, entry))
    }

    fn latest_reference(&self, id: &ObjectId) -> Option<ObjectReference> {
        let (version, entry) = self.latest(id)?;
        Some(match &entry.version {
            StoredVersion::Live { object, .. } => object.object_ref(),
            StoredVersion::Tombstone(digest) => ObjectReference::new(*id, version, *digest),
        })
    }

    fn latest_live(&self, id: &ObjectId) -> Option<Object> {
        match &self.latest(id)?.1.version {
            StoredVersion::Live { object, .. } => Some(object.clone()),
            StoredVersion::Tombstone(_) => None,
        }
    }

    fn by_key(&self, id: &ObjectId, version: Version) -> Option<Object> {
        match &self.objects.get(id)?.get(&version)?.version {
            StoredVersion::Live { object, pruned } if !pruned => Some(object.clone()),
            _ => None,
        }
    }
}

/// The deletion markers an execution writes: one at the tombstone version of
/// every shared object it deletes, and one at its own version for every
/// shared input that was already deleted.
fn deletion_markers(execution: &Execution) -> Vec<(ObjectId, Version)> {
    let deleted = execution.changes.iter().filter_map(|change| {
        let shared_before = change
            .before
            .as_ref()
            .is_some_and(|before| before.owner.is_shared());
        (shared_before && matches!(change.after, Slot::Deleted(_)))
            .then(|| (change.id, change.after.version()))
    });
    let accessed = execution
        .deleted_shared_inputs
        .iter()
        .map(|id| (*id, execution.lamport));
    deleted.chain(accessed).collect()
}

/// The object store of one validator: the versions its executions have
/// produced, tiered into dirty and durable like the writeback cache, plus
/// tombstones, shared-deletion markers and pruning.
struct FakeStore(Mutex<StoreState>);

impl FakeStore {
    fn new(history: &History) -> Self {
        let mut state = StoreState::default();
        let genesis = |object: Object| StoredEntry {
            version: StoredVersion::Live {
                object,
                pruned: false,
            },
            dirty: false,
            bookkept: false,
        };
        for (id, slot) in &history.epoch_start.slots {
            let entry = match slot {
                Slot::Live(object) => genesis(object.clone()),
                Slot::Deleted(_) => StoredEntry {
                    version: StoredVersion::Tombstone(ObjectDigest::OBJECT_DELETED),
                    dirty: false,
                    bookkept: false,
                },
                Slot::Wrapped(_) => StoredEntry {
                    version: StoredVersion::Tombstone(ObjectDigest::OBJECT_WRAPPED),
                    dirty: false,
                    bookkept: false,
                },
            };
            state
                .objects
                .entry(*id)
                .or_default()
                .insert(slot.version(), entry);
        }
        for object in &history.retained_pre_epoch {
            state
                .objects
                .entry(object.id())
                .or_default()
                .insert(object.version(), genesis(object.clone()));
        }
        Self(Mutex::new(state))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, StoreState> {
        self.0
            .lock()
            .expect("the fake store is only locked by the harness thread")
    }

    /// Makes an execution's outputs and deletion markers readable, in the
    /// dirty tier. `handler_known` says whether the handler assigned the
    /// execution's commit; only then do handler rows cover its outputs.
    fn apply(&self, execution: &Execution, handler_known: bool) {
        let mut state = self.lock();
        for change in &execution.changes {
            let bookkept = handler_known
                && !(change.kind == ChangeKind::Mutate
                    && matches!(change.after, Slot::Live(ref object) if object.owner.is_shared()));
            let version = change.after.version();
            let stored = match &change.after {
                Slot::Live(object) => StoredVersion::Live {
                    object: object.clone(),
                    pruned: false,
                },
                Slot::Deleted(_) => StoredVersion::Tombstone(ObjectDigest::OBJECT_DELETED),
                Slot::Wrapped(_) => StoredVersion::Tombstone(ObjectDigest::OBJECT_WRAPPED),
            };
            state.objects.entry(change.id).or_default().insert(
                version,
                StoredEntry {
                    version: stored,
                    dirty: true,
                    bookkept,
                },
            );
        }
        for (id, version) in deletion_markers(execution) {
            state.markers.entry(id).or_default().insert(
                version,
                (MarkerValue::SharedDeleted(execution.digest), true),
            );
        }
    }

    fn make_durable(&self, execution: &Execution) {
        let mut state = self.lock();
        for change in &execution.changes {
            let version = change.after.version();
            if let Some(entry) = state
                .objects
                .get_mut(&change.id)
                .and_then(|versions| versions.get_mut(&version))
            {
                entry.dirty = false;
            }
        }
        for (id, version) in deletion_markers(execution) {
            if let Some(marker) = state
                .markers
                .get_mut(&id)
                .and_then(|markers| markers.get_mut(&version))
            {
                marker.1 = false;
            }
        }
    }

    /// Deletes a version's bytes, as the pruner does; the latest reference of
    /// the id is unaffected.
    fn prune(&self, key: &ObjectKey) {
        let mut state = self.lock();
        let Some(versions) = state.objects.get_mut(&key.0) else {
            return;
        };
        let is_latest = versions.keys().next_back() == Some(&key.1);
        assert!(!is_latest, "the pruner never deletes the latest version");
        if let Some(entry) = versions.get_mut(&key.1) {
            if let StoredVersion::Live { pruned, .. } = &mut entry.version {
                *pruned = true;
            }
        }
    }

    fn crash(&self) {
        let mut state = self.lock();
        for versions in state.objects.values_mut() {
            versions.retain(|_, entry| !entry.dirty);
        }
        state.objects.retain(|_, versions| !versions.is_empty());
        for markers in state.markers.values_mut() {
            markers.retain(|_, (_, dirty)| !*dirty);
        }
        state.markers.retain(|_, markers| !markers.is_empty());
    }

    /// Every readable version the bookkeeping must cover.
    fn bookkept_keys(&self) -> Vec<ObjectKey> {
        let state = self.lock();
        state
            .objects
            .iter()
            .flat_map(|(id, versions)| {
                versions
                    .iter()
                    .filter(|(_, entry)| entry.bookkept)
                    .map(move |(version, _)| ObjectKey(*id, *version))
            })
            .collect()
    }
}

impl ObjectCacheRead for FakeStore {
    fn try_get_package_object(&self, id: &ObjectId) -> IotaResult<Option<PackageObject>> {
        Ok(self
            .lock()
            .latest_live(id)
            .filter(|object| object.is_package())
            .map(PackageObject::new))
    }

    fn force_reload_system_packages(&self, _system_package_ids: &[ObjectId]) {}

    fn try_get_object(&self, id: &ObjectId) -> IotaResult<Option<Object>> {
        Ok(self.lock().latest_live(id))
    }

    fn try_get_latest_object_ref_or_tombstone(
        &self,
        object_id: ObjectId,
    ) -> IotaResult<Option<ObjectReference>> {
        Ok(self.lock().latest_reference(&object_id))
    }

    fn try_get_latest_object_or_tombstone(
        &self,
        object_id: ObjectId,
    ) -> IotaResult<Option<(ObjectKey, ObjectOrTombstone)>> {
        let state = self.lock();
        Ok(state.latest(&object_id).map(|(version, entry)| {
            let value = match &entry.version {
                StoredVersion::Live { object, .. } => ObjectOrTombstone::Object(object.clone()),
                StoredVersion::Tombstone(digest) => {
                    ObjectOrTombstone::Tombstone(ObjectReference::new(object_id, version, *digest))
                }
            };
            (ObjectKey(object_id, version), value)
        }))
    }

    fn try_get_object_by_key(
        &self,
        object_id: &ObjectId,
        version: Version,
    ) -> IotaResult<Option<Object>> {
        Ok(self.lock().by_key(object_id, version))
    }

    fn try_multi_get_objects_by_key(
        &self,
        object_keys: &[ObjectKey],
    ) -> IotaResult<Vec<Option<Object>>> {
        let state = self.lock();
        Ok(object_keys
            .iter()
            .map(|key| state.by_key(&key.0, key.1))
            .collect())
    }

    fn try_object_exists_by_key(&self, object_id: &ObjectId, version: Version) -> IotaResult<bool> {
        Ok(self.lock().by_key(object_id, version).is_some())
    }

    fn try_multi_object_exists_by_key(&self, object_keys: &[ObjectKey]) -> IotaResult<Vec<bool>> {
        let state = self.lock();
        Ok(object_keys
            .iter()
            .map(|key| state.by_key(&key.0, key.1).is_some())
            .collect())
    }

    fn multi_input_objects_available_cache_only(&self, _keys: &[InputKey]) -> Vec<bool> {
        unreachable!("post-consensus validation does not check input availability")
    }

    fn try_find_object_lt_or_eq_version(
        &self,
        _object_id: ObjectId,
        _version: Version,
    ) -> IotaResult<Option<Object>> {
        unreachable!("post-consensus validation does not read child objects by bound")
    }

    fn try_get_lock(
        &self,
        _obj_ref: ObjectReference,
        _epoch_store: &AuthorityPerEpochStore,
    ) -> IotaLockResult {
        unreachable!("post-consensus validation does not read pre-consensus locks")
    }

    fn _try_get_live_objref(&self, _object_id: ObjectId) -> IotaResult<ObjectReference> {
        unreachable!("post-consensus validation does not map admission errors")
    }

    fn try_check_owned_objects_are_live(
        &self,
        _owned_object_refs: &[ObjectReference],
    ) -> IotaResult {
        unreachable!("post-consensus validation does not check liveness before execution")
    }

    fn try_get_iota_system_state_object_unsafe(&self) -> IotaResult<IotaSystemState> {
        unreachable!("post-consensus validation does not read the system state")
    }

    fn try_get_highest_pruned_checkpoint(&self) -> IotaResult<Option<CheckpointSequenceNumber>> {
        unreachable!("post-consensus validation does not read the pruning watermark")
    }

    fn notify_read_input_objects<'a>(
        &'a self,
        _input_and_receiving_keys: &'a [InputKey],
        _receiving_keys: &'a HashSet<InputKey>,
        _epoch: EpochId,
    ) -> BoxFuture<'a, ()> {
        unreachable!("post-consensus validation does not wait for inputs")
    }

    fn try_get_marker_value(
        &self,
        object_id: &ObjectId,
        version: Version,
        _epoch_id: EpochId,
    ) -> IotaResult<Option<MarkerValue>> {
        Ok(self
            .lock()
            .markers
            .get(object_id)
            .and_then(|markers| markers.get(&version))
            .map(|(marker, _)| *marker))
    }

    fn try_get_latest_marker(
        &self,
        object_id: &ObjectId,
        _epoch_id: EpochId,
    ) -> IotaResult<Option<(Version, MarkerValue)>> {
        Ok(self
            .lock()
            .markers
            .get(object_id)
            .and_then(|markers| markers.iter().next_back())
            .map(|(version, (marker, _))| (*version, *marker)))
    }
}

// ---------------------------------------------------------------------------
// Effects
// ---------------------------------------------------------------------------

/// The effects every validator's execution of `execution` produces.
fn effects_of(execution: &Execution) -> TransactionEffects {
    let mut changed_objects = BTreeMap::new();
    let mut shared_objects = Vec::new();
    for change in &execution.changes {
        let input_state = match &change.before {
            Some(object) => {
                if object.owner.is_shared() {
                    shared_objects.push(SharedInput::Existing(object.object_ref()));
                }
                ObjectIn::Data {
                    version: object.version(),
                    digest: object.digest(),
                    owner: object.owner,
                }
            }
            None => ObjectIn::Missing,
        };
        let output_state = match &change.after {
            Slot::Live(object) if object.is_package() => ObjectOut::PackageWrite {
                version: object.version(),
                digest: object.digest(),
            },
            Slot::Live(object) => ObjectOut::ObjectWrite {
                digest: object.digest(),
                owner: object.owner,
            },
            Slot::Deleted(_) | Slot::Wrapped(_) => ObjectOut::Missing,
        };
        let id_operation = match change.kind {
            ChangeKind::Create | ChangeKind::Publish => IdOperation::Created,
            ChangeKind::Delete => IdOperation::Deleted,
            ChangeKind::Mutate | ChangeKind::Wrap | ChangeKind::Unwrap => IdOperation::None,
        };
        changed_objects.insert(
            change.id,
            ChangedObject {
                object_id: change.id,
                input_state,
                output_state,
                id_operation,
            },
        );
    }
    shared_objects.extend(
        execution
            .read_only_shared
            .iter()
            .filter_map(|(_, slot)| slot.live())
            .map(|object| SharedInput::Existing(object.object_ref())),
    );
    let status = if execution.failed {
        ExecutionStatus::Failure {
            error: ExecutionError::InsufficientGas,
            command: None,
        }
    } else {
        ExecutionStatus::Success
    };
    TransactionEffects::new_from_execution_v1(
        status,
        0,
        GasCostSummary::default(),
        shared_objects,
        BTreeSet::new(),
        execution.digest,
        execution.lamport,
        changed_objects,
        Some(execution.gas),
        None,
        Vec::new(),
    )
}

// ---------------------------------------------------------------------------
// Schedule
// ---------------------------------------------------------------------------

/// A step of the validator's local schedule that changes state without
/// validating; can run between two reads of one input.
#[derive(Clone, Copy, Debug)]
enum WriterStep {
    /// Executes the next transaction in consensus order, once the handler
    /// has assigned its commit, or at once after the switch to executing
    /// synced checkpoints.
    Execute,
    /// Executes one transaction whose inputs are all in the store, chosen by
    /// `pick` among them, in any consensus order, among the commits the
    /// handler has assigned, or among all of them after the switch.
    ExecuteAny { pick: u8 },
    /// Completes the oldest assigned commit whose roots have all executed.
    Watcher,
    /// Executes the next checkpoint: runs its transactions, persists their
    /// bookkeeping, commits their outputs. Crashing in between leaves the
    /// bookkeeping durable and the outputs not. Before the switch, only a
    /// checkpoint of commits the handler has assigned, which is the one the
    /// node builds itself.
    Checkpoint { crash_after_bookkeeping: bool },
    /// `Checkpoint` over the next `commits` commits as one checkpoint, with
    /// one bookkeeping batch for all of them.
    CheckpointCommits {
        commits: u8,
        crash_after_bookkeeping: bool,
    },
    /// Flushes the next validated commit whose checkpoint has executed:
    /// `FlushStage` then `FlushWrite`, or the write of a staged flush.
    Flush,
    /// `Flush` of up to `commits` commits in one batch.
    FlushCommits { commits: u8 },
    /// Stages the next flushable commit's rows, locks, processed keys,
    /// resume point and verdict retirement into a batch
    /// without writing it, as a flush does before its write; the batch stays
    /// pending while other steps run.
    FlushStage,
    /// `FlushStage` of up to `commits` commits in one batch.
    FlushStageCommits { commits: u8 },
    /// Writes the staged batch and evicts the flushed commits' entries.
    FlushWrite,
    /// Deletes every version a checkpointed transaction consumed at a commit
    /// no validation can keep it at any more, and with `pre_epoch_too` the
    /// retained versions from before the epoch.
    Prune { pre_epoch_too: bool },
}

/// Where in an input's read a writer step is injected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReaderPoint {
    /// After the first row read (owned, shared) or the store read (package).
    Row,
    /// After the store read, before the row re-read (owned, shared, package).
    Store,
    /// After the read of the rows above an owned version, or of a gone
    /// shared object's highest row (owned, shared).
    LastTableRead,
}

#[derive(Clone, Debug)]
struct Interleave {
    /// Which input of the first candidate that reaches the reader, modulo
    /// its input count.
    input: u8,
    writers: Vec<(ReaderPoint, WriterStep)>,
}

/// How the wait before validation brings the execution frontier to the
/// horizon when it is behind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WaitPath {
    /// The execution watcher completes each commit once its roots executed.
    Watcher,
    /// The checkpoint executor runs each commit's checkpoint and the
    /// quarantine flush completes the commit before the watcher does.
    Flush,
}

/// Where a crash lands while a commit is validated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ValidationCrash {
    /// The verdicts are computed and not saved.
    BeforePersist,
    /// The verdicts are saved and the commit's roots are not assigned yet.
    AfterPersist,
}

#[derive(Clone, Debug)]
enum Step {
    /// Validates the next commit, after the wait for commit `C - K`.
    Validate {
        /// Each applies to the next candidate that reaches the reader.
        interleave: Vec<Interleave>,
        wait: WaitPath,
        crash: Option<ValidationCrash>,
    },
    Writer(WriterStep),
    /// The checkpoint executor learns the epoch's last checkpoint and
    /// switches the node to executing synced checkpoints: the handler
    /// validates no further commit.
    Switch,
    Crash,
}

#[derive(Clone, Debug)]
struct Schedule {
    steps: Vec<Step>,
}

/// A choice among constants with weights.
fn weighted<T: Clone + std::fmt::Debug + 'static>(
    choices: impl IntoIterator<Item = (u32, T)>,
) -> BoxedStrategy<T> {
    let choices: Vec<(u32, BoxedStrategy<T>)> = choices
        .into_iter()
        .map(|(weight, value)| (weight, Just(value).boxed()))
        .collect();
    Union::new_weighted(choices).boxed()
}

fn writer_step() -> impl Strategy<Value = WriterStep> {
    prop_oneof![
        3 => Just(WriterStep::Execute),
        2 => any::<u8>().prop_map(|pick| WriterStep::ExecuteAny { pick }),
        3 => Just(WriterStep::Watcher),
        2 => proptest::bool::weighted(0.15)
            .prop_map(|crash_after_bookkeeping| WriterStep::Checkpoint { crash_after_bookkeeping }),
        1 => (2..=3u8, proptest::bool::weighted(0.15)).prop_map(
            |(commits, crash_after_bookkeeping)| WriterStep::CheckpointCommits {
                commits,
                crash_after_bookkeeping,
            }
        ),
        2 => Just(WriterStep::Flush),
        1 => (2..=3u8).prop_map(|commits| WriterStep::FlushCommits { commits }),
        1 => any::<bool>().prop_map(|pre_epoch_too| WriterStep::Prune { pre_epoch_too }),
        1 => prop_oneof![
            3 => Just(WriterStep::Flush),
            1 => Just(WriterStep::FlushStage),
            1 => (2..=3u8).prop_map(|commits| WriterStep::FlushStageCommits { commits }),
            1 => Just(WriterStep::FlushWrite),
        ],
    ]
}

fn reader_point() -> impl Strategy<Value = ReaderPoint> {
    prop_oneof![
        Just(ReaderPoint::Row),
        Just(ReaderPoint::Store),
        Just(ReaderPoint::LastTableRead),
    ]
}

fn interleave() -> impl Strategy<Value = Interleave> {
    (
        any::<u8>(),
        proptest::collection::vec((reader_point(), writer_step()), 1..=2),
    )
        .prop_map(|(input, writers)| Interleave { input, writers })
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        5 => (
            proptest::option::weighted(0.3, proptest::collection::vec(interleave(), 1..=2))
                .prop_map(Option::unwrap_or_default),
            weighted([(3, WaitPath::Watcher), (1, WaitPath::Flush)]),
            proptest::option::weighted(
                0.1,
                weighted([
                    (1, ValidationCrash::BeforePersist),
                    (1, ValidationCrash::AfterPersist),
                ]),
            ),
        )
            .prop_map(|(interleave, wait, crash)| Step::Validate {
                interleave,
                wait,
                crash,
            }),
        10 => writer_step().prop_map(Step::Writer),
        1 => Just(Step::Switch),
        1 => Just(Step::Crash),
    ]
}

fn schedule() -> impl Strategy<Value = Schedule> {
    proptest::collection::vec(step(), 0..=28).prop_map(|steps| Schedule { steps })
}

// ---------------------------------------------------------------------------
// Validator
// ---------------------------------------------------------------------------

struct ExecutedTransaction {
    effects: TransactionEffects,
    durable: bool,
}

/// A validated commit's output while it sits in the consensus quarantine.
#[derive(Default)]
struct Quarantined {
    locks: BTreeMap<ObjectReference, TransactionDigest>,
    processed: Vec<TransactionDigest>,
}

/// The harness abandons the step in progress when a crash lands inside it.
struct Crashed;

/// A flush between staging its batch and writing it: the commits it holds,
/// in order, with the rows each stages.
struct StagedFlush {
    commits: Vec<(CommitIndex, Vec<(ObjectKey, HandlerProcessedObject)>)>,
    batch: typed_store::rocks::DBBatch,
}

/// [`reopen`], tried again while the closed epoch DB still holds its RocksDB
/// lock. A property run reopens the epoch store tens of thousands of times in
/// one process, and on a loaded machine the lock can outlast the retries
/// [`reopen`] makes.
fn reopen_when_closed(
    authority: &AuthorityState,
    store: &AuthorityPerEpochStore,
) -> Arc<AuthorityPerEpochStore> {
    const ATTEMPTS: u32 = 20;
    for attempt in 1..=ATTEMPTS {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| reopen(authority, store))) {
            Ok(reopened) => return reopened,
            Err(panic) if attempt == ATTEMPTS => std::panic::resume_unwind(panic),
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(500)),
        }
    }
    unreachable!("the last attempt returns or resumes its panic")
}

/// The shared test authority; its epoch store is reset between cases.
struct Env {
    authority: Arc<AuthorityState>,
    epoch_store: Mutex<Arc<AuthorityPerEpochStore>>,
    /// The horizon distance the protocol config override hands every epoch
    /// store opened after it is set.
    horizon_distance: Arc<AtomicU64>,
}

impl Env {
    async fn new(horizon_distance: Arc<AtomicU64>) -> Self {
        let authority = TestAuthorityBuilder::new().build().await;
        let initial = (*authority.epoch_store_for_testing()).clone();
        let epoch_store = reopen_when_closed(&authority, &initial);
        assert!(
            epoch_store.pcool_bookkeeping_enabled(),
            "the P-COOL flags must be on for the epoch store to keep the bookkeeping"
        );
        Self {
            authority,
            epoch_store: Mutex::new(epoch_store),
            horizon_distance,
        }
    }

    /// A fresh epoch store over emptied tables, as at an epoch start, whose
    /// protocol config has horizon distance `k`.
    fn reset(&self, k: CommitIndex) -> IotaResult<Arc<AuthorityPerEpochStore>> {
        let mut current = self
            .epoch_store
            .lock()
            .expect("the epoch store slot is only locked by the harness thread");
        clear_pcool_validation_tables(&current)?;
        self.horizon_distance.store(k, Ordering::SeqCst);
        let reopened = reopen_when_closed(&self.authority, &current);
        assert_eq!(
            reopened
                .protocol_config()
                .pcool_deterministic_validation_horizon_distance_or_default(),
            k,
            "the reopened epoch store must use the history's horizon distance"
        );
        *current = reopened.clone();
        Ok(reopened)
    }
}

/// One validator replaying a judged history under a local schedule.
struct Validator<'a> {
    history: &'a History,
    env: &'a Env,
    epoch_store: Arc<AuthorityPerEpochStore>,
    store: Arc<FakeStore>,
    executed: BTreeMap<TransactionDigest, ExecutedTransaction>,
    quarantine: BTreeMap<CommitIndex, Quarantined>,
    validated_through: CommitIndex,
    flushed_through: CommitIndex,
    checkpointed_through: CommitIndex,
    /// Commits the handler assigned since the last restart and that no
    /// completion path has finished yet.
    assigned: BTreeSet<CommitIndex>,
    staged_flush: Option<StagedFlush>,
    trace: Vec<String>,
    /// Inputs where the reader's drop-versus-missing class differed from the
    /// oracle's while the verdict agreed.
    drop_or_missing_differences: Vec<String>,
}

fn consensus_key(digest: TransactionDigest) -> SequencedConsensusTransactionKey {
    SequencedConsensusTransactionKey::External(ConsensusTransactionKey::UserTransaction(digest))
}

/// The verdict classes compared with the oracle: the implementation reports
/// stale and missing inputs with one error, so the two are one class here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VerdictClass {
    Kept,
    LockConflict,
    InvisibleInput,
}

/// The oracle's verdict as a class; `None` for a duplicate, which never
/// reaches validation.
fn oracle_class(verdict: Verdict) -> Option<VerdictClass> {
    match verdict {
        Verdict::Kept => Some(VerdictClass::Kept),
        Verdict::Duplicate => None,
        Verdict::Dropped(oracle::DropClass::Lock) => Some(VerdictClass::LockConflict),
        Verdict::Dropped(oracle::DropClass::Stale | oracle::DropClass::Missing) => {
            Some(VerdictClass::InvisibleInput)
        }
    }
}

fn real_class(verdict: &SavedPostConsensusDecision) -> VerdictClass {
    match verdict.decision {
        PostConsensusDecision::Kept => VerdictClass::Kept,
        PostConsensusDecision::Dropped
            if matches!(verdict.error(), IotaError::ObjectLockConflict { .. }) =>
        {
            VerdictClass::LockConflict
        }
        PostConsensusDecision::Dropped | PostConsensusDecision::DroppedInputNotVisibleYet => {
            VerdictClass::InvisibleInput
        }
    }
}

/// The reader's answer for one input, reduced to what the oracle models.
struct InputRead {
    class: InputClass,
    reason: String,
}

impl<'a> Validator<'a> {
    fn new(history: &'a History, env: &'a Env) -> IotaResult<Self> {
        let epoch_store = env.reset(history.k())?;
        Ok(Self {
            history,
            env,
            epoch_store,
            store: Arc::new(FakeStore::new(history)),
            executed: BTreeMap::new(),
            quarantine: BTreeMap::new(),
            validated_through: 0,
            flushed_through: 0,
            checkpointed_through: 0,
            assigned: BTreeSet::new(),
            staged_flush: None,
            trace: Vec::new(),
            drop_or_missing_differences: Vec::new(),
        })
    }

    fn horizon(&self, commit: CommitIndex) -> CommitIndex {
        commit.saturating_sub(self.history.k())
    }

    fn fail<T>(&self, message: impl std::fmt::Display) -> Outcome<T> {
        Err(TestCaseError::fail(format!(
            "{message}\n\nhistory:\n{}\ntrace:\n{}",
            self.history.describe(),
            self.trace.join("\n")
        )))
    }

    fn storage<T>(&self, result: IotaResult<T>) -> Outcome<T> {
        match result {
            Ok(value) => Ok(value),
            Err(error) => self.fail(format!("storage error: {error}")),
        }
    }

    fn frontier(&self) -> CommitIndex {
        *self
            .epoch_store
            .subscribe_highest_fully_executed_commit()
            .unwrap()
            .borrow()
    }

    fn reader(&self, commit: CommitIndex) -> CommitIndexedReader {
        CommitIndexedReader::new(self.store.clone(), self.epoch_store.clone(), commit)
    }

    /// Whether the node executes synced checkpoints for the rest of the
    /// epoch, as the epoch store says.
    fn switched(&self) -> bool {
        self.epoch_store.is_executing_synced_checkpoints()
    }

    /// The highest commit whose transactions the executor may run: those the
    /// handler assigned, or every commit once the node switched.
    fn executable_through(&self) -> CommitIndex {
        if self.switched() {
            self.history.last_commit()
        } else {
            self.validated_through
        }
    }

    /// The switch to executing synced checkpoints, through the real call. The
    /// handler is between commits here, so it returns at once.
    fn switch(&mut self) -> Outcome<()> {
        if self.switched() {
            return Ok(());
        }
        if self
            .epoch_store
            .start_executing_synced_checkpoints()
            .now_or_never()
            .is_none()
        {
            return self.fail("the switch waits although the handler is between commits");
        }
        self.trace.push(format!(
            "switch to executing synced checkpoints after validating {}",
            self.validated_through
        ));
        Ok(())
    }

    // -- steps ---------------------------------------------------------------

    fn run(&mut self, step: &Step) -> Outcome<()> {
        // A crash inside a step abandons the rest of it; the next step runs
        // on the restarted validator either way.
        let _ = match step {
            Step::Validate {
                interleave,
                wait,
                crash,
            } => self.validate_next(interleave, *wait, *crash)?,
            Step::Writer(writer) => self.writer(*writer)?,
            Step::Switch => {
                self.switch()?;
                Ok(())
            }
            Step::Crash => {
                self.crash()?;
                Err(Crashed)
            }
        };
        self.check_invariants()
    }

    fn writer(&mut self, step: WriterStep) -> Outcome<Result<(), Crashed>> {
        match step {
            WriterStep::Execute => {
                self.execute_next()?;
                Ok(Ok(()))
            }
            WriterStep::ExecuteAny { pick } => {
                self.execute_any(pick)?;
                Ok(Ok(()))
            }
            WriterStep::Watcher => {
                self.watcher()?;
                Ok(Ok(()))
            }
            WriterStep::Checkpoint {
                crash_after_bookkeeping,
            } => self.checkpoint(crash_after_bookkeeping),
            WriterStep::CheckpointCommits {
                commits,
                crash_after_bookkeeping,
            } => self.checkpoint_commits(commits.into(), crash_after_bookkeeping),
            WriterStep::Flush => {
                self.flush()?;
                Ok(Ok(()))
            }
            WriterStep::FlushCommits { commits } => {
                if self.staged_flush.is_none() {
                    self.flush_stage_commits(commits.into())?;
                }
                self.flush_write()?;
                Ok(Ok(()))
            }
            WriterStep::FlushStage => {
                self.flush_stage()?;
                Ok(Ok(()))
            }
            WriterStep::FlushStageCommits { commits } => {
                self.flush_stage_commits(commits.into())?;
                Ok(Ok(()))
            }
            WriterStep::FlushWrite => {
                self.flush_write()?;
                Ok(Ok(()))
            }
            WriterStep::Prune { pre_epoch_too } => {
                self.prune(pre_epoch_too);
                Ok(Ok(()))
            }
        }
    }

    /// Runs every step of the schedule, then finishes the history: every
    /// commit validated, unless the node switched, then every commit
    /// checkpointed and every validated one flushed.
    fn run_schedule(&mut self, schedule: &Schedule) -> Outcome<()> {
        for step in &schedule.steps {
            self.run(step)?;
        }
        self.trace.push("drain".to_string());
        while !self.switched() && self.validated_through < self.history.last_commit() {
            let _ = self.validate_next(&[], WaitPath::Watcher, None)?;
            self.check_invariants()?;
        }
        while self.checkpointed_through < self.history.last_commit() {
            let _ = self.checkpoint(false)?;
            self.check_invariants()?;
        }
        while self.flushed_through < self.validated_through {
            self.flush()?;
            self.check_invariants()?;
        }
        self.final_checks()
    }

    fn next_unexecuted(&self) -> Option<(CommitIndex, &'a Execution)> {
        let history = self.history;
        history
            .executions()
            .find(|(_, execution)| !self.executed.contains_key(&execution.digest))
    }

    fn execute_next(&mut self) -> Outcome<()> {
        let Some((commit, execution)) = self.next_unexecuted() else {
            return Ok(());
        };
        if commit > self.executable_through() {
            return Ok(());
        }
        self.execute(execution)
    }

    /// Whether every input `execution` reads is in the store as it was at
    /// the transaction's consensus position, so it can execute now.
    fn ready(&self, execution: &Execution) -> bool {
        let store = self.store.lock();
        let consumed_readable = execution
            .consumed()
            .all(|object| store.by_key(&object.id(), object.version()).as_ref() == Some(object));
        let read_only_readable = execution
            .read_only_shared
            .iter()
            .all(|(id, slot)| match slot {
                Slot::Live(object) => store.by_key(id, object.version()).as_ref() == Some(object),
                Slot::Deleted(version) | Slot::Wrapped(version) => store
                    .objects
                    .get(id)
                    .is_some_and(|versions| versions.contains_key(version)),
            });
        let deletions_executed = execution.deleted_shared_inputs.iter().all(|id| {
            store
                .latest(id)
                .is_some_and(|(_, entry)| matches!(entry.version, StoredVersion::Tombstone(_)))
        });
        let unwrapped_wrapped = execution
            .changes
            .iter()
            .filter(|change| change.kind == ChangeKind::Unwrap)
            .all(|change| {
                store.latest(&change.id).is_some_and(|(_, entry)| {
                    matches!(
                        entry.version,
                        StoredVersion::Tombstone(digest) if digest == ObjectDigest::OBJECT_WRAPPED
                    )
                })
            });
        consumed_readable && read_only_readable && deletions_executed && unwrapped_wrapped
    }

    fn execute_any(&mut self, pick: u8) -> Outcome<()> {
        let history = self.history;
        let executable_through = self.executable_through();
        let ready: Vec<&Execution> = history
            .executions()
            .filter(|(commit, execution)| {
                !self.executed.contains_key(&execution.digest)
                    && *commit <= executable_through
                    && self.ready(execution)
            })
            .map(|(_, execution)| execution)
            .collect();
        if ready.is_empty() {
            return Ok(());
        }
        self.execute(ready[pick as usize % ready.len()])
    }

    /// Runs the execution hook for `execution`, then makes its outputs
    /// readable, as `commit_transaction` does.
    fn execute(&mut self, execution: &Execution) -> Outcome<()> {
        let inputs: Vec<Object> = execution.consumed().cloned().collect();
        for input in &inputs {
            let readable = self.store.lock().by_key(&input.id(), input.version());
            if readable.as_ref() != Some(input) {
                return self.fail(format!(
                    "input {:?} of {} is not readable at execution",
                    input.object_ref(),
                    execution.digest
                ));
            }
        }
        let effects = effects_of(execution);
        let key = TransactionKey::Digest(execution.digest);
        let assigned_at = self
            .epoch_store
            .handler_object_state_for_testing()
            .commit_index_of(&key);
        let classification = match assigned_at {
            Some(index) => format!("handler-known at {index}"),
            None if self.switched() => "synced, no rows".to_string(),
            None => {
                return self.fail(format!(
                    "{} executes before the handler assigned its commit",
                    execution.digest
                ));
            }
        };
        self.epoch_store.record_executed_transaction(&key, &effects);
        self.store.apply(execution, assigned_at.is_some());
        self.executed.insert(
            execution.digest,
            ExecutedTransaction {
                effects,
                durable: false,
            },
        );
        self.trace
            .push(format!("execute {} ({classification})", execution.digest));
        Ok(())
    }

    fn execute_roots(&mut self, commit: CommitIndex) -> Outcome<()> {
        let history = self.history;
        for digest in history.oracle.outcome(commit).roots() {
            if !self.executed.contains_key(&digest) {
                let (_, execution) = history
                    .execution(&digest)
                    .expect("every root is an execution of the history");
                self.execute(execution)?;
            }
        }
        Ok(())
    }

    fn roots_executed(&self, commit: CommitIndex) -> bool {
        self.history
            .oracle
            .outcome(commit)
            .roots()
            .iter()
            .all(|digest| self.executed.contains_key(digest))
    }

    fn commit_rows(&self, commit: CommitIndex) -> Vec<(ObjectKey, HandlerProcessedObject)> {
        let effects: Vec<&TransactionEffects> = self
            .history
            .oracle
            .outcome(commit)
            .roots()
            .iter()
            .map(|digest| &self.executed[digest].effects)
            .collect();
        handler_rows_for_commit(effects, commit)
    }

    /// Marks `commit` fully executed as the execution watcher does, through
    /// the real call, which takes the quarantine lock only around its
    /// upserts.
    fn complete_by_watcher(&mut self, commit: CommitIndex) -> Outcome<()> {
        let rows = self.commit_rows(commit);
        let result = self.against_staged_flush(move |store| {
            store.record_commit_fully_executed(commit, &rows, &[])
        })?;
        self.storage(result)?;
        self.assigned.remove(&commit);
        Ok(())
    }

    /// The execution watcher: completes the oldest assigned commit once its
    /// roots have effects.
    fn watcher(&mut self) -> Outcome<()> {
        let Some(&commit) = self.assigned.first() else {
            return Ok(());
        };
        if !self.roots_executed(commit) {
            return Ok(());
        }
        self.complete_by_watcher(commit)?;
        self.trace.push(format!("watcher completes {commit}"));
        Ok(())
    }

    /// The handler's wait before validating: every commit at or below the
    /// horizon fully executed. Their roots execute handler-known, since the
    /// handler assigned them; `path` decides which completion reaches each
    /// commit first. Once the frontier is there, the real wait must return at
    /// once.
    fn wait_for_horizon(&mut self, target: CommitIndex, path: WaitPath) -> Outcome<()> {
        while self.frontier() < target {
            let commit = self.frontier() + 1;
            self.execute_roots(commit)?;
            match path {
                WaitPath::Watcher => self.complete_by_watcher(commit)?,
                WaitPath::Flush => {
                    while self.checkpointed_through < commit {
                        if let Err(Crashed) = self.checkpoint(false)? {
                            return self.fail("a checkpoint without a crash point crashed");
                        }
                    }
                    while self.flushed_through < commit {
                        self.flush()?;
                    }
                }
            }
            if self.frontier() < commit {
                return self.fail(format!(
                    "commit {commit} did not complete although its roots executed"
                ));
            }
            self.trace
                .push(format!("wait completes {commit} by {path:?}"));
        }
        if self
            .epoch_store
            .wait_for_fully_executed_commit(target)
            .now_or_never()
            .is_none()
        {
            return self.fail(format!(
                "the wait for commit {target} blocks although the frontier is {}",
                self.frontier()
            ));
        }
        Ok(())
    }

    /// The checkpoint executor on the next checkpoint, which holds the next
    /// commit's roots.
    fn checkpoint(&mut self, crash_after_bookkeeping: bool) -> Outcome<Result<(), Crashed>> {
        self.checkpoint_commits(1, crash_after_bookkeeping)
    }

    /// The checkpoint executor on one checkpoint holding the roots of the
    /// next `commits` commits: executes them, persists the bookkeeping,
    /// commits the outputs, then prunes. Before the switch it waits for the
    /// handler: only commits the handler assigned are in a checkpoint the
    /// node built.
    fn checkpoint_commits(
        &mut self,
        commits: CommitIndex,
        crash_after_bookkeeping: bool,
    ) -> Outcome<Result<(), Crashed>> {
        let history = self.history;
        let first = self.checkpointed_through + 1;
        let last = (first + commits - 1).min(self.executable_through());
        if first > last {
            return Ok(Ok(()));
        }
        let mut roots = Vec::new();
        for commit in first..=last {
            self.execute_roots(commit)?;
            roots.extend(history.oracle.outcome(commit).roots());
        }
        let effects: Vec<TransactionEffects> = roots
            .iter()
            .map(|digest| &self.executed[digest])
            .filter(|executed| !executed.durable)
            .map(|executed| executed.effects.clone())
            .collect();
        let result =
            self.against_staged_flush(move |store| store.persist_checkpoint_bookkeeping(&effects))?;
        self.storage(result)?;
        if crash_after_bookkeeping {
            self.trace.push(format!(
                "checkpoint of commits {first}..={last} bookkeeping written, crash"
            ));
            self.crash()?;
            return Ok(Err(Crashed));
        }
        for digest in &roots {
            let (_, execution) = history
                .execution(digest)
                .expect("every root is an execution of the history");
            self.store.make_durable(execution);
            self.executed
                .get_mut(digest)
                .expect("the root was executed above")
                .durable = true;
        }
        self.checkpointed_through = last;
        self.trace
            .push(format!("checkpoint of commits {first}..={last} committed"));
        // The executor nudges the pruner after every checkpoint.
        self.prune(false);
        Ok(Ok(()))
    }

    /// The quarantine flush of the next commit: writes its rows, locks,
    /// processed keys and resume point in one batch,
    /// retires its saved decisions, then evicts the overlay entries.
    fn flush(&mut self) -> Outcome<()> {
        if self.staged_flush.is_none() {
            self.flush_stage()?;
        }
        self.flush_write()
    }

    fn flush_stage(&mut self) -> Outcome<()> {
        self.flush_stage_commits(1)
    }

    /// The flush up to its batch write, as `commit_with_batch` stages it, for
    /// up to `commits` commits that are validated and checkpointed: for
    /// each, the rows a watcher completion left in the overlay, or else the
    /// rows from effects with the commit completed at the flush.
    fn flush_stage_commits(&mut self, commits: CommitIndex) -> Outcome<()> {
        if self.staged_flush.is_some() {
            return Ok(());
        }
        let first = self.flushed_through + 1;
        let last = (first + commits - 1)
            .min(self.validated_through)
            .min(self.checkpointed_through);
        if first > last {
            return Ok(());
        }
        let tables = self.storage(self.epoch_store.tables())?;
        let mut batch = self.epoch_store.db_batch_for_test();
        let mut staged = Vec::new();
        for commit in first..=last {
            let state = self.epoch_store.handler_object_state_for_testing();
            let (rows, completed_by) = match state.take_completed_commit_rows(commit) {
                Some(rows) => (rows, "watcher"),
                None => {
                    let rows = self.commit_rows(commit);
                    state.complete_commit_at_flush(commit, &rows);
                    (rows, "flush")
                }
            };
            self.storage(state.write_commit_rows_to_batch(&tables, &mut batch, &rows, &[]))?;
            let quarantined = self
                .quarantine
                .get(&commit)
                .expect("a validated commit above the flushed one is quarantined");
            let mut output = ConsensusCommitOutput::new(commit, commit);
            output.set_owned_object_locks(quarantined.locks.clone().into_iter().collect());
            for digest in &quarantined.processed {
                output.record_consensus_message_processed(consensus_key(*digest));
            }
            output.record_consensus_commit_stats(ExecutionIndicesWithStats {
                index: ExecutionIndices {
                    last_committed_round: commit,
                    sub_dag_index: commit,
                    transaction_index: 0,
                },
                ..Default::default()
            });
            self.storage(output.write_to_batch(&self.epoch_store, &mut batch))?;
            self.trace.push(format!(
                "flush {commit} staged (completed by {completed_by})"
            ));
            staged.push((commit, rows));
        }
        self.staged_flush = Some(StagedFlush {
            commits: staged,
            batch,
        });
        Ok(())
    }

    /// Runs `step` on another thread while this one holds the consensus
    /// quarantine write lock, as a staged flush holds it until its write and
    /// eviction. A step that finishes meanwhile takes no quarantine lock and
    /// lands between the flush's staging and its write. Otherwise the flush
    /// writes and evicts first, and `step` finishes once the lock is
    /// released. With no staged flush, `step` just runs.
    fn against_staged_flush<T: Send>(
        &mut self,
        step: impl FnOnce(&AuthorityPerEpochStore) -> T + Send,
    ) -> Outcome<T> {
        let Some(staged) = self.staged_flush.take() else {
            return Ok(step(&self.epoch_store));
        };
        let epoch_store = self.epoch_store.clone();
        let (result, staged, write) = std::thread::scope(|scope| {
            let (start, started) = std::sync::mpsc::channel::<()>();
            let (finish, finished) = std::sync::mpsc::channel::<()>();
            let step_store = epoch_store.clone();
            let handle = scope.spawn(move || {
                started
                    .recv()
                    .expect("the harness starts the step while it holds the lock");
                let result = step(&step_store);
                finish.send(()).ok();
                result
            });
            let mut staged = Some(staged);
            let mut write = None;
            epoch_store.hold_consensus_quarantine_for_testing(|| {
                start.send(()).expect("the step thread waits for the start");
                let ran_first = finished
                    .recv_timeout(std::time::Duration::from_millis(50))
                    .is_ok();
                if !ran_first {
                    let StagedFlush { commits, batch } =
                        staged.take().expect("the staged flush is still pending");
                    write = Some((batch.write().map_err(IotaError::from), commits));
                    if let Some((Ok(()), commits)) = &write {
                        for (commit, rows) in commits {
                            epoch_store
                                .handler_object_state_for_testing()
                                .evict_flushed_commit_rows(*commit, rows, &[]);
                        }
                    }
                }
            });
            let result = handle.join().expect("the step thread does not panic");
            (result, staged, write)
        });
        match write {
            None => {
                self.trace
                    .push("    ran between the flush's staging and its write".to_string());
                self.staged_flush = staged;
            }
            Some((result, commits)) => {
                self.storage(result)?;
                self.finish_flush(&commits)?;
            }
        }
        Ok(result)
    }

    /// Finishes a staged flush before a step that holds the quarantine read
    /// lock throughout, which a flush holding the write lock from staging to
    /// eviction makes wait: the processed and lock checks.
    fn settle_staged_flush(&mut self) -> Outcome<()> {
        if self.staged_flush.is_some() {
            self.flush_write()?;
        }
        Ok(())
    }

    /// The staged flush's batch write and eviction.
    fn flush_write(&mut self) -> Outcome<()> {
        let Some(StagedFlush { commits, batch }) = self.staged_flush.take() else {
            return Ok(());
        };
        self.storage(batch.write().map_err(IotaError::from))?;
        for (commit, rows) in &commits {
            self.epoch_store
                .handler_object_state_for_testing()
                .evict_flushed_commit_rows(*commit, rows, &[]);
        }
        self.finish_flush(&commits)
    }

    /// The harness's side of a written flush: the commits leave the modeled
    /// quarantine, and their saved decisions must be retired.
    fn finish_flush(
        &mut self,
        commits: &[(CommitIndex, Vec<(ObjectKey, HandlerProcessedObject)>)],
    ) -> Outcome<()> {
        for (commit, _) in commits {
            let commit = *commit;
            self.quarantine.remove(&commit);
            self.assigned.remove(&commit);
            self.flushed_through = commit;
            self.trace.push(format!("flush {commit} written"));
            for candidate in &self.history.commits[commit as usize - 1] {
                let saved = self.storage(
                    self.epoch_store
                        .post_consensus_decision(commit, candidate.digest),
                )?;
                if saved.is_some() {
                    return self.fail(format!(
                        "verdict of {} at {commit} survived the commit's flush",
                        candidate.digest
                    ));
                }
            }
        }
        Ok(())
    }

    /// The object pruner. A version consumed at commit `c` can still be kept
    /// by a validation at a commit below `c + K`, and the next validation is
    /// at least the commit after the last flushed one, where a restart
    /// resumes. So only a version consumed at `c` with `c + K` at or below
    /// that commit is pruned, and only once its consumer's checkpoint is
    /// committed.
    fn prune(&mut self, pre_epoch_too: bool) {
        let history = self.history;
        let next_validation = self.flushed_through + 1;
        let consumed: Vec<ObjectKey> = self
            .executed
            .iter()
            .filter(|(digest, executed)| {
                executed.durable
                    && history
                        .execution(digest)
                        .is_some_and(|(commit, _)| commit + history.k() <= next_validation)
            })
            .flat_map(|(_, executed)| {
                executed
                    .effects
                    .old_object_metadata()
                    .into_iter()
                    .map(|owned| ObjectKey::from(*owned.reference()))
            })
            .collect();
        for key in &consumed {
            self.store.prune(key);
        }
        if pre_epoch_too {
            for object in &self.history.retained_pre_epoch {
                self.store.prune(&ObjectKey(object.id(), object.version()));
            }
        }
        self.trace.push(format!(
            "prune {} consumed versions{}",
            consumed.len(),
            if pre_epoch_too {
                " and the retained pre-epoch versions"
            } else {
                ""
            }
        ));
    }

    /// Loses everything in memory and reopens the epoch store, resuming
    /// consensus replay from the last flushed commit.
    fn crash(&mut self) -> Outcome<()> {
        self.store.crash();
        self.executed.retain(|_, executed| executed.durable);
        self.quarantine.clear();
        self.assigned.clear();
        self.staged_flush = None;
        self.validated_through = self.flushed_through;
        let reopened = reopen_when_closed(&self.env.authority, &self.epoch_store);
        *self
            .env
            .epoch_store
            .lock()
            .expect("the epoch store slot is only locked by the harness thread") = reopened.clone();
        self.epoch_store = reopened;
        self.trace.push(format!(
            "crash; resume after {}, checkpoint watermark {}",
            self.flushed_through, self.checkpointed_through
        ));
        if self.frontier() != self.flushed_through {
            return self.fail(format!(
                "the reopened store resumes at {} instead of the flushed commit {}",
                self.frontier(),
                self.flushed_through
            ));
        }
        Ok(())
    }

    // -- validation ------------------------------------------------------------

    /// The consensus handler's processed check: the quarantine, then the
    /// table, under the quarantine read lock.
    fn is_processed(&mut self, digest: TransactionDigest) -> Outcome<bool> {
        self.settle_staged_flush()?;
        if self
            .quarantine
            .values()
            .any(|quarantined| quarantined.processed.contains(&digest))
        {
            return Ok(true);
        }
        self.storage(
            self.epoch_store
                .is_consensus_message_processed(&consensus_key(digest)),
        )
    }

    /// The three lock tiers: this commit, the quarantine under its read
    /// lock, the table.
    fn find_lock(
        &mut self,
        reference: &ObjectReference,
        commit_locks: &BTreeMap<ObjectReference, TransactionDigest>,
    ) -> Outcome<Option<TransactionDigest>> {
        if let Some(holder) = commit_locks.get(reference) {
            return Ok(Some(*holder));
        }
        self.settle_staged_flush()?;
        for quarantined in self.quarantine.values() {
            if let Some(holder) = quarantined.locks.get(reference) {
                return Ok(Some(*holder));
            }
        }
        let tables = self.storage(self.epoch_store.tables())?;
        self.storage(tables.get_locked_transaction(reference))
    }

    /// The candidates of `commit` that reach validation, after the consensus
    /// handler drops same-commit copies and digests already processed. A
    /// deferral reload comes from the deferred table and skips both.
    fn validation_candidates(&mut self, commit: CommitIndex) -> Outcome<Vec<&'a Candidate>> {
        let history = self.history;
        let expected = history.oracle.outcome(commit);
        let mut seen = BTreeSet::new();
        let mut candidates = Vec::new();
        for candidate in &history.commits[commit as usize - 1] {
            if !seen.insert(candidate.digest) {
                continue;
            }
            let duplicate = !candidate.deferral_reload && self.is_processed(candidate.digest)?;
            let expected_duplicate =
                expected.verdict(&candidate.digest) == Some(Verdict::Duplicate);
            if duplicate != expected_duplicate {
                return self.fail(format!(
                    "duplicate filter mismatch at {commit} for {}: processed {duplicate}, oracle \
                     {:?}",
                    describe_candidate(candidate),
                    expected.verdict(&candidate.digest)
                ));
            }
            if duplicate {
                self.trace
                    .push(format!("  {} -> duplicate", describe_candidate(candidate)));
            } else {
                candidates.push(candidate);
            }
        }
        Ok(candidates)
    }

    /// Validates the next commit as `validate_and_resolve_conflicts` does,
    /// with the reader answering every input, and compares each verdict,
    /// each input read and the lock set with the oracle.
    fn validate_next(
        &mut self,
        interleave: &[Interleave],
        wait: WaitPath,
        crash: Option<ValidationCrash>,
    ) -> Outcome<Result<(), Crashed>> {
        let commit = self.validated_through + 1;
        if commit > self.history.last_commit() || self.switched() {
            return Ok(Ok(()));
        }
        // A node restarted after the switch replays up to the commit it
        // switched after, and switches again at the next one, as the consensus
        // handler does.
        let last_before_switch = self.storage(
            self.epoch_store
                .last_commit_before_synced_checkpoint_execution(),
        )?;
        if last_before_switch.is_some_and(|last| commit > last) {
            self.epoch_store.resume_executing_synced_checkpoints();
            self.trace
                .push(format!("switch again at {commit} after the restart"));
            return Ok(Ok(()));
        }
        self.wait_for_horizon(self.horizon(commit), wait)?;
        self.trace.push(format!(
            "validate {commit} (frontier {}, flushed {}, checkpointed {})",
            self.frontier(),
            self.flushed_through,
            self.checkpointed_through
        ));
        let history = self.history;
        let expected = history.oracle.outcome(commit);
        let tables = self.storage(self.epoch_store.tables())?;
        let saved_candidates = self.storage(
            tables
                .post_consensus_decision_candidates
                .get(&commit)
                .map_err(IotaError::from),
        )?;
        drop(tables);
        let candidates = self.validation_candidates(commit)?;
        if let Some(saved_candidates) = &saved_candidates {
            let now: Vec<TransactionDigest> = candidates
                .iter()
                .map(|candidate| candidate.digest)
                .collect();
            if *saved_candidates != now {
                return self.fail(format!(
                    "validation candidates changed at {commit}: saved {saved_candidates:?}, now \
                     {now:?}"
                ));
            }
        }

        let mut commit_locks: BTreeMap<ObjectReference, TransactionDigest> = BTreeMap::new();
        let mut verdicts: Vec<(TransactionDigest, SavedPostConsensusDecision)> = Vec::new();
        let mut processed = Vec::new();
        let mut interleave = interleave.iter();
        for candidate in candidates {
            let saved = self.storage(
                self.epoch_store
                    .post_consensus_decision(commit, candidate.digest),
            )?;
            if saved.is_some() != saved_candidates.is_some() {
                return self.fail(format!(
                    "incomplete saved decisions at {commit}: {} has {saved:?} while the \
                     candidate list is {saved_candidates:?}",
                    candidate.digest
                ));
            }
            let verdict = match &saved {
                // A saved drop is repeated without checks.
                Some(saved) if saved.decision == PostConsensusDecision::Dropped => saved.clone(),
                // A saved keep with effects: no input is read.
                Some(saved)
                    if saved.decision == PostConsensusDecision::Kept
                        && self.executed.contains_key(&candidate.digest) =>
                {
                    for reference in &candidate.owned_inputs {
                        if let Some(holder) = self.find_lock(reference, &commit_locks)? {
                            if holder != candidate.digest {
                                return self.fail(format!(
                                    "executed {} has {reference:?} locked by {holder}",
                                    candidate.digest
                                ));
                            }
                        }
                    }
                    SavedPostConsensusDecision::kept()
                }
                _ => {
                    let mut conflict = None;
                    for reference in &candidate.owned_inputs {
                        if let Some(holder) = self.find_lock(reference, &commit_locks)? {
                            if holder != candidate.digest {
                                conflict = Some(IotaError::ObjectLockConflict {
                                    obj_ref: *reference,
                                    pending_transaction: holder,
                                });
                                break;
                            }
                        }
                    }
                    match conflict {
                        Some(error) => SavedPostConsensusDecision::dropped(
                            PostConsensusDecision::Dropped,
                            &error,
                        ),
                        None => {
                            let this_interleave = interleave.next();
                            match self.read_inputs(commit, candidate, this_interleave)? {
                                Ok(verdict) => verdict,
                                Err(Crashed) => return Ok(Err(Crashed)),
                            }
                        }
                    }
                }
            };
            if let Some(saved) = &saved {
                if *saved != verdict {
                    return self.fail(format!(
                        "saved decision changed at {commit} for {}: saved {saved:?}, now {verdict:?}",
                        describe_candidate(candidate)
                    ));
                }
            }
            let expected_verdict = expected
                .verdict(&candidate.digest)
                .expect("the oracle judged every deduplicated candidate");
            self.trace.push(format!(
                "  {} -> {verdict:?}",
                describe_candidate(candidate)
            ));
            if Some(real_class(&verdict)) != oracle_class(expected_verdict) {
                return self.fail(format!(
                    "verdict mismatch at {commit} for {}: got {verdict:?}, oracle {expected_verdict:?}",
                    describe_candidate(candidate)
                ));
            }
            if verdict.decision == PostConsensusDecision::Kept {
                for reference in &candidate.owned_inputs {
                    commit_locks.insert(*reference, candidate.digest);
                }
                processed.push(candidate.digest);
            }
            verdicts.push((candidate.digest, verdict));
        }
        if commit_locks != expected.locks {
            return self.fail(format!(
                "lock set mismatch at {commit}: got {commit_locks:?}, oracle {:?}",
                expected.locks
            ));
        }
        if crash == Some(ValidationCrash::BeforePersist) {
            self.trace
                .push(format!("crash before saving the verdicts of {commit}"));
            self.crash()?;
            return Ok(Err(Crashed));
        }
        if saved_candidates.is_none() {
            self.storage(
                self.epoch_store
                    .persist_post_consensus_decisions(commit, &verdicts),
            )?;
        }
        if crash == Some(ValidationCrash::AfterPersist) {
            self.trace.push(format!(
                "crash after saving the verdicts of {commit}, before its assignment"
            ));
            self.crash()?;
            return Ok(Err(Crashed));
        }
        let roots = expected.roots();
        self.epoch_store.assign_commit_to_transactions(
            commit,
            roots.iter().copied().map(TransactionKey::Digest).collect(),
        );
        self.assigned.insert(commit);
        self.quarantine.insert(
            commit,
            Quarantined {
                locks: commit_locks,
                processed,
            },
        );
        self.validated_through = commit;
        self.epoch_store.record_handled_commit(commit);
        Ok(Ok(()))
    }

    /// Reads every input of `candidate` at `commit` and compares each answer
    /// with the oracle. The verdict is the first input the reader did not
    /// keep, as the input loader reports it.
    fn read_inputs(
        &mut self,
        commit: CommitIndex,
        candidate: &Candidate,
        interleave: Option<&Interleave>,
    ) -> Outcome<Result<SavedPostConsensusDecision, Crashed>> {
        let inputs = candidate.inputs();
        let expected = self.history.oracle.input_classes(commit, candidate);
        let interleaved_input =
            interleave.map(|interleave| interleave.input as usize % inputs.len());
        let mut verdict = SavedPostConsensusDecision::kept();
        for (index, input) in inputs.iter().enumerate() {
            let writers = match (interleaved_input == Some(index), interleave) {
                (true, Some(interleave)) => interleave.writers.as_slice(),
                _ => &[],
            };
            let ctx = self.reader(commit);
            let horizon = self.horizon(commit);
            let read = match input {
                // A receiving object goes through the owned machine, as in
                // the input loader, with no lock and no sender check after.
                Input::Owned(reference) | Input::Receiving(reference) => {
                    self.read_owned(&ctx, horizon, *reference, writers)?
                }
                Input::Shared(id, initial) => {
                    self.read_shared(&ctx, horizon, *id, *initial, writers)?
                }
                Input::Package(id) => self.read_package(&ctx, horizon, *id, writers)?,
            };
            let read = match read {
                Ok(read) => read,
                Err(Crashed) => return Ok(Err(Crashed)),
            };
            self.trace.push(format!(
                "    read {input:?} -> {:?} ({})",
                read.class, read.reason
            ));
            let visible = read.class == InputClass::Visible;
            let expected_visible = expected[index] == InputClass::Visible;
            if visible != expected_visible {
                return self.fail(format!(
                    "input read mismatch at {commit} for {}: {input:?} read as {:?} ({}), oracle {:?}",
                    describe_candidate(candidate),
                    read.class,
                    read.reason,
                    expected[index]
                ));
            }
            if read.class != expected[index] {
                self.drop_or_missing_differences.push(format!(
                    "commit {commit} {input:?}: reader {:?} ({}), oracle {:?}",
                    read.class, read.reason, expected[index]
                ));
            }
            if !visible && verdict.decision == PostConsensusDecision::Kept {
                let kind = match input {
                    // The loader reports a receiving reference under its
                    // owned kind.
                    Input::Owned(reference) | Input::Receiving(reference) => {
                        InputObjectKind::ImmOrOwnedMoveObject(*reference)
                    }
                    Input::Shared(id, initial) => InputObjectKind::SharedMoveObject {
                        id: *id,
                        initial_shared_version: *initial,
                        mutable: true,
                    },
                    Input::Package(id) => InputObjectKind::MovePackage(*id),
                };
                verdict = SavedPostConsensusDecision::dropped(
                    PostConsensusDecision::Dropped,
                    &kind.object_not_found_error().into(),
                );
            }
        }
        Ok(Ok(verdict))
    }

    /// Runs the writer steps registered for `points`; a crash abandons the
    /// read.
    fn inject(
        &mut self,
        points: &[ReaderPoint],
        writers: &[(ReaderPoint, WriterStep)],
    ) -> Outcome<Result<(), Crashed>> {
        for (at, writer) in writers {
            if points.contains(at) {
                self.trace.push(format!("    inject {writer:?} {at:?}"));
                if let Err(Crashed) = self.writer(*writer)? {
                    return Ok(Err(Crashed));
                }
            }
        }
        Ok(Ok(()))
    }

    /// The bytes at exactly the kept reference, as the reader loads them: a
    /// stored version with another digest drops, and with no bytes, a handler
    /// row of the id above the version drops.
    fn load_bytes(
        &self,
        need_bytes: owned::OwnedReader<owned::NeedBytes>,
    ) -> Outcome<OwnedVerdict> {
        let reference = need_bytes.into_reference();
        let key = ObjectKey(reference.object_id, reference.version);
        if let Some(object) = self.storage(self.store.try_get_object_by_key(&key.0, key.1))? {
            if object.digest() != reference.digest {
                return Ok(OwnedVerdict::Drop(DropReason(
                    DropKind::StoreDigestMismatch,
                )));
            }
            return Ok(OwnedVerdict::Keep(KeptObject(object)));
        }
        if self
            .storage(
                self.epoch_store
                    .lowest_handler_processed_row_above(&key.0, key.1),
            )?
            .is_some()
        {
            return Ok(OwnedVerdict::Drop(DropReason(
                DropKind::HandlerRowAbovePrunedVersion,
            )));
        }
        self.fail(format!(
            "storage error: kept input {key:?} has no bytes in the object store and no handler \
             row above it"
        ))
    }

    /// The owned machine, step by step, with writer steps between the reads;
    /// the same dispatch as `CommitIndexedReader::read_owned`.
    fn read_owned(
        &mut self,
        ctx: &CommitIndexedReader,
        horizon: CommitIndex,
        reference: ObjectReference,
        writers: &[(ReaderPoint, WriterStep)],
    ) -> Outcome<Result<InputRead, Crashed>> {
        use owned::{OwnedReader, TablesLookup, TablesRecheck};

        if writers.is_empty() {
            let verdict = self.storage(ctx.read_owned(reference))?;
            return Ok(Ok(self.owned_read(verdict, reference)?));
        }
        let lookup = self.storage(OwnedReader::start(reference, horizon).read_handler_row(ctx))?;
        if let Err(Crashed) = self.inject(&[ReaderPoint::Row], writers)? {
            return Ok(Err(Crashed));
        }
        let no_entry = match lookup {
            TablesLookup::LiveAtVersion(live_at_version) => {
                return self.owned_rows_above(ctx, live_at_version, reference, writers);
            }
            TablesLookup::Missing(reason) => {
                return Ok(Ok(
                    self.owned_read(OwnedVerdict::Missing(reason), reference)?
                ));
            }
            TablesLookup::Drop(reason) => {
                return Ok(Ok(self.owned_read(OwnedVerdict::Drop(reason), reference)?));
            }
            TablesLookup::NoEntry(no_entry) => no_entry,
        };
        let store_answered = self.storage(no_entry.read_store(ctx))?;
        if let Err(Crashed) = self.inject(&[ReaderPoint::Store], writers)? {
            return Ok(Err(Crashed));
        }
        let verdict = match self.storage(store_answered.reread_handler_row(ctx))? {
            TablesRecheck::LiveAtVersion(live_at_version) => {
                return self.owned_rows_above(ctx, live_at_version, reference, writers);
            }
            TablesRecheck::Superseded(superseded) => {
                let lookup = self.storage(superseded.read_rows_above(ctx))?;
                if let Err(Crashed) = self.inject(&[ReaderPoint::LastTableRead], writers)? {
                    return Ok(Err(Crashed));
                }
                match lookup {
                    owned::RowsAboveLookup::NeedBytes(need_bytes) => self.load_bytes(need_bytes)?,
                    owned::RowsAboveLookup::Drop(reason) => OwnedVerdict::Drop(reason),
                }
            }
            TablesRecheck::NeedBytes(need_bytes) => self.load_bytes(need_bytes)?,
            TablesRecheck::Missing(reason) => OwnedVerdict::Missing(reason),
            TablesRecheck::Drop(reason) => OwnedVerdict::Drop(reason),
        };
        Ok(Ok(self.owned_read(verdict, reference)?))
    }

    /// The rows above a kept owned version, then its bytes.
    fn owned_rows_above(
        &mut self,
        ctx: &CommitIndexedReader,
        live_at_version: owned::OwnedReader<owned::LiveAtVersion>,
        reference: ObjectReference,
        writers: &[(ReaderPoint, WriterStep)],
    ) -> Outcome<Result<InputRead, Crashed>> {
        let lookup = self.storage(live_at_version.read_rows_above(ctx))?;
        if let Err(Crashed) = self.inject(&[ReaderPoint::LastTableRead], writers)? {
            return Ok(Err(Crashed));
        }
        let verdict = match lookup {
            owned::RowsAboveLookup::NeedBytes(need_bytes) => self.load_bytes(need_bytes)?,
            owned::RowsAboveLookup::Drop(reason) => OwnedVerdict::Drop(reason),
        };
        Ok(Ok(self.owned_read(verdict, reference)?))
    }

    /// Reduces an owned verdict to its class, checking on keep that the
    /// loaded bytes are the named version.
    fn owned_read(&self, verdict: OwnedVerdict, reference: ObjectReference) -> Outcome<InputRead> {
        Ok(match verdict {
            OwnedVerdict::Keep(kept) => {
                let object = kept.into_object();
                if object.id() != reference.object_id || object.version() != reference.version {
                    return self.fail(format!(
                        "kept {reference:?} loaded bytes of {:?}",
                        object.object_ref()
                    ));
                }
                if object.digest() != reference.digest {
                    // The input loader's digest check drops the transaction.
                    InputRead {
                        class: InputClass::Stale,
                        reason: "keep, loaded bytes have another digest".to_string(),
                    }
                } else {
                    InputRead {
                        class: InputClass::Visible,
                        reason: "keep".to_string(),
                    }
                }
            }
            OwnedVerdict::Drop(reason) => InputRead {
                class: InputClass::Stale,
                reason: format!("{:?}", reason.kind()),
            },
            OwnedVerdict::Missing(reason) => InputRead {
                class: InputClass::Missing,
                reason: format!("{:?}", reason.kind()),
            },
        })
    }

    /// The shared machine, step by step; the same dispatch as
    /// `CommitIndexedReader::read_shared`.
    fn read_shared(
        &mut self,
        ctx: &CommitIndexedReader,
        horizon: CommitIndex,
        id: ObjectId,
        initial: Version,
        writers: &[(ReaderPoint, WriterStep)],
    ) -> Outcome<Result<InputRead, Crashed>> {
        use shared::{CreatedObjectLookup, SharedReader, SharedTablesLookup, SharedTablesRecheck};

        if writers.is_empty() {
            let verdict = self.storage(ctx.read_shared(id, initial))?;
            return Ok(Ok(shared_read(verdict)));
        }
        let lookup =
            self.storage(SharedReader::start(id, initial, horizon).read_creation_row(ctx))?;
        if let Err(Crashed) = self.inject(&[ReaderPoint::Row], writers)? {
            return Ok(Err(Crashed));
        }
        let absent = match lookup {
            SharedTablesLookup::Created(created) => match self.storage(created.read_object(ctx))? {
                CreatedObjectLookup::Exists => {
                    return Ok(Ok(shared_read(SharedVerdict::Exists)));
                }
                CreatedObjectLookup::Absent(absent) => absent,
            },
            SharedTablesLookup::Missing(reason) => {
                return Ok(Ok(shared_read(SharedVerdict::Missing(reason))));
            }
            SharedTablesLookup::Drop(reason) => {
                return Ok(Ok(shared_read(SharedVerdict::Drop(reason))));
            }
            SharedTablesLookup::NoEntry(no_entry) => {
                let object_answered = self.storage(no_entry.read_object(ctx))?;
                if let Err(Crashed) = self.inject(&[ReaderPoint::Store], writers)? {
                    return Ok(Err(Crashed));
                }
                match self.storage(object_answered.reread_creation_row(ctx))? {
                    SharedTablesRecheck::Created(created) => {
                        match self.storage(created.read_object(ctx))? {
                            CreatedObjectLookup::Exists => {
                                return Ok(Ok(shared_read(SharedVerdict::Exists)));
                            }
                            CreatedObjectLookup::Absent(absent) => absent,
                        }
                    }
                    SharedTablesRecheck::Exists => {
                        return Ok(Ok(shared_read(SharedVerdict::Exists)));
                    }
                    SharedTablesRecheck::Missing(reason) => {
                        return Ok(Ok(shared_read(SharedVerdict::Missing(reason))));
                    }
                    SharedTablesRecheck::Drop(reason) => {
                        return Ok(Ok(shared_read(SharedVerdict::Drop(reason))));
                    }
                    SharedTablesRecheck::Absent(absent) => absent,
                }
            }
        };
        let verdict = match self.check_deletion(ctx, absent, writers)? {
            Ok(verdict) => verdict,
            Err(Crashed) => return Ok(Err(Crashed)),
        };
        Ok(Ok(shared_read(verdict)))
    }

    /// The deletion read of a shared object that is gone: its highest
    /// handler row.
    fn check_deletion(
        &mut self,
        ctx: &CommitIndexedReader,
        absent: shared::SharedReader<shared::ObjectAbsent>,
        writers: &[(ReaderPoint, WriterStep)],
    ) -> Outcome<Result<SharedVerdict, Crashed>> {
        let lookup = self.storage(absent.read_deletion_row(ctx))?;
        if let Err(Crashed) = self.inject(&[ReaderPoint::LastTableRead], writers)? {
            return Ok(Err(Crashed));
        }
        Ok(Ok(match lookup {
            shared::DeletionRowLookup::Deleted(version, digest) => {
                SharedVerdict::Deleted(version, digest)
            }
            shared::DeletionRowLookup::Drop(reason) => SharedVerdict::Drop(reason),
            shared::DeletionRowLookup::Missing(reason) => SharedVerdict::Missing(reason),
        }))
    }

    /// The package machine, step by step; the same dispatch as
    /// `CommitIndexedReader::read_package`. The machine has one store read
    /// and one row read, so every point folds onto the gap between them.
    fn read_package(
        &mut self,
        ctx: &CommitIndexedReader,
        horizon: CommitIndex,
        id: ObjectId,
        writers: &[(ReaderPoint, WriterStep)],
    ) -> Outcome<Result<InputRead, Crashed>> {
        use package::{PackageLookup, PackageReader, PackageRowLookup};

        if writers.is_empty() {
            let verdict = self.storage(ctx.read_package(id))?;
            return Ok(Ok(package_read(verdict)));
        }
        let loaded = match self.storage(PackageReader::start(id, horizon).read_package(ctx))? {
            PackageLookup::Loaded(loaded) => loaded,
            PackageLookup::Missing(reason) => {
                return Ok(Ok(package_read(PackageVerdict::Missing(reason))));
            }
        };
        let points = [
            ReaderPoint::Store,
            ReaderPoint::Row,
            ReaderPoint::LastTableRead,
        ];
        if let Err(Crashed) = self.inject(&points, writers)? {
            return Ok(Err(Crashed));
        }
        let verdict = match self.storage(loaded.read_row(ctx))? {
            PackageRowLookup::Visible(package) => PackageVerdict::Visible(package),
            PackageRowLookup::Missing(reason) => PackageVerdict::Missing(reason),
        };
        Ok(Ok(package_read(verdict)))
    }

    // -- invariants ------------------------------------------------------------

    /// The bookkeeping invariant: no readable object version an execution of
    /// an assigned commit produced without its row. Verdict retirement is
    /// checked at every flush and the keep-path bytes where the reader keeps.
    fn check_invariants(&self) -> Outcome<()> {
        for key in self.store.bookkept_keys() {
            let row = self.storage(self.epoch_store.handler_processed_object(&key))?;
            if row.is_none() {
                return self.fail(format!("{key:?} is readable without a handler row"));
            }
        }
        Ok(())
    }

    /// After the switch the epoch ends with commits never flushed, and their
    /// saved decisions go with the epoch's tables.
    fn final_checks(&self) -> Outcome<()> {
        if self.switched() {
            return Ok(());
        }
        let tables = self.storage(self.epoch_store.tables())?;
        let verdicts = tables.post_consensus_decisions.safe_iter().next();
        if let Some(entry) = verdicts {
            let entry = self.storage(entry.map_err(IotaError::from))?;
            return self.fail(format!(
                "verdict {entry:?} survived although every commit flushed"
            ));
        }
        let candidates = tables.post_consensus_decision_candidates.safe_iter().next();
        if candidates.is_some() {
            return self.fail("a candidate list survived although every commit flushed");
        }
        if !self.assigned.is_empty() {
            return self.fail(format!(
                "commits {:?} stayed assigned after their flush",
                self.assigned
            ));
        }
        Ok(())
    }
}

fn shared_read(verdict: SharedVerdict) -> InputRead {
    match verdict {
        SharedVerdict::Exists => InputRead {
            class: InputClass::Visible,
            reason: "exists".to_string(),
        },
        SharedVerdict::Deleted(version, _) => InputRead {
            class: InputClass::Visible,
            reason: format!(
                "deleted above the horizon or ahead of the handler at {}",
                version.as_u64()
            ),
        },
        SharedVerdict::Drop(reason) => InputRead {
            class: InputClass::Stale,
            reason: format!("{:?}", reason.kind()),
        },
        SharedVerdict::Missing(reason) => InputRead {
            class: InputClass::Missing,
            reason: format!("{:?}", reason.kind()),
        },
    }
}

fn package_read(verdict: PackageVerdict) -> InputRead {
    match verdict {
        PackageVerdict::Visible(_) => InputRead {
            class: InputClass::Visible,
            reason: "visible".to_string(),
        },
        PackageVerdict::Missing(reason) => InputRead {
            class: InputClass::Missing,
            reason: format!("{:?}", reason.kind()),
        },
    }
}

// ---------------------------------------------------------------------------
// Properties
// ---------------------------------------------------------------------------

/// Turns deterministic validation on, with the horizon distance read from
/// `horizon_distance` each time a protocol config is built.
fn pcool_flags(horizon_distance: Arc<AtomicU64>) -> OverrideGuard {
    ProtocolConfig::apply_overrides_for_testing(move |_, mut config| {
        config.enable_pcool_deterministic_validation_for_testing();
        config.set_pcool_deterministic_validation_horizon_distance_for_testing(
            horizon_distance.load(Ordering::SeqCst),
        );
        config
    })
}

fn run_case(env: &Env, history: &History, schedule: &Schedule) -> Outcome<Vec<String>> {
    let mut validator = match Validator::new(history, env) {
        Ok(validator) => validator,
        Err(error) => return Err(TestCaseError::fail(format!("reset failed: {error}"))),
    };
    validator.run_schedule(schedule)?;
    Ok(validator.drop_or_missing_differences)
}

fn case_count() -> u32 {
    std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|cases| cases.parse().ok())
        .unwrap_or(64)
}

/// The seed of the random run: `PROPTEST_RNG_SEED` when set, else 1, so a
/// run without it is the same every time.
fn rng_seed() -> u64 {
    std::env::var("PROPTEST_RNG_SEED")
        .ok()
        .and_then(|seed| seed.parse().ok())
        .unwrap_or(1)
}

/// Random histories under random local schedules: every verdict, input
/// read, lock set and the bookkeeping invariant match the oracle; replays
/// reproduce the saved verdicts; verdicts retire with their commit's flush.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn random_histories_and_schedules_match_the_oracle() {
    let horizon_distance = Arc::new(AtomicU64::new(2));
    let _flags = pcool_flags(horizon_distance.clone());
    let env = Env::new(horizon_distance).await;
    let seed = rng_seed();
    let cases = case_count();
    eprintln!("random histories: {cases} cases from PROPTEST_RNG_SEED={seed}");
    tokio::task::block_in_place(|| {
        let drop_or_missing_differences = Mutex::new(BTreeMap::<String, usize>::new());
        let mut runner = TestRunner::new(Config {
            cases,
            max_shrink_iters: 4096,
            failure_persistence: None,
            rng_seed: RngSeed::Fixed(seed),
            ..Config::default()
        });
        // Counts every run, shrinking included, so the first failure's count
        // is the number of cases it took to find it.
        let runs = AtomicU64::new(0);
        let first_failure = AtomicU64::new(0);
        let result = runner.run(&(history_spec(), schedule()), |(spec, schedule)| {
            let run = runs.fetch_add(1, Ordering::SeqCst) + 1;
            let history = build_history(&spec);
            let mismatches = run_case(&env, &history, &schedule).inspect_err(|_| {
                let _ = first_failure.compare_exchange(0, run, Ordering::SeqCst, Ordering::SeqCst);
            })?;
            let mut counts = drop_or_missing_differences
                .lock()
                .expect("the mismatch counter is only locked by the harness thread");
            for mismatch in mismatches {
                // Keyed by the reader's and the oracle's classes, not the
                // commit and input.
                let key = mismatch
                    .split_once(": ")
                    .map(|(_, classes)| classes.to_string())
                    .unwrap_or(mismatch);
                *counts.entry(key).or_default() += 1;
            }
            Ok(())
        });
        let counts = drop_or_missing_differences
            .lock()
            .expect("the mismatch counter is only locked by the harness thread");
        eprintln!(
            "drop-versus-missing class differences between the reader and the oracle (same \
             verdict): {counts:#?}"
        );
        if let Err(error) = result {
            panic!(
                "{error}\n\nfirst failure at case {}\nreplay with PROPTEST_RNG_SEED={seed} \
                 PROPTEST_CASES={cases}",
                first_failure.load(Ordering::SeqCst)
            );
        }
    });
}

// ---------------------------------------------------------------------------
// Known scenarios, as explicit histories and schedules
// ---------------------------------------------------------------------------

fn owned_object(index: u8) -> ObjectId {
    object_id(0xA0, index)
}

/// `n` owned coins at the epoch start version.
fn epoch_start_with_owned(n: u8) -> State {
    let mut state = State::default();
    for index in 0..n {
        let id = owned_object(index);
        state.slots.insert(
            id,
            Slot::Live(coin(
                id,
                Version::from_u64(EPOCH_START_VERSION),
                Owner::Address(SENDER),
            )),
        );
    }
    state
}

fn live_reference(state: &State, id: ObjectId) -> ObjectReference {
    state
        .live(&id)
        .expect("the object is live in the state")
        .object_ref()
}

fn candidate(
    commit: CommitIndex,
    index: usize,
    owned_inputs: Vec<ObjectReference>,
    writes: Vec<Write>,
) -> Candidate {
    Candidate {
        digest: transaction_digest(commit, index),
        owned_inputs,
        shared_inputs: Vec::new(),
        read_only_shared: Vec::new(),
        packages: Vec::new(),
        receiving: Vec::new(),
        writes,
        fails: false,
        defer: false,
        deferral_reload: false,
    }
}

fn validate() -> Step {
    Step::Validate {
        interleave: Vec::new(),
        wait: WaitPath::Watcher,
        crash: None,
    }
}

fn validates(n: usize) -> Vec<Step> {
    std::iter::repeat_with(validate).take(n).collect()
}

fn validate_interleaved(input: u8, writers: Vec<(ReaderPoint, WriterStep)>) -> Step {
    Step::Validate {
        interleave: vec![Interleave { input, writers }],
        wait: WaitPath::Watcher,
        crash: None,
    }
}

fn writer(step: WriterStep) -> Step {
    Step::Writer(step)
}

fn checkpoints(n: usize) -> Vec<Step> {
    std::iter::repeat_n(
        writer(WriterStep::Checkpoint {
            crash_after_bookkeeping: false,
        }),
        n,
    )
    .collect()
}

/// The version `object` takes from the execution at `commit` that changes it.
fn produced(oracle: &Oracle, commit: CommitIndex, object: ObjectId) -> ObjectReference {
    oracle
        .outcome(commit)
        .executions
        .iter()
        .flat_map(|execution| execution.changes.iter())
        .find(|change| change.id == object)
        .and_then(|change| change.after.live())
        .expect("an execution of the commit produced the object")
        .object_ref()
}

async fn run_scenario(name: &str, history: &History, steps: Vec<Step>) {
    let horizon_distance = Arc::new(AtomicU64::new(history.k()));
    let _flags = pcool_flags(horizon_distance.clone());
    let env = Env::new(horizon_distance).await;
    tokio::task::block_in_place(|| {
        let schedule = Schedule { steps };
        if let Err(error) = run_case(&env, history, &schedule) {
            panic!("scenario {name} failed: {error}");
        }
    });
}

/// The history behind review problems R1 to R3: producer `P` at commit 11
/// creates `X`; consumer `T` names `X` at commit 12, where the horizon is
/// 10, and again at commit 13, where it is 11.
fn producer_consumer_history() -> History {
    let epoch_start = epoch_start_with_owned(2);
    let gas = owned_object(0);
    let other_gas = owned_object(1);
    let created = object_id(0xA1, 0);
    let mut commits: Vec<Vec<Candidate>> = vec![Vec::new(); 10];
    let producer = candidate(
        11,
        0,
        vec![live_reference(&epoch_start, gas)],
        vec![Write::CreateOwned(created)],
    );
    commits.push(vec![producer]);
    let mut oracle = Oracle::new(epoch_start.clone(), 2);
    for commit in &commits {
        oracle.judge_commit(commit);
    }
    let consumer = candidate(
        12,
        0,
        vec![
            live_reference(&epoch_start, other_gas),
            produced(&oracle, 11, created),
        ],
        Vec::new(),
    );
    commits.push(vec![consumer.clone()]);
    commits.push(vec![consumer]);
    History::judge(2, epoch_start, Vec::new(), commits)
}

/// Review problem R1, with the producer of `X` executed while the
/// consumer's read of `X` at commit 12 is in flight, after `executed_at`, and
/// its commit completed after the store read, before the row re-read.
async fn completion_between_the_final_reads(executed_at: ReaderPoint) {
    let history = producer_consumer_history();
    let mut steps = validates(11);
    steps.push(validate_interleaved(
        1,
        vec![
            (executed_at, WriterStep::Execute),
            (ReaderPoint::Store, WriterStep::Watcher),
        ],
    ));
    run_scenario(
        &format!("R1, executed after {executed_at:?}"),
        &history,
        steps,
    )
    .await;
}

/// R1 with the first row read missed before the producer executes: the read
/// must answer missing.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn review_1_completion_between_the_final_reads() {
    completion_between_the_final_reads(ReaderPoint::Row).await;
}

/// R1 with the store answer taken before the producer executes: the read
/// must answer missing.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn review_1_completion_after_the_store_answer() {
    completion_between_the_final_reads(ReaderPoint::Store).await;
}

/// Review problem R3: the producer's checkpoint commits its outputs before
/// commit 11 flushes; a crash then replays. Commit 12, validated for the
/// first time after the restart, must find the producer's durable row and
/// answer missing, not take the object for epoch-start state.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn review_3_bookkeeping_durable_before_outputs() {
    let history = producer_consumer_history();
    let mut steps = validates(11);
    steps.push(writer(WriterStep::Execute));
    steps.extend(checkpoints(11));
    steps.push(Step::Crash);
    steps.push(validate());
    steps.push(validate());
    run_scenario("R3", &history, steps).await;
}

/// The history behind review problems R4 and R5: handler-known `T` at commit
/// 12 consumes the epoch-start version of `G`, and a contender `Q` names the
/// same version at the same commit.
fn consumed_epoch_start_history() -> History {
    let epoch_start = epoch_start_with_owned(3);
    let gas = owned_object(0);
    let other_gas = owned_object(1);
    let object = owned_object(2);
    let mut commits: Vec<Vec<Candidate>> = vec![Vec::new(); 11];
    commits.push(vec![
        candidate(
            12,
            0,
            vec![
                live_reference(&epoch_start, gas),
                live_reference(&epoch_start, object),
            ],
            Vec::new(),
        ),
        candidate(
            12,
            1,
            vec![
                live_reference(&epoch_start, other_gas),
                live_reference(&epoch_start, object),
            ],
            Vec::new(),
        ),
    ]);
    History::judge(2, epoch_start, Vec::new(), commits)
}

/// Review problem R4: commit 12 executes and checkpoints but does not flush
/// before a crash; its replay must keep `T` and drop `Q` from the saved
/// decisions, although the store's latest version of `G` is now above the
/// one both name.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn review_4_replay_of_a_consumed_epoch_start_input() {
    let history = consumed_epoch_start_history();
    let mut steps = validates(12);
    steps.push(writer(WriterStep::Execute));
    steps.extend(checkpoints(12));
    steps.push(Step::Crash);
    steps.push(validate());
    run_scenario("R4", &history, steps).await;
}

/// Review problem R5: like R4 with the consumed version pruned before the
/// crash; the replay must need no bytes of it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn review_5_replay_after_pruning_the_consumed_input() {
    let history = consumed_epoch_start_history();
    let mut steps = validates(12);
    steps.push(writer(WriterStep::Execute));
    steps.extend(checkpoints(12));
    steps.push(writer(WriterStep::Prune {
        pre_epoch_too: true,
    }));
    steps.push(Step::Crash);
    steps.push(validate());
    run_scenario("R5", &history, steps).await;
}

/// A pre-epoch shared object deleted at commit 3, and a candidate naming it
/// at commit 5, where the horizon is 3: the deletion is visible, so the
/// candidate drops.
fn shared_deletion_history() -> History {
    let mut epoch_start = epoch_start_with_owned(2);
    let shared = object_id(0xA5, 0);
    let initial = Version::from_u64(1);
    epoch_start.slots.insert(
        shared,
        Slot::Live(coin(
            shared,
            Version::from_u64(RETAINED_VERSION),
            Owner::Shared(initial),
        )),
    );
    let mut commits: Vec<Vec<Candidate>> = vec![Vec::new(); 2];
    let deleter = Candidate {
        shared_inputs: vec![(shared, initial)],
        writes: vec![Write::DeleteShared(shared)],
        ..candidate(
            3,
            0,
            vec![live_reference(&epoch_start, owned_object(0))],
            Vec::new(),
        )
    };
    commits.push(vec![deleter]);
    commits.push(Vec::new());
    let contender = Candidate {
        shared_inputs: vec![(shared, initial)],
        ..candidate(
            5,
            0,
            vec![live_reference(&epoch_start, owned_object(1))],
            Vec::new(),
        )
    };
    commits.push(vec![contender]);
    History::judge(2, epoch_start, Vec::new(), commits)
}

/// The deletion executes handler-known and its commit is completed by the
/// wait before commit 5: the contender reads the deletion row and drops.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn shared_deletion_by_the_handler_drops_a_later_candidate() {
    let history = shared_deletion_history();
    run_scenario("shared deletion, handler-known", &history, validates(5)).await;
}

/// A receivable object `R` at the epoch start version, with its previous
/// version retained. `T1` at commit 11 receives `R`. `T2` at commit 12,
/// where the horizon is 10, names `R` at `named` as an owned input.
fn received_epoch_start_history(named: u64) -> History {
    let received = object_id(0xA2, 0);
    let mut epoch_start = epoch_start_with_owned(2);
    epoch_start.slots.insert(
        received,
        Slot::Live(coin(
            received,
            Version::from_u64(EPOCH_START_VERSION),
            Owner::Address(SENDER),
        )),
    );
    let retained = vec![coin(
        received,
        Version::from_u64(RETAINED_VERSION),
        Owner::Address(SENDER),
    )];
    let mut commits: Vec<Vec<Candidate>> = vec![Vec::new(); 10];
    commits.push(vec![Candidate {
        receiving: vec![live_reference(&epoch_start, received)],
        ..candidate(
            11,
            0,
            vec![live_reference(&epoch_start, owned_object(0))],
            Vec::new(),
        )
    }]);
    let named = coin(received, Version::from_u64(named), Owner::Address(SENDER)).object_ref();
    commits.push(vec![candidate(
        12,
        0,
        vec![live_reference(&epoch_start, owned_object(1)), named],
        Vec::new(),
    )]);
    History::judge(2, epoch_start, retained, commits)
}

/// The retained version of `R` before the epoch start version stays stale at
/// commit 12 whether the validator executed the receive or not: the receive's
/// row above it replaced another version.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn version_older_than_a_received_one_stays_stale() {
    let history = received_epoch_start_history(RETAINED_VERSION);
    let mut executed = validates(11);
    executed.push(writer(WriterStep::Execute));
    executed.push(validate());
    run_scenario(
        "stale version below a receive, executed",
        &history,
        executed,
    )
    .await;
    run_scenario(
        "stale version below a receive, not executed",
        &history,
        validates(12),
    )
    .await;
}

/// `T1` at commit 1 spends `G0`; `T2` at commit 2 spends `G1` and the epoch
/// start version of `X`; K is 1.
fn switch_history() -> History {
    let epoch_start = epoch_start_with_owned(3);
    let first = candidate(
        1,
        0,
        vec![live_reference(&epoch_start, owned_object(0))],
        Vec::new(),
    );
    let second = candidate(
        2,
        0,
        vec![
            live_reference(&epoch_start, owned_object(1)),
            live_reference(&epoch_start, owned_object(2)),
        ],
        Vec::new(),
    );
    History::judge(1, epoch_start, Vec::new(), vec![vec![first], vec![second]])
}

/// The node validates commit 1, switches to executing synced checkpoints,
/// executes and commits both checkpoints, `T2` without a handler row, and
/// crashes.
fn switch_then_crash_steps() -> Vec<Step> {
    let mut steps = vec![validate(), Step::Switch];
    steps.extend(checkpoints(2));
    steps.push(Step::Crash);
    steps.extend(validates(2));
    steps
}

/// After the restart the node replays commit 1, the last one it processed
/// before the switch, and switches again at commit 2 instead of validating it
/// against the store synced execution advanced without handler rows, where
/// `T2`'s own output supersedes the epoch start version of `G1` it names.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn restart_after_the_switch_replays_only_up_to_the_switch() {
    run_scenario(
        "restart after the switch",
        &switch_history(),
        switch_then_crash_steps(),
    )
    .await;
}
