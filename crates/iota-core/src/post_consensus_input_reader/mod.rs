// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Reads the inputs of a transaction for post-consensus validation as of a
//! consensus commit, so that every validator reaches the same result,
//! however far its own execution has got. It also reads a dynamic field as
//! of the horizon for the deny checks at the commit: a field of the coin deny
//! list, or an account's authenticator function field. That read answers
//! present or absent, not keep, drop or missing.
//!
//! Terms used here and in the submodules:
//! - `C` is the commit being validated.
//! - `K` is how many commits behind `C` the object state is read: the protocol config's
//!   `pcool_deterministic_validation_horizon_distance`. Validation at `C` reads the state as of
//!   commit `C - K`, called the horizon. Every validator has executed the commits at or below the
//!   horizon before it validates `C`, while a commit above the horizon may be executed on some
//!   validators and not yet on others.
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
//! ask the store. For an id with no row at all, that answer is the same on
//! every validator, for three reasons.
//!
//! 1. Every write this epoch leaves its row before its object, so the store is never ahead of the
//!    rows for an id.
//! 2. The rows are kept per epoch and start empty. An id with no row has not been written this
//!    epoch on this validator, so the store holds the version the epoch started with, and every
//!    validator starts the epoch with the same state. A shared object is the exception: its
//!    mutations write no rows, so the store can hold a later version of it, with the same owner.
//! 3. The pruner removes only older versions that a newer one replaced. The latest version of a
//!    live object and the tombstone of a deleted one stay.
//!
//! An id can have rows at other versions than the one a reader looks for.
//! Rows at or below the horizon are on every validator, but a row above the
//! horizon is only on the validators that executed its commit, and their
//! store already shows that write. So a reader decides from such a store
//! answer only after reading the other rows of the id, and answers as a
//! validator that has not executed any commit above the horizon would: the
//! owned reader from the lowest row of the id above the named version, the
//! shared reader by checking a drop against the lowest row of the id. The
//! owned and shared modules explain the rules. Mutations of a shared object
//! write no rows, so a shared object named as owned is decided from its
//! creation row, not from its version.
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
//! that every validator which has not executed that commit also sees. So one
//! more read is enough, and a third would only face the same window again.
//!
//! # Loading the object of a kept version
//!
//! A kept owned input is loaded by exact key, and this validator may already
//! have consumed and pruned that version. A version consumed by a commit at
//! or below the horizon drops, from the consuming commit's row, before any
//! load. A version consumed above the horizon is kept, as on a validator that
//! has not executed that commit, unless the consuming transaction holds a
//! lock that the lock conflict check finds first. That holds for a version
//! the epoch started with too: a newer version in the store does not drop it
//! by itself, as the owned module explains. A version that a transaction in
//! the last `K` commits consumed without a lock (a received object or a
//! dynamic-field child) can be pruned here while a validation still keeps
//! it. If this validator has already pruned it, the consuming commit's row
//! above `V` drops the input. That is also the result on a validator that
//! still holds the object: the input checks reject a child named as an owned
//! input, and a received object unless the sender is the address of the
//! object it was sent to.
//!
//! # Dynamic fields as of the horizon
//!
//! The dynamic field reader walks down from a field's highest row to the
//! highest one at or below the horizon. That needs the field's versions to
//! rise with the commits that change it, which holds for a field that changes
//! only through its shared parent object, so in the order of the commits that
//! change it. The fields of the coin deny list change only through the
//! shared deny list object, and an account's authenticator function field
//! only through the shared account. A field with no row reads the store and
//! then the rows again, as explained under "Why the store answer for an id
//! with no row is the epoch-start state" above.
//!
//! The coin deny check also needs the type of a kept shared input, whose
//! object the shared reader does not read. That read takes the latest store
//! object even when rows exist, because an object's type never changes, and
//! once the object is gone, the version its deletion replaced.

use iota_sdk_types::{ObjectReference, TransactionDigest, Version};
use iota_types::{
    object::Object,
    storage::PackageObject,
    transaction::{InputObjectKind, InputObjects, ObjectReadResult, ReceivingObjects},
};

mod dynamic_field;
mod owned;
mod package;
pub mod reader;
mod shared;

