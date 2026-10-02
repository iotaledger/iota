// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Commit-indexed reads for post-consensus validation. Answers keep, drop or
//! missing for every input of a transaction at commit `C` from the
//! bookkeeping tables and epoch start state, and holds the verdict types the
//! three input machines share.
//!
//! Every read that feeds a verdict runs in the reverse order of the writer
//! that could change it, because an ordering on a writer's two writes
//! protects a reader only if the reader reads in the opposite order. The hook
//! writes a row or record before the object, so a store answer is followed by
//! a read of both tables. The watcher's completion inserts the row before it
//! removes the record, so the record is read before the row wherever no
//! earlier record read anchors that order. Each machine's module doc says
//! which of the two it relies on.
//!
//! # Why a store answer for an id neither table knows is epoch-start state
//!
//! The owned and shared machines ask the store only when the handler row and
//! the sync-ahead record are both absent. That answer is the same on every
//! validator for three reasons.
//!
//! 1. Every write this epoch leaves a table entry before its object. The hook writes the row or the
//!    record and then the outputs reach the store, and a completion inserts a row before it removes
//!    a record. The store is never ahead of the tables for an id.
//! 2. The tables are per epoch and start empty. An id in neither table has not been written this
//!    epoch on this validator, so its latest reference is what the epoch started with, and
//!    epoch-start state is committed state every validator shares.
//! 3. The pruner removes only superseded lower versions. The latest version of a live object and
//!    the tombstone of a deleted one stay, and a consumed version validation may still keep is
//!    sheltered by the hook.
//!
//! The re-read after the store answer closes the window between the table
//! reads and the store read. A write whose object the store answer already
//! reflects put its entry in place first, by point 1, so the re-read finds
//! the entry and decides from it. The re-read reads the record before the row
//! and lets the row decide, because a completion can land between the two
//! reads: it inserts the row and then removes the record, so a record the
//! re-read misses was removed after a row the later row read finds. The
//! other order can miss both. The hook classifies by the round map before it
//! writes, so a record can be born for an already assigned commit during
//! validation and be removed by that commit's completion moments later.
//!
//! A write landing after the store answer belongs to a commit above the
//! horizon, because the wait before validation completes every commit at or
//! below it. Its entry answers missing when the re-read sees it, and when the
//! re-read does not, the held answer is the state every validator that has
//! not executed that commit shares. A version such a commit consumed is
//! caught by the lock check before the reader runs. So one re-read is enough,
//! and a second would face the same window again.
//!
//! Visibility is `pub` until the validation entry point consumes the module;
//! `pub(crate)` would be dead code under `-D warnings` until then.

use iota_sdk_types::{ObjectReference, TransactionDigest, Version};
use iota_types::{
    object::Object,
    storage::PackageObject,
    transaction::{InputObjectKind, InputObjects, ObjectReadResult, ReceivingObjects},
};

mod owned;
mod package;
pub mod reader;
mod shared;

/// The reader's answer for one owned input.
#[must_use]
pub enum OwnedVerdict {
    Keep(KeptObject),
    Drop(DropReason),
    Missing(MissingReason),
}

/// The reader's answer for one shared input. Existence and the owner are
/// what the reader decided. Contents are never read.
#[must_use]
pub enum SharedVerdict {
    /// Live, with owner `Shared` at the declared initial version. Carries the
    /// object as this validator holds it, for the kind and owner checks
    /// downstream. Its version differs between validators and decides
    /// nothing.
    Exists(Object),
    /// Deleted above the horizon, or by execution the handler has not
    /// reached. Kept, and handed on as `DeletedSharedObject`.
    Deleted(Version, TransactionDigest),
    Drop(DropReason),
    Missing(MissingReason),
}

/// The reader's answer for one package input. A denied package is the deny
/// check's verdict, so a package never drops here.
#[must_use]
pub enum PackageVerdict {
    /// Visible at this commit. The loaded package, for the input loader and
    /// the deny check.
    Visible(PackageObject),
    Missing(MissingReason),
}

