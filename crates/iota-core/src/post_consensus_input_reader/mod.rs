// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Reads the inputs of a transaction for post-consensus validation as of a
//! consensus commit, so that every validator reaches the same result,
//! however far its own execution has got.
//!
//! Terms used here and in the submodules:
//! - `C` is the commit being validated.
//! - `K` ([`reader::K`]) is how many commits behind `C` the object state is read. Validation at `C`
//!   reads the state as of commit `C - K`, called the horizon. Every validator has executed the
//!   commits at or below the horizon before it validates `C`, while a commit above the horizon may
//!   be executed on some validators and not yet on others.
//! - A row is one entry of the handler object state (`handler_object_state`). It records that a
//!   transaction in some commit produced version `V` of object `id`: the commit, the digest, and
//!   whether the object is live, deleted or wrapped at that version. "The row at `(id, V)`" is that
//!   entry for the version a transaction names.
//! - The store is this validator's object store, read through the execution cache.
//!
//! Each input gets one of three results:
//! - keep: the input is valid as of the horizon, and the transaction goes on to the other checks;
//! - drop: the input can never become valid (the version is already consumed, the digest or the
//!   owner is wrong), so the transaction is dropped;
//! - missing: the input is not visible as of the horizon, but a later commit may still produce it,
//!   so validation at `C` cannot decide it yet.
//!
//! Owned, shared and package inputs each have their own submodule. Each
//! reads in a fixed order of steps, and each step is its own type that offers
//! only the next allowed read, so a read cannot be skipped, repeated or taken
//! out of order.
//!
//! On a validator that keeps the handler object state, every transaction
//! executed this epoch belongs to a commit the consensus handler has
//! assigned, so every object written this epoch has a row, and the row is
//! written before the object reaches the store. The only executions without
//! a row are the change-epoch transaction and genesis, whose writes are the
//! state the epoch starts with, and synced checkpoints executed after the
//! handler has stopped for the rest of the epoch, when no validation runs
//! any more.
//!
//! # Why the store answer for an id with no row is the epoch-start state
//!
//! When the row they look for does not exist, the owned and shared readers
//! ask the store. That answer is the same on every validator, for three
//! reasons.
//!
//! 1. Every write this epoch leaves its row before its object, so the store is never ahead of the
//!    rows for an id.
//! 2. The rows are kept per epoch and start empty. An id with no row has not been written this
//!    epoch on this validator, so the store holds the version the epoch started with, and every
//!    validator starts the epoch with the same state.
//! 3. The pruner removes only older versions that a newer one replaced. The latest version of a
//!    live object and the tombstone of a deleted one stay.
//!
//! After the store answer, the row is read once more. This covers a write
//! that lands between the first row read and the store read. Such a write
//! put its row in place before its object (point 1), and a row stays
//! readable once written (a row held in memory is removed only after it is
//! written to the table), so the second read finds the row, and the row
//! decides.
//!
//! A write that lands after the store read belongs to a commit above the
//! horizon, because validation at `C` starts only once every commit at or
//! below the horizon is executed. If the second read sees that write's row,
//! the row answers missing. If it does not, the store answer is the state
//! that every validator which has not executed that commit also sees. A
//! version such a commit consumed is caught by the lock conflict check,
//! which runs before the reader. So one more read is enough, and a third
//! would only face the same window again.

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

/// The reader's result for one owned or immutable input. The module doc
/// says what keep, drop and missing mean.
#[must_use]
pub enum OwnedVerdict {
    Keep(KeptObject),
    Drop(DropReason),
    Missing(MissingReason),
}

/// The reader's result for one shared input. Only whether the object exists
/// and is shared at the declared initial version is checked, never its
/// contents.
#[must_use]
pub enum SharedVerdict {
    /// Live and shared at the declared initial version. Holds the object at
    /// this validator's latest version, for the kind and owner checks that
    /// follow. That version can differ between validators, so no result may
    /// depend on it.
    Exists(Object),
    /// Deleted by a commit above the horizon. The input is kept and passed
    /// on as `DeletedSharedObject`, with the deleted version and the deleting
    /// transaction.
    Deleted(Version, TransactionDigest),
    Drop(DropReason),
    Missing(MissingReason),
}

/// The reader's result for one package input. A denied package is reported
/// by the deny check, so a package input never drops here.
#[must_use]
pub enum PackageVerdict {
    /// Published at or below the horizon, or before this epoch. Holds the
    /// package, for the input loader and the deny check.
    Visible(PackageObject),
    Missing(MissingReason),
}

/// The object of a kept input, loaded at exactly the version the
/// transaction names. Only the reader creates it.
pub struct KeptObject(Object);

impl KeptObject {
    pub fn into_object(self) -> Object {
        self.0
    }
}

/// Why an input drops. Only the reader creates it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DropReason(DropKind);

impl DropReason {
    pub fn kind(&self) -> DropKind {
        self.0
    }
}

/// Why an input is missing. Only the reader creates it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MissingReason(MissingKind);

impl MissingReason {
    pub fn kind(&self) -> MissingKind {
        self.0
    }
}

/// Which read decided a drop. `V` is the version the transaction names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropKind {
    /// The row at `(id, V)` is a tombstone: the object was deleted or
    /// wrapped at that version.
    HandlerRowTombstone,
    /// The row at `(id, V)` is live, with another digest.
    HandlerRowDigestMismatch,
    /// No row, and the store's latest reference is a newer version or a
    /// tombstone.
    StoreSuperseded,
    /// No row, and the store's latest reference is `V` with another digest.
    StoreDigestMismatch,
    /// No row, and the store has nothing for `id`.
    StoreNotFound,
    /// Named as shared, but the row or the store object is not shared at the
    /// declared initial version.
    SharedNotCreatedShared,
    /// Shared at another initial version than declared.
    SharedInitialVersionMismatch,
    /// A shared object deleted by a commit at or below the horizon.
    SharedDeletedAtOrBelowHorizon,
    /// No live shared object, and no deletion this epoch.
    SharedNotFound,
}

/// Which read decided a missing. `V` is the version the transaction names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissingKind {
    /// The row at `(id, V)` was produced above the horizon.
    HandlerRowAboveHorizon,
    /// No row, and the store's latest reference is below `V`: this validator
    /// has not produced the version yet.
    StoreBelowVersion,
    /// The shared object's creation row was produced above the horizon.
    SharedCreationAboveHorizon,
    /// The package is not in the store.
    PackageNotFound,
    /// The package's row was produced above the horizon.
    PackageAboveHorizon,
}

/// The loader's result for one transaction's inputs at a commit. `Loaded`
/// holds every input as the input checks expect it. Receiving objects are
/// read by the caller. `Drop` and `Missing` name the first input that
/// decided. A missing input is not an error, so the caller must handle it
/// rather than turn it into one.
#[must_use]
pub enum InputResolution {
    Loaded(InputObjects),
    Drop(InputObjectKind, DropReason),
    Missing(InputObjectKind, MissingReason),
}

/// One transaction's inputs for validation at a commit, grouped as the input
/// checks take them, or the first input that dropped or is missing.
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

/// The result of validating one transaction at a commit. A check after
/// loading that rejects the transaction returns the same `Err` as at
/// admission, not a result.
#[must_use]
#[derive(Debug)]
pub enum ValidationAtCommit {
    /// Every check passed. Holds the owned object references to lock.
    Keep(Vec<ObjectReference>),
    Drop(InputObjectKind, DropReason),
    Missing(InputObjectKind, MissingReason),
}
