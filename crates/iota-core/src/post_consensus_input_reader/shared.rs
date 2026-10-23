// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Reads one shared input: whether the object exists and is shared at the
//! declared initial version. It reads the object's creation row (the row at
//! the declared initial shared version) and the latest object in the store,
//! for an object that is gone the highest row of `id`, and after a drop the
//! lowest row of `id`. These reads
//! never use the object's contents. As for owned inputs, each step is its own
//! type, and a store answer is kept until the creation row has been read a
//! second time. A separate read, once the input is kept, loads an object of
//! the input for its type, which the coin deny check needs; its section below
//! explains it.

use iota_sdk_types::{ObjectId, Owner, TransactionDigest, Version};
use iota_types::{
    error::{IotaError, IotaResult},
    object::Object,
    storage::ObjectKey,
};

use super::{DropKind, DropReason, MissingKind, MissingReason, reader::CommitIndexedReader};
use crate::authority::authority_per_epoch_store::handler_object_state::{
    CommitIndex, HandlerProcessedObject, HandlerProcessedObjectKind,
};

/// A step of a shared input's read.
pub trait SharedState {}

/// One shared input being read for validation at commit `C`. `S` is the step
/// reached so far. It offers only the next allowed read, and each read
/// consumes this value.
pub struct SharedReader<S: SharedState> {
    id: ObjectId,
    /// The initial shared version the transaction declares. The object's
    /// creation row is the row at this version.
    initial_shared_version: Version,
    /// The horizon, `C - K`. A creation row produced above it answers
    /// missing, and a tombstone row produced above it keeps the input as
    /// deleted.
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

    /// Decides from the creation row, or `None` when there is no row at
    /// `(id, declared initial version)`. Used by both reads of the creation
    /// row.
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

/// Outcome of the first read of the creation row.
#[must_use]
pub enum SharedTablesLookup {
    /// A live row at or below the horizon that records the declared version
    /// as the initial shared version. Next: the store object, to check for a
    /// deletion.
    Created(SharedReader<CreatedShared>),
    /// A creation row produced above the horizon. The store is not asked.
    Missing(MissingReason),
    /// A row that records another initial shared version or none, or a
    /// tombstone: the object was not created shared at the declared version.
    Drop(DropReason),
    /// No creation row at `(id, declared initial version)`, so no result yet:
    /// the read goes on to the store.
    NoEntry(SharedReader<NoTablesEntry>),
}

impl SharedReader<Start> {
    /// Starts reading shared input `id` declared at `initial_shared_version`,
    /// with the horizon `C - K`. Together with
    /// [`SharedReader::start_type_read`], this is how every later step is
    /// reached.
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

