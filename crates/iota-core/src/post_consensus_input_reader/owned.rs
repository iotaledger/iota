// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Reads one owned or immutable input, named by `(id, V)`. Each step is its
//! own type that offers only the next allowed read, so a read cannot be
//! skipped, repeated or taken out of order. The one exception is the last
//! read of the other rows of `id`, whose follow-up reads depend only on what
//! the first one returned. The decisions on what a read returned are plain
//! functions.
//!
//! The row at `(id, V)`, when there is one, decides a drop or a missing, and
//! a keep from it is checked against the rows above `V` as described below.
//! Without one, the store's
//! latest reference for `id` is read and kept, and the row is read once more
//! before that store answer may decide. The doc of the parent module explains
//! why one more read is enough.
//!
//! A keep from the row at `(id, V)` is final only after the lowest row of `id`
//! above `V` is read. Every consumption of `V` writes a row of `id` above it,
//! including a receive and a change through a dynamic-field parent, which
//! take no lock. If that row was produced at or below the horizon, every
//! validator has seen the consumption, so `V` drops even when this validator
//! has already pruned its object. A row above the horizon is ignored, because
//! a validator that has not executed its commit cannot see it. A keep from
//! the store skips this read: a row above `V` at or below the horizon belongs
//! to a commit that is executed before validation starts, so the store would
//! already have answered a newer version. Mutations of a shared object write
//! no rows, so a shared object's creation version named as owned drops from
//! the creation row without this read.
//!
//! With no row at `(id, V)`, any other store answer does not decide by
//! itself: this validator may have executed writes of `id` in commits above
//! the horizon that a slower validator has not, so its store can be ahead of
//! the state at the horizon. So the lowest row of `id` above `V` is read, and
//! decides when there is one. A row at or below the horizon is a consumption
//! every validator has seen, so `V` drops. A row above the horizon shows what
//! `id` was before that write, which is the state at the horizon as far as
//! `V` is concerned, since no row lies between `V` and it: if the version it
//! replaced is `V`, `V` is kept; if it is above `V`, `V` was replaced before
//! the epoch and drops; if it is below `V` or there is none, `V` was not
//! produced yet or `id` was not live, so `V` is missing. The keep then needs
//! `V`'s object, which this validator's pruner can remove once the consuming
//! checkpoint is executed. Loading it then fails and the input drops here,
//! while a validator that still holds it keeps the input, unless pruning
//! waits for the consuming commit to be flushed.
//!
//! With no row above `V`, the store answer decides, read so that it does not
//! depend on writes above the horizon. A tombstone of a deletion whose row is
//! above the horizon is missing, as `id` was live below `V` at the horizon.
//! A tombstone left by wrapping is missing, as the unwrap that ends it writes
//! a row that names no replaced version, and the reads above then answer
//! missing.
//!
//! Mutations of a shared object write no rows, so its versions can move above
//! the horizon without a trace. A shared object named as owned therefore
//! drops whatever its version, once its creation is at or below the horizon
//! or before the epoch, and is missing while its creation row is above the
//! horizon, as `id` did not exist at the horizon. This is checked whenever
//! the store answer is a live object, and on a keep once the loaded object
//! shows the owner.

use std::cmp::Ordering;

use iota_sdk_types::{ObjectId, ObjectReference, Owner, Version};
use iota_types::{error::IotaResult, storage::ObjectKey};

use super::{DropKind, DropReason, MissingKind, MissingReason, reader::CommitIndexedReader};
use crate::authority::authority_per_epoch_store::handler_object_state::{
    CommitIndex, HandlerProcessedObject, HandlerProcessedObjectKind,
};

/// A step of an owned input's read.
pub trait OwnedState {}

/// One owned input being read for validation at commit `C`. `S` is the step
/// reached so far. It offers only the next allowed read, and each read
/// consumes this value.
pub struct OwnedReader<S: OwnedState> {
    /// The `(id, version, digest)` the transaction names.
    input: ObjectReference,
    /// The horizon, `C - K`. The row at `(id, V)` answers missing when it was
    /// produced above it, and a row of `id` above `V` produced above it is
    /// ignored.
    horizon: CommitIndex,
    state: S,
}

impl<S: OwnedState> OwnedReader<S> {
    fn into_state<T: OwnedState>(self, state: T) -> OwnedReader<T> {
        OwnedReader {
            input: self.input,
            horizon: self.horizon,
            state,
        }
    }

    fn key(&self) -> ObjectKey {
        ObjectKey(self.input.object_id, self.input.version)
    }

    /// Decides from the row at `(id, V)`, or `None` when there is no row.
    /// Used by both reads of the row.
    fn row_classification(&self, ctx: &CommitIndexedReader) -> IotaResult<Option<Classification>> {
        Ok(ctx
            .epoch_store
            .handler_processed_object(&self.key())?
            .map(|row| classify_row(&row, &self.input, self.horizon)))
    }
}

// ---------------------------------------------------------------------------
// First read: the row at (id, V)
// ---------------------------------------------------------------------------

/// Nothing read yet.
pub struct Start;
impl OwnedState for Start {}

/// Outcome of the first read of the row at `(id, V)`.
#[must_use]
pub enum TablesLookup {
    /// A live row at or below the horizon with the named digest that is not
    /// a shared object's creation row. Next: the rows above `V`.
    LiveAtVersion(OwnedReader<LiveAtVersion>),
    /// A row produced above the horizon. The store is not asked.
    Missing(MissingReason),
    /// A tombstone row, a `Live` row with another digest or a shared
    /// object's creation row.
    Drop(DropReason),
    /// No row at `(id, V)`, so no result yet: the read goes on to the store.
    NoEntry(OwnedReader<NoTablesEntry>),
}

