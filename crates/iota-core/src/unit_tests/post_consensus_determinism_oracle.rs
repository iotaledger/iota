// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The oracle for P-COOL deterministic validation: the verdict a validator
//! with perfect knowledge gives at commit `C`, as a pure function of the
//! consensus history and the epoch-start state. It knows nothing about
//! execution progress, state sync, crashes, caches or pruning, which is what
//! makes it the specification the real code is checked against.
//!
//! The model keeps, per commit `c`, the object state `S_c` after every
//! transaction that executes at `c`, the lock table `L_c` and the processed
//! set `P_c`. A candidate at commit `C` is judged in the commit's order:
//!
//! 1. a same-commit duplicate of an earlier candidate is silently dropped;
//! 2. a digest already processed, other than a deferral reload, is a duplicate: it never reaches
//!    validation and has no saved decision;
//! 3. an owned input locked by another digest, counting locks taken earlier in this commit, drops
//!    the candidate (lock conflict); a lock the candidate itself holds, from the commit that
//!    deferred it, is no conflict;
//! 4. every input is checked against `S_{C-K}`: an owned input is visible only if the state has
//!    exactly the named version and digest live, stale if the state has a newer version or a
//!    tombstone at or above it, and missing if the version is not produced yet or the id is
//!    unknown; a shared input is visible if the object is live with the declared initial shared
//!    version, stale if it is live otherwise or deleted, and missing if it is absent; a package is
//!    visible if present and missing otherwise. A shared object deleted above the horizon is
//!    therefore visible, and its transaction executes against the deletion. Receiving objects are
//!    not checked, as at a commit they are checked by reference only;
//! 5. a candidate with every input visible is kept: its owned inputs are locked, its digest is
//!    processed, and unless deferred it executes at `C`. Receiving references take no lock.
//!
//! Content checks (gas balance, ownership, deny rules) are not modeled: the
//! synthetic harness has no content, and the real-execution level exercises
//! them through the production checks.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

use iota_sdk_types::{
    Address, ObjectDigest, ObjectId, ObjectReference, Owner, TransactionDigest, Version,
};
use iota_types::object::Object;
use move_binary_format::CompiledModule;

use crate::authority::authority_per_epoch_store::handler_object_state::CommitIndex;

/// The one address every synthetic object is owned by and every candidate
/// is sent from.
pub const SENDER: Address = Address::new([0x5E; 32]);

/// What the state holds for an object id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Slot {
    Live(Object),
    Deleted(Version),
    Wrapped(Version),
}

impl Slot {
    pub fn version(&self) -> Version {
        match self {
            Slot::Live(object) => object.version(),
            Slot::Deleted(version) | Slot::Wrapped(version) => *version,
        }
    }

    pub fn live(&self) -> Option<&Object> {
        match self {
            Slot::Live(object) => Some(object),
            Slot::Deleted(_) | Slot::Wrapped(_) => None,
        }
    }
}

/// The object state as of one commit.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub slots: BTreeMap<ObjectId, Slot>,
}

impl State {
    pub fn live(&self, id: &ObjectId) -> Option<&Object> {
        self.slots.get(id).and_then(Slot::live)
    }
}

/// What a kept candidate does beyond mutating every consumed input, which is
/// the default. Writes naming an object the candidate does not consume, or
/// one the state cannot support (unwrapping an object that is not wrapped,
/// creating an id that exists), are ignored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Write {
    Delete(ObjectId),
    Wrap(ObjectId),
    DeleteShared(ObjectId),
    CreateOwned(ObjectId),
    CreateShared(ObjectId),
    Unwrap(ObjectId),
    Publish(ObjectId),
}

