// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! [`CommitIndexedReader`], the reader for one commit. It holds the horizon
//! and the store handles, and takes each input through its read steps, from
//! the first read to a result.

use std::sync::Arc;

use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{ObjectId, ObjectReference, Version};
use iota_types::{
    error::{IotaError, IotaResult},
    storage::{BackingPackageStore, ObjectKey, PackageObject},
};

use super::{
    DropKind, DropReason, KeptObject, OwnedVerdict, PackageVerdict, SharedVerdict,
    owned::{
        LiveAtVersion, NeedBytes, OwnedReader, RowsAboveLookup, SupersededInStore, TablesLookup,
        TablesRecheck,
    },
    package::{PackageLookup, PackageReader, PackageRowLookup},
    shared::{
        CreatedObjectLookup, DeletionRowLookup, ObjectAbsent, SharedReader, SharedTablesLookup,
        SharedTablesRecheck,
    },
};
use crate::{
    authority::authority_per_epoch_store::{
        AuthorityPerEpochStore, handler_object_state::CommitIndex,
    },
    execution_cache::ObjectCacheRead,
};

/// The horizon of `commit_index`: the highest commit whose rows validation at
/// `commit_index` relies on, which is the commit minus the protocol config's
/// horizon distance. The reader treats rows produced above it as missing, and
/// the handler waits until it is fully executed before validating.
pub fn horizon(protocol_config: &ProtocolConfig, commit_index: CommitIndex) -> CommitIndex {
    commit_index.saturating_sub(
        protocol_config.pcool_deterministic_validation_horizon_distance_or_default(),
    )
}

/// Reads inputs as of one consensus commit. Built once per commit and passed
/// to every transition that touches a store.
pub struct CommitIndexedReader {
    pub(super) cache: Arc<dyn ObjectCacheRead>,
    pub(super) epoch_store: Arc<AuthorityPerEpochStore>,
    /// The horizon, `C - K`.
    horizon: CommitIndex,
}

impl CommitIndexedReader {
    pub fn new(
        cache: Arc<dyn ObjectCacheRead>,
        epoch_store: Arc<AuthorityPerEpochStore>,
        commit_index: CommitIndex,
    ) -> Self {
        let horizon = horizon(epoch_store.protocol_config(), commit_index);
        Self {
            cache,
            epoch_store,
            horizon,
        }
    }

    /// Reads one owned or immutable input: the row at the named version or
    /// the store, then, for a kept version, the rows above it. A storage error
    /// is returned as `Err`, never as a result.
    pub fn read_owned(&self, input: ObjectReference) -> IotaResult<OwnedVerdict> {
        // The row at the named version decides when it exists.
        let no_tables_entry =
            match OwnedReader::start(input, self.horizon).read_handler_row(self)? {
                TablesLookup::LiveAtVersion(live_at_version) => {
                    return self.read_rows_above(live_at_version);
                }
                TablesLookup::Missing(reason) => return Ok(OwnedVerdict::Missing(reason)),
                TablesLookup::Drop(reason) => return Ok(OwnedVerdict::Drop(reason)),
                TablesLookup::NoEntry(no_tables_entry) => no_tables_entry,
            };

        // No row: read the store, keep its answer, and read the row once more
        // before using that answer.
        match no_tables_entry.read_store(self)?.reread_handler_row(self)? {
            TablesRecheck::LiveAtVersion(live_at_version) => self.read_rows_above(live_at_version),
            TablesRecheck::NeedBytes(need_bytes) => self.load_bytes(need_bytes),
            TablesRecheck::Superseded(superseded) => self.read_superseded(superseded),
            TablesRecheck::Missing(reason) => Ok(OwnedVerdict::Missing(reason)),
            TablesRecheck::Drop(reason) => Ok(OwnedVerdict::Drop(reason)),
        }
    }

    /// Drops a kept owned input when a row of `id` above it, produced at or
    /// below the horizon, shows it consumed. Otherwise loads its object.
    fn read_rows_above(
        &self,
        live_at_version: OwnedReader<LiveAtVersion>,
    ) -> IotaResult<OwnedVerdict> {
        match live_at_version.read_rows_above(self)? {
            RowsAboveLookup::NeedBytes(need_bytes) => self.load_bytes(need_bytes),
            RowsAboveLookup::Drop(reason) => Ok(OwnedVerdict::Drop(reason)),
        }
    }

    /// Decides a version the store has replaced: it drops unless this epoch's
    /// consumption of exactly that version is above the horizon, where a
    /// validator that has not executed it still keeps it.
    fn read_superseded(
        &self,
        superseded: OwnedReader<SupersededInStore>,
    ) -> IotaResult<OwnedVerdict> {
        match superseded.read_rows_above(self)? {
            RowsAboveLookup::NeedBytes(need_bytes) => self.load_bytes(need_bytes),
            RowsAboveLookup::Drop(reason) => Ok(OwnedVerdict::Drop(reason)),
        }
    }

