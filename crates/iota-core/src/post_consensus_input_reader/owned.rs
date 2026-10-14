// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Reads one owned or immutable input, named by `(id, V)`. Each step is its
//! own type that offers only the next allowed read, so a read cannot be
//! skipped, repeated or taken out of order. The decisions on what a read
//! returned are plain functions.
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
//! With no row at `(id, V)`, a newer version or a tombstone in the store does
//! not drop `V` by itself: this validator may have executed a consumption of
//! `V` in a commit above the horizon that a slower validator has not, and
//! there `V` is still the latest version. So the lowest row of `id` above `V`
//! is read. `V` is kept only when that row's write replaced exactly `V` and
//! the row is above the horizon. Otherwise `V` drops: with no such row, or
//! with one that replaced another version, `V` was replaced before the epoch,
//! and a row at or below the horizon is a consumption every validator has
//! seen. The keep then needs `V`'s object, which this validator's pruner can
//! remove once the consuming checkpoint is executed. Loading it then fails
//! and the input drops here, while a validator that still holds it keeps the
//! input, unless pruning waits for the consuming commit to be flushed.

use std::cmp::Ordering;

use iota_sdk_types::{ObjectReference, Version};
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
/// read. Otherwise the kept store answer decides: the named reference keeps;
/// a newer version or a tombstone goes on to the rows above `V`; an older
/// version or nothing answers missing.
#[must_use]
pub enum TablesRecheck {
    /// A keep from a row that appeared since the first read. Next: the rows
    /// above `V`.
    LiveAtVersion(OwnedReader<LiveAtVersion>),
    /// A keep from the store. Next: the bytes.
    NeedBytes(OwnedReader<NeedBytes>),
    /// The store holds a newer version or a tombstone. Next: the rows above
    /// `V`.
    Superseded(OwnedReader<SupersededInStore>),
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
        Ok(
            match classify_latest(self.state.latest.as_ref(), &self.input) {
                LatestClass::Keep => TablesRecheck::NeedBytes(self.into_state(NeedBytes)),
                LatestClass::Superseded => {
                    TablesRecheck::Superseded(self.into_state(SupersededInStore))
                }
                LatestClass::Missing(reason) => TablesRecheck::Missing(reason),
                LatestClass::Drop(reason) => TablesRecheck::Drop(reason),
            },
        )
    }
}

// ---------------------------------------------------------------------------
// A newer store version: the lowest row above V
// ---------------------------------------------------------------------------

/// No row at `(id, V)`, and the store holds a newer version or a tombstone.
/// The rows of `id` above `V` have not been read.
pub struct SupersededInStore;
impl OwnedState for SupersededInStore {}

impl OwnedReader<SupersededInStore> {
    /// Reads the lowest row of `id` above `V`: the rows held in memory
    /// first, then the table. Decides.
    pub fn read_rows_above(self, ctx: &CommitIndexedReader) -> IotaResult<RowsAboveLookup> {
        let row_above = ctx
            .epoch_store
            .lowest_handler_processed_row_above(&self.input.object_id, self.input.version)?;
        Ok(
            match classify_superseded(
                row_above.map(|(_, row)| row).as_ref(),
                self.input.version,
                self.horizon,
            ) {
                Some(reason) => RowsAboveLookup::Drop(reason),
                None => RowsAboveLookup::NeedBytes(self.into_state(NeedBytes)),
            },
        )
    }
}

// ---------------------------------------------------------------------------
// The rows above V, then the object
// ---------------------------------------------------------------------------

/// The row at `(id, V)` kept the input. The rows of `id` above `V` have not
/// been read.
pub struct LiveAtVersion;
impl OwnedState for LiveAtVersion {}

/// Outcome of the read of the lowest row above `V`.
#[must_use]
pub enum RowsAboveLookup {
    /// No row above `V`, or the lowest one is above the horizon. Next: load
    /// the object.
    NeedBytes(OwnedReader<NeedBytes>),
    /// The lowest row above `V` is at or below the horizon.
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

/// What the kept store answer decided about `(id, V)`.
#[must_use]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LatestClass {
    Keep,
    /// A newer version or a tombstone. The rows above `V` decide.
    Superseded,
    Missing(MissingReason),
    Drop(DropReason),
}

