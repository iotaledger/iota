// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The shared-input machine. Answers existence and creation metadata from the
//! creation row at the initial shared version, the store's deletion info and
//! the store object, with no content read. One state per read, the store
//! answer held until both tables are re-read, as in the owned machine.
//!
//! The re-read reads the creation row before the record, which is safe for
//! the reason the owned machine's module doc gives: the first pass read the
//! record before the store, so a record missed there means the completion
//! had already inserted its row.

use iota_sdk_types::{ObjectId, Owner, TransactionDigest, Version};
use iota_types::{error::IotaResult, object::Object, storage::ObjectKey};

use super::{DropKind, DropReason, MissingKind, MissingReason, reader::CommitIndexedReader};
use crate::authority::authority_per_epoch_store::handler_object_state::{
    CommitIndex, HandlerProcessedObject, HandlerProcessedObjectKind, SyncAheadRecord,
};

// ---------------------------------------------------------------------------
// Carrier
// ---------------------------------------------------------------------------

/// Marker for the shared machine's states.
pub trait SharedState {}

/// One shared input being read at commit `C`. `S` is what the reader has
/// established so far. Each state exposes only the transition the algorithm
/// allows next, and every transition consumes the reader.
pub struct SharedReader<S: SharedState> {
    id: ObjectId,
    /// The initial shared version the transaction declares. The creation row
    /// sits at this version.
    initial_shared_version: Version,
    /// `C - K`. A row produced above it answers missing.
    horizon: CommitIndex,
    state: S,
}

impl<S: SharedState> SharedReader<S> {
    fn into_state<T: SharedState>(self, state: T) -> SharedReader<T> {
        SharedReader {
            id: self.id,
            initial_shared_version: self.initial_shared_version,
            horizon: self.horizon,
            state,
        }
    }

    /// The creation row against the current tables, `None` when no row
    /// exists at `(id, declared initial version)`. Shared by the first pass
    /// and the re-read.
    fn creation_row_classification(
        &self,
        ctx: &CommitIndexedReader,
    ) -> IotaResult<Option<CreationClass>> {
        let key = ObjectKey(self.id, self.initial_shared_version);
        Ok(ctx
            .epoch_store
            .handler_processed_object(&key)?
            .map(|row| classify_creation_row(&row, self.initial_shared_version, self.horizon)))
    }

    /// The sync-ahead record against the current tables, `None` when no
    /// record exists for `id`. Shared by the first pass and the re-read.
    fn record_classification(&self, ctx: &CommitIndexedReader) -> IotaResult<Option<RecordClass>> {
        Ok(ctx
            .epoch_store
            .sync_ahead_record(&self.id)?
            .map(|record| classify_record(&record)))
    }
}

// ---------------------------------------------------------------------------
// Creation: the handler-processed row at (id, initial shared version)
// ---------------------------------------------------------------------------

/// Nothing read yet.
pub struct Start;
impl SharedState for Start {}

/// Outcome of the creation row lookup at `(id, declared initial version)`.
#[must_use]
pub enum CreationRowLookup {
    /// A `Live` row with the created-shared flag, at or below `C - K`. Next:
    /// the store object, for deletion.
    Created(SharedReader<CreatedShared>),
    /// A row above `C - K`. Never skipped.
    Missing(MissingReason),
    /// A row without the flag, or a tombstone at that key: not created
    /// shared at the declared version.
    Drop(DropReason),
    /// Not a verdict. No row, so the machine continues with the sync-ahead
    /// record.
    NoRow(SharedReader<NoCreationRow>),
}

impl SharedReader<Start> {
    /// Begins the read of shared input `id` declared at
    /// `initial_shared_version`, at horizon `C - K`. The only constructor of a
    /// reader in any state.
    pub fn start(
        id: ObjectId,
        initial_shared_version: Version,
        horizon: CommitIndex,
    ) -> SharedReader<Start> {
        SharedReader {
            id,
            initial_shared_version,
            horizon,
            state: Start,
        }
    }

