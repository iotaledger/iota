// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Reads one package input, for both the input loader and the deny check.
//!
//! A package input names only an id, not a version, so the package is loaded
//! from the store first, and the row at its version is read after it.
//! Executing a publish writes its row before the package reaches the store,
//! so a package the store returns that was published this epoch already has
//! its row. No row means the package was published before this epoch.

use iota_sdk_types::ObjectId;
use iota_types::{
    error::IotaResult,
    storage::{ObjectKey, PackageObject},
};

use super::{MissingKind, MissingReason, reader::CommitIndexedReader};
use crate::authority::authority_per_epoch_store::handler_object_state::{
    CommitIndex, HandlerProcessedObject,
};

/// A step of a package input's read.
pub trait PackageState {}

/// One package input being read for validation at commit `C`. `S` is the
/// step reached so far. It offers only the next allowed read, and each read
/// consumes this value.
pub struct PackageReader<S: PackageState> {
    id: ObjectId,
    /// The horizon, `C - K`. A row produced above it answers missing.
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
    /// In this validator's store. Next: the row at its version.
    Loaded(PackageReader<PackageLoaded>),
    /// Not in this validator's store: not published here yet, or not at all.
    Missing(MissingReason),
}

impl PackageReader<Start> {
    /// Starts reading package `id` with the horizon `C - K`. Every later step
    /// is reached from here.
    pub fn start(id: ObjectId, horizon: CommitIndex) -> PackageReader<Start> {
        PackageReader {
            id,
            horizon,
            state: Start,
        }
    }

    /// Reads the package from the store, at its latest version. Packages are
    /// never pruned, so a package this store lacks is also missing on a
    /// validator that has executed less. A system package that the
    /// change-epoch transaction upgraded in place is read at its new version:
    /// that transaction writes no rows, so the upgraded version counts as the
    /// state the epoch started with.
    pub fn read_package(self, ctx: &CommitIndexedReader) -> IotaResult<PackageLookup> {
        Ok(match ctx.cache.try_get_package_object(&self.id)? {
            Some(package) => PackageLookup::Loaded(self.into_state(PackageLoaded { package })),
            None => PackageLookup::Missing(MissingReason(MissingKind::PackageNotFound)),
        })
    }
}

// ---------------------------------------------------------------------------
// Row: the row at (id, package version)
// ---------------------------------------------------------------------------

/// The package is in the store. Its producing commit is not known yet.
pub struct PackageLoaded {
    package: PackageObject,
}
impl PackageState for PackageLoaded {}

/// Outcome of the row lookup at `(id, package version)`.
#[must_use]
pub enum PackageRowLookup {
    /// Published at or below the horizon, or before this epoch.
    Visible(PackageObject),
    /// Published above the horizon.
    Missing(MissingReason),
}

impl PackageReader<PackageLoaded> {
    /// Reads the row at the package's version: the rows held in memory first,
    /// then the table.
    pub fn read_row(self, ctx: &CommitIndexedReader) -> IotaResult<PackageRowLookup> {
        let key = ObjectKey(self.id, self.state.package.object().version());
        let row = ctx.epoch_store.handler_processed_object(&key)?;
        Ok(match classify_package(row.as_ref(), self.horizon) {
            PackageClass::Visible => PackageRowLookup::Visible(self.state.package),
            PackageClass::Missing(reason) => PackageRowLookup::Missing(reason),
        })
    }
}

// ---------------------------------------------------------------------------
// Pure comparison
// ---------------------------------------------------------------------------

/// What the row decided about the package.
#[must_use]
#[derive(Debug, PartialEq, Eq)]
enum PackageClass {
    Visible,
    Missing(MissingReason),
}