impl OwnedReader<Start> {
    /// Starts reading `input` with the horizon `C - K`. Every later step is
    /// reached from here.
    pub fn start(input: ObjectReference, horizon: CommitIndex) -> OwnedReader<Start> {
        OwnedReader {
            input,
            horizon,
            state: Start,
        }
    }

    /// Reads the row at `(id, V)`: the rows held in memory first, then the
    /// table. Decides when a row exists.
    pub fn read_handler_row(self, ctx: &CommitIndexedReader) -> IotaResult<TablesLookup> {
        let Some(classification) = self.row_classification(ctx)? else {
            return Ok(TablesLookup::NoEntry(self.into_state(NoTablesEntry)));
        };
        Ok(match classification {
            Classification::Keep => TablesLookup::LiveAtVersion(self.into_state(LiveAtVersion)),
            Classification::Missing(reason) => TablesLookup::Missing(reason),
            Classification::Drop(reason) => TablesLookup::Drop(reason),
        })
    }
}

// ---------------------------------------------------------------------------
// No row: the store, then the row again
// ---------------------------------------------------------------------------

/// No row at `(id, V)`. The store has not been asked.
pub struct NoTablesEntry;
impl OwnedState for NoTablesEntry {}

impl OwnedReader<NoTablesEntry> {
    /// Reads the latest reference or tombstone of `id`. The answer is kept,
    /// not decided yet: the row is read again first.
    pub fn read_store(self, ctx: &CommitIndexedReader) -> IotaResult<OwnedReader<StoreAnswered>> {
        let latest = ctx
            .cache
            .try_get_latest_object_ref_or_tombstone(self.input.object_id)?;
        Ok(self.into_state(StoreAnswered { latest }))
    }
}

/// The store answered with the latest reference, a tombstone, or nothing.
/// Kept until the row has been read again.
pub struct StoreAnswered {
    latest: Option<ObjectReference>,
}
impl OwnedState for StoreAnswered {}

/// Outcome of the second read of the row. A row decides as on the first
/// read. Otherwise the kept store answer decides: the named reference keeps,
/// and any other answer goes on to the other rows of `id`.
#[must_use]
pub enum TablesRecheck {
    /// A keep from a row that appeared since the first read. Next: the rows
    /// above `V`.
    LiveAtVersion(OwnedReader<LiveAtVersion>),
    /// A keep from the store. Next: the bytes.
    NeedBytes(OwnedReader<NeedBytes>),
    /// The store holds another version, a tombstone or nothing. Next: the
    /// id's other rows.
    NotAtVersion(OwnedReader<NotAtVersion>),
    Missing(MissingReason),
    Drop(DropReason),
}

impl OwnedReader<StoreAnswered> {
    /// Reads the row at `(id, V)` again. A row decides as on the first read.
    /// Without one, the kept store answer decides: the named reference keeps,
    /// and a newer version or a tombstone goes on to the rows above `V`.
    pub fn reread_handler_row(self, ctx: &CommitIndexedReader) -> IotaResult<TablesRecheck> {
        // A row decides as on the first read. Without one, the kept store
        // answer is used.
        if let Some(classification) = self.row_classification(ctx)? {
            return Ok(match classification {
                Classification::Keep => {
                    TablesRecheck::LiveAtVersion(self.into_state(LiveAtVersion))
                }
                Classification::Missing(reason) => TablesRecheck::Missing(reason),
                Classification::Drop(reason) => TablesRecheck::Drop(reason),
            });
        }
        let latest = self.state.latest;
        Ok(if is_named_reference(latest.as_ref(), &self.input) {
            TablesRecheck::NeedBytes(self.into_state(NeedBytes))
        } else {
            TablesRecheck::NotAtVersion(self.into_state(NotAtVersion { latest }))
        })
    }
}

// ---------------------------------------------------------------------------
// Another store answer: the id's other rows
// ---------------------------------------------------------------------------

/// No row at `(id, V)`, and the store holds another version, a tombstone or
/// nothing. The id's other rows have not been read.
pub struct NotAtVersion {
    latest: Option<ObjectReference>,
}
impl OwnedState for NotAtVersion {}

impl OwnedReader<NotAtVersion> {
    /// Reads the lowest row of `id` above `V`, then, as needed, the highest
    /// row of `id`, the latest object's owner and a shared object's creation
    /// row, each from the rows held in memory first. Decides.
    pub fn read_other_rows(self, ctx: &CommitIndexedReader) -> IotaResult<RowsAboveLookup> {
        let id = self.input.object_id;
        let row_above = ctx
            .epoch_store
            .lowest_handler_processed_row_above(&id, self.input.version)?;
        let class = match (row_above, self.state.latest) {
            (Some((_, row)), _) => {
                classify_lowest_row_above(&row, self.input.version, self.horizon)
            }
            (None, None) => OtherRows::Decided(missing_classification(MissingKind::StoreNotFound)),
            (None, Some(latest)) if latest.digest.is_wrapped() => {
                OtherRows::Decided(missing_classification(MissingKind::StoreWrapped))
            }
            (None, Some(latest)) if !latest.digest.is_alive() => {
                let highest = ctx.epoch_store.highest_handler_processed_row(&id)?;
                classify_deletion(highest.as_ref().map(|(_, row)| row), self.horizon)
            }
            (None, Some(latest)) => match read_initial_shared_version(ctx, &id)? {
                Some(initial) => OtherRows::Shared(initial),
                None => OtherRows::Decided(classify_live_latest(&latest, &self.input)),
            },
        };
        let classification = match class {
            OtherRows::Decided(classification) => classification,
            OtherRows::Shared(initial) => {
                let creation_row = ctx
                    .epoch_store
                    .handler_processed_object(&ObjectKey(id, initial))?;
                classify_shared_named_as_owned(creation_row.as_ref(), self.horizon).into()
            }
        };
        Ok(match classification {
            Classification::Keep => RowsAboveLookup::NeedBytes(self.into_state(NeedBytes)),
            Classification::Missing(reason) => RowsAboveLookup::Missing(reason),
            Classification::Drop(reason) => RowsAboveLookup::Drop(reason),
        })
    }
}

