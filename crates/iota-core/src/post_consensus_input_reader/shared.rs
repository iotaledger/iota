// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The shared-input machine. Answers existence and creation metadata from the
//! creation row at the initial shared version, the store's deletion info and
//! the store object, with no content read. One state per read, the store
//! answer held until both tables are re-read, as in the owned machine.
//!
//! Both passes over the tables read the record before the creation row and
//! let the row decide, for the reasons the owned machine's module doc gives:
//! the completion inserts the row before it removes the record, and the hook
//! writes the row before the object a sync-ahead deletion consumes.

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
    /// The commit being validated minus the protocol config's horizon
    /// distance. A row produced above it answers missing.
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
// Record and creation: the sync-ahead record for id, then the row at
// (id, initial shared version)
// ---------------------------------------------------------------------------

/// Nothing read yet.
pub struct Start;
impl SharedState for Start {}

impl SharedReader<Start> {
    /// Begins the read of shared input `id` declared at
    /// `initial_shared_version`, at `horizon`. The only constructor of a
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

    /// Overlay first, then the table. Holds what the record said. Not a
    /// verdict: the creation row is read next and has precedence.
    pub fn read_sync_ahead_record(
        self,
        ctx: &CommitIndexedReader,
    ) -> IotaResult<SharedReader<RecordRead>> {
        let record = self.record_classification(ctx)?;
        Ok(self.into_state(RecordRead { record }))
    }
}

/// The sync-ahead record was read. Held until the creation row is read.
pub struct RecordRead {
    record: Option<RecordClass>,
}
impl SharedState for RecordRead {}

/// Outcome of the first pass over both tables. A creation row decides, else
/// the record does.
#[must_use]
pub enum SharedTablesLookup {
    /// A `Live` row with the created-shared flag, at or below the horizon.
    /// Next: the store object, for deletion.
    Created(SharedReader<CreatedShared>),
    /// A row above the horizon, never skipped. Or no row and a record with
    /// `base_version` `None`: the id was created ahead of the handler.
    Missing(MissingReason),
    /// A row without the flag, or a tombstone at that key: not created
    /// shared at the declared version.
    Drop(DropReason),
    /// No row and a record with a base version: the object existed before
    /// sync ran ahead. Next: the store object, for the owner.
    PreSyncExisted(SharedReader<PreSyncExisted>),
    /// Not a verdict. Neither table knows `id`, so the machine continues
    /// with the store.
    NoEntry(SharedReader<NoTablesEntry>),
}

impl SharedReader<RecordRead> {
    /// Overlay first, then the table through the cache. Decides.
    pub fn read_creation_row(self, ctx: &CommitIndexedReader) -> IotaResult<SharedTablesLookup> {
        // The row decides over the held record.
        Ok(
            match (self.creation_row_classification(ctx)?, self.state.record) {
                (Some(CreationClass::Created), _) => {
                    SharedTablesLookup::Created(self.into_state(CreatedShared))
                }
                (Some(CreationClass::Missing(reason)), _) => SharedTablesLookup::Missing(reason),
                (Some(CreationClass::Drop(reason)), _) => SharedTablesLookup::Drop(reason),
                (None, Some(RecordClass::Missing(reason))) => SharedTablesLookup::Missing(reason),
                (None, Some(RecordClass::PreSyncExisted(recorded_initial_shared_version))) => {
                    SharedTablesLookup::PreSyncExisted(self.into_state(PreSyncExisted {
                        recorded_initial_shared_version,
                    }))
                }
                (None, None) => SharedTablesLookup::NoEntry(self.into_state(NoTablesEntry)),
            },
        )
    }
}

/// Created shared this epoch at the declared version, by a commit at or below
/// the horizon. Deletion not checked yet.
pub struct CreatedShared;
impl SharedState for CreatedShared {}

/// Outcome of the store object read after the creation row proved the flag.
#[must_use]
pub enum CreatedObjectLookup {
    /// The object is live locally, at whatever version this validator holds.
    Exists(Object),
    /// No live object locally. Next: the deletion info.
    Absent(SharedReader<ObjectAbsent>),
}

impl SharedReader<CreatedShared> {
    /// The latest object by id. The row already proved the owner, so only
    /// presence matters here.
    pub fn read_object(self, ctx: &CommitIndexedReader) -> IotaResult<CreatedObjectLookup> {
        Ok(match ctx.cache.try_get_object(&self.id)? {
            Some(object) => CreatedObjectLookup::Exists(object),
            None => CreatedObjectLookup::Absent(self.into_state(ObjectAbsent)),
        })
    }
}