    /// Overlay first, then the table through the cache.
    pub fn read_creation_row(self, ctx: &CommitIndexedReader) -> IotaResult<CreationRowLookup> {
        Ok(match self.creation_row_classification(ctx)? {
            None => CreationRowLookup::NoRow(self.into_state(NoCreationRow)),
            Some(CreationClass::Created) => {
                CreationRowLookup::Created(self.into_state(CreatedShared))
            }
            Some(CreationClass::Missing(reason)) => CreationRowLookup::Missing(reason),
            Some(CreationClass::Drop(reason)) => CreationRowLookup::Drop(reason),
        })
    }
}

/// Created shared this epoch at the declared version, by a commit at or below
/// the horizon. Deletion not checked yet.
pub struct CreatedShared;
impl SharedState for CreatedShared {}

/// Outcome of the store object read after the creation row proved the flag.
#[must_use]
pub enum CreatedObjectLookup {
    /// The object is live locally.
    Exists,
    /// No live object locally. Next: the deletion info.
    Absent(SharedReader<ObjectAbsent>),
}

impl SharedReader<CreatedShared> {
    /// The latest object by id. The row already proved the owner, so only
    /// presence matters here.
    pub fn read_object(self, ctx: &CommitIndexedReader) -> IotaResult<CreatedObjectLookup> {
        Ok(match ctx.cache.try_get_object(&self.id)? {
            Some(_) => CreatedObjectLookup::Exists,
            None => CreatedObjectLookup::Absent(self.into_state(ObjectAbsent)),
        })
    }
}

// ---------------------------------------------------------------------------
// Record: the sync-ahead record for id
// ---------------------------------------------------------------------------

/// No handler-processed row at `(id, declared initial version)`.
pub struct NoCreationRow;
impl SharedState for NoCreationRow {}

/// Outcome of the sync-ahead record lookup.
#[must_use]
pub enum SharedRecordLookup {
    /// `base_version` is `None`: the id was created ahead of the handler.
    Missing(MissingReason),
    /// `base_version` is `Some`: the object existed before sync ran ahead.
    /// Next: the store object, for the owner.
    PreSyncExisted(SharedReader<PreSyncExisted>),
    /// Not a verdict. No record, so the machine continues with the store.
    NoRecord(SharedReader<NoRecord>),
}

impl SharedReader<NoCreationRow> {
    /// Overlay first, then the table.
    pub fn read_sync_ahead_record(
        self,
        ctx: &CommitIndexedReader,
    ) -> IotaResult<SharedRecordLookup> {
        Ok(match self.record_classification(ctx)? {
            None => SharedRecordLookup::NoRecord(self.into_state(NoRecord)),
            Some(RecordClass::PreSyncExisted) => {
                SharedRecordLookup::PreSyncExisted(self.into_state(PreSyncExisted))
            }
            Some(RecordClass::Missing(reason)) => SharedRecordLookup::Missing(reason),
        })
    }
}

/// A sync-ahead record with a base version: the object existed before sync
/// ran ahead. Owner not checked yet.
pub struct PreSyncExisted;
impl SharedState for PreSyncExisted {}

/// Outcome of the store object read when a record restores pre-sync
/// existence.
#[must_use]
pub enum PreSyncObjectLookup {
    /// Owner `Shared` at the declared initial version, or no live object
    /// because sync deleted it. Shared inputs are never sheltered, so the
    /// record alone answers in that case.
    Exists,
    /// Owner not shared, or shared at another initial version.
    Drop(DropReason),
}

impl SharedReader<PreSyncExisted> {
    /// The latest object by id, for the owner check. No live object means
    /// sync deleted it, and the record already answered existence.
    pub fn read_object(self, ctx: &CommitIndexedReader) -> IotaResult<PreSyncObjectLookup> {
        let object = ctx.cache.try_get_object(&self.id)?;
        Ok(
            match classify_object(object.as_ref(), self.initial_shared_version) {
                ObjectClass::Exists | ObjectClass::Absent => PreSyncObjectLookup::Exists,
                ObjectClass::Drop(reason) => PreSyncObjectLookup::Drop(reason),
            },
        )
    }
}