/// The initial shared version of `id`'s latest object, `None` when it is not
/// shared. An object deleted since the store answer shows its owner in its
/// deletion row.
fn read_initial_shared_version(
    ctx: &CommitIndexedReader,
    id: &ObjectId,
) -> IotaResult<Option<Version>> {
    if let Some(object) = ctx.cache.try_get_object(id)? {
        return Ok(match object.owner {
            Owner::Shared(initial) => Some(initial),
            _ => None,
        });
    }
    Ok(ctx
        .epoch_store
        .highest_handler_processed_row(id)?
        .and_then(|(_, row)| row.initial_shared_version))
}

// ---------------------------------------------------------------------------
// The rows above V, then the object
// ---------------------------------------------------------------------------

/// The row at `(id, V)` kept the input. The rows of `id` above `V` have not
/// been read.
pub struct LiveAtVersion;
impl OwnedState for LiveAtVersion {}

/// Outcome of the read of the rows above `V`.
#[must_use]
pub enum RowsAboveLookup {
    /// A keep. Next: load the object.
    NeedBytes(OwnedReader<NeedBytes>),
    /// Only from [`OwnedReader::read_other_rows`]: `V` is not produced at the
    /// horizon, or `id` was not live there.
    Missing(MissingReason),
    Drop(DropReason),
}

impl OwnedReader<LiveAtVersion> {
    /// Reads the lowest row of `id` above `V`: the rows held in memory
    /// first, then the table. Decides.
    pub fn read_rows_above(self, ctx: &CommitIndexedReader) -> IotaResult<RowsAboveLookup> {
        let row_above = ctx
            .epoch_store
            .lowest_handler_processed_row_above(&self.input.object_id, self.input.version)?;
        Ok(
            match classify_row_above(row_above.map(|(_, row)| row).as_ref(), self.horizon) {
                Some(reason) => RowsAboveLookup::Drop(reason),
                None => RowsAboveLookup::NeedBytes(self.into_state(NeedBytes)),
            },
        )
    }
}

/// A keep so far: the object at `(id, V)` is loaded next, and its digest can
/// still drop the input. Only a keep result creates this step, so an object
/// is loaded only for an input kept so far.
pub struct NeedBytes;
impl OwnedState for NeedBytes {}

impl OwnedReader<NeedBytes> {
    /// The named reference to load by exact key. Consumes this step, so the
    /// object is loaded once.
    pub fn into_reference(self) -> ObjectReference {
        self.input
    }
}

// ---------------------------------------------------------------------------
// Decisions on what a read returned
// ---------------------------------------------------------------------------

/// The result one read gave for `(id, V)`.
#[must_use]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Classification {
    Keep,
    Missing(MissingReason),
    Drop(DropReason),
}

/// Decides from the row at `(id, V)`: first the horizon, then the kind, then
/// the digest, then whether the row is a shared object's creation row.
fn classify_row(
    row: &HandlerProcessedObject,
    input: &ObjectReference,
    horizon: CommitIndex,
) -> Classification {
    if row.produced_at > horizon {
        return Classification::Missing(MissingReason(MissingKind::HandlerRowAboveHorizon));
    }
    match row.kind {
        HandlerProcessedObjectKind::Deleted | HandlerProcessedObjectKind::Wrapped => {
            Classification::Drop(DropReason(DropKind::HandlerRowTombstone))
        }
        HandlerProcessedObjectKind::Live if row.digest != input.digest => {
            Classification::Drop(DropReason(DropKind::HandlerRowDigestMismatch))
        }
        HandlerProcessedObjectKind::Live if row.initial_shared_version.is_some() => {
            Classification::Drop(DropReason(DropKind::HandlerRowSharedObject))
        }
        HandlerProcessedObjectKind::Live => Classification::Keep,
    }
}

/// The lowest row of `id` above a kept `V`: one produced at or below the
/// horizon consumed `V` and drops it; otherwise `None`, and the keep stands.
fn classify_row_above(
    row: Option<&HandlerProcessedObject>,
    horizon: CommitIndex,
) -> Option<DropReason> {
    row.filter(|row| row.produced_at <= horizon)
        .map(|_| DropReason(DropKind::HandlerRowSuperseded))
}

/// Whether the kept store answer is the named reference. A tombstone never
/// is, so a transaction naming a tombstone reference never keeps from the
/// store.
fn is_named_reference(latest: Option<&ObjectReference>, input: &ObjectReference) -> bool {
    latest.is_some_and(|latest| latest.digest.is_alive() && latest == input)
}

fn missing_classification(kind: MissingKind) -> Classification {
    Classification::Missing(MissingReason(kind))
}

fn drop_classification(kind: DropKind) -> Classification {
    Classification::Drop(DropReason(kind))
}

/// What the other rows of `id` decided about `(id, V)`.
#[must_use]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OtherRows {
    Decided(Classification),
    /// A shared object with this initial shared version: its creation row
    /// decides.
    Shared(Version),
}