// ---------------------------------------------------------------------------
// Pre-sync existence: the store object, for the owner
// ---------------------------------------------------------------------------

/// A sync-ahead record with a base version and no creation row: the object
/// existed before sync ran ahead. Owner not checked yet.
pub struct PreSyncExisted {
    /// The record's `initial_shared_version`, the base version's owner.
    recorded_initial_shared_version: Option<Version>,
}
impl SharedState for PreSyncExisted {}

/// Outcome of the store object read when a record restores pre-sync
/// existence.
#[must_use]
pub enum PreSyncObjectLookup {
    /// Live with owner `Shared` at the declared initial version.
    Exists(Object),
    /// Owner not shared, or shared at another initial version, read from the
    /// object or from the record's field.
    Drop(DropReason),
    /// No live object, and the record's field matches: sync deleted it here.
    /// Next: the deletion info.
    Absent(SharedReader<ObjectAbsent>),
}

impl SharedReader<PreSyncExisted> {
    /// The latest object by id, for the owner check. No live object means
    /// sync deleted it, so the record's field is checked instead.
    pub fn read_object(self, ctx: &CommitIndexedReader) -> IotaResult<PreSyncObjectLookup> {
        Ok(match ctx.cache.try_get_object(&self.id)? {
            Some(object) => match classify_object(&object, self.initial_shared_version) {
                ObjectClass::Exists => PreSyncObjectLookup::Exists(object),
                ObjectClass::Drop(reason) => PreSyncObjectLookup::Drop(reason),
            },
            None => match classify_recorded_initial_version(
                self.state.recorded_initial_shared_version,
                self.initial_shared_version,
            ) {
                RecordedOwnerClass::Matches => {
                    PreSyncObjectLookup::Absent(self.into_state(ObjectAbsent))
                }
                RecordedOwnerClass::Drop(reason) => PreSyncObjectLookup::Drop(reason),
            },
        })
    }
}

// ---------------------------------------------------------------------------
// Store: the latest object, then both tables again
// ---------------------------------------------------------------------------

/// Neither table knows `id`. The store has not been asked.
pub struct NoTablesEntry;
impl SharedState for NoTablesEntry {}

