// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The package machine. Loads the package from the store first, then decides
//! visibility from the sync-ahead record and the row at its version, for both
//! the input loader and the deny check's package store.
//!
//! A package input names only an id, so the store is read first. The two
//! table reads that follow run in the reverse order of the writer that can
//! change them. The watcher's completion inserts the handler row and then
//! removes the sync-ahead record. A reader that reads row then record can see
//! neither when the completion lands between its two reads, and would take a
//! package published above the horizon for one published before the epoch.
//! Reading the record first closes that: a record seen before its removal
//! answers missing, and a record missed because it was removed means the row
//! is already in place for the read that follows. The owned and shared
//! machines need no such reordering because their first pass reads the record
//! before the store, which anchors the same ordering for their re-read.

use iota_sdk_types::ObjectId;
use iota_types::{
    error::IotaResult,
    storage::{ObjectKey, PackageObject},
};

use super::{MissingKind, MissingReason, reader::CommitIndexedReader};
use crate::authority::authority_per_epoch_store::handler_object_state::{
    CommitIndex, HandlerProcessedObject,
};

// ---------------------------------------------------------------------------
// Carrier
// ---------------------------------------------------------------------------

/// Marker for the package machine's states.
pub trait PackageState {}

/// One package input being read at commit `C`. `S` is what the reader has
/// established so far. Each state exposes only the transition the algorithm
/// allows next, and every transition consumes the reader.
pub struct PackageReader<S: PackageState> {
    id: ObjectId,
    /// `C - K`. A row produced above it answers missing.
    horizon: CommitIndex,
    state: S,
}

impl<S: PackageState> PackageReader<S> {
    fn into_state<T: PackageState>(self, state: T) -> PackageReader<T> {
        PackageReader {
            id: self.id,
            horizon: self.horizon,
            state,
        }
    }
}

// ---------------------------------------------------------------------------
// Store: the package by id
// ---------------------------------------------------------------------------

/// Nothing read yet.
pub struct Start;
impl PackageState for Start {}

/// Outcome of the package load.
#[must_use]
pub enum PackageLookup {
    /// Published locally. Next: the sync-ahead record.
    Loaded(PackageReader<PackageLoaded>),
    /// Not published locally, or not at all.
    Missing(MissingReason),
}

impl PackageReader<Start> {
    /// Begins the read of package `id` at horizon `C - K`. The only
    /// constructor of a reader in any state.
    pub fn start(id: ObjectId, horizon: CommitIndex) -> PackageReader<Start> {
        PackageReader {
            id,
            horizon,
            state: Start,
        }
    }

    /// The package from the store. Packages are never superseded or pruned,
    /// so presence is the only question, and a miss is already missing on a
    /// slower validator too.
    pub fn read_package(self, ctx: &CommitIndexedReader) -> IotaResult<PackageLookup> {
        Ok(match ctx.cache.try_get_package_object(&self.id)? {
            Some(package) => PackageLookup::Loaded(self.into_state(PackageLoaded { package })),
            None => PackageLookup::Missing(MissingReason(MissingKind::PackageNotFound)),
        })
    }
}

// ---------------------------------------------------------------------------
// Record: the sync-ahead record for id, read before the row
// ---------------------------------------------------------------------------

/// The package is in the store. Its producing commit is not known yet.
pub struct PackageLoaded {
    package: PackageObject,
}
impl PackageState for PackageLoaded {}

impl PackageReader<PackageLoaded> {
    /// Overlay first, then the table. The answer is held, not decided: the
    /// row read next wins when present, see the module doc for the order.
    pub fn read_sync_ahead_record(
        self,
        ctx: &CommitIndexedReader,
    ) -> IotaResult<PackageReader<PackageRecordRead>> {
        let record_present = ctx.epoch_store.sync_ahead_record(&self.id)?.is_some();
        let package = self.state.package;
        Ok(PackageReader {
            id: self.id,
            horizon: self.horizon,
            state: PackageRecordRead {
                package,
                record_present,
            },
        })
    }
}

