// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The owned-input machine. One state per fact established about the named
//! `(id, version)` and one transition per read, so a read cannot be skipped,
//! repeated or taken out of order. The comparisons on what a read returned
//! are pure functions shared by the first pass and the re-check.

use std::cmp::Ordering;

use iota_sdk_types::{ObjectReference, Version};
use iota_types::{error::IotaResult, storage::ObjectKey};

use super::{DropKind, DropReason, MissingKind, MissingReason, reader::CommitIndexedReader};
use crate::authority::authority_per_epoch_store::handler_object_state::{
    CommitIndex, HandlerProcessedObject, HandlerProcessedObjectKind, SyncAheadRecord,
};

// ---------------------------------------------------------------------------
// Carrier
// ---------------------------------------------------------------------------

/// Marker for the owned machine's states.
pub trait OwnedState {}

/// One owned input being read at commit `C`. `S` is what the reader has
/// established so far. Each state exposes only the transition the algorithm
/// allows next, and every transition consumes the reader.
pub struct OwnedReader<S: OwnedState> {
    /// The `(id, version, digest)` the transaction names.
    input: ObjectReference,
    /// `C - K`. A row produced above it answers missing.
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

    /// Rule 1 against the current tables, `None` when no row exists at
    /// `(id, V)`. Shared by the first pass and the re-read.
    fn row_classification(&self, ctx: &CommitIndexedReader) -> IotaResult<Option<Classification>> {
        Ok(ctx
            .epoch_store
            .handler_processed_object(&self.key())?
            .map(|row| classify_row(&row, &self.input, self.horizon)))
    }

    /// Rule 2 against the current tables, `None` when no record exists for
    /// `id`. Shared by the first pass and the re-read.
    fn record_classification(
        &self,
        ctx: &CommitIndexedReader,
    ) -> IotaResult<Option<Classification>> {
        Ok(ctx
            .epoch_store
            .sync_ahead_record(&self.input.object_id)?
            .map(|record| classify_record(&record, self.input.version)))
    }
}

// ---------------------------------------------------------------------------
// Rule 1: the handler-processed row at (id, V)
// ---------------------------------------------------------------------------

/// Nothing read yet.
pub struct Start;
impl OwnedState for Start {}

/// Outcome of rule 1: the exact-key lookup at `(id, V)` and, on a hit, the
/// horizon, kind and digest of the row.
#[must_use]
pub enum HandlerRowLookup {
    /// A `Live` row at or below `C - K` with the named digest. Next: the
    /// bytes.
    NeedBytes(OwnedReader<NeedBytes>),
    /// A row above `C - K`. Never skipped.
    Missing(MissingReason),
    /// A tombstone row, or a `Live` row with another digest.
    Drop(DropReason),
    /// Not a verdict. No row, so the machine continues with the sync-ahead
    /// record.
    NoRow(OwnedReader<NoHandlerRow>),
}

impl OwnedReader<Start> {
    /// Begins the read of `input` at horizon `C - K`. The only constructor of
    /// a reader in any state.
    pub fn start(input: ObjectReference, horizon: CommitIndex) -> OwnedReader<Start> {
        OwnedReader {
            input,
            horizon,
            state: Start,
        }
    }

    /// Overlay first, then the table through the cache.
    pub fn read_handler_row(self, ctx: &CommitIndexedReader) -> IotaResult<HandlerRowLookup> {
        Ok(match self.row_classification(ctx)? {
            None => HandlerRowLookup::NoRow(self.into_state(NoHandlerRow)),
            Some(Classification::Keep) => HandlerRowLookup::NeedBytes(self.into_state(NeedBytes)),
            Some(Classification::Missing(reason)) => HandlerRowLookup::Missing(reason),
            Some(Classification::Drop(reason)) => HandlerRowLookup::Drop(reason),
        })
    }
}

// ---------------------------------------------------------------------------
// Rule 2: the sync-ahead record for id
// ---------------------------------------------------------------------------