#[cfg(all(test, not(msim)))]
#[path = "../unit_tests/post_consensus_determinism_tests.rs"]
mod post_consensus_determinism_tests;

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
    /// Live and shared at the declared initial version. Holds no object:
    /// execution reads it at the version consensus assigns.
    Exists,
    /// Deleted by a commit above the horizon. The input is kept and passed
    /// on as `DeletedSharedObject`, with the version and the deleting
    /// transaction from the tombstone row. Neither decides anything at the
    /// commit.
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

/// The reader's result for one dynamic field, as of the horizon. The check
/// that reads the field decides from it.
#[must_use]
#[derive(Debug, PartialEq, Eq)]
pub enum DynamicFieldAtHorizon {
    /// The field's object at the version live at the horizon.
    Live(Object),
    /// The field did not exist at the horizon, or was deleted by then.
    Absent,
}

/// The reader's result for a Move authenticator's account.
#[must_use]
pub enum AccountVerdict {
    /// The account had its authenticator function field at the horizon: the
    /// field's object at the version live then.
    Account(Object),
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
    /// A row of `id` at a version above `V`, produced at or below the horizon:
    /// a commit every validator has executed consumed `V`.
    HandlerRowSuperseded,
    /// The row at `(id, V)` is a shared object's creation row, but the
    /// transaction names the object as owned.
    HandlerRowSharedObject,
    /// A row of `id` above `V` while this validator has already pruned `V`:
    /// `V` was consumed without a lock.
    HandlerRowAbovePrunedVersion,
    /// No row at `(id, V)`, and `V` was replaced before this epoch: the store
    /// holds a newer version or a tombstone, and the lowest row of `id` above
    /// `V`, if there is one, replaced another version.
    StoreSuperseded,
    /// No row at `(id, V)`, and the store's object at `V` has another digest:
    /// as the latest reference, or as the object loaded for a kept input.
    StoreDigestMismatch,
    /// Named as owned, but the id is a shared object created at or below the
    /// horizon or before the epoch.
    SharedObjectNamedAsOwned,
    /// Named as shared, but the row or the store object is not shared at the
    /// declared initial version.
    SharedNotCreatedShared,
    /// Shared at another initial version than declared.
    SharedInitialVersionMismatch,
    /// A shared object deleted by a commit at or below the horizon.
    SharedDeletedAtOrBelowHorizon,
}

/// Which read decided a missing. `V` is the version the transaction names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissingKind {
    /// The row at `(id, V)` was produced above the horizon.
    HandlerRowAboveHorizon,
    /// No row, and the store's latest reference is below `V`: this validator
    /// has not produced the version yet.
    StoreBelowVersion,
    /// No row at `(id, V)`, and the store has nothing for `id`.
    StoreNotFound,
    /// No row at `(id, V)`, and the store's latest entry for `id` is a
    /// tombstone left by wrapping.
    StoreWrapped,
    /// No row at `(id, V)`, and a row of `id` above the horizon shows that at
    /// the horizon `id` was live at a version below `V`, or not live.
    VersionNotLiveAtHorizon,
    /// The shared object's creation row was produced above the horizon.
    SharedCreationAboveHorizon,
    /// No creation row, the store has no live object and no handler row
    /// shows a deletion.
    SharedNotFound,
    /// The package is not in the store.
    PackageNotFound,
    /// The package's row was produced above the horizon.
    PackageAboveHorizon,
    /// A Move authenticator's account had no authenticator function field at
    /// the horizon: it was not an account yet, or never became one.
    AuthenticatorFunctionNotFound,
}

/// The loader's result for one transaction's inputs at a commit. `Loaded`
/// holds every input as the input checks expect it. Receiving objects are
/// read separately, see [`ReceivingObjectsResolution`]. `Drop` and `Missing`
/// name the first input that decided. A missing input is not an error, so
/// the caller must handle it rather than turn it into one.
#[must_use]
pub enum InputResolution {
    Loaded(InputObjects),
    Drop(InputObjectKind, DropReason),
    Missing(InputObjectKind, MissingReason),
}

/// The loader's answer for one transaction's receiving objects at a commit.
/// `Loaded` holds every one as the existing receiving checks expect it. The
/// other two name the first reference that decided, under its owned kind.
#[must_use]
pub enum ReceivingObjectsResolution {
    Loaded(ReceivingObjects),
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