/// A candidate as consensus delivers it: what it names and what it does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub digest: TransactionDigest,
    /// The gas coin first, then the other owned inputs.
    pub owned_inputs: Vec<ObjectReference>,
    /// `(id, declared initial shared version)`.
    pub shared_inputs: Vec<(ObjectId, Version)>,
    /// The shared inputs taken by immutable reference: never mutated or
    /// deleted, and no deletion marker when already deleted.
    pub read_only_shared: Vec<ObjectId>,
    pub packages: Vec<ObjectId>,
    /// Objects received at the named versions. They take no lock, and no
    /// check at the commit reads them.
    pub receiving: Vec<ObjectReference>,
    pub writes: Vec<Write>,
    /// The execution fails: it only charges gas, mutating its mutable inputs
    /// and nothing else.
    pub fails: bool,
    /// Congestion control defers the kept candidate to the next commit
    /// instead of executing it at this one.
    pub defer: bool,
    /// This occurrence is the intentional reload of a deferred transaction.
    pub deferral_reload: bool,
}

/// One input of a candidate, as the oracle and the reader identify it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Input {
    Owned(ObjectReference),
    Shared(ObjectId, Version),
    Package(ObjectId),
}

impl Candidate {
    pub fn inputs(&self) -> Vec<Input> {
        self.owned_inputs
            .iter()
            .map(|reference| Input::Owned(*reference))
            .chain(
                self.shared_inputs
                    .iter()
                    .map(|(id, initial)| Input::Shared(*id, *initial)),
            )
            .chain(self.packages.iter().map(|id| Input::Package(*id)))
            .collect()
    }
}

/// Rule 4 for one input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputClass {
    Visible,
    Stale,
    Missing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropClass {
    Lock,
    Stale,
    Missing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Kept,
    Duplicate,
    Dropped(DropClass),
}

/// How an execution changed one object, the write kind its effects record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Mutate,
    Delete,
    Wrap,
    Create,
    Unwrap,
    Publish,
}

/// One object the execution of a kept candidate changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectChange {
    pub id: ObjectId,
    pub kind: ChangeKind,
    /// The consumed input; `None` for a creation, an unwrap or a publish.
    pub before: Option<Object>,
    pub after: Slot,
}

/// The execution of a kept candidate: what every other validator's
/// execution of it must produce.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Execution {
    pub digest: TransactionDigest,
    pub gas: ObjectId,
    pub lamport: Version,
    pub changes: Vec<ObjectChange>,
    /// Mutable shared inputs already deleted when the transaction executed.
    /// The execution aborts and writes a deletion marker for each at
    /// `lamport`.
    pub deleted_shared_inputs: Vec<ObjectId>,
    /// Read-only shared inputs and what the state held for each at
    /// execution.
    pub read_only_shared: Vec<(ObjectId, Slot)>,
    /// The execution ended with a failure status.
    pub failed: bool,
}

impl Execution {
    /// The consumed input versions, the objects the execution superseded.
    pub fn consumed(&self) -> impl Iterator<Item = &Object> {
        self.changes
            .iter()
            .filter_map(|change| change.before.as_ref())
    }
}

/// What the oracle decided for one commit.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommitOutcome {
    /// One verdict per candidate that survived same-commit deduplication, in
    /// order.
    pub verdicts: Vec<(TransactionDigest, Verdict)>,
    /// The locks this commit's kept candidates took.
    pub locks: BTreeMap<ObjectReference, TransactionDigest>,
    /// The executions at this commit, in order: kept candidates not deferred,
    /// including reloads.
    pub executions: Vec<Execution>,
}

impl CommitOutcome {
    /// The digests that execute at this commit, which is what the handler
    /// registers as the commit's roots.
    pub fn roots(&self) -> Vec<TransactionDigest> {
        self.executions
            .iter()
            .map(|execution| execution.digest)
            .collect()
    }

    pub fn verdict(&self, digest: &TransactionDigest) -> Option<Verdict> {
        self.verdicts
            .iter()
            .find(|(candidate, _)| candidate == digest)
            .map(|(_, verdict)| *verdict)
    }
}

/// The consensus history as the oracle has judged it so far.
#[derive(Clone, Debug)]
pub struct Oracle {
    /// Horizon distance: a verdict at `C` reads `S_{C-K}`.
    pub k: CommitIndex,
    /// `states[c]` is `S_c`; `states[0]` is the epoch start.
    states: Vec<State>,
    locks: BTreeMap<ObjectReference, TransactionDigest>,
    processed: BTreeSet<TransactionDigest>,
    pub outcomes: Vec<CommitOutcome>,
}