/// Bytes of a kept input, read by exact key or from the shelter. Built only
/// by a machine's bytes step.
pub struct KeptObject(Object);

impl KeptObject {
    pub fn into_object(self) -> Object {
        self.0
    }
}

/// Why an input drops. Built only by a machine's transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DropReason(DropKind);

impl DropReason {
    pub fn kind(&self) -> DropKind {
        self.0
    }
}

/// Why an input is missing. Built only by a machine's transition. Every
/// missing outcome leaves the reader through one function.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MissingReason(MissingKind);

impl MissingReason {
    pub fn kind(&self) -> MissingKind {
        self.0
    }
}

/// Which source decided a drop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropKind {
    /// A handler-processed tombstone at `(id, V)`.
    HandlerRowTombstone,
    /// A `Live` handler-processed row at `(id, V)` with another digest.
    HandlerRowDigestMismatch,
    /// A sync-ahead record whose `base_version` is above `V`.
    SyncAheadBaseAboveVersion,
    /// The store's latest reference is a newer version or a tombstone.
    StoreSuperseded,
    /// The store's latest reference is `V` with another digest.
    StoreDigestMismatch,
    /// The store has no entry for `id`.
    StoreNotFound,
    /// Named as shared, but the row or the store object is not shared at the
    /// declared initial version.
    SharedNotCreatedShared,
    /// Shared at another initial version than declared.
    SharedInitialVersionMismatch,
    /// A shared object deleted at or below `C - K`.
    SharedDeletedAtOrBelowHorizon,
    /// No live shared object and no deletion this epoch.
    SharedNotFound,
}

/// Which source decided a missing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissingKind {
    /// A handler-processed row at `(id, V)` produced above `C - K`.
    HandlerRowAboveHorizon,
    /// A sync-ahead record with `base_version` `None`: the id was created
    /// ahead of the handler.
    SyncAheadCreatedId,
    /// A sync-ahead record with `base_version` below `V`: the version was
    /// created ahead of the handler.
    SyncAheadCreatedVersion,
    /// The store's latest reference is below `V`: the version is not produced
    /// on this validator yet.
    StoreBelowVersion,
    /// A shared creation row produced above `C - K`.
    SharedCreationAboveHorizon,
    /// A sync-ahead record with `base_version` `None`: the shared object was
    /// created ahead of the handler.
    SharedSyncCreated,
    /// The package is not in the store.
    PackageNotFound,
    /// The package's row was produced above `C - K`.
    PackageAboveHorizon,
    /// The package has a sync-ahead record: published by execution the
    /// handler has not reached.
    PackageSyncPublished,
}

/// The loader's answer for one transaction's inputs at a commit. `Loaded`
/// holds every input as the existing input checks expect it. Receiving
/// objects are resolved by the caller. The other two name the first input
/// that decided, so the caller must handle missing itself rather than map it
/// to an error.
#[must_use]
pub enum InputResolution {
    Loaded(InputObjects),
    Drop(InputObjectKind, DropReason),
    Missing(InputObjectKind, MissingReason),
}

/// One transaction's inputs for validation at a commit, grouped as the input
/// checks take them, or the first input that decided.
#[must_use]
pub enum ValidationInputsAtCommit {
    Loaded {
        tx_input_objects: InputObjects,
        tx_receiving_objects: ReceivingObjects,
        per_authenticator_inputs: Vec<(InputObjects, ObjectReadResult)>,
    },
    Drop(InputObjectKind, DropReason),
    Missing(InputObjectKind, MissingReason),
}

/// The entry point's answer for one transaction at a commit. Checks that
/// reject after loading still return `Err`, as at admission, so the caller's
/// storage-or-epoch split stays as it is.
#[must_use]
#[derive(Debug)]
pub enum ValidationAtCommit {
    /// Every check passed. The owned references to lock.
    Keep(Vec<ObjectReference>),
    Drop(InputObjectKind, DropReason),
    Missing(InputObjectKind, MissingReason),
}