/// No handler-processed row at `(id, V)`.
pub struct NoHandlerRow;
impl OwnedState for NoHandlerRow {}

/// Outcome of rule 2: the lookup by id and, on a hit, the record's
/// `base_version` against `V`.
#[must_use]
pub enum SyncAheadLookup {
    /// `base_version == Some(V)`. Next: the bytes.
    NeedBytes(OwnedReader<NeedBytes>),
    /// `base_version` is `None` or below `V`.
    Missing(MissingReason),
    /// `base_version` is above `V`.
    Drop(DropReason),
    /// Not a verdict. No record, so the machine continues with the store.
    NoRecord(OwnedReader<NoSyncAheadRecord>),
}

impl OwnedReader<NoHandlerRow> {
    /// Overlay first, then the table.
    pub fn read_sync_ahead_record(self, ctx: &CommitIndexedReader) -> IotaResult<SyncAheadLookup> {
        Ok(match self.record_classification(ctx)? {
            None => SyncAheadLookup::NoRecord(self.into_state(NoSyncAheadRecord)),
            Some(Classification::Keep) => SyncAheadLookup::NeedBytes(self.into_state(NeedBytes)),
            Some(Classification::Missing(reason)) => SyncAheadLookup::Missing(reason),
            Some(Classification::Drop(reason)) => SyncAheadLookup::Drop(reason),
        })
    }
}

// ---------------------------------------------------------------------------
// Rule 3: the store, then both tables again
// ---------------------------------------------------------------------------

/// Neither table knows `id`. The store has not been asked.
pub struct NoSyncAheadRecord;
impl OwnedState for NoSyncAheadRecord {}

impl OwnedReader<NoSyncAheadRecord> {
    /// The latest reference or tombstone of `id`. The answer is held, not
    /// decided: both tables must be read again first.
    pub fn read_store(self, ctx: &CommitIndexedReader) -> IotaResult<OwnedReader<StoreAnswered>> {
        let latest = ctx
            .cache
            .try_get_latest_object_ref_or_tombstone(self.input.object_id)?;
        Ok(self.into_state(StoreAnswered { latest }))
    }
}

/// The store answered with the latest reference, a tombstone, or nothing.
/// Held until both tables are re-read.
pub struct StoreAnswered {
    latest: Option<ObjectReference>,
}
impl OwnedState for StoreAnswered {}

/// Outcome of re-reading the handler-processed row after the store answered.
/// A row that landed in the window decides as in rule 1.
#[must_use]
pub enum HandlerRowRecheck {
    NeedBytes(OwnedReader<NeedBytes>),
    Missing(MissingReason),
    Drop(DropReason),
    /// Not a verdict. Still no row, so the machine continues with the
    /// re-read of the sync-ahead record.
    NoRow(OwnedReader<StoreAnsweredNoHandlerRow>),
}

impl OwnedReader<StoreAnswered> {
    pub fn reread_handler_row(self, ctx: &CommitIndexedReader) -> IotaResult<HandlerRowRecheck> {
        let latest = self.state.latest;
        Ok(match self.row_classification(ctx)? {
            None => HandlerRowRecheck::NoRow(self.into_state(StoreAnsweredNoHandlerRow { latest })),
            Some(Classification::Keep) => HandlerRowRecheck::NeedBytes(self.into_state(NeedBytes)),
            Some(Classification::Missing(reason)) => HandlerRowRecheck::Missing(reason),
            Some(Classification::Drop(reason)) => HandlerRowRecheck::Drop(reason),
        })
    }
}

/// The store answered and the re-read found no handler-processed row. The
/// answer is still held.
pub struct StoreAnsweredNoHandlerRow {
    latest: Option<ObjectReference>,
}
impl OwnedState for StoreAnsweredNoHandlerRow {}

/// Outcome of re-reading the sync-ahead record after the store answered. A
/// record that landed in the window decides as in rule 2. With nothing in
/// either table the held answer stands: equal to the named reference keeps,
/// a newer version, a tombstone or no entry drops.
#[must_use]
pub enum SyncAheadRecheck {
    NeedBytes(OwnedReader<NeedBytes>),
    Missing(MissingReason),
    Drop(DropReason),
}