// ---------------------------------------------------------------------------
// Store: the latest object, then both tables again
// ---------------------------------------------------------------------------

/// Neither table knows `id`. The store has not been asked.
pub struct NoRecord;
impl SharedState for NoRecord {}

impl SharedReader<NoRecord> {
    /// The latest object by id, or `None` for a tombstone or an unknown id.
    /// Held, not decided: both tables must be read again first.
    pub fn read_object(
        self,
        ctx: &CommitIndexedReader,
    ) -> IotaResult<SharedReader<ObjectAnswered>> {
        let object = ctx.cache.try_get_object(&self.id)?;
        Ok(self.into_state(ObjectAnswered { object }))
    }
}

/// The store answered with the latest object or nothing. Held until both
/// tables are re-read.
pub struct ObjectAnswered {
    object: Option<Object>,
}
impl SharedState for ObjectAnswered {}

/// Outcome of re-reading the creation row after the store answered. A row
/// that landed in the window decides as on the first pass.
#[must_use]
pub enum CreationRowRecheck {
    Created(SharedReader<CreatedShared>),
    Missing(MissingReason),
    Drop(DropReason),
    /// Not a verdict. Still no row, so the machine continues with the
    /// re-read of the sync-ahead record.
    NoRow(SharedReader<ObjectAnsweredNoCreationRow>),
}

impl SharedReader<ObjectAnswered> {
    pub fn reread_creation_row(self, ctx: &CommitIndexedReader) -> IotaResult<CreationRowRecheck> {
        // An `Arc` bump: `Object` wraps its contents in one.
        let object = self.state.object.clone();
        Ok(match self.creation_row_classification(ctx)? {
            None => {
                CreationRowRecheck::NoRow(self.into_state(ObjectAnsweredNoCreationRow { object }))
            }
            Some(CreationClass::Created) => {
                CreationRowRecheck::Created(self.into_state(CreatedShared))
            }
            Some(CreationClass::Missing(reason)) => CreationRowRecheck::Missing(reason),
            Some(CreationClass::Drop(reason)) => CreationRowRecheck::Drop(reason),
        })
    }
}

/// The store answered and the re-read found no creation row. The answer is
/// still held.
pub struct ObjectAnsweredNoCreationRow {
    object: Option<Object>,
}
impl SharedState for ObjectAnsweredNoCreationRow {}

/// Outcome of re-reading the sync-ahead record after the store answered. A
/// record with no base answers missing. Otherwise the held object decides:
/// owner `Shared` at the declared version exists, another owner drops, and
/// no object with no record moves on to the deletion info.
#[must_use]
pub enum SharedRecordRecheck {
    Exists,
    Missing(MissingReason),
    Drop(DropReason),
    /// No record and no live object. Next: the deletion info.
    Absent(SharedReader<ObjectAbsent>),
}