    /// Reads one shared input declared at `initial_shared_version`: whether
    /// the object exists and is shared at that version. Its contents are not
    /// read. A storage error is returned as `Err`, never as a result.
    pub fn read_shared(
        &self,
        id: ObjectId,
        initial_shared_version: Version,
    ) -> IotaResult<SharedVerdict> {
        // The row at the declared initial shared version is the object's
        // creation row, and decides when it exists. One at or below the
        // horizon shows the object was created shared at that version, so
        // only whether the object still exists is left to check.
        let no_tables_entry = match SharedReader::start(id, initial_shared_version, self.horizon)
            .read_creation_row(self)?
        {
            SharedTablesLookup::Created(created) => {
                return match created.read_object(self)? {
                    CreatedObjectLookup::Exists(object) => Ok(SharedVerdict::Exists(object)),
                    CreatedObjectLookup::Absent(object_absent) => {
                        self.check_deletion(object_absent)
                    }
                };
            }
            SharedTablesLookup::Missing(reason) => return Ok(SharedVerdict::Missing(reason)),
            SharedTablesLookup::Drop(reason) => return Ok(SharedVerdict::Drop(reason)),
            SharedTablesLookup::NoEntry(no_tables_entry) => no_tables_entry,
        };

        // No creation row: read the latest object from the store, keep it, and
        // read the creation row once more before using it.
        match no_tables_entry
            .read_object(self)?
            .reread_creation_row(self)?
        {
            SharedTablesRecheck::Created(created) => match created.read_object(self)? {
                CreatedObjectLookup::Exists(object) => Ok(SharedVerdict::Exists(object)),
                CreatedObjectLookup::Absent(object_absent) => self.check_deletion(object_absent),
            },
            SharedTablesRecheck::Exists(object) => Ok(SharedVerdict::Exists(object)),
            SharedTablesRecheck::Missing(reason) => Ok(SharedVerdict::Missing(reason)),
            SharedTablesRecheck::Drop(reason) => Ok(SharedVerdict::Drop(reason)),
            SharedTablesRecheck::Absent(object_absent) => self.check_deletion(object_absent),
        }
    }

    /// Decides a shared input whose object is not in the store, from the
    /// highest row of `id`. Called after either read of the creation row.
    fn check_deletion(
        &self,
        object_absent: SharedReader<ObjectAbsent>,
    ) -> IotaResult<SharedVerdict> {
        Ok(match object_absent.read_deletion_row(self)? {
            DeletionRowLookup::Deleted(version, digest) => SharedVerdict::Deleted(version, digest),
            DeletionRowLookup::Drop(reason) => SharedVerdict::Drop(reason),
            DeletionRowLookup::Missing(reason) => SharedVerdict::Missing(reason),
        })
    }

    /// Reads one package input: whether it is visible at this commit. The
    /// store is read first, because a package input names no version, then
    /// the row. The package module doc explains the order. A storage error is
    /// returned as `Err`, never as a result, and so is an id that names a Move
    /// object (`UserInputError::MoveObjectAsPackage`).
    pub fn read_package(&self, id: ObjectId) -> IotaResult<PackageVerdict> {
        let loaded = match PackageReader::start(id, self.horizon).read_package(self)? {
            PackageLookup::Loaded(loaded) => loaded,
            PackageLookup::Missing(reason) => return Ok(PackageVerdict::Missing(reason)),
        };
        Ok(match loaded.read_row(self)? {
            PackageRowLookup::Visible(package) => PackageVerdict::Visible(package),
            PackageRowLookup::Missing(reason) => PackageVerdict::Missing(reason),
        })
    }

    /// Loads the object of a kept input at exactly `(id, V)`. Takes the
    /// `NeedBytes` step, which only a keep result creates. An object with
    /// another digest drops the input: a keep through a row that replaced `V`
    /// has not compared the named digest yet. A kept version must still be in
    /// the store, so a missing object is a storage error, which stops commit
    /// processing, and never a result.
    fn load_bytes(&self, need_bytes: OwnedReader<NeedBytes>) -> IotaResult<OwnedVerdict> {
        let input = need_bytes.into_reference();
        let key = ObjectKey(input.object_id, input.version);
        if let Some(object) = self.cache.try_get_object_by_key(&key.0, key.1)? {
            if object.digest() != input.digest {
                return Ok(OwnedVerdict::Drop(DropReason(
                    DropKind::StoreDigestMismatch,
                )));
            }
            return Ok(OwnedVerdict::Keep(KeptObject(object)));
        }
        Err(IotaError::Storage(format!(
            "kept input {key:?} has no bytes in the object store"
        )))
    }
}

/// Lets the deny check load packages as of the commit. A package the reader
/// answers missing reads as absent, which the deny check reports as not
/// found, so validation runs the deny check only after every package input
/// has loaded.
impl BackingPackageStore for CommitIndexedReader {
    fn get_package_object(&self, package_id: &ObjectId) -> IotaResult<Option<PackageObject>> {
        Ok(match self.read_package(*package_id)? {
            PackageVerdict::Visible(package) => Some(package),
            PackageVerdict::Missing(_) => None,
        })
    }
}