// ---------------------------------------------------------------------------
// Row: the handler-processed row at (id, package version)
// ---------------------------------------------------------------------------

/// The package is in the store and the record has been read. The row at the
/// package's version decides.
pub struct PackageRecordRead {
    package: PackageObject,
    record_present: bool,
}
impl PackageState for PackageRecordRead {}

/// Outcome of the row lookup at `(id, package version)`, with the held record
/// as the fallback.
#[must_use]
pub enum PackageRowLookup {
    /// Published at or below `C - K`, or before this epoch.
    Visible(PackageObject),
    /// Published above `C - K`, or by execution the handler has not reached.
    Missing(MissingReason),
}

impl PackageReader<PackageRecordRead> {
    /// Overlay first, then the table through the cache.
    pub fn read_row(self, ctx: &CommitIndexedReader) -> IotaResult<PackageRowLookup> {
        let key = ObjectKey(self.id, self.state.package.object().version());
        let row = ctx.epoch_store.handler_processed_object(&key)?;
        Ok(
            match classify_package(row.as_ref(), self.state.record_present, self.horizon) {
                PackageClass::Visible => PackageRowLookup::Visible(self.state.package),
                PackageClass::Missing(reason) => PackageRowLookup::Missing(reason),
            },
        )
    }
}

// ---------------------------------------------------------------------------
// Pure comparison
// ---------------------------------------------------------------------------

/// What the row and the record decided about the package.
#[must_use]
#[derive(Debug, PartialEq, Eq)]
enum PackageClass {
    Visible,
    Missing(MissingReason),
}