/// The id's lowest row above `V`, with no row at `(id, V)`. One at or below
/// the horizon consumed `V`, or superseded a `V` that never existed. One
/// above the horizon decides from the version it replaced, see the module
/// doc.
fn classify_lowest_row_above(
    row: &HandlerProcessedObject,
    version: Version,
    horizon: CommitIndex,
) -> OtherRows {
    if row.produced_at <= horizon {
        let kind = if row.replaced_version == Some(version) {
            DropKind::HandlerRowSuperseded
        } else {
            DropKind::StoreSuperseded
        };
        return OtherRows::Decided(drop_classification(kind));
    }
    match (row.initial_shared_version, row.replaced_version) {
        // A shared object's deletion: its versions moved without rows.
        (Some(initial), Some(_)) => OtherRows::Shared(initial),
        (_, Some(replaced)) => OtherRows::Decided(match replaced.cmp(&version) {
            Ordering::Equal => Classification::Keep,
            Ordering::Greater => drop_classification(DropKind::StoreSuperseded),
            Ordering::Less => missing_classification(MissingKind::VersionNotLiveAtHorizon),
        }),
        // A creation or an unwrap.
        (_, None) => {
            OtherRows::Decided(missing_classification(MissingKind::VersionNotLiveAtHorizon))
        }
    }
}

/// The id's highest row, with a deletion tombstone as the store answer and no
/// row above `V`. A deletion above the horizon leaves `id` live below `V` at
/// the horizon. Any other deletion every validator has seen.
fn classify_deletion(row: Option<&HandlerProcessedObject>, horizon: CommitIndex) -> OtherRows {
    match row {
        Some(row)
            if row.kind == HandlerProcessedObjectKind::Deleted && row.produced_at > horizon =>
        {
            match row.initial_shared_version {
                Some(initial) => OtherRows::Shared(initial),
                None => {
                    OtherRows::Decided(missing_classification(MissingKind::VersionNotLiveAtHorizon))
                }
            }
        }
        _ => OtherRows::Decided(drop_classification(DropKind::StoreSuperseded)),
    }
}

/// A live store answer that is not the named reference, with no row above
/// `V` and an object that is not shared. A newer version has no row, so it
/// is the epoch-start version every validator holds.
fn classify_live_latest(latest: &ObjectReference, input: &ObjectReference) -> Classification {
    match latest.version.cmp(&input.version) {
        Ordering::Greater => drop_classification(DropKind::StoreSuperseded),
        Ordering::Less => missing_classification(MissingKind::StoreBelowVersion),
        Ordering::Equal if latest.digest == input.digest => Classification::Keep,
        Ordering::Equal => drop_classification(DropKind::StoreDigestMismatch),
    }
}

/// The verdict for a shared object named as owned, from its creation row.
#[must_use]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SharedNamedAsOwned {
    /// Created above the horizon: `id` did not exist at the horizon.
    Missing(MissingReason),
    Drop(DropReason),
}

impl From<SharedNamedAsOwned> for Classification {
    fn from(verdict: SharedNamedAsOwned) -> Self {
        match verdict {
            SharedNamedAsOwned::Missing(reason) => Classification::Missing(reason),
            SharedNamedAsOwned::Drop(reason) => Classification::Drop(reason),
        }
    }
}