impl SharedReader<ObjectAnsweredNoCreationRow> {
    pub fn reread_sync_ahead_record(
        self,
        ctx: &CommitIndexedReader,
    ) -> IotaResult<SharedRecordRecheck> {
        let record = self.record_classification(ctx)?;
        let object = classify_object(self.state.object.as_ref(), self.initial_shared_version);
        Ok(match (record, object) {
            (Some(RecordClass::Missing(reason)), _) => SharedRecordRecheck::Missing(reason),
            // The record restores pre-sync existence. A held object still has
            // to be shared at the declared version.
            (Some(RecordClass::PreSyncExisted), ObjectClass::Exists | ObjectClass::Absent) => {
                SharedRecordRecheck::Exists
            }
            (Some(RecordClass::PreSyncExisted), ObjectClass::Drop(reason)) => {
                SharedRecordRecheck::Drop(reason)
            }
            // Nothing appeared in either table, so the held object predates
            // every this-epoch write for `id` and stands.
            (None, ObjectClass::Exists) => SharedRecordRecheck::Exists,
            (None, ObjectClass::Drop(reason)) => SharedRecordRecheck::Drop(reason),
            (None, ObjectClass::Absent) => {
                SharedRecordRecheck::Absent(self.into_state(ObjectAbsent))
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Deletion: this epoch's deletion marker, then the row at the deleted version
// ---------------------------------------------------------------------------

/// No live object for `id` in the store.
pub struct ObjectAbsent;
impl SharedState for ObjectAbsent {}

/// Outcome of the deletion info read for this epoch.
#[must_use]
pub enum DeletionInfoLookup {
    /// No deletion this epoch: the id never existed or was deleted before
    /// this epoch.
    Drop(DropReason),
    /// Deleted this epoch. Next: the row at the deleted version.
    Found(SharedReader<DeletionInfoFound>),
}

impl SharedReader<ObjectAbsent> {
    /// This epoch's deletion marker for `id`. A deletion in an earlier epoch
    /// leaves no marker and reads as not found.
    pub fn read_deletion_info(self, ctx: &CommitIndexedReader) -> IotaResult<DeletionInfoLookup> {
        let epoch = ctx.epoch_store.epoch();
        Ok(
            match ctx
                .cache
                .try_get_last_shared_object_deletion_info(&self.id, epoch)?
            {
                Some((version, digest)) => DeletionInfoLookup::Found(
                    self.into_state(DeletionInfoFound { version, digest }),
                ),
                None => DeletionInfoLookup::Drop(DropReason(DropKind::SharedNotFound)),
            },
        )
    }
}

/// The store recorded a deletion of `id` this epoch, at `version` by
/// `digest`. Its commit is not known yet.
pub struct DeletionInfoFound {
    version: Version,
    digest: TransactionDigest,
}
impl SharedState for DeletionInfoFound {}

/// Outcome of the row lookup at `(id, deleted version)`.
#[must_use]
pub enum DeletionRowLookup {
    /// Deleted above the horizon, or by execution the handler has not
    /// reached. Kept: the transaction executes against the deletion.
    Deleted(Version, TransactionDigest),
    /// Deleted at or below the horizon.
    Drop(DropReason),
}

impl SharedReader<DeletionInfoFound> {
    /// The handler-processed row at the deleted version, which carries the
    /// deleting commit.
    pub fn read_deletion_row(self, ctx: &CommitIndexedReader) -> IotaResult<DeletionRowLookup> {
        let DeletionInfoFound { version, digest } = self.state;
        let row = ctx
            .epoch_store
            .handler_processed_object(&ObjectKey(self.id, version))?;
        Ok(match classify_deletion_row(row.as_ref(), self.horizon) {
            DeletionClass::Deleted => DeletionRowLookup::Deleted(version, digest),
            DeletionClass::Drop(reason) => DeletionRowLookup::Drop(reason),
        })
    }
}

// ---------------------------------------------------------------------------
// Pure comparisons, shared by the first pass and the re-check
// ---------------------------------------------------------------------------

/// What the creation row decided about `(id, declared initial version)`.
#[must_use]
#[derive(Debug, PartialEq, Eq)]
enum CreationClass {
    Created,
    Missing(MissingReason),
    Drop(DropReason),
}

/// What the sync-ahead record decided about `id`.
#[must_use]
#[derive(Debug, PartialEq, Eq)]
enum RecordClass {
    /// The object existed before sync ran ahead.
    PreSyncExisted,
    /// The chain created the id. Every version answers missing.
    Missing(MissingReason),
}

/// The record for `id`: a base version restores pre-sync existence, none
/// means the id was created ahead of the handler.
fn classify_record(record: &SyncAheadRecord) -> RecordClass {
    match record.base_version {
        Some(_) => RecordClass::PreSyncExisted,
        None => RecordClass::Missing(MissingReason(MissingKind::SharedSyncCreated)),
    }
}

/// What the latest store object decided about `id`.
#[must_use]
#[derive(Debug, PartialEq, Eq)]
enum ObjectClass {
    /// Owner `Shared` at the declared initial version.
    Exists,
    /// Another owner, or shared at another initial version.
    Drop(DropReason),
    /// No live object.
    Absent,
}

/// The latest object against the declared initial version. Only the owner
/// is consulted, never the contents.
fn classify_object(object: Option<&Object>, declared: Version) -> ObjectClass {
    let Some(object) = object else {
        return ObjectClass::Absent;
    };
    match object.owner {
        Owner::Shared(initial) if initial == declared => ObjectClass::Exists,
        Owner::Shared(_) => ObjectClass::Drop(DropReason(DropKind::SharedInitialVersionMismatch)),
        _ => ObjectClass::Drop(DropReason(DropKind::SharedNotCreatedShared)),
    }
}

/// What the row at the deleted version decided.
#[must_use]
#[derive(Debug, PartialEq, Eq)]
enum DeletionClass {
    /// Deleted above the horizon, or by execution the handler has not
    /// reached. The deletion is not visible at this commit.
    Deleted,
    /// Deleted at or below the horizon.
    Drop(DropReason),
}

/// The row at `(id, deleted version)`. A row at or below the horizon means
/// the handler passed the deletion. No row means sync ran ahead, and row
/// before object rules out any other reading.
fn classify_deletion_row(
    row: Option<&HandlerProcessedObject>,
    horizon: CommitIndex,
) -> DeletionClass {
    match row {
        Some(row) if row.produced_at <= horizon => {
            DeletionClass::Drop(DropReason(DropKind::SharedDeletedAtOrBelowHorizon))
        }
        Some(_) | None => DeletionClass::Deleted,
    }
}

/// The creation row at `(id, declared)`: horizon first, then the row must be
/// `Live` and carry the created-shared flag at the declared version.
fn classify_creation_row(
    row: &HandlerProcessedObject,
    declared: Version,
    horizon: CommitIndex,
) -> CreationClass {
    if row.produced_at > horizon {
        return CreationClass::Missing(MissingReason(MissingKind::SharedCreationAboveHorizon));
    }
    match (row.kind, row.initial_shared_version) {
        (HandlerProcessedObjectKind::Live, Some(initial)) if initial == declared => {
            CreationClass::Created
        }
        (HandlerProcessedObjectKind::Live, Some(_)) => {
            CreationClass::Drop(DropReason(DropKind::SharedInitialVersionMismatch))
        }
        (HandlerProcessedObjectKind::Live, None)
        | (HandlerProcessedObjectKind::Deleted, _)
        | (HandlerProcessedObjectKind::Wrapped, _) => {
            CreationClass::Drop(DropReason(DropKind::SharedNotCreatedShared))
        }
    }
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::{Address, ObjectDigest};

    use super::*;

    const HORIZON: CommitIndex = 10;
    const DECLARED: u64 = 5;

    fn declared() -> Version {
        Version::from_u64(DECLARED)
    }

    fn creation_row(
        kind: HandlerProcessedObjectKind,
        initial_shared_version: Option<u64>,
        produced_at: CommitIndex,
    ) -> HandlerProcessedObject {
        HandlerProcessedObject {
            digest: ObjectDigest::random(),
            kind,
            produced_at,
            initial_shared_version: initial_shared_version.map(Version::from_u64),
        }
    }

    fn deletion_row(produced_at: CommitIndex) -> HandlerProcessedObject {
        HandlerProcessedObject {
            digest: ObjectDigest::OBJECT_DELETED,
            kind: HandlerProcessedObjectKind::Deleted,
            produced_at,
            initial_shared_version: None,
        }
    }

    fn record(base_version: Option<u64>) -> SyncAheadRecord {
        SyncAheadRecord {
            base_version: base_version.map(Version::from_u64),
            latest_created: Version::from_u64(100),
            initial_shared_version: None,
        }
    }

    fn object(owner: Owner) -> Object {
        Object::with_id_owner_version_for_testing(ObjectId::random(), Version::from_u64(7), owner)
    }

    fn drop(kind: DropKind) -> DropReason {
        DropReason(kind)
    }

    #[test]
    fn creation_row_at_the_horizon_decides_and_one_above_answers_missing() {
        let created = |produced_at| {
            creation_row(
                HandlerProcessedObjectKind::Live,
                Some(DECLARED),
                produced_at,
            )
        };
        assert_eq!(
            classify_creation_row(&created(HORIZON), declared(), HORIZON),
            CreationClass::Created
        );
        assert_eq!(
            classify_creation_row(&created(HORIZON + 1), declared(), HORIZON),
            CreationClass::Missing(MissingReason(MissingKind::SharedCreationAboveHorizon))
        );
    }

    #[test]
    fn creation_row_above_the_horizon_answers_missing_before_the_flag() {
        let not_shared = creation_row(HandlerProcessedObjectKind::Live, None, HORIZON + 1);
        assert_eq!(
            classify_creation_row(&not_shared, declared(), HORIZON),
            CreationClass::Missing(MissingReason(MissingKind::SharedCreationAboveHorizon))
        );
    }

    #[test]
    fn creation_row_without_the_flag_or_a_tombstone_drops() {
        for kind in [
            HandlerProcessedObjectKind::Live,
            HandlerProcessedObjectKind::Deleted,
            HandlerProcessedObjectKind::Wrapped,
        ] {
            assert_eq!(
                classify_creation_row(&creation_row(kind, None, HORIZON), declared(), HORIZON),
                CreationClass::Drop(drop(DropKind::SharedNotCreatedShared))
            );
        }
    }

    #[test]
    fn creation_row_with_the_flag_at_another_version_drops() {
        let other = creation_row(
            HandlerProcessedObjectKind::Live,
            Some(DECLARED + 1),
            HORIZON,
        );
        assert_eq!(
            classify_creation_row(&other, declared(), HORIZON),
            CreationClass::Drop(drop(DropKind::SharedInitialVersionMismatch))
        );
    }

    #[test]
    fn record_with_a_base_restores_existence_and_none_answers_missing() {
        assert_eq!(
            classify_record(&record(Some(3))),
            RecordClass::PreSyncExisted
        );
        assert_eq!(
            classify_record(&record(None)),
            RecordClass::Missing(MissingReason(MissingKind::SharedSyncCreated))
        );
    }

    #[test]
    fn object_owner_against_the_declared_initial_version() {
        assert_eq!(
            classify_object(Some(&object(Owner::Shared(declared()))), declared()),
            ObjectClass::Exists
        );
        assert_eq!(
            classify_object(
                Some(&object(Owner::Shared(Version::from_u64(DECLARED + 1)))),
                declared()
            ),
            ObjectClass::Drop(drop(DropKind::SharedInitialVersionMismatch))
        );
        for owner in [
            Owner::Address(Address::ZERO),
            Owner::Object(ObjectId::random()),
            Owner::Immutable,
        ] {
            assert_eq!(
                classify_object(Some(&object(owner)), declared()),
                ObjectClass::Drop(drop(DropKind::SharedNotCreatedShared))
            );
        }
        assert_eq!(classify_object(None, declared()), ObjectClass::Absent);
    }

    #[test]
    fn deletion_row_at_the_horizon_drops_and_above_it_keeps_the_deletion_invisible() {
        assert_eq!(
            classify_deletion_row(Some(&deletion_row(HORIZON)), HORIZON),
            DeletionClass::Drop(drop(DropKind::SharedDeletedAtOrBelowHorizon))
        );
        assert_eq!(
            classify_deletion_row(Some(&deletion_row(HORIZON + 1)), HORIZON),
            DeletionClass::Deleted
        );
        assert_eq!(classify_deletion_row(None, HORIZON), DeletionClass::Deleted);
    }
}