/// The row at `(id, package version)` against the horizon, with the record as
/// the fallback when there is no row. Packages are only ever created, so the
/// row's kind needs no check.
fn classify_package(
    row: Option<&HandlerProcessedObject>,
    record_present: bool,
    horizon: CommitIndex,
) -> PackageClass {
    match row {
        Some(row) if row.produced_at > horizon => {
            PackageClass::Missing(MissingReason(MissingKind::PackageAboveHorizon))
        }
        Some(_) => PackageClass::Visible,
        None if record_present => {
            PackageClass::Missing(MissingReason(MissingKind::PackageSyncPublished))
        }
        None => PackageClass::Visible,
    }
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::ObjectDigest;

    use super::*;
    use crate::authority::authority_per_epoch_store::handler_object_state::HandlerProcessedObjectKind;

    const HORIZON: CommitIndex = 10;

    fn row(produced_at: CommitIndex) -> HandlerProcessedObject {
        HandlerProcessedObject {
            digest: ObjectDigest::random(),
            kind: HandlerProcessedObjectKind::Live,
            produced_at,
            initial_shared_version: None,
        }
    }

    #[test]
    fn package_row_at_the_horizon_is_visible_and_one_above_answers_missing() {
        for record_present in [false, true] {
            assert_eq!(
                classify_package(Some(&row(HORIZON)), record_present, HORIZON),
                PackageClass::Visible
            );
            assert_eq!(
                classify_package(Some(&row(HORIZON + 1)), record_present, HORIZON),
                PackageClass::Missing(MissingReason(MissingKind::PackageAboveHorizon))
            );
        }
    }

    #[test]
    fn package_without_a_row_is_decided_by_the_record() {
        assert_eq!(
            classify_package(None, true, HORIZON),
            PackageClass::Missing(MissingReason(MissingKind::PackageSyncPublished))
        );
        assert_eq!(
            classify_package(None, false, HORIZON),
            PackageClass::Visible
        );
    }

    /// A package published by state sync at commit `HORIZON + 1` must answer
    /// missing whether the watcher completes that commit before, between or
    /// after the reader's two table reads. Reading the row before the record
    /// made the "between" case visible, which forks against every validator
    /// that never synced ahead.
    #[tokio::test]
    async fn package_stays_missing_when_completion_lands_between_the_table_reads() {
        use iota_sdk_types::{Address, ObjectReference, Owner, SenderSignedTransaction, Version};
        use iota_test_transaction_builder::TestTransactionBuilder;
        use iota_types::{
            effects::{TestEffectsBuilder, TransactionEffectsAPI},
            object::Object,
            transaction::TransactionKey,
        };

        use crate::authority::{
            authority_per_epoch_store::handler_object_state::handler_latest_upserts,
            authority_tests::init_state_with_objects_and_object_basics,
        };

        const PUBLISHING_COMMIT: CommitIndex = HORIZON + 1;

        // A gas coin at version 0 makes the synthetic publish's lamport version
        // 1, the version the real package below was published at.
        let gas_id = ObjectId::random();
        let gas = Object::with_id_owner_version_for_testing(
            gas_id,
            Version::from_u64(0),
            Owner::Address(Address::ZERO),
        );
        let (authority, package_ref) = init_state_with_objects_and_object_basics([gas]).await;
        let epoch_store = authority.epoch_store_for_testing().clone();
        let package_id = package_ref.object_id;

        // State sync executes the publish before the handler reaches its
        // commit: the hook misses the round map and writes the record.
        let transaction = SenderSignedTransaction::new(
            TestTransactionBuilder::new(
                Address::ZERO,
                ObjectReference::new(gas_id, Version::from_u64(0), ObjectDigest::random()),
                0,
            )
            .transfer_iota(None, Address::ZERO)
            .build(),
            vec![],
        );
        let effects = TestEffectsBuilder::new(&transaction)
            .with_created_objects([(package_id, Owner::Immutable)])
            .build();
        assert_eq!(effects.lamport_version(), package_ref.version);
        let key = TransactionKey::Digest(*effects.transaction_digest());
        epoch_store
            .record_executed_transaction(&key, &effects, authority.get_object_store().as_ref())
            .unwrap();
        assert!(
            epoch_store
                .sync_ahead_record(&package_id)
                .unwrap()
                .is_some()
        );

        let ctx = CommitIndexedReader::new(
            authority.get_object_cache_reader().clone(),
            epoch_store.clone(),
            PUBLISHING_COMMIT + 1,
        );
        let read_package =
            |ctx: &CommitIndexedReader| match PackageReader::start(package_id, HORIZON)
                .read_package(ctx)
                .unwrap()
            {
                PackageLookup::Loaded(loaded) => loaded,
                PackageLookup::Missing(_) => panic!("the package is in the store"),
            };
        let missing_kind = |lookup: PackageRowLookup| match lookup {
            PackageRowLookup::Missing(reason) => reason.kind(),
            PackageRowLookup::Visible(_) => {
                panic!("a package published above the horizon is visible")
            }
        };

        // Before completion: the record decides.
        let record_read = read_package(&ctx).read_sync_ahead_record(&ctx).unwrap();
        assert_eq!(
            missing_kind(record_read.read_row(&ctx).unwrap()),
            MissingKind::PackageSyncPublished
        );

        // Between the reads: the record was seen, then the watcher inserts the
        // row and removes the record, then the row is read.
        let record_read = read_package(&ctx).read_sync_ahead_record(&ctx).unwrap();
        epoch_store.assign_commit_to_transactions(PUBLISHING_COMMIT, vec![key]);
        epoch_store
            .record_commit_fully_executed(
                PUBLISHING_COMMIT,
                &handler_latest_upserts(&effects, PUBLISHING_COMMIT),
            )
            .unwrap();
        assert!(
            epoch_store
                .sync_ahead_record(&package_id)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            missing_kind(record_read.read_row(&ctx).unwrap()),
            MissingKind::PackageAboveHorizon
        );

        // After completion: the row decides.
        let record_read = read_package(&ctx).read_sync_ahead_record(&ctx).unwrap();
        assert_eq!(
            missing_kind(record_read.read_row(&ctx).unwrap()),
            MissingKind::PackageAboveHorizon
        );
    }
}