impl OwnedReader<StoreAnsweredNoHandlerRow> {
    pub fn reread_sync_ahead_record(
        self,
        ctx: &CommitIndexedReader,
    ) -> IotaResult<SyncAheadRecheck> {
        let classification = match self.record_classification(ctx)? {
            Some(classification) => classification,
            // Nothing appeared in either table, so the store answer predates
            // every this-epoch write for `id` and stands.
            None => classify_latest(self.state.latest.as_ref(), &self.input),
        };
        Ok(match classification {
            Classification::Keep => SyncAheadRecheck::NeedBytes(self.into_state(NeedBytes)),
            Classification::Missing(reason) => SyncAheadRecheck::Missing(reason),
            Classification::Drop(reason) => SyncAheadRecheck::Drop(reason),
        })
    }
}

// ---------------------------------------------------------------------------
// Keep: the bytes are loaded by the caller
// ---------------------------------------------------------------------------

/// The verdict is keep once the bytes at `(id, V)` are loaded. Only the
/// machine constructs this, so bytes are only ever loaded on a keep path.
pub struct NeedBytes;
impl OwnedState for NeedBytes {}

impl OwnedReader<NeedBytes> {
    /// The exact key to load, consuming the token so it is loaded once.
    pub fn into_key(self) -> ObjectKey {
        self.key()
    }
}

// ---------------------------------------------------------------------------
// Pure comparisons, shared by the first pass and the re-check
// ---------------------------------------------------------------------------

/// What one source decided about `(id, V)`.
#[must_use]
#[derive(Debug, PartialEq, Eq)]
enum Classification {
    Keep,
    Missing(MissingReason),
    Drop(DropReason),
}

/// Rule 1 on a row at `(id, V)`: horizon first, then kind, then digest.
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

/// Rule 2 on a record for `id`: `base_version` against `V`.
fn classify_record(record: &SyncAheadRecord, version: Version) -> Classification {
    let Some(base) = record.base_version else {
        return Classification::Missing(MissingReason(MissingKind::SyncAheadCreatedId));
    };
    match base.cmp(&version) {
        Ordering::Equal => Classification::Keep,
        Ordering::Less => {
            Classification::Missing(MissingReason(MissingKind::SyncAheadCreatedVersion))
        }
        Ordering::Greater => Classification::Drop(DropReason(DropKind::SyncAheadBaseAboveVersion)),
    }
}

/// Rule 3 on the held store answer: the latest reference against the named
/// reference. A tombstone is checked before equality so that a transaction
/// naming a tombstone reference drops instead of reaching the bytes load.
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
    use iota_sdk_types::{ObjectDigest, ObjectId};

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
        }
    }

    fn record(base_version: Option<u64>) -> SyncAheadRecord {
        SyncAheadRecord {
            base_version: base_version.map(Version::from_u64),
            latest_created: Version::from_u64(100),
            initial_shared_version: None,
        }
    }

    fn missing(kind: MissingKind) -> Classification {
        Classification::Missing(MissingReason(kind))
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
    fn record_without_base_answers_missing_for_any_version() {
        for version in [1, 50, 100, 101] {
            assert_eq!(
                classify_record(&record(None), Version::from_u64(version)),
                missing(MissingKind::SyncAheadCreatedId)
            );
        }
    }

    #[test]
    fn record_base_against_the_named_version() {
        let base = record(Some(5));
        assert_eq!(
            classify_record(&base, Version::from_u64(5)),
            Classification::Keep
        );
        assert_eq!(
            classify_record(&base, Version::from_u64(6)),
            missing(MissingKind::SyncAheadCreatedVersion)
        );
        assert_eq!(
            classify_record(&base, Version::from_u64(4)),
            drop(DropKind::SyncAheadBaseAboveVersion)
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
