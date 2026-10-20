// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Reads one owned or immutable input, named by `(id, V)`. Each step is its
//! own type that offers only the next allowed read, so a read cannot be
//! skipped, repeated or taken out of order. The decisions on what a read
//! returned are plain functions, used by both reads of the row.
//!
//! The row at `(id, V)` decides when there is one. Without one, the store's
//! latest reference for `id` is read and kept, and the row is read once more
//! before that store answer may decide. The doc of the parent module explains
//! why one more read is enough.

use std::cmp::Ordering;

use iota_sdk_types::ObjectReference;
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
    /// The horizon, `C - K`. A row produced above it answers missing.
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
    /// A live row at or below the horizon with the named digest. Next: load
    /// the object.
    NeedBytes(OwnedReader<NeedBytes>),
    /// A row produced above the horizon. The store is not asked.
    Missing(MissingReason),
    /// A tombstone row or a `Live` row with another digest.
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
            Classification::Keep => TablesLookup::NeedBytes(self.into_state(NeedBytes)),
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
/// an older version is missing, and a newer version, a tombstone or nothing
/// drops.
#[must_use]
pub enum TablesRecheck {
    NeedBytes(OwnedReader<NeedBytes>),
    Missing(MissingReason),
    Drop(DropReason),
}

impl OwnedReader<StoreAnswered> {
    /// Reads the row at `(id, V)` again and decides.
    pub fn reread_handler_row(self, ctx: &CommitIndexedReader) -> IotaResult<TablesRecheck> {
        // A row decides. Without one, the kept store answer does.
        let classification = self
            .row_classification(ctx)?
            .unwrap_or_else(|| classify_latest(self.state.latest.as_ref(), &self.input));
        Ok(match classification {
            Classification::Keep => TablesRecheck::NeedBytes(self.into_state(NeedBytes)),
            Classification::Missing(reason) => TablesRecheck::Missing(reason),
            Classification::Drop(reason) => TablesRecheck::Drop(reason),
        })
    }
}

// ---------------------------------------------------------------------------
// Keep: the caller loads the object
// ---------------------------------------------------------------------------

/// The input is kept once its object at `(id, V)` is loaded. Only a keep
/// result creates this step, so an object is loaded only for a kept input.
pub struct NeedBytes;
impl OwnedState for NeedBytes {}

impl OwnedReader<NeedBytes> {
    /// The exact key to load. Consumes this step, so the object is loaded
    /// once.
    pub fn into_key(self) -> ObjectKey {
        self.key()
    }
}

// ---------------------------------------------------------------------------
// Decisions on what a read returned, used by both reads of the row
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
/// the digest.
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
        HandlerProcessedObjectKind::Live if row.digest == input.digest => Classification::Keep,
        HandlerProcessedObjectKind::Live => {
            Classification::Drop(DropReason(DropKind::HandlerRowDigestMismatch))
        }
    }
}

/// Decides from the kept store answer: the latest reference against the named
/// one. A tombstone is checked before equality, so a transaction that names a
/// tombstone reference drops instead of reaching the object load.
fn classify_latest(latest: Option<&ObjectReference>, input: &ObjectReference) -> Classification {
    let Some(latest) = latest else {
        return Classification::Drop(DropReason(DropKind::StoreNotFound));
    };
    if !latest.digest.is_alive() {
        return Classification::Drop(DropReason(DropKind::StoreSuperseded));
    }
    match latest.version.cmp(&input.version) {
        Ordering::Greater => Classification::Drop(DropReason(DropKind::StoreSuperseded)),
        Ordering::Less => Classification::Missing(MissingReason(MissingKind::StoreBelowVersion)),
        Ordering::Equal if latest.digest == input.digest => Classification::Keep,
        Ordering::Equal => Classification::Drop(DropReason(DropKind::StoreDigestMismatch)),
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
                config.set_pcool_deterministic_validation_for_testing(true);
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
            TablesRecheck::NeedBytes(_) => panic!("kept a version produced above the horizon"),
            TablesRecheck::Drop(reason) => panic!("dropped: {reason:?}"),
        }

        assert!(matches!(
            ctx.read_owned(input).unwrap(),
            OwnedVerdict::Missing(reason) if reason.kind() == MissingKind::HandlerRowAboveHorizon
        ));
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
    fn store_without_an_entry_drops() {
        assert_eq!(
            classify_latest(None, &named(5)),
            drop(DropKind::StoreNotFound)
        );
    }

    #[test]
    fn store_tombstone_drops_even_when_the_transaction_names_it() {
        let id = ObjectId::random();
        for digest in [ObjectDigest::OBJECT_DELETED, ObjectDigest::OBJECT_WRAPPED] {
            let tombstone = ObjectReference::new(id, Version::from_u64(7), digest);
            let live_below = ObjectReference::new(id, Version::from_u64(5), ObjectDigest::random());
            assert_eq!(
                classify_latest(Some(&tombstone), &tombstone),
                drop(DropKind::StoreSuperseded)
            );
            assert_eq!(
                classify_latest(Some(&tombstone), &live_below),
                drop(DropKind::StoreSuperseded)
            );
        }
    }

    #[test]
    fn store_latest_against_the_named_reference() {
        let input = named(5);
        let at = |version: u64, digest| {
            ObjectReference::new(input.object_id, Version::from_u64(version), digest)
        };
        assert_eq!(classify_latest(Some(&input), &input), Classification::Keep);
        assert_eq!(
            classify_latest(Some(&at(6, ObjectDigest::random())), &input),
            drop(DropKind::StoreSuperseded)
        );
        assert_eq!(
            classify_latest(Some(&at(4, ObjectDigest::random())), &input),
            missing(MissingKind::StoreBelowVersion)
        );
        assert_eq!(
            classify_latest(Some(&at(5, ObjectDigest::random())), &input),
            drop(DropKind::StoreDigestMismatch)
        );
    }
}