    /// Reads the creation row: the rows held in memory first, then the table.
    /// Decides when a row exists.
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

/// Outcome of the store read after the creation row showed the object was
/// created shared.
#[must_use]
pub enum CreatedObjectLookup {
    /// The object is live in this validator's store.
    Exists,
    /// No live object locally. Next: the deletion row.
    Absent(SharedReader<ObjectAbsent>),
}

impl SharedReader<CreatedShared> {
    /// The latest object by id. The row already proved the owner, so only
    /// presence matters here.
    pub fn read_object(self, ctx: &CommitIndexedReader) -> IotaResult<CreatedObjectLookup> {
        Ok(match ctx.cache.try_get_object(&self.id)? {
            Some(_) => CreatedObjectLookup::Exists,
            None => CreatedObjectLookup::Absent(self.into_state(ObjectAbsent {
                origin: AbsentOrigin::Created,
            })),
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
    /// Reads the latest object by id, or `None` for a tombstone or an unknown
    /// id. The answer is kept, not decided yet: the creation row is read again
    /// first.
    pub fn read_object(
        self,
        ctx: &CommitIndexedReader,
    ) -> IotaResult<SharedReader<ObjectAnswered>> {
        let object = ctx.cache.try_get_object(&self.id)?;
        Ok(self.into_state(ObjectAnswered { object }))
    }
}

/// The store answered with the latest object or nothing. Kept until the
/// creation row has been read again.
pub struct ObjectAnswered {
    object: Option<Object>,
}
impl SharedState for ObjectAnswered {}

/// Outcome of the second read of the creation row. A creation row decides as
/// on the first read. Otherwise the kept object decides: owner `Shared` at
/// the declared version exists, another owner drops, and no object goes on to
/// the highest row of `id`.
#[must_use]
pub enum SharedTablesRecheck {
    Created(SharedReader<CreatedShared>),
    Exists,
    Missing(MissingReason),
    Drop(DropReason),
    /// No live object. Next: the deletion row.
    Absent(SharedReader<ObjectAbsent>),
}

impl SharedReader<ObjectAnswered> {
    /// Reads the creation row again and decides.
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
            // No creation row at the declared version: the kept object
            // decides, from its owner alone.
            Some(object) => match classify_object(&object, self.initial_shared_version) {
                ObjectClass::Exists => SharedTablesRecheck::Exists,
                ObjectClass::Drop(reason) => SharedTablesRecheck::Drop(reason),
            },
            None => SharedTablesRecheck::Absent(self.into_state(ObjectAbsent {
                origin: AbsentOrigin::NoEntry,
            })),
        })
    }
}

// ---------------------------------------------------------------------------
// Deletion: the highest row of `id`
// ---------------------------------------------------------------------------
//
// Execution writes a deletion record in the store at the tombstone version,
// and again at the version of every later transaction that names the deleted
// object mutably. Those later transactions write no row for the object, so
// the latest deletion record depends on which of them a validator has
// executed, and the deleting execution writes its tombstone before its
// deletion record, so a read alongside it can find the one without the
// other. The deletion is therefore decided from the highest row of `id`
// alone, which execution writes before the outputs and which names the
// deleting transaction. The deletion record is never read.

/// No live object for `id` in the store.
pub struct ObjectAbsent {
    origin: AbsentOrigin,
}
impl SharedState for ObjectAbsent {}

/// Which step the read was at when the store showed no live object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AbsentOrigin {
    /// A creation row at or below the horizon.
    Created,
    /// No creation row.
    NoEntry,
}

/// Outcome of the read of the highest row of `id`.
#[must_use]
pub enum DeletionRowLookup {
    /// Deleted above the horizon, by the transaction. Kept: the transaction
    /// being validated executes against the deletion.
    Deleted(Version, TransactionDigest),
    /// Deleted at or below the horizon, or the deleted object was not shared
    /// at the declared initial version.
    Drop(DropReason),
    /// No creation row and no tombstone row: the id never existed, was
    /// deleted before this epoch, or was created above the horizon.
    Missing(MissingReason),
}

impl SharedReader<ObjectAbsent> {
    /// Reads the row of `id` at its highest version, which decides. A shared
    /// object has at most a creation row and a tombstone row this epoch, so a
    /// tombstone row is a deletion whose commit the handler has assigned, and
    /// it records the deleting commit, the deleted object's initial shared
    /// version and the deleting transaction.
    ///
    /// A creation row at or below the horizon with no live object and no
    /// tombstone row is a storage error, which stops commit processing, and
    /// never a result: executing a deletion writes its row before the
    /// tombstone reaches the store, so only a broken write leaves that state.
    pub fn read_deletion_row(self, ctx: &CommitIndexedReader) -> IotaResult<DeletionRowLookup> {
        let row = ctx.epoch_store.highest_handler_processed_row(&self.id)?;
        let class = classify_deletion_row(
            row.as_ref().map(|(_, row)| row),
            self.initial_shared_version,
            self.horizon,
        );
        let tombstone = row.and_then(|(ObjectKey(_, version), row)| {
            row.deleted_by.map(|deleted_by| (version, deleted_by))
        });
        match (class, tombstone) {
            (DeletionClass::Drop(reason), _) => Ok(DeletionRowLookup::Drop(reason)),
            (DeletionClass::AboveHorizon, Some((version, deleted_by))) => {
                Ok(DeletionRowLookup::Deleted(version, deleted_by))
            }
            (DeletionClass::AboveHorizon, None) => Err(IotaError::Storage(format!(
                "shared input {} declared at {} has a tombstone row above the horizon that \
                 names no transaction",
                self.id, self.initial_shared_version
            ))),
            (DeletionClass::NotHandlerKnown, _) => match self.state.origin {
                AbsentOrigin::NoEntry => Ok(DeletionRowLookup::Missing(MissingReason(
                    MissingKind::SharedNotFound,
                ))),
                AbsentOrigin::Created => Err(IotaError::Storage(format!(
                    "shared input {} declared at {} has no live object and no tombstone row, \
                     though its creation row is at or below the horizon",
                    self.id, self.initial_shared_version
                ))),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// A drop: the lowest row of `id`
// ---------------------------------------------------------------------------
//
// A drop from the store object, or from a tombstone row above the horizon,
// assumes `id` existed at the horizon. A validator that has not executed the
// commit that created `id`, or unwrapped it, has no object and no tombstone
// row for it and answers missing. So a drop is checked against the lowest
// row of `id`: one above the horizon that replaced no version is that
// creation or unwrap, and the drop becomes missing. A drop decided from a
// row at or below the horizon has a lowest row at or below it too, as an
// id's row versions rise with the commits that write them.

/// A drop decided by the reads above. The lowest row of `id` has not been
/// read.
pub struct Dropped {
    reason: DropReason,
}
impl SharedState for Dropped {}

impl SharedReader<Start> {
    /// Starts the check of a drop the reads above decided for `id`.
    pub fn start_drop_check(
        id: ObjectId,
        initial_shared_version: Version,
        horizon: CommitIndex,
        reason: DropReason,
    ) -> SharedReader<Dropped> {
        SharedReader {
            id,
            initial_shared_version,
            horizon,
            state: Dropped { reason },
        }
    }
}

/// Outcome of the read of the id's lowest row after a drop.
#[must_use]
pub enum DropCheck {
    Drop(DropReason),
    /// `id` did not exist at the horizon.
    Missing(MissingReason),
}

impl SharedReader<Dropped> {
    /// Reads the lowest row of `id`, which decides.
    pub fn read_lowest_row(self, ctx: &CommitIndexedReader) -> IotaResult<DropCheck> {
        let row = ctx
            .epoch_store
            .lowest_handler_processed_row_above(&self.id, Version::MIN_VALID_INCL)?;
        Ok(
            match classify_lowest_row(row.as_ref().map(|(_, row)| row), self.horizon) {
                Some(reason) => DropCheck::Missing(reason),
                None => DropCheck::Drop(self.state.reason),
            },
        )
    }
}

// ---------------------------------------------------------------------------
// Type: an object of a kept input, for its type
// ---------------------------------------------------------------------------
//
// A separate read, after the shared input's read kept it. The coin deny
// check at the commit needs the coin type of every input, and a shared
// input's object is not read for the keep. An object's type never changes, so
// any stored version gives it, and the latest store object is used even when
// rows exist: that answer is the same on every validator. Once the object is
// gone, the version its deletion replaced gives it. The deletion is above the
// horizon, since one at or below it drops the input, so pruning still keeps
// that version.

/// A kept shared input whose type is wanted. Nothing read yet.
pub struct TypeStart;
impl SharedState for TypeStart {}

/// Outcome of the store read for the type.
#[must_use]
pub enum TypeObjectLookup {
    /// The live object.
    Live(Object),
    /// Gone from the store. Next: the deletion row.
    Deleted(SharedReader<TypeDeleted>),
}

impl SharedReader<TypeStart> {
    /// Begins the type read of the kept shared input `id` declared at
    /// `initial_shared_version`.
    pub fn start_type_read(
        id: ObjectId,
        initial_shared_version: Version,
        horizon: CommitIndex,
    ) -> SharedReader<TypeStart> {
        SharedReader {
            id,
            initial_shared_version,
            horizon,
            state: TypeStart,
        }
    }

    /// The latest object by id.
    pub fn read_object(self, ctx: &CommitIndexedReader) -> IotaResult<TypeObjectLookup> {
        Ok(match ctx.cache.try_get_object(&self.id)? {
            Some(object) => TypeObjectLookup::Live(object),
            None => TypeObjectLookup::Deleted(self.into_state(TypeDeleted)),
        })
    }
}

/// The kept input is gone from the store.
pub struct TypeDeleted;
impl SharedState for TypeDeleted {}

impl SharedReader<TypeDeleted> {
    /// Reads the version the deletion replaced from the highest row of `id`:
    /// the deleting execution writes its row before the object leaves the
    /// store, and no later write adds a row. A missing row or version is a
    /// storage error, never a result.
    pub fn read_deletion_row(
        self,
        ctx: &CommitIndexedReader,
    ) -> IotaResult<SharedReader<TypeNeedBytes>> {
        let version = ctx
            .epoch_store
            .highest_handler_processed_row(&self.id)?
            .and_then(|(_, row)| row.replaced_version)
            .ok_or_else(|| {
                IotaError::Storage(format!(
                    "kept shared input {} is not in the object store and has no handler row \
                     with the version it replaced",
                    self.id
                ))
            })?;
        Ok(self.into_state(TypeNeedBytes { version }))
    }
}

/// The version the deletion of the kept input replaced.
pub struct TypeNeedBytes {
    version: Version,
}
impl SharedState for TypeNeedBytes {}

impl SharedReader<TypeNeedBytes> {
    /// Loads the object at exactly that version. Pruning keeps it, so a
    /// missing object is a storage error.
    pub fn load_bytes(self, ctx: &CommitIndexedReader) -> IotaResult<Object> {
        let key = ObjectKey(self.id, self.state.version);
        ctx.cache
            .try_get_object_by_key(&key.0, key.1)?
            .ok_or_else(|| IotaError::Storage(format!("no bytes for {key:?} in the object store")))
    }
}

// ---------------------------------------------------------------------------
// Decisions on what a read returned
// ---------------------------------------------------------------------------

/// What the creation row decided about `(id, declared initial version)`.
#[must_use]
#[derive(Debug, PartialEq, Eq)]
enum CreationClass {
    Created,
    Missing(MissingReason),
    Drop(DropReason),
}

/// What a row's `initial_shared_version` decided, once the object that
/// carried the owner is gone.
#[must_use]
#[derive(Debug, PartialEq, Eq)]
enum RowOwnerClass {
    /// Shared at the declared initial version.
    Matches,
    /// Shared at another initial version, or not shared at all.
    Drop(DropReason),
}

/// A row's initial shared version against the declared one. `None` means the
/// deleted object was not shared.
fn classify_row_initial_version(row_initial: Option<Version>, declared: Version) -> RowOwnerClass {
    match row_initial {
        Some(initial) if initial == declared => RowOwnerClass::Matches,
        Some(_) => RowOwnerClass::Drop(DropReason(DropKind::SharedInitialVersionMismatch)),
        None => RowOwnerClass::Drop(DropReason(DropKind::SharedNotCreatedShared)),
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

/// What the highest row of `id` decided about a deletion.
#[must_use]
#[derive(Debug, PartialEq, Eq)]
enum DeletionClass {
    /// A tombstone row above the horizon: the deletion is not visible at this
    /// commit.
    AboveHorizon,
    /// No tombstone row: deleted by execution the handler has not reached, if
    /// at all.
    NotHandlerKnown,
    /// Deleted at or below the horizon, or not shared at the declared
    /// initial version.
    Drop(DropReason),
}

/// The highest row of `id`. Only a tombstone row is a deletion whose commit
/// the handler has assigned: a creation row as the highest row, or no row,
/// means the handler has not reached a deletion, and since a row is always
/// written before its object, no other reading is possible. The tombstone's
/// initial shared version is checked first: a wrong declared version drops at
/// every horizon. Then a tombstone at or below the horizon means every
/// validator has executed the deletion.
fn classify_deletion_row(
    row: Option<&HandlerProcessedObject>,
    declared: Version,
    horizon: CommitIndex,
) -> DeletionClass {
    let Some(row) = row.filter(|row| row.kind != HandlerProcessedObjectKind::Live) else {
        return DeletionClass::NotHandlerKnown;
    };
    match classify_row_initial_version(row.initial_shared_version, declared) {
        RowOwnerClass::Drop(reason) => DeletionClass::Drop(reason),
        RowOwnerClass::Matches if row.produced_at <= horizon => {
            DeletionClass::Drop(DropReason(DropKind::SharedDeletedAtOrBelowHorizon))
        }
        RowOwnerClass::Matches => DeletionClass::AboveHorizon,
    }
}

/// The id's lowest row after a drop: missing when it is above the horizon
/// and replaced no version, a creation or an unwrap there.
fn classify_lowest_row(
    row: Option<&HandlerProcessedObject>,
    horizon: CommitIndex,
) -> Option<MissingReason> {
    row.filter(|row| row.produced_at > horizon && row.replaced_version.is_none())
        .map(|_| MissingReason(MissingKind::SharedNotFound))
}

/// The creation row at `(id, declared)`: horizon first, then the row must be
/// live and record the declared version as the initial shared version.
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
            deleted_by: None,
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
            deleted_by: Some(TransactionDigest::random()),
        }
    }

    /// A transaction in commit 11 creates a shared object with initial shared
    /// version 8. Commit 12 is validated with horizon 10, so the object must
    /// not be visible yet. The first read finds no creation row. Commit 11
    /// then executes, and writes its row before the object reaches the store.
    /// The store now returns the object, so the store answer alone would
    /// report it as existing, but the second read of the creation row finds
    /// the row and answers missing, the same as a fresh read.
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
                config.enable_pcool_deterministic_validation_for_testing();
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
            SharedTablesRecheck::Exists => panic!("kept an object created above the horizon"),
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

    /// A transaction in commit 11, above the horizon of commit 12, creates a
    /// shared object with initial shared version 8. A transaction declares
    /// the object at version 5. A validator that has not executed commit 11
    /// has no object and no row for it, and answers missing. One that has
    /// holds the object shared at version 8, and must answer missing too, as
    /// the object did not exist at the horizon. Once commit 11 is at the
    /// horizon, both drop.
    #[tokio::test]
    async fn wrong_initial_version_of_an_object_created_above_the_horizon_is_missing() {
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
                config.enable_pcool_deterministic_validation_for_testing();
                config
            });

        const PRODUCING_COMMIT: CommitIndex = HORIZON + 1;

        let sender = Address::ZERO;
        let s_id = ObjectId::random();
        let initial = Version::from_u64(8);
        let gas = Object::with_id_owner_version_for_testing(
            ObjectId::random(),
            Version::from_u64(7),
            Owner::Address(sender),
        );
        let (authority, _) = init_state_with_objects_and_object_basics([gas.clone()]).await;
        let epoch_store = authority.epoch_store_for_testing().clone();
        let read_at = |commit_index| {
            CommitIndexedReader::new(
                authority.get_object_cache_reader().clone(),
                epoch_store.clone(),
                commit_index,
            )
            .read_shared(s_id, declared())
            .unwrap()
        };
        let assert_missing = |verdict: SharedVerdict, kind: MissingKind| {
            assert!(
                matches!(verdict, SharedVerdict::Missing(reason) if reason.kind() == kind),
                "expected missing with {kind:?}"
            )
        };

        assert_missing(read_at(PRODUCING_COMMIT + 1), MissingKind::SharedNotFound);

        let transaction = SenderSignedTransaction::new(
            TestTransactionBuilder::new(sender, gas.object_ref(), 0)
                .transfer_iota(None, sender)
                .build(),
            vec![],
        );
        let effects = TestEffectsBuilder::new(&transaction)
            .with_created_objects([(s_id, Owner::Shared(initial))])
            .build();
        let key = TransactionKey::Digest(*effects.transaction_digest());
        epoch_store.assign_commit_to_transactions(PRODUCING_COMMIT, vec![key]);
        epoch_store
            .record_commit_fully_executed(
                PRODUCING_COMMIT,
                &handler_processed_upserts(&effects, PRODUCING_COMMIT),
            )
            .unwrap();
        authority.insert_genesis_object(Object::with_id_owner_version_for_testing(
            s_id,
            initial,
            Owner::Shared(initial),
        ));

        assert_missing(read_at(PRODUCING_COMMIT + 1), MissingKind::SharedNotFound);
        assert!(matches!(
            read_at(PRODUCING_COMMIT + 2),
            SharedVerdict::Drop(reason) if reason.kind() == DropKind::SharedInitialVersionMismatch
        ));
    }

    /// A creation row at or below the horizon proves the object was created
    /// this epoch, and the outputs follow the row. An absent object with no
    /// tombstone row breaks that, so the read is a storage error rather than
    /// a result.
    #[tokio::test]
    async fn creation_row_without_object_or_tombstone_row_is_a_storage_error() {
        use iota_sdk_types::SenderSignedTransaction;
        use iota_test_transaction_builder::TestTransactionBuilder;
        use iota_types::{
            effects::{TestEffectsBuilder, TransactionEffectsAPI},
            transaction::TransactionKey,
        };

        use crate::authority::{
            authority_per_epoch_store::handler_object_state::handler_processed_upserts,
            authority_tests::init_state_with_objects_and_object_basics,
        };
        let _guard =
            iota_protocol_config::ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
                config.enable_pcool_deterministic_validation_for_testing();
                config
            });

