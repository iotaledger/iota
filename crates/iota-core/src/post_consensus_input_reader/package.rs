// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The package machine. Loads the package from the store first, then decides
//! visibility from the row at its version and the sync-ahead record, for both
//! the input loader and the deny check's package store. A package input names
//! only an id, so the store read comes first and the two table reads after it
//! are already the re-read the other machines do explicitly.

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
    /// Published locally. Next: the row at its version.
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
// Row: the handler-processed row at (id, package version)
// ---------------------------------------------------------------------------

/// The package is in the store. Its producing commit is not known yet.
pub struct PackageLoaded {
    package: PackageObject,
}
impl PackageState for PackageLoaded {}

/// Outcome of the row lookup at `(id, package version)`.
#[must_use]
pub enum PackageRowLookup {
    /// Published at or below `C - K`.
    Visible(PackageObject),
    /// Published above `C - K`. Never skipped.
    Missing(MissingReason),
    /// No row. Next: the sync-ahead record.
    NotFound(PackageReader<NoPackageRow>),
}

impl PackageReader<PackageLoaded> {
    /// Overlay first, then the table through the cache.
    pub fn read_row(self, ctx: &CommitIndexedReader) -> IotaResult<PackageRowLookup> {
        let key = ObjectKey(self.id, self.state.package.object().version());
        let row = ctx.epoch_store.handler_processed_object(&key)?;
        Ok(match classify_package_row(row.as_ref(), self.horizon) {
            PackageRowClass::Visible => PackageRowLookup::Visible(self.state.package),
            PackageRowClass::Missing(reason) => PackageRowLookup::Missing(reason),
            PackageRowClass::NoRow => {
                let package = self.state.package;
                PackageRowLookup::NotFound(PackageReader {
                    id: self.id,
                    horizon: self.horizon,
                    state: NoPackageRow { package },
                })
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Record: the sync-ahead record for id
// ---------------------------------------------------------------------------

/// The package is in the store with no handler-processed row at its version.
pub struct NoPackageRow {
    package: PackageObject,
}
impl PackageState for NoPackageRow {}

/// Outcome of the sync-ahead record lookup.
#[must_use]
pub enum PackageRecordLookup {
    /// No record: published before this epoch.
    Visible(PackageObject),
    /// Any record: published by execution the handler has not reached.
    Missing(MissingReason),
}

impl PackageReader<NoPackageRow> {
    /// Overlay first, then the table. Presence alone decides.
    pub fn read_sync_ahead_record(
        self,
        ctx: &CommitIndexedReader,
    ) -> IotaResult<PackageRecordLookup> {
        Ok(match ctx.epoch_store.sync_ahead_record(&self.id)? {
            Some(_) => {
                PackageRecordLookup::Missing(MissingReason(MissingKind::PackageSyncPublished))
            }
            None => PackageRecordLookup::Visible(self.state.package),
        })
    }
}

// ---------------------------------------------------------------------------
// Pure comparison
// ---------------------------------------------------------------------------

/// What the row at the package's version decided.
#[must_use]
#[derive(Debug, PartialEq, Eq)]
enum PackageRowClass {
    /// Published by a commit at or below the horizon.
    Visible,
    /// Published by a commit above the horizon.
    Missing(MissingReason),
    /// No row at that version.
    NoRow,
}

/// The row at `(id, package version)` against the horizon. Packages are only
/// ever created, so the kind needs no check.
fn classify_package_row(
    row: Option<&HandlerProcessedObject>,
    horizon: CommitIndex,
) -> PackageRowClass {
    match row {
        Some(row) if row.produced_at > horizon => {
            PackageRowClass::Missing(MissingReason(MissingKind::PackageAboveHorizon))
        }
        Some(_) => PackageRowClass::Visible,
        None => PackageRowClass::NoRow,
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
        assert_eq!(
            classify_package_row(Some(&row(HORIZON)), HORIZON),
            PackageRowClass::Visible
        );
        assert_eq!(
            classify_package_row(Some(&row(HORIZON + 1)), HORIZON),
            PackageRowClass::Missing(MissingReason(MissingKind::PackageAboveHorizon))
        );
    }

    #[test]
    fn package_without_a_row_falls_through_to_the_record() {
        assert_eq!(classify_package_row(None, HORIZON), PackageRowClass::NoRow);
    }
}