impl Oracle {
    pub fn new(epoch_start: State, k: CommitIndex) -> Self {
        assert!(k >= 1, "a verdict must never read the commit it validates");
        Self {
            k,
            states: vec![epoch_start],
            locks: BTreeMap::new(),
            processed: BTreeSet::new(),
            outcomes: Vec::new(),
        }
    }

    /// The index of the next commit to judge; commits are dense from 1.
    pub fn next_commit(&self) -> CommitIndex {
        self.outcomes.len() as CommitIndex + 1
    }

    /// `S_c`; `c` must not exceed the last judged commit.
    pub fn state(&self, commit: CommitIndex) -> &State {
        &self.states[commit as usize]
    }

    /// The state a verdict at `commit` reads: `S_{C-K}`.
    pub fn horizon_state(&self, commit: CommitIndex) -> &State {
        self.state(commit.saturating_sub(self.k))
    }

    pub fn outcome(&self, commit: CommitIndex) -> &CommitOutcome {
        &self.outcomes[commit as usize - 1]
    }

    /// Rule 4 for every input of `candidate` at `commit`, in input order.
    pub fn input_classes(&self, commit: CommitIndex, candidate: &Candidate) -> Vec<InputClass> {
        let state = self.horizon_state(commit);
        candidate
            .inputs()
            .iter()
            .map(|input| input_class(state, input))
            .collect()
    }

    /// Judges the next commit's candidates in order and advances the state.
    pub fn judge_commit(&mut self, candidates: &[Candidate]) -> &CommitOutcome {
        let commit = self.next_commit();
        let mut seen = BTreeSet::new();
        let mut outcome = CommitOutcome::default();
        let mut state = self.states[commit as usize - 1].clone();
        for candidate in candidates {
            if !seen.insert(candidate.digest) {
                continue;
            }
            let verdict = self.verdict(commit, &outcome.locks, candidate);
            outcome.verdicts.push((candidate.digest, verdict));
            if verdict != Verdict::Kept {
                continue;
            }
            for reference in &candidate.owned_inputs {
                outcome.locks.insert(*reference, candidate.digest);
                self.locks.insert(*reference, candidate.digest);
            }
            self.processed.insert(candidate.digest);
            if !candidate.defer {
                let execution = execute(&mut state, candidate);
                outcome.executions.push(execution);
            }
        }
        self.states.push(state);
        self.outcomes.push(outcome);
        self.outcomes.last().expect("the outcome was just pushed")
    }

    fn verdict(
        &self,
        commit: CommitIndex,
        commit_locks: &BTreeMap<ObjectReference, TransactionDigest>,
        candidate: &Candidate,
    ) -> Verdict {
        if self.processed.contains(&candidate.digest) && !candidate.deferral_reload {
            return Verdict::Duplicate;
        }
        for reference in &candidate.owned_inputs {
            let holder = commit_locks
                .get(reference)
                .or_else(|| self.locks.get(reference));
            if holder.is_some_and(|holder| *holder != candidate.digest) {
                return Verdict::Dropped(DropClass::Lock);
            }
        }
        for class in self.input_classes(commit, candidate) {
            match class {
                InputClass::Visible => {}
                InputClass::Stale => return Verdict::Dropped(DropClass::Stale),
                InputClass::Missing => return Verdict::Dropped(DropClass::Missing),
            }
        }
        Verdict::Kept
    }
}

/// Rule 4 for one input against the state at the horizon.
pub fn input_class(state: &State, input: &Input) -> InputClass {
    match input {
        Input::Owned(reference) => match state.slots.get(&reference.object_id) {
            None => InputClass::Missing,
            Some(Slot::Live(object)) => match object.version().cmp(&reference.version) {
                std::cmp::Ordering::Less => InputClass::Missing,
                std::cmp::Ordering::Greater => InputClass::Stale,
                std::cmp::Ordering::Equal if object.digest() == reference.digest => {
                    InputClass::Visible
                }
                std::cmp::Ordering::Equal => InputClass::Stale,
            },
            Some(Slot::Deleted(version) | Slot::Wrapped(version)) => {
                if *version >= reference.version {
                    InputClass::Stale
                } else {
                    InputClass::Missing
                }
            }
        },
        Input::Shared(id, declared) => match state.slots.get(id) {
            None => InputClass::Missing,
            Some(Slot::Live(object)) => match object.owner {
                Owner::Shared(initial) if initial == *declared => InputClass::Visible,
                _ => InputClass::Stale,
            },
            Some(Slot::Deleted(_) | Slot::Wrapped(_)) => InputClass::Stale,
        },
        Input::Package(id) => match state.live(id) {
            Some(object) if object.is_package() => InputClass::Visible,
            _ => InputClass::Missing,
        },
    }
}

