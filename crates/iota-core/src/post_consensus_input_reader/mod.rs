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
//! Visibility is `pub` until the validation entry point consumes the module;
//! `pub(crate)` would be dead code under `-D warnings` until then.

use iota_sdk_types::{TransactionDigest, Version};
use iota_types::{object::Object, storage::PackageObject};

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

/// The reader's answer for one shared input. Content checks are skipped, so
/// a kept shared input carries no bytes.
#[must_use]
pub enum SharedVerdict {
    /// Visible. Downstream checks skip it.
    Exists,
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