impl SharedReader<NoTablesEntry> {
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

impl SharedReader<ObjectAnswered> {
    /// Re-reads the sync-ahead record and holds what it said. Not a verdict:
    /// the creation row is read next and has precedence.
    pub fn reread_sync_ahead_record(
        self,
        ctx: &CommitIndexedReader,
    ) -> IotaResult<SharedReader<ObjectAnsweredRecordReread>> {
        // An `Arc` bump: `Object` wraps its contents in one.
        let object = self.state.object.clone();
        let record = self.record_classification(ctx)?;
        Ok(self.into_state(ObjectAnsweredRecordReread { object, record }))
    }
}

/// The store answered and the sync-ahead record was re-read. Both are held
/// until the creation row is re-read.
pub struct ObjectAnsweredRecordReread {
    object: Option<Object>,
    record: Option<RecordClass>,
}
impl SharedState for ObjectAnsweredRecordReread {}

/// Outcome of the re-read of both tables. A creation row decides as on the
/// first pass. Else a record with no base answers missing. Else the held
/// object decides: owner `Shared` at the declared version exists, another
/// owner drops, and no object moves on to the deletion info, after the
/// record's field is checked when there is a record.
#[must_use]
pub enum SharedTablesRecheck {
    Created(SharedReader<CreatedShared>),
    Exists(Object),
    Missing(MissingReason),
    Drop(DropReason),
    /// No live object. Next: the deletion info.
    Absent(SharedReader<ObjectAbsent>),
}

impl SharedReader<ObjectAnsweredRecordReread> {
    /// Re-reads the creation row and decides.
    pub fn reread_creation_row(self, ctx: &CommitIndexedReader) -> IotaResult<SharedTablesRecheck> {
        match self.creation_row_classification(ctx)? {
            Some(CreationClass::Created) => {
                return Ok(SharedTablesRecheck::Created(self.into_state(CreatedShared)));
            }
            Some(CreationClass::Missing(reason)) => {
                return Ok(SharedTablesRecheck::Missing(reason));
            }
            Some(CreationClass::Drop(reason)) => return Ok(SharedTablesRecheck::Drop(reason)),
            None => {}
        }
        // An `Arc` bump: `Object` wraps its contents in one.
        let object = self.state.object.clone();
        Ok(match (self.state.record, object) {
            (Some(RecordClass::Missing(reason)), _) => SharedTablesRecheck::Missing(reason),
            // The record restores pre-sync existence. A held object still has
            // to be shared at the declared version. With no record the held
            // object predates every this-epoch write for `id` and stands.
            (Some(RecordClass::PreSyncExisted(_)) | None, Some(object)) => {
                match classify_object(&object, self.initial_shared_version) {
                    ObjectClass::Exists => SharedTablesRecheck::Exists(object),
                    ObjectClass::Drop(reason) => SharedTablesRecheck::Drop(reason),
                }
            }
            // Sync deleted the object here. The record kept the owner's initial
            // shared version.
            (Some(RecordClass::PreSyncExisted(recorded)), None) => {
                match classify_recorded_initial_version(recorded, self.initial_shared_version) {
                    RecordedOwnerClass::Matches => {
                        SharedTablesRecheck::Absent(self.into_state(ObjectAbsent))
                    }
                    RecordedOwnerClass::Drop(reason) => SharedTablesRecheck::Drop(reason),
                }
            }
            (None, None) => SharedTablesRecheck::Absent(self.into_state(ObjectAbsent)),
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
    /// Deleted at or below the horizon, or the deleted object was not shared
    /// at the declared initial version.
    Drop(DropReason),
}

impl SharedReader<DeletionInfoFound> {
    /// The handler-processed row at the deleted version, which carries the
    /// deleting commit and the deleted object's initial shared version.
    pub fn read_deletion_row(self, ctx: &CommitIndexedReader) -> IotaResult<DeletionRowLookup> {
        let DeletionInfoFound { version, digest } = self.state;
        let row = ctx
            .epoch_store
            .handler_processed_object(&ObjectKey(self.id, version))?;
        Ok(
            match classify_deletion_row(row.as_ref(), self.initial_shared_version, self.horizon) {
                DeletionClass::Deleted => DeletionRowLookup::Deleted(version, digest),
                DeletionClass::Drop(reason) => DeletionRowLookup::Drop(reason),
            },
        )
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RecordClass {
    /// The object existed before sync ran ahead. Carries the record's
    /// `initial_shared_version`, the base version's owner.
    PreSyncExisted(Option<Version>),
    /// The chain created the id. Every version answers missing.
    Missing(MissingReason),
}

/// The record for `id`: a base version restores pre-sync existence, none
/// means the id was created ahead of the handler.
fn classify_record(record: &SyncAheadRecord) -> RecordClass {
    match record.base_version {
        Some(_) => RecordClass::PreSyncExisted(record.initial_shared_version),
        None => RecordClass::Missing(MissingReason(MissingKind::SharedSyncCreated)),
    }
}

/// What a bookkeeping entry's `initial_shared_version` decided, once the
/// object bytes that carried the owner are gone.
#[must_use]
#[derive(Debug, PartialEq, Eq)]
enum RecordedOwnerClass {
    /// Shared at the declared initial version.
    Matches,
    /// Shared at another initial version, or not shared at all.
    Drop(DropReason),
}

/// The recorded initial shared version against the declared one. `None` means
/// the deleted object was not shared.
fn classify_recorded_initial_version(
    recorded: Option<Version>,
    declared: Version,
) -> RecordedOwnerClass {
    match recorded {
        Some(initial) if initial == declared => RecordedOwnerClass::Matches,
        Some(_) => RecordedOwnerClass::Drop(DropReason(DropKind::SharedInitialVersionMismatch)),
        None => RecordedOwnerClass::Drop(DropReason(DropKind::SharedNotCreatedShared)),
    }
}

/// What a live store object decided about `id`.
#[must_use]
#[derive(Debug, PartialEq, Eq)]
enum ObjectClass {
    /// Owner `Shared` at the declared initial version.
    Exists,
    /// Another owner, or shared at another initial version.
    Drop(DropReason),
}

/// The live object against the declared initial version. Only the owner is
/// consulted, never the contents.
fn classify_object(object: &Object, declared: Version) -> ObjectClass {
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
    /// Deleted at or below the horizon, or not shared at the declared
    /// initial version.
    Drop(DropReason),
}

/// The row at `(id, deleted version)`. The row's initial shared version is
/// checked first: a wrong declared version drops at every horizon. Then a row
/// at or below the horizon means the handler passed the deletion. No row
/// means sync ran ahead, and row before object rules out any other reading.
fn classify_deletion_row(
    row: Option<&HandlerProcessedObject>,
    declared: Version,
    horizon: CommitIndex,
) -> DeletionClass {
    let Some(row) = row else {
        return DeletionClass::Deleted;
    };
    match classify_recorded_initial_version(row.initial_shared_version, declared) {
        RecordedOwnerClass::Drop(reason) => DeletionClass::Drop(reason),
        RecordedOwnerClass::Matches if row.produced_at <= horizon => {
            DeletionClass::Drop(DropReason(DropKind::SharedDeletedAtOrBelowHorizon))
        }
        RecordedOwnerClass::Matches => DeletionClass::Deleted,
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

    fn deletion_row(
        initial_shared_version: Option<u64>,
        produced_at: CommitIndex,
    ) -> HandlerProcessedObject {
        HandlerProcessedObject {
            digest: ObjectDigest::OBJECT_DELETED,
            kind: HandlerProcessedObjectKind::Deleted,
            produced_at,
            initial_shared_version: initial_shared_version.map(Version::from_u64),
        }
    }

    fn record(base_version: Option<u64>) -> SyncAheadRecord {
        SyncAheadRecord {
            base_version: base_version.map(Version::from_u64),
            latest_created: Version::from_u64(100),
            initial_shared_version: None,
        }
    }

    /// Review finding R1 for the shared machine. `P` creates `S` shared at
    /// version 8 in commit 11, above the horizon of commit 12. The hook
    /// classified `P` before commit 11 was assigned, so it writes a record,
    /// and the completion of commit 11 lands between the two table re-reads.
    /// The held store object must not decide. A fresh read answers missing.
    #[tokio::test]
    async fn recheck_stays_missing_when_completion_lands_between_the_table_reads() {
        use std::collections::BTreeMap;

        use iota_sdk_types::SenderSignedTransaction;
        use iota_test_transaction_builder::TestTransactionBuilder;
        use iota_types::{
            effects::{TestEffectsBuilder, TransactionEffectsAPI},
            transaction::TransactionKey,
        };

        use crate::{
            authority::{
                authority_per_epoch_store::handler_object_state::handler_processed_upserts,
                authority_tests::init_state_with_objects_and_object_basics,
            },
            post_consensus_input_reader::SharedVerdict,
        };

        const PRODUCING_COMMIT: CommitIndex = HORIZON + 1;

        let sender = Address::ZERO;
        let gas_id = ObjectId::random();
        let s_id = ObjectId::random();
        let initial = Version::from_u64(8);
        let gas = Object::with_id_owner_version_for_testing(
            gas_id,
            Version::from_u64(7),
            Owner::Address(sender),
        );
        let (authority, _) = init_state_with_objects_and_object_basics([gas.clone()]).await;
        let epoch_store = authority.epoch_store_for_testing().clone();

        let shared =
            Object::with_id_owner_version_for_testing(s_id, initial, Owner::Shared(initial));
        let transaction = SenderSignedTransaction::new(
            TestTransactionBuilder::new(sender, gas.object_ref(), 0)
                .transfer_iota(None, sender)
                .build(),
            vec![],
        );
        let effects = TestEffectsBuilder::new(&transaction)
            .with_created_objects([(s_id, Owner::Shared(initial))])
            .build();
        assert_eq!(effects.lamport_version(), initial);
        let key = TransactionKey::Digest(*effects.transaction_digest());

        let ctx = CommitIndexedReader::new(
            authority.get_object_cache_reader().clone(),
            epoch_store.clone(),
            PRODUCING_COMMIT + 1,
        );

        // First pass: nothing written yet.
        let no_tables_entry = match SharedReader::start(s_id, initial, HORIZON)
            .read_sync_ahead_record(&ctx)
            .unwrap()
            .read_creation_row(&ctx)
            .unwrap()
        {
            SharedTablesLookup::NoEntry(no_tables_entry) => no_tables_entry,
            _ => panic!("nothing in either table before the hook ran"),
        };

        // The hook, classified before commit 11 was assigned, writes the
        // record, then S reaches the store.
        epoch_store
            .record_executed_transaction(&key, &effects, &BTreeMap::from([(gas_id, gas)]))
            .unwrap();
        authority.insert_genesis_object(shared);

        // The store answers S. The first table re-read, whichever table it
        // is, happens before the completion.
        let object_answered = no_tables_entry.read_object(&ctx).unwrap();
        let record_reread = object_answered.reread_sync_ahead_record(&ctx).unwrap();

        // Completion of commit 11 inserts the row and removes the record.
        epoch_store.assign_commit_to_transactions(PRODUCING_COMMIT, vec![key]);
        epoch_store
            .record_commit_fully_executed(
                PRODUCING_COMMIT,
                &handler_processed_upserts(&effects, PRODUCING_COMMIT),
            )
            .unwrap();

        match record_reread.reread_creation_row(&ctx).unwrap() {
            SharedTablesRecheck::Missing(reason) => {
                assert_eq!(reason.kind(), MissingKind::SharedCreationAboveHorizon)
            }
            SharedTablesRecheck::Exists(_) => panic!("kept an object created above the horizon"),
            SharedTablesRecheck::Created(_) => panic!("the row is above the horizon"),
            SharedTablesRecheck::Drop(reason) => panic!("dropped: {reason:?}"),
            SharedTablesRecheck::Absent(_) => panic!("the object is in the store"),
        }

        // The stable answer the interleaving must match.
        assert!(matches!(
            ctx.read_shared(s_id, initial).unwrap(),
            SharedVerdict::Missing(reason)
                if reason.kind() == MissingKind::SharedCreationAboveHorizon
        ));
    }

    /// Review finding U2a for the shared machine. `P` creates `S` shared at
    /// version 8 in commit 11, above the horizon of commit 12, and is
    /// handler-known, so the hook writes the creation row. State sync then
    /// executes `Q`, which deletes `S`, and the hook writes a record with
    /// base 8. Both land between the record read and the creation-row read,
    /// so the row read must find the row. A fresh read, with both entries in
    /// place, must let the row decide over the record. The object is left in
    /// the store so that the wrong answer is a keep.
    #[tokio::test]
    async fn first_pass_record_hit_defers_to_a_creation_row_that_landed_above_the_horizon() {
        use std::collections::BTreeMap;

        use iota_sdk_types::SenderSignedTransaction;
        use iota_test_transaction_builder::TestTransactionBuilder;
        use iota_types::{
            effects::{TestEffectsBuilder, TransactionEffectsAPI},
            transaction::TransactionKey,
        };

        use crate::{
            authority::authority_tests::init_state_with_objects_and_object_basics,
            post_consensus_input_reader::SharedVerdict,
        };

        const PRODUCING_COMMIT: CommitIndex = HORIZON + 1;

        let sender = Address::ZERO;
        let gas_id = ObjectId::random();
        let s_id = ObjectId::random();
        let initial = Version::from_u64(8);
        let gas = Object::with_id_owner_version_for_testing(
            gas_id,
            Version::from_u64(7),
            Owner::Address(sender),
        );
        let (authority, _) = init_state_with_objects_and_object_basics([gas.clone()]).await;
        let epoch_store = authority.epoch_store_for_testing().clone();

        let gas_after = Object::with_id_owner_version_for_testing(
            gas_id,
            Version::from_u64(8),
            Owner::Address(sender),
        );
        let shared =
            Object::with_id_owner_version_for_testing(s_id, initial, Owner::Shared(initial));

        let creator = SenderSignedTransaction::new(
            TestTransactionBuilder::new(sender, gas.object_ref(), 0)
                .transfer_iota(None, sender)
                .build(),
            vec![],
        );
        let creator_effects = TestEffectsBuilder::new(&creator)
            .with_created_objects([(s_id, Owner::Shared(initial))])
            .build();
        assert_eq!(creator_effects.lamport_version(), initial);
        let creator_key = TransactionKey::Digest(*creator_effects.transaction_digest());

        let deleter = SenderSignedTransaction::new(
            TestTransactionBuilder::new(sender, gas_after.object_ref(), 0)
                .transfer_iota(None, sender)
                .build(),
            vec![],
        );
        let deleter_effects = TestEffectsBuilder::new(&deleter)
            .with_deleted_objects_owned_by([(s_id, initial, Owner::Shared(initial))])
            .build();
        let deleter_key = TransactionKey::Digest(*deleter_effects.transaction_digest());

        let ctx = CommitIndexedReader::new(
            authority.get_object_cache_reader().clone(),
            epoch_store.clone(),
            PRODUCING_COMMIT + 1,
        );

        // First pass: the record is read first and finds nothing.
        let record_read = SharedReader::start(s_id, initial, HORIZON)
            .read_sync_ahead_record(&ctx)
            .unwrap();

        // Commit 11 is assigned and executes here: the hook writes the
        // creation row at (S, 8), then S reaches the store.
        epoch_store.assign_commit_to_transactions(PRODUCING_COMMIT, vec![creator_key]);
        epoch_store
            .record_executed_transaction(
                &creator_key,
                &creator_effects,
                &BTreeMap::from([(gas_id, gas)]),
            )
            .unwrap();
        authority.insert_genesis_object(shared.clone());

        // State sync executes the deletion of S ahead of the handler: the
        // hook writes a record with base 8.
        epoch_store
            .record_executed_transaction(
                &deleter_key,
                &deleter_effects,
                &BTreeMap::from([(gas_id, gas_after), (s_id, shared)]),
            )
            .unwrap();
        assert_eq!(
            epoch_store
                .sync_ahead_record(&s_id)
                .unwrap()
                .unwrap()
                .base_version,
            Some(initial)
        );

        match record_read.read_creation_row(&ctx).unwrap() {
            SharedTablesLookup::Missing(reason) => {
                assert_eq!(reason.kind(), MissingKind::SharedCreationAboveHorizon)
            }
            SharedTablesLookup::PreSyncExisted(_) => {
                panic!("the record decided over a creation row above the horizon")
            }
            SharedTablesLookup::Created(_) => panic!("the row is above the horizon"),
            SharedTablesLookup::Drop(reason) => panic!("dropped: {reason:?}"),
            SharedTablesLookup::NoEntry(_) => panic!("the row is there"),
        }

        // Both entries present: the row decides over the record.
        assert!(matches!(
            ctx.read_shared(s_id, initial).unwrap(),
            SharedVerdict::Missing(reason)
                if reason.kind() == MissingKind::SharedCreationAboveHorizon
        ));
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
            RecordClass::PreSyncExisted(None)
        );
        assert_eq!(
            classify_record(&SyncAheadRecord {
                initial_shared_version: Some(declared()),
                ..record(Some(3))
            }),
            RecordClass::PreSyncExisted(Some(declared()))
        );
        assert_eq!(
            classify_record(&record(None)),
            RecordClass::Missing(MissingReason(MissingKind::SharedSyncCreated))
        );
    }

    #[test]
    fn recorded_initial_version_against_the_declared_one() {
        assert_eq!(
            classify_recorded_initial_version(Some(declared()), declared()),
            RecordedOwnerClass::Matches
        );
        assert_eq!(
            classify_recorded_initial_version(Some(Version::from_u64(DECLARED + 1)), declared()),
            RecordedOwnerClass::Drop(drop(DropKind::SharedInitialVersionMismatch))
        );
        assert_eq!(
            classify_recorded_initial_version(None, declared()),
            RecordedOwnerClass::Drop(drop(DropKind::SharedNotCreatedShared))
        );
    }

    #[test]
    fn object_owner_against_the_declared_initial_version() {
        assert_eq!(
            classify_object(&object(Owner::Shared(declared())), declared()),
            ObjectClass::Exists
        );
        assert_eq!(
            classify_object(
                &object(Owner::Shared(Version::from_u64(DECLARED + 1))),
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
                classify_object(&object(owner), declared()),
                ObjectClass::Drop(drop(DropKind::SharedNotCreatedShared))
            );
        }
    }

    #[test]
    fn deletion_row_at_the_horizon_drops_and_above_it_keeps_the_deletion_invisible() {
        assert_eq!(
            classify_deletion_row(
                Some(&deletion_row(Some(DECLARED), HORIZON)),
                declared(),
                HORIZON
            ),
            DeletionClass::Drop(drop(DropKind::SharedDeletedAtOrBelowHorizon))
        );
        assert_eq!(
            classify_deletion_row(
                Some(&deletion_row(Some(DECLARED), HORIZON + 1)),
                declared(),
                HORIZON
            ),
            DeletionClass::Deleted
        );
        assert_eq!(
            classify_deletion_row(None, declared(), HORIZON),
            DeletionClass::Deleted
        );
    }

    #[test]
    fn deletion_row_checks_the_initial_shared_version_before_the_horizon() {
        for produced_at in [HORIZON, HORIZON + 1] {
            assert_eq!(
                classify_deletion_row(
                    Some(&deletion_row(Some(DECLARED + 1), produced_at)),
                    declared(),
                    HORIZON
                ),
                DeletionClass::Drop(drop(DropKind::SharedInitialVersionMismatch)),
                "produced_at {produced_at}"
            );
            assert_eq!(
                classify_deletion_row(Some(&deletion_row(None, produced_at)), declared(), HORIZON),
                DeletionClass::Drop(drop(DropKind::SharedNotCreatedShared)),
                "produced_at {produced_at}"
            );
        }
    }
}