/// Executes a kept candidate against `state`, the state at its commit before
/// it, and applies the changes. An owned input is consumed at the named
/// version: a consumer between the horizon and the commit that named the
/// version as an owned input took a lock that would have dropped the
/// candidate. A receive takes no lock, so an owned input it consumed is not
/// live here, which fails the execution and leaves that object as it is. A shared input already
/// deleted at execution, a receiving object not live at its named version, or
/// a failing transaction, aborts: it only charges gas, mutating every live
/// owned input and every live mutable shared input.
fn execute(state: &mut State, candidate: &Candidate) -> Execution {
    let is_live_at = |state: &State, reference: &ObjectReference| {
        state
            .live(&reference.object_id)
            .is_some_and(|object| object.object_ref() == *reference)
    };
    let gas = candidate.owned_inputs[0].object_id;
    let received_away = candidate
        .owned_inputs
        .iter()
        .any(|reference| !is_live_at(state, reference));
    let owned: Vec<Object> = candidate
        .owned_inputs
        .iter()
        .filter(|reference| is_live_at(state, reference) || reference.object_id == gas)
        .map(|reference| {
            let object = state
                .live(&reference.object_id)
                .unwrap_or_else(|| panic!("kept gas {reference:?} must be live at execution"));
            assert_eq!(object.version(), reference.version, "kept gas version");
            object.clone()
        })
        .collect();
    let receiving_live = candidate
        .receiving
        .iter()
        .all(|reference| is_live_at(state, reference));
    let received: Vec<Object> = candidate
        .receiving
        .iter()
        .filter(|reference| {
            !candidate
                .owned_inputs
                .iter()
                .any(|owned| owned.object_id == reference.object_id)
        })
        .filter_map(|reference| state.live(&reference.object_id).cloned())
        .filter(|object| {
            candidate
                .receiving
                .iter()
                .any(|reference| object.object_ref() == *reference)
        })
        .collect();
    let shared: Vec<Option<Object>> = candidate
        .shared_inputs
        .iter()
        .map(|(id, _)| state.live(id).cloned())
        .collect();
    let shared_versions = candidate
        .shared_inputs
        .iter()
        .map(|(id, _)| state.slots[id].version());
    // Unwrapping names the wrapper, whose version is at least the wrapped
    // one, so the unwrapped object always takes a version above its
    // tombstone.
    let wrapped_versions = candidate.writes.iter().filter_map(|write| match write {
        Write::Unwrap(id) => match state.slots.get(id) {
            Some(Slot::Wrapped(version)) => Some(*version),
            _ => None,
        },
        _ => None,
    });
    let lamport = Version::lamport_increment(
        owned
            .iter()
            .chain(&received)
            .map(|object| object.version())
            .chain(shared_versions)
            .chain(wrapped_versions),
    )
    .expect("a candidate consumes at least its gas coin");
    let read_only = |id: &ObjectId| candidate.read_only_shared.contains(id);
    let deleted_shared: Vec<ObjectId> = candidate
        .shared_inputs
        .iter()
        .zip(&shared)
        .filter(|(_, object)| object.is_none())
        .map(|((id, _), _)| *id)
        .collect();
    let deleted_shared_inputs: Vec<ObjectId> = deleted_shared
        .iter()
        .filter(|id| !read_only(id))
        .copied()
        .collect();
    let read_only_shared: Vec<(ObjectId, Slot)> = candidate
        .shared_inputs
        .iter()
        .filter(|(id, _)| read_only(id))
        .map(|(id, _)| (*id, state.slots[id].clone()))
        .collect();
    let failed = candidate.fails || !deleted_shared.is_empty() || received_away || !receiving_live;
    let aborted = failed;
    let has_write = |write: &Write| candidate.writes.contains(write);

    let mut changes = Vec::new();
    for object in &owned {
        let id = object.id();
        let (kind, after) = if aborted || id == gas {
            (
                ChangeKind::Mutate,
                Slot::Live(coin(id, lamport, object.owner)),
            )
        } else if has_write(&Write::Delete(id)) {
            (ChangeKind::Delete, Slot::Deleted(lamport))
        } else if has_write(&Write::Wrap(id)) {
            (ChangeKind::Wrap, Slot::Wrapped(lamport))
        } else {
            (
                ChangeKind::Mutate,
                Slot::Live(coin(id, lamport, object.owner)),
            )
        };
        changes.push(ObjectChange {
            id,
            kind,
            before: Some(object.clone()),
            after,
        });
    }
    for object in shared.iter().flatten() {
        let id = object.id();
        if read_only(&id) {
            continue;
        }
        let (kind, after) = if !aborted && has_write(&Write::DeleteShared(id)) {
            (ChangeKind::Delete, Slot::Deleted(lamport))
        } else {
            (
                ChangeKind::Mutate,
                Slot::Live(coin(id, lamport, object.owner)),
            )
        };
        changes.push(ObjectChange {
            id,
            kind,
            before: Some(object.clone()),
            after,
        });
    }
    if !aborted {
        for object in &received {
            changes.push(ObjectChange {
                id: object.id(),
                kind: ChangeKind::Mutate,
                before: Some(object.clone()),
                after: Slot::Live(coin(object.id(), lamport, object.owner)),
            });
        }
        for write in &candidate.writes {
            let (id, kind, after) = match write {
                Write::CreateOwned(id) if !state.slots.contains_key(id) => (
                    *id,
                    ChangeKind::Create,
                    Slot::Live(coin(*id, lamport, Owner::Address(SENDER))),
                ),
                Write::CreateShared(id) if !state.slots.contains_key(id) => (
                    *id,
                    ChangeKind::Create,
                    Slot::Live(coin(*id, lamport, Owner::Shared(lamport))),
                ),
                Write::Unwrap(id) if matches!(state.slots.get(id), Some(Slot::Wrapped(_))) => (
                    *id,
                    ChangeKind::Unwrap,
                    Slot::Live(coin(*id, lamport, Owner::Address(SENDER))),
                ),
                Write::Publish(id) if !state.slots.contains_key(id) => {
                    (*id, ChangeKind::Publish, Slot::Live(package(lamport)))
                }
                _ => continue,
            };
            if changes.iter().any(|change| change.id == id) {
                continue;
            }
            changes.push(ObjectChange {
                id,
                kind,
                before: None,
                after,
            });
        }
    }
    for change in &changes {
        state.slots.insert(change.id, change.after.clone());
    }
    Execution {
        digest: candidate.digest,
        gas,
        lamport,
        changes,
        deleted_shared_inputs,
        read_only_shared,
        failed,
    }
}

/// A coin object at `version`; its digest is a function of id, version and
/// owner, so every validator's execution produces the same reference.
pub fn coin(id: ObjectId, version: Version, owner: Owner) -> Object {
    Object::with_id_owner_version_for_testing(id, version, owner)
}

/// The one package a history can publish: the Move standard library at
/// `version`, the only package object that can be built without compiling
/// modules. Its id is fixed.
pub const PACKAGE_ID: ObjectId = ObjectId::new([
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
]);

pub fn package(version: Version) -> Object {
    static MODULES: OnceLock<Vec<CompiledModule>> = OnceLock::new();
    let modules = MODULES
        .get_or_init(|| iota_framework::BuiltInFramework::get_package_by_id(&PACKAGE_ID).modules());
    Object::new_system_package(modules, version, vec![], TransactionDigest::ZERO)
}

/// A digest that names no object, for candidates naming a wrong digest.
pub fn wrong_digest(seed: u8) -> ObjectDigest {
    ObjectDigest::new([seed; 32])
}