/// A shared object named as owned, whatever its version: its creation row
/// at its initial shared version decides, none meaning it was created before
/// the epoch.
pub(super) fn classify_shared_named_as_owned(
    creation_row: Option<&HandlerProcessedObject>,
    horizon: CommitIndex,
) -> SharedNamedAsOwned {
    match creation_row {
        Some(row) if row.produced_at > horizon => {
            SharedNamedAsOwned::Missing(MissingReason(MissingKind::SharedCreationAboveHorizon))
        }
        _ => SharedNamedAsOwned::Drop(DropReason(DropKind::SharedObjectNamedAsOwned)),
    }
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::{ObjectDigest, ObjectId, Version};

    use super::*;

    const HORIZON: CommitIndex = 10;

    fn named(version: u64) -> ObjectReference {
        ObjectReference::new(
            ObjectId::random(),
            Version::from_u64(version),
            ObjectDigest::random(),
        )
    }

    fn row(
        kind: HandlerProcessedObjectKind,
        digest: ObjectDigest,
        produced_at: CommitIndex,
    ) -> HandlerProcessedObject {
        HandlerProcessedObject {
            digest,
            kind,
            produced_at,
            initial_shared_version: None,
            replaced_version: None,
            deleted_by: None,
        }
    }

    fn missing(kind: MissingKind) -> Classification {
        Classification::Missing(MissingReason(kind))
    }

    /// A transaction in commit 11 mutates object `x` to version 8. Commit 12
    /// is validated with horizon 10, so version 8 must not be visible yet.
    /// The first read finds no row at `(x, 8)`. Commit 11 then executes, and
    /// writes its row before the new version reaches the store. The store now
    /// returns version 8, so the store answer alone would keep it, but the
    /// second read of the row finds the row and answers missing, the same as
    /// a fresh read.
    #[tokio::test]
    async fn recheck_finds_the_row_of_a_write_the_store_answer_reflects() {
        use iota_sdk_types::{Address, Owner, SenderSignedTransaction};
        use iota_test_transaction_builder::TestTransactionBuilder;
        use iota_types::{
            effects::{TestEffectsBuilder, TransactionEffectsAPI},
            object::Object,
            transaction::TransactionKey,
        };

        use crate::{
            authority::{
                authority_per_epoch_store::handler_object_state::handler_processed_upserts,
                authority_tests::init_state_with_objects_and_object_basics,
            },
            post_consensus_input_reader::OwnedVerdict,
        };
        let _guard =
            iota_protocol_config::ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
                config.enable_pcool_deterministic_validation_for_testing();
                config
            });

        const PRODUCING_COMMIT: CommitIndex = HORIZON + 1;

        let sender = Address::ZERO;
        let owner = Owner::Address(sender);
        let gas_id = ObjectId::random();
        let x_id = ObjectId::random();
        let gas = Object::with_id_owner_version_for_testing(gas_id, Version::from_u64(7), owner);
        let x_before = Object::with_id_owner_version_for_testing(x_id, Version::from_u64(7), owner);
        let (authority, _) =
            init_state_with_objects_and_object_basics([gas.clone(), x_before]).await;
        let epoch_store = authority.epoch_store_for_testing().clone();

        let x_after = Object::with_id_owner_version_for_testing(x_id, Version::from_u64(8), owner);
        let input = x_after.object_ref();
        let transaction = SenderSignedTransaction::new(
            TestTransactionBuilder::new(sender, gas.object_ref(), 0)
                .transfer_iota(None, sender)
                .build(),
            vec![],
        );
        let effects = TestEffectsBuilder::new(&transaction)
            .with_mutated_objects([(x_id, Version::from_u64(7), owner)])
            .build();
        assert_eq!(effects.lamport_version(), input.version);
        let key = TransactionKey::Digest(*effects.transaction_digest());

        let ctx = CommitIndexedReader::new(
            authority.get_object_cache_reader().clone(),
            epoch_store.clone(),
            PRODUCING_COMMIT + 1,
        );

        let no_tables_entry = match OwnedReader::start(input, HORIZON)
            .read_handler_row(&ctx)
            .unwrap()
        {
            TablesLookup::NoEntry(no_tables_entry) => no_tables_entry,
            _ => panic!("no row before commit 11 executes"),
        };

        // Commit 11 executes: the row first, then the outputs.
        epoch_store.assign_commit_to_transactions(PRODUCING_COMMIT, vec![key]);
        epoch_store
            .record_commit_fully_executed(
                PRODUCING_COMMIT,
                &handler_processed_upserts(&effects, PRODUCING_COMMIT),
            )
            .unwrap();
        authority.insert_genesis_object(x_after);

        match no_tables_entry
            .read_store(&ctx)
            .unwrap()
            .reread_handler_row(&ctx)
            .unwrap()
        {
            TablesRecheck::Missing(reason) => {
                assert_eq!(reason.kind(), MissingKind::HandlerRowAboveHorizon)
            }
            TablesRecheck::LiveAtVersion(_) | TablesRecheck::NeedBytes(_) => {
                panic!("kept a version produced above the horizon")
            }
            TablesRecheck::Drop(reason) => panic!("dropped: {reason:?}"),
            TablesRecheck::NotAtVersion(_) => panic!("the store holds the named version"),
        }

        assert!(matches!(
            ctx.read_owned(input).unwrap(),
            OwnedVerdict::Missing(reason) if reason.kind() == MissingKind::HandlerRowAboveHorizon
        ));
    }

    /// Object `x` is at version 7 when the epoch starts, so there is no row
    /// for version 7. A transaction in commit 11, above the horizon of commit
    /// 12, replaces version 7 without a lock, as a receive does, and writes
    /// version 8. A validator that has not executed commit 11 still holds
    /// version 7 as the latest and keeps it. A validator that has executed it
    /// holds the row for version 8 and version 8 as the latest, and must keep
    /// version 7 too. Once commit 11 is at the horizon, its row drops version
    /// 7.
    ///
    /// Version 6 of `x`, replaced before the epoch, drops on both validators
    /// even though a row above it exists. So does version 3 of `y`, replaced
    /// before the epoch with no row above it.
    #[tokio::test]
    async fn epoch_start_version_consumed_above_the_horizon_keeps_as_on_a_validator_behind() {
        use iota_sdk_types::{Address, Owner, SenderSignedTransaction};
        use iota_test_transaction_builder::TestTransactionBuilder;
        use iota_types::{
            effects::{TestEffectsBuilder, TransactionEffectsAPI},
            object::Object,
            transaction::TransactionKey,
        };

        use crate::{
            authority::{
                authority_per_epoch_store::handler_object_state::handler_processed_upserts,
                authority_tests::init_state_with_objects_and_object_basics,
            },
            post_consensus_input_reader::OwnedVerdict,
        };
        let _guard =
            iota_protocol_config::ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
                config.enable_pcool_deterministic_validation_for_testing();
                config
            });

        const CONSUMING_COMMIT: CommitIndex = HORIZON + 1;

        let owner = Owner::Address(Address::ZERO);
        let gas_id = ObjectId::random();
        let x_id = ObjectId::random();
        let y_id = ObjectId::random();
        let object = |id, version| {
            Object::with_id_owner_version_for_testing(id, Version::from_u64(version), owner)
        };
        let gas = object(gas_id, 7);
        let x_stale = object(x_id, 6);
        let x_epoch_start = object(x_id, 7);
        let y_stale = object(y_id, 3);
        let (authority, _) = init_state_with_objects_and_object_basics([
            gas.clone(),
            x_stale.clone(),
            y_stale.clone(),
        ])
        .await;
        let epoch_store = authority.epoch_store_for_testing().clone();
        authority.insert_genesis_object(x_epoch_start.clone());
        authority.insert_genesis_object(object(y_id, 4));

        let read_at = |commit_index, input| {
            CommitIndexedReader::new(
                authority.get_object_cache_reader().clone(),
                epoch_store.clone(),
                commit_index,
            )
            .read_owned(input)
            .unwrap()
        };
        let assert_keeps = |verdict: OwnedVerdict, input: ObjectReference| match verdict {
            OwnedVerdict::Keep(object) => assert_eq!(object.0.object_ref(), input),
            OwnedVerdict::Drop(reason) => panic!("dropped {input:?}: {reason:?}"),
            OwnedVerdict::Missing(reason) => panic!("missing {input:?}: {reason:?}"),
        };
        let assert_drops = |verdict: OwnedVerdict, kind: DropKind| {
            assert!(
                matches!(verdict, OwnedVerdict::Drop(reason) if reason.kind() == kind),
                "expected a drop with {kind:?}"
            )
        };

        // A validator that has not executed commit 11.
        assert_keeps(
            read_at(CONSUMING_COMMIT + 1, x_epoch_start.object_ref()),
            x_epoch_start.object_ref(),
        );
        assert_drops(
            read_at(CONSUMING_COMMIT + 1, x_stale.object_ref()),
            DropKind::StoreSuperseded,
        );
        assert_drops(
            read_at(CONSUMING_COMMIT + 1, y_stale.object_ref()),
            DropKind::StoreSuperseded,
        );

        // Commit 11 executes here: the row for version 8 of `x`, then the
        // object.
        let consumer = SenderSignedTransaction::new(
            TestTransactionBuilder::new(Address::ZERO, gas.object_ref(), 0)
                .transfer_iota(None, Address::ZERO)
                .build(),
            vec![],
        );
        let effects = TestEffectsBuilder::new(&consumer)
            .with_mutated_objects([(x_id, x_epoch_start.version(), owner)])
            .build();
        let key = TransactionKey::Digest(*effects.transaction_digest());
        epoch_store.assign_commit_to_transactions(CONSUMING_COMMIT, vec![key]);
        epoch_store
            .record_commit_fully_executed(
                CONSUMING_COMMIT,
                &handler_processed_upserts(&effects, CONSUMING_COMMIT),
            )
            .unwrap();
        authority.insert_genesis_object(object(x_id, effects.lamport_version().as_u64()));

        // The same reads on this validator.
        assert_keeps(
            read_at(CONSUMING_COMMIT + 1, x_epoch_start.object_ref()),
            x_epoch_start.object_ref(),
        );
        assert_drops(
            read_at(CONSUMING_COMMIT + 1, x_stale.object_ref()),
            DropKind::StoreSuperseded,
        );
        assert_drops(
            read_at(CONSUMING_COMMIT + 1, y_stale.object_ref()),
            DropKind::StoreSuperseded,
        );

        // Commit 11 at the horizon.
        assert_drops(
            read_at(CONSUMING_COMMIT + 2, x_epoch_start.object_ref()),
            DropKind::HandlerRowSuperseded,
        );
    }

    /// `X@7` is the epoch-start version of `X`. Commit 11, above the horizon
    /// of commit 12, replaces it with `X@11` and creates `Z@11`. `X@9`, a
    /// version between the two, and `Z@5`, below the creation, do not exist
    /// at the horizon: a validator that has not executed commit 11 answers
    /// missing, and one that has must too. Once commit 11 is at the horizon,
    /// both drop.
    #[tokio::test]
    async fn versions_not_live_at_the_horizon_are_missing_after_a_write_above_it() {
        use iota_sdk_types::{Address, Owner, SenderSignedTransaction};
        use iota_test_transaction_builder::TestTransactionBuilder;
        use iota_types::{
            effects::{TestEffectsBuilder, TransactionEffectsAPI},
            object::Object,
            transaction::TransactionKey,
        };

        use crate::{
            authority::{
                authority_per_epoch_store::handler_object_state::handler_processed_upserts,
                authority_tests::init_state_with_objects_and_object_basics,
            },
            post_consensus_input_reader::OwnedVerdict,
        };
        let _guard =
            iota_protocol_config::ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
                config.enable_pcool_deterministic_validation_for_testing();
                config
            });

        const WRITING_COMMIT: CommitIndex = HORIZON + 1;

        let owner = Owner::Address(Address::ZERO);
        let x_id = ObjectId::random();
        let z_id = ObjectId::random();
        let object = |id, version| {
            Object::with_id_owner_version_for_testing(id, Version::from_u64(version), owner)
        };
        let gas = object(ObjectId::random(), 10);
        let (authority, _) =
            init_state_with_objects_and_object_basics([gas.clone(), object(x_id, 7)]).await;
        let epoch_store = authority.epoch_store_for_testing().clone();
        let x_between = ObjectReference::new(x_id, Version::from_u64(9), ObjectDigest::random());
        let z_below = ObjectReference::new(z_id, Version::from_u64(5), ObjectDigest::random());

        let read_at = |commit_index, input| {
            CommitIndexedReader::new(
                authority.get_object_cache_reader().clone(),
                epoch_store.clone(),
                commit_index,
            )
            .read_owned(input)
            .unwrap()
        };
        let assert_missing = |verdict: OwnedVerdict, kind: MissingKind| {
            assert!(
                matches!(verdict, OwnedVerdict::Missing(reason) if reason.kind() == kind),
                "expected missing with {kind:?}"
            )
        };
        let assert_drops = |verdict: OwnedVerdict, kind: DropKind| {
            assert!(
                matches!(verdict, OwnedVerdict::Drop(reason) if reason.kind() == kind),
                "expected a drop with {kind:?}"
            )
        };

        // A validator that has not executed commit 11.
        assert_missing(
            read_at(WRITING_COMMIT + 1, x_between),
            MissingKind::StoreBelowVersion,
        );
        assert_missing(
            read_at(WRITING_COMMIT + 1, z_below),
            MissingKind::StoreNotFound,
        );

        // Commit 11 executes here: the rows, then X@11 and Z@11.
        let writer = SenderSignedTransaction::new(
            TestTransactionBuilder::new(Address::ZERO, gas.object_ref(), 0)
                .transfer_iota(None, Address::ZERO)
                .build(),
            vec![],
        );
        let effects = TestEffectsBuilder::new(&writer)
            .with_mutated_objects([(x_id, Version::from_u64(7), owner)])
            .with_created_objects([(z_id, owner)])
            .build();
        let written = effects.lamport_version().as_u64();
        assert_eq!(written, 11);
        let key = TransactionKey::Digest(*effects.transaction_digest());
        epoch_store.assign_commit_to_transactions(WRITING_COMMIT, vec![key]);
        epoch_store
            .record_commit_fully_executed(
                WRITING_COMMIT,
                &handler_processed_upserts(&effects, WRITING_COMMIT),
            )
            .unwrap();
        authority.insert_genesis_object(object(x_id, written));
        authority.insert_genesis_object(object(z_id, written));

        // The same reads on this validator.
        assert_missing(
            read_at(WRITING_COMMIT + 1, x_between),
            MissingKind::VersionNotLiveAtHorizon,
        );
        assert_missing(
            read_at(WRITING_COMMIT + 1, z_below),
            MissingKind::VersionNotLiveAtHorizon,
        );

        // Commit 11 at the horizon.
        assert_drops(
            read_at(WRITING_COMMIT + 2, x_between),
            DropKind::StoreSuperseded,
        );
        assert_drops(
            read_at(WRITING_COMMIT + 2, z_below),
            DropKind::StoreSuperseded,
        );
    }

    fn drop(kind: DropKind) -> Classification {
        Classification::Drop(DropReason(kind))
    }

    #[test]
    fn row_at_the_horizon_decides_and_one_above_answers_missing() {
        let input = named(5);
        let live = |produced_at| row(HandlerProcessedObjectKind::Live, input.digest, produced_at);
        assert_eq!(
            classify_row(&live(HORIZON), &input, HORIZON),
            Classification::Keep
        );
        assert_eq!(
            classify_row(&live(HORIZON + 1), &input, HORIZON),
            missing(MissingKind::HandlerRowAboveHorizon)
        );
    }

    #[test]
    fn row_above_the_horizon_answers_missing_before_kind_and_digest() {
        let input = named(5);
        let tombstone = row(
            HandlerProcessedObjectKind::Deleted,
            ObjectDigest::OBJECT_DELETED,
            HORIZON + 1,
        );
        let other_digest = row(
            HandlerProcessedObjectKind::Live,
            ObjectDigest::random(),
            HORIZON + 1,
        );
        assert_eq!(
            classify_row(&tombstone, &input, HORIZON),
            missing(MissingKind::HandlerRowAboveHorizon)
        );
        assert_eq!(
            classify_row(&other_digest, &input, HORIZON),
            missing(MissingKind::HandlerRowAboveHorizon)
        );
    }

    #[test]
    fn tombstone_rows_drop() {
        let input = named(5);
        for (kind, digest) in [
            (
                HandlerProcessedObjectKind::Deleted,
                ObjectDigest::OBJECT_DELETED,
            ),
            (
                HandlerProcessedObjectKind::Wrapped,
                ObjectDigest::OBJECT_WRAPPED,
            ),
        ] {
            assert_eq!(
                classify_row(&row(kind, digest, HORIZON), &input, HORIZON),
                drop(DropKind::HandlerRowTombstone)
            );
        }
    }

    #[test]
    fn live_row_with_another_digest_drops() {
        let input = named(5);
        let other_digest = row(
            HandlerProcessedObjectKind::Live,
            ObjectDigest::random(),
            HORIZON,
        );
        assert_eq!(
            classify_row(&other_digest, &input, HORIZON),
            drop(DropKind::HandlerRowDigestMismatch)
        );
    }

    #[test]
    fn shared_creation_row_drops_after_the_horizon_and_digest_checks() {
        let input = named(5);
        let shared = |digest, produced_at| HandlerProcessedObject {
            initial_shared_version: Some(Version::from_u64(5)),
            ..row(HandlerProcessedObjectKind::Live, digest, produced_at)
        };
        assert_eq!(
            classify_row(&shared(input.digest, HORIZON), &input, HORIZON),
            drop(DropKind::HandlerRowSharedObject)
        );
        assert_eq!(
            classify_row(&shared(ObjectDigest::random(), HORIZON), &input, HORIZON),
            drop(DropKind::HandlerRowDigestMismatch)
        );
        assert_eq!(
            classify_row(&shared(input.digest, HORIZON + 1), &input, HORIZON),
            missing(MissingKind::HandlerRowAboveHorizon)
        );
    }

    #[test]
    fn lowest_row_above_at_the_horizon_drops_and_above_it_keeps() {
        let above = |kind, produced_at| row(kind, ObjectDigest::random(), produced_at);
        for kind in [
            HandlerProcessedObjectKind::Live,
            HandlerProcessedObjectKind::Deleted,
            HandlerProcessedObjectKind::Wrapped,
        ] {
            assert_eq!(
                classify_row_above(Some(&above(kind, HORIZON)), HORIZON),
                Some(DropReason(DropKind::HandlerRowSuperseded))
            );
            assert_eq!(
                classify_row_above(Some(&above(kind, HORIZON + 1)), HORIZON),
                None
            );
        }
        assert_eq!(classify_row_above(None, HORIZON), None);
    }

    #[test]
    fn store_keeps_only_the_named_live_reference() {
        let input = named(5);
        assert!(is_named_reference(Some(&input), &input));
        assert!(!is_named_reference(None, &input));
        let id = input.object_id;
        for digest in [ObjectDigest::OBJECT_DELETED, ObjectDigest::OBJECT_WRAPPED] {
            let tombstone = ObjectReference::new(id, Version::from_u64(7), digest);
            assert!(!is_named_reference(Some(&tombstone), &tombstone));
        }
    }

    #[test]
    fn live_store_answer_against_the_named_reference() {
        let input = named(5);
        let at = |version: u64, digest| {
            ObjectReference::new(input.object_id, Version::from_u64(version), digest)
        };
        assert_eq!(
            classify_live_latest(&at(6, ObjectDigest::random()), &input),
            drop(DropKind::StoreSuperseded)
        );
        assert_eq!(
            classify_live_latest(&at(4, ObjectDigest::random()), &input),
            missing(MissingKind::StoreBelowVersion)
        );
        assert_eq!(
            classify_live_latest(&at(5, ObjectDigest::random()), &input),
            drop(DropKind::StoreDigestMismatch)
        );
    }

    #[test]
    fn lowest_row_above_at_or_below_the_horizon_drops() {
        let replacing = |replaced: Option<u64>| HandlerProcessedObject {
            replaced_version: replaced.map(Version::from_u64),
            ..row(
                HandlerProcessedObjectKind::Live,
                ObjectDigest::random(),
                HORIZON,
            )
        };
        let v = Version::from_u64(5);
        assert_eq!(
            classify_lowest_row_above(&replacing(Some(5)), v, HORIZON),
            OtherRows::Decided(drop(DropKind::HandlerRowSuperseded))
        );
        for replaced in [Some(3), Some(6), None] {
            assert_eq!(
                classify_lowest_row_above(&replacing(replaced), v, HORIZON),
                OtherRows::Decided(drop(DropKind::StoreSuperseded))
            );
        }
    }

    /// Above the horizon, the version the row replaced is what `id` was at
    /// the horizon as far as `V` is concerned.
    #[test]
    fn lowest_row_above_the_horizon_decides_from_the_replaced_version() {
        let replacing = |replaced: Option<u64>| HandlerProcessedObject {
            replaced_version: replaced.map(Version::from_u64),
            ..row(
                HandlerProcessedObjectKind::Live,
                ObjectDigest::random(),
                HORIZON + 1,
            )
        };
        let v = Version::from_u64(5);
        assert_eq!(
            classify_lowest_row_above(&replacing(Some(5)), v, HORIZON),
            OtherRows::Decided(Classification::Keep)
        );
        assert_eq!(
            classify_lowest_row_above(&replacing(Some(6)), v, HORIZON),
            OtherRows::Decided(drop(DropKind::StoreSuperseded))
        );
        for replaced in [Some(3), None] {
            assert_eq!(
                classify_lowest_row_above(&replacing(replaced), v, HORIZON),
                OtherRows::Decided(missing(MissingKind::VersionNotLiveAtHorizon))
            );
        }
        let shared_deletion = HandlerProcessedObject {
            initial_shared_version: Some(Version::from_u64(2)),
            ..replacing(Some(5))
        };
        assert_eq!(
            classify_lowest_row_above(&shared_deletion, v, HORIZON),
            OtherRows::Shared(Version::from_u64(2))
        );
    }

    #[test]
    fn deletion_above_the_horizon_answers_missing() {
        let deletion = |produced_at, initial_shared_version: Option<u64>| HandlerProcessedObject {
            initial_shared_version: initial_shared_version.map(Version::from_u64),
            replaced_version: Some(Version::from_u64(4)),
            ..row(
                HandlerProcessedObjectKind::Deleted,
                ObjectDigest::OBJECT_DELETED,
                produced_at,
            )
        };
        assert_eq!(
            classify_deletion(Some(&deletion(HORIZON + 1, None)), HORIZON),
            OtherRows::Decided(missing(MissingKind::VersionNotLiveAtHorizon))
        );
        assert_eq!(
            classify_deletion(Some(&deletion(HORIZON + 1, Some(2))), HORIZON),
            OtherRows::Shared(Version::from_u64(2))
        );
        assert_eq!(
            classify_deletion(Some(&deletion(HORIZON, None)), HORIZON),
            OtherRows::Decided(drop(DropKind::StoreSuperseded))
        );
        assert_eq!(
            classify_deletion(None, HORIZON),
            OtherRows::Decided(drop(DropKind::StoreSuperseded))
        );
    }

    #[test]
    fn shared_object_named_as_owned_is_missing_only_while_created_above_the_horizon() {
        let creation = |produced_at| HandlerProcessedObject {
            initial_shared_version: Some(Version::from_u64(2)),
            ..row(
                HandlerProcessedObjectKind::Live,
                ObjectDigest::random(),
                produced_at,
            )
        };
        assert_eq!(
            classify_shared_named_as_owned(Some(&creation(HORIZON + 1)), HORIZON),
            SharedNamedAsOwned::Missing(MissingReason(MissingKind::SharedCreationAboveHorizon))
        );
        for creation_row in [Some(creation(HORIZON)), None] {
            assert_eq!(
                classify_shared_named_as_owned(creation_row.as_ref(), HORIZON),
                SharedNamedAsOwned::Drop(DropReason(DropKind::SharedObjectNamedAsOwned))
            );
        }
    }
}