/// The row at `(id, package version)` against the horizon. No row means the
/// package was published before the epoch. The row's kind needs no check: a
/// package is never deleted or wrapped, and the only in-place upgrade, of a
/// system package by the change-epoch transaction, writes no row.
fn classify_package(row: Option<&HandlerProcessedObject>, horizon: CommitIndex) -> PackageClass {
    match row {
        Some(row) if row.produced_at > horizon => {
            PackageClass::Missing(MissingReason(MissingKind::PackageAboveHorizon))
        }
        Some(_) | None => PackageClass::Visible,
    }
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::ObjectDigest;

    use super::*;
    use crate::authority::authority_per_epoch_store::handler_object_state::HandlerProcessedObjectKind;

    const HORIZON: CommitIndex = 10;
    /// The horizon distance these tests set, so validation at
    /// `VALIDATED_COMMIT` reads the state as of `HORIZON`.
    const HORIZON_DISTANCE: CommitIndex = 4;
    const VALIDATED_COMMIT: CommitIndex = HORIZON + HORIZON_DISTANCE;

    fn row(produced_at: CommitIndex) -> HandlerProcessedObject {
        HandlerProcessedObject {
            digest: ObjectDigest::random(),
            kind: HandlerProcessedObjectKind::Live,
            produced_at,
            initial_shared_version: None,
            replaced_version: None,
            deleted_by: None,
        }
    }

    #[test]
    fn package_row_at_the_horizon_is_visible_and_one_above_answers_missing() {
        assert_eq!(
            classify_package(Some(&row(HORIZON)), HORIZON),
            PackageClass::Visible
        );
        assert_eq!(
            classify_package(Some(&row(HORIZON + 1)), HORIZON),
            PackageClass::Missing(MissingReason(MissingKind::PackageAboveHorizon))
        );
    }

    #[test]
    fn package_without_a_row_is_visible() {
        assert_eq!(classify_package(None, HORIZON), PackageClass::Visible);
    }

    /// A package published by commit 11, above the horizon (10) of commit 14,
    /// answers missing once its row exists. The row is written before the
    /// package reaches the store.
    #[tokio::test]
    async fn package_published_above_the_horizon_is_missing() {
        use iota_sdk_types::{Address, ObjectReference, Owner, SenderSignedTransaction, Version};
        use iota_test_transaction_builder::TestTransactionBuilder;
        use iota_types::{
            effects::{TestEffectsBuilder, TransactionEffectsAPI},
            object::Object,
            transaction::TransactionKey,
        };

        use crate::authority::{
            authority_per_epoch_store::handler_object_state::handler_processed_upserts,
            authority_tests::init_state_with_objects_and_object_basics,
        };
        let _guard =
            iota_protocol_config::ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
                config.set_enable_pcool_flow_for_testing(true);
                config.set_pcool_deterministic_validation_horizon_distance_for_testing(
                    HORIZON_DISTANCE,
                );
                config
            });

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

        let read_at = |commit_index| {
            let ctx = CommitIndexedReader::new(
                authority.get_object_cache_reader().clone(),
                epoch_store.clone(),
                commit_index,
            );
            let loaded = match PackageReader::start(package_id, HORIZON)
                .read_package(&ctx)
                .unwrap()
            {
                PackageLookup::Loaded(loaded) => loaded,
                PackageLookup::Missing(_) => panic!("the package is in the store"),
            };
            loaded.read_row(&ctx).unwrap()
        };

        // No row: published before the epoch.
        assert!(matches!(
            read_at(VALIDATED_COMMIT),
            PackageRowLookup::Visible(_)
        ));

        epoch_store.assign_commit_to_transactions(PUBLISHING_COMMIT, vec![key]);
        epoch_store
            .record_commit_fully_executed(
                PUBLISHING_COMMIT,
                &handler_processed_upserts(&effects, PUBLISHING_COMMIT),
            )
            .unwrap();
        assert!(matches!(
            read_at(VALIDATED_COMMIT),
            PackageRowLookup::Missing(reason) if reason.kind() == MissingKind::PackageAboveHorizon
        ));
    }
}