        const PRODUCING_COMMIT: CommitIndex = 5;

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

        // Commit 5 writes the creation row at (S, 8). S never reaches the
        // store.
        epoch_store.assign_commit_to_transactions(PRODUCING_COMMIT, vec![key]);
        epoch_store
            .record_commit_fully_executed(
                PRODUCING_COMMIT,
                &handler_processed_upserts(&effects, PRODUCING_COMMIT),
            )
            .unwrap();

        let ctx =
            CommitIndexedReader::new(authority.get_object_cache_reader().clone(), epoch_store, 12);
        assert!(matches!(
            ctx.read_shared(s_id, initial),
            Err(IotaError::Storage(_))
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
    fn creation_row_above_the_horizon_answers_missing_before_the_initial_shared_version() {
        let not_shared = creation_row(HandlerProcessedObjectKind::Live, None, HORIZON + 1);
        assert_eq!(
            classify_creation_row(&not_shared, declared(), HORIZON),
            CreationClass::Missing(MissingReason(MissingKind::SharedCreationAboveHorizon))
        );
    }

    #[test]
    fn creation_row_without_an_initial_shared_version_or_a_tombstone_drops() {
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
    fn creation_row_with_another_initial_shared_version_drops() {
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
    fn row_initial_version_against_the_declared_one() {
        assert_eq!(
            classify_row_initial_version(Some(declared()), declared()),
            RowOwnerClass::Matches
        );
        assert_eq!(
            classify_row_initial_version(Some(Version::from_u64(DECLARED + 1)), declared()),
            RowOwnerClass::Drop(drop(DropKind::SharedInitialVersionMismatch))
        );
        assert_eq!(
            classify_row_initial_version(None, declared()),
            RowOwnerClass::Drop(drop(DropKind::SharedNotCreatedShared))
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
            DeletionClass::AboveHorizon
        );
        assert_eq!(
            classify_deletion_row(None, declared(), HORIZON),
            DeletionClass::NotHandlerKnown
        );
    }

    /// The creation row as the id's highest row means the handler has not
    /// reached a deletion, whatever commit produced it.
    #[test]
    fn creation_row_as_the_highest_row_is_no_handler_known_deletion() {
        for produced_at in [HORIZON, HORIZON + 1] {
            let created = creation_row(
                HandlerProcessedObjectKind::Live,
                Some(DECLARED),
                produced_at,
            );
            assert_eq!(
                classify_deletion_row(Some(&created), declared(), HORIZON),
                DeletionClass::NotHandlerKnown,
                "produced_at {produced_at}"
            );
        }
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