/// Decides from the kept store answer: the latest reference against the named
/// one. No entry answers missing: the id is not visible here, as for a
/// package the store lacks. A tombstone is checked before equality, so a
/// transaction that names a tombstone reference never keeps from the store.
fn classify_latest(latest: Option<&ObjectReference>, input: &ObjectReference) -> LatestClass {
    let Some(latest) = latest else {
        return LatestClass::Missing(MissingReason(MissingKind::StoreNotFound));
    };
    if !latest.digest.is_alive() {
        return LatestClass::Superseded;
    }
    match latest.version.cmp(&input.version) {
        Ordering::Greater => LatestClass::Superseded,
        Ordering::Less => LatestClass::Missing(MissingReason(MissingKind::StoreBelowVersion)),
        Ordering::Equal if latest.digest == input.digest => LatestClass::Keep,
        Ordering::Equal => LatestClass::Drop(DropReason(DropKind::StoreDigestMismatch)),
    }
}

/// The id's lowest row above a `V` the store superseded: kept (`None`) only
/// when the row's write replaced exactly `V` above the horizon. No row, or a
/// row that replaced another version, means `V` was superseded before the
/// epoch.
fn classify_superseded(
    row: Option<&HandlerProcessedObject>,
    version: Version,
    horizon: CommitIndex,
) -> Option<DropReason> {
    match row {
        Some(row) if row.replaced_version == Some(version) => {
            classify_row_above(Some(row), horizon)
        }
        Some(_) | None => Some(DropReason(DropKind::StoreSuperseded)),
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
                &[],
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
            TablesRecheck::Superseded(_) => panic!("the store holds the named version"),
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
                &[],
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
    fn store_without_an_entry_answers_missing() {
        assert_eq!(
            classify_latest(None, &named(5)),
            LatestClass::Missing(MissingReason(MissingKind::StoreNotFound))
        );
    }

    #[test]
    fn store_tombstone_is_superseded_even_when_the_transaction_names_it() {
        let id = ObjectId::random();
        for digest in [ObjectDigest::OBJECT_DELETED, ObjectDigest::OBJECT_WRAPPED] {
            let tombstone = ObjectReference::new(id, Version::from_u64(7), digest);
            let live_below = ObjectReference::new(id, Version::from_u64(5), ObjectDigest::random());
            assert_eq!(
                classify_latest(Some(&tombstone), &tombstone),
                LatestClass::Superseded
            );
            assert_eq!(
                classify_latest(Some(&tombstone), &live_below),
                LatestClass::Superseded
            );
        }
    }

    #[test]
    fn store_latest_against_the_named_reference() {
        let input = named(5);
        let at = |version: u64, digest| {
            ObjectReference::new(input.object_id, Version::from_u64(version), digest)
        };
        assert_eq!(classify_latest(Some(&input), &input), LatestClass::Keep);
        assert_eq!(
            classify_latest(Some(&at(6, ObjectDigest::random())), &input),
            LatestClass::Superseded
        );
        assert_eq!(
            classify_latest(Some(&at(4, ObjectDigest::random())), &input),
            LatestClass::Missing(MissingReason(MissingKind::StoreBelowVersion))
        );
        assert_eq!(
            classify_latest(Some(&at(5, ObjectDigest::random())), &input),
            LatestClass::Drop(DropReason(DropKind::StoreDigestMismatch))
        );
    }

    #[test]
    fn superseded_version_keeps_only_when_a_row_above_the_horizon_replaced_it() {
        let replacing = |replaced: Option<u64>, produced_at| HandlerProcessedObject {
            replaced_version: replaced.map(Version::from_u64),
            ..row(
                HandlerProcessedObjectKind::Live,
                ObjectDigest::random(),
                produced_at,
            )
        };
        let v = Version::from_u64(5);
        assert_eq!(
            classify_superseded(Some(&replacing(Some(5), HORIZON + 1)), v, HORIZON),
            None
        );
        assert_eq!(
            classify_superseded(Some(&replacing(Some(5), HORIZON)), v, HORIZON),
            Some(DropReason(DropKind::HandlerRowSuperseded))
        );
        for row in [
            replacing(Some(6), HORIZON + 1),
            replacing(None, HORIZON + 1),
        ] {
            assert_eq!(
                classify_superseded(Some(&row), v, HORIZON),
                Some(DropReason(DropKind::StoreSuperseded))
            );
        }
        assert_eq!(
            classify_superseded(None, v, HORIZON),
            Some(DropReason(DropKind::StoreSuperseded))
        );
    }
}
