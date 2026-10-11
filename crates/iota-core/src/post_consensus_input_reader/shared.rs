// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The shared-input machine. Answers existence and creation metadata from the
//! creation row at the initial shared version, the store's deletion info and
//! the store object, with no content read. One state per read, the store
//! answer held until the creation row is re-read, as in the owned machine.

use iota_sdk_types::{ObjectId, Owner, TransactionDigest, Version};
use iota_types::{error::IotaResult, object::Object, storage::ObjectKey};

use super::{DropKind, DropReason, MissingKind, MissingReason, reader::CommitIndexedReader};
use crate::authority::authority_per_epoch_store::handler_object_state::{
    CommitIndex, HandlerProcessedObject, HandlerProcessedObjectKind,
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
}

// ---------------------------------------------------------------------------
// Creation: the row at (id, initial shared version)
// ---------------------------------------------------------------------------

/// Nothing read yet.
pub struct Start;
impl SharedState for Start {}

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
}

/// Outcome of the first read of the creation row.
#[must_use]
pub enum SharedTablesLookup {
    /// A `Live` row with the created-shared flag, at or below `C - K`. Next:
    /// the store object, for deletion.
    Created(SharedReader<CreatedShared>),
    /// A row above `C - K`, never skipped.
    Missing(MissingReason),
    /// A row without the flag, or a tombstone at that key: not created
    /// shared at the declared version.
    Drop(DropReason),
    /// Not a verdict. No creation row at `(id, declared initial version)`, so
    /// the machine continues with the store.
    NoEntry(SharedReader<NoTablesEntry>),
}

impl SharedReader<Start> {
    /// Overlay first, then the table through the cache. Decides when a row
    /// exists.
    pub fn read_creation_row(self, ctx: &CommitIndexedReader) -> IotaResult<SharedTablesLookup> {
        Ok(match self.creation_row_classification(ctx)? {
            Some(CreationClass::Created) => {
                SharedTablesLookup::Created(self.into_state(CreatedShared))
            }
            Some(CreationClass::Missing(reason)) => SharedTablesLookup::Missing(reason),
            Some(CreationClass::Drop(reason)) => SharedTablesLookup::Drop(reason),
            None => SharedTablesLookup::NoEntry(self.into_state(NoTablesEntry)),
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
// Store: the latest object, then the creation row again
// ---------------------------------------------------------------------------

/// No creation row at `(id, declared initial version)`. The store has not
/// been asked.
pub struct NoTablesEntry;
impl SharedState for NoTablesEntry {}

impl SharedReader<NoTablesEntry> {
    /// The latest object by id, or `None` for a tombstone or an unknown id.
    /// Held, not decided: the creation row must be read again first.
    pub fn read_object(
        self,
        ctx: &CommitIndexedReader,
    ) -> IotaResult<SharedReader<ObjectAnswered>> {
        let object = ctx.cache.try_get_object(&self.id)?;
        Ok(self.into_state(ObjectAnswered { object }))
    }
}

/// The store answered with the latest object or nothing. Held until the
/// creation row is re-read.
pub struct ObjectAnswered {
    object: Option<Object>,
}
impl SharedState for ObjectAnswered {}

/// Outcome of the re-read of the creation row. A creation row decides as on
/// the first pass. Else the held object decides: owner `Shared` at the
/// declared version exists, another owner drops, and no object moves on to
/// the deletion info.
#[must_use]
pub enum SharedTablesRecheck {
    Created(SharedReader<CreatedShared>),
    Exists(Object),
    Missing(MissingReason),
    Drop(DropReason),
    /// No live object. Next: the deletion info.
    Absent(SharedReader<ObjectAbsent>),
}

impl SharedReader<ObjectAnswered> {
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
        Ok(match self.state.object.clone() {
            // No creation row: the held object predates every write this
            // epoch for `id` and stands.
            Some(object) => match classify_object(&object, self.initial_shared_version) {
                ObjectClass::Exists => SharedTablesRecheck::Exists(object),
                ObjectClass::Drop(reason) => SharedTablesRecheck::Drop(reason),
            },
            None => SharedTablesRecheck::Absent(self.into_state(ObjectAbsent)),
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
    /// Deleted above the horizon. Kept: the transaction executes against the
    /// deletion.
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
    /// Deleted above the horizon. The deletion is not visible at this
    /// commit.
    Deleted,
    /// Deleted at or below the horizon, or not shared at the declared
    /// initial version.
    Drop(DropReason),
}

/// The row at `(id, deleted version)`. The row's initial shared version is
/// checked first: a wrong declared version drops at every horizon. Then a row
/// at or below the horizon means the handler passed the deletion.
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
            replaced_version: None,
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
            replaced_version: None,
        }
    }

    /// `P` creates `S` shared at version 8 in commit 11, above the horizon of
    /// commit 12. The first read finds no creation row. Commit 11 then
    /// executes: its row is in place before `S` reaches the store. The store
    /// answers `S`, so the held object alone would exist. The re-read finds
    /// the row and answers missing, as a fresh read does.
    #[tokio::test]
    async fn recheck_finds_the_creation_row_of_an_object_the_store_answer_holds() {
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
        let _guard =
            iota_protocol_config::ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
                config.set_pcool_deterministic_validation_for_testing(true);
                config
            });

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

        let no_tables_entry = match SharedReader::start(s_id, initial, HORIZON)
            .read_creation_row(&ctx)
            .unwrap()
        {
            SharedTablesLookup::NoEntry(no_tables_entry) => no_tables_entry,
            _ => panic!("no creation row before commit 11 executes"),
        };

        // Commit 11 executes: the row first, then the outputs.
        epoch_store.assign_commit_to_transactions(PRODUCING_COMMIT, vec![key]);
        epoch_store
            .record_commit_fully_executed(
                PRODUCING_COMMIT,
                &handler_processed_upserts(&effects, PRODUCING_COMMIT),
            )
            .unwrap();
        authority.insert_genesis_object(shared);

        match no_tables_entry
            .read_object(&ctx)
            .unwrap()
            .reread_creation_row(&ctx)
            .unwrap()
        {
            SharedTablesRecheck::Missing(reason) => {
                assert_eq!(reason.kind(), MissingKind::SharedCreationAboveHorizon)
            }
            SharedTablesRecheck::Exists(_) => panic!("kept an object created above the horizon"),
            SharedTablesRecheck::Created(_) => panic!("the row is above the horizon"),
            SharedTablesRecheck::Drop(reason) => panic!("dropped: {reason:?}"),
            SharedTablesRecheck::Absent(_) => panic!("the object is in the store"),
        }

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
