// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! [`CommitIndexedReader`], the reader for one commit. It holds the horizon
//! and the store handles, and takes each input through its read steps, from
//! the first read to a result.

use std::sync::Arc;

use iota_sdk_types::{ObjectId, ObjectReference, Version};
use iota_types::{
    error::{IotaError, IotaResult},
    storage::{BackingPackageStore, PackageObject},
};

use super::{
    KeptObject, OwnedVerdict, PackageVerdict, SharedVerdict,
    owned::{NeedBytes, OwnedReader, TablesLookup, TablesRecheck},
    package::{PackageLookup, PackageReader, PackageRowLookup},
    shared::{
        CreatedObjectLookup, DeletionInfoLookup, DeletionRowLookup, ObjectAbsent, SharedReader,
        SharedTablesLookup, SharedTablesRecheck,
    },
};
use crate::{
    authority::authority_per_epoch_store::{
        AuthorityPerEpochStore, handler_object_state::CommitIndex,
    },
    execution_cache::ObjectCacheRead,
};

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
        let horizon_distance = epoch_store
            .protocol_config()
            .pcool_deterministic_validation_horizon_distance_or_default();
        Self {
            cache,
            epoch_store,
            horizon: commit_index.saturating_sub(horizon_distance),
        }
    }

    /// Reads one owned or immutable input. A storage error is returned as
    /// `Err`, never as a result.
    pub fn read_owned(&self, input: ObjectReference) -> IotaResult<OwnedVerdict> {
        // The row at the named version decides when it exists.
        let no_tables_entry =
            match OwnedReader::start(input, self.horizon).read_handler_row(self)? {
                TablesLookup::NeedBytes(need_bytes) => {
                    return Ok(OwnedVerdict::Keep(self.load_bytes(need_bytes)?));
                }
                TablesLookup::Missing(reason) => return Ok(OwnedVerdict::Missing(reason)),
                TablesLookup::Drop(reason) => return Ok(OwnedVerdict::Drop(reason)),
                TablesLookup::NoEntry(no_tables_entry) => no_tables_entry,
            };

        // No row: read the store, keep its answer, and read the row once more
        // before using that answer.
        match no_tables_entry.read_store(self)?.reread_handler_row(self)? {
            TablesRecheck::NeedBytes(need_bytes) => {
                Ok(OwnedVerdict::Keep(self.load_bytes(need_bytes)?))
            }
            TablesRecheck::Missing(reason) => Ok(OwnedVerdict::Missing(reason)),
            TablesRecheck::Drop(reason) => Ok(OwnedVerdict::Drop(reason)),
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

    /// Decides a shared input whose object is not in the store: reads the
    /// store's record of a deletion in this epoch, then the row at the deleted
    /// version. Called after either read of the creation row.
    fn check_deletion(
        &self,
        object_absent: SharedReader<ObjectAbsent>,
    ) -> IotaResult<SharedVerdict> {
        let deletion_info_found = match object_absent.read_deletion_info(self)? {
            DeletionInfoLookup::Drop(reason) => return Ok(SharedVerdict::Drop(reason)),
            DeletionInfoLookup::Found(deletion_info_found) => deletion_info_found,
        };
        Ok(match deletion_info_found.read_deletion_row(self)? {
            DeletionRowLookup::Deleted(version, digest) => SharedVerdict::Deleted(version, digest),
            DeletionRowLookup::Drop(reason) => SharedVerdict::Drop(reason),
        })
    }

    /// Reads one package input: whether it is visible at this commit. The
    /// store is read first, because a package input names no version, then
    /// the row. The package module doc explains the order. A storage error is
    /// returned as `Err`, never as a result.
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
    /// `NeedBytes` step, which only a keep result creates. A kept version must
    /// still be in the store, so a missing object is a storage error, which
    /// stops commit processing, and never a result.
    fn load_bytes(&self, need_bytes: OwnedReader<NeedBytes>) -> IotaResult<KeptObject> {
        let key = need_bytes.into_key();
        if let Some(object) = self.cache.try_get_object_by_key(&key.0, key.1)? {
            return Ok(KeptObject(object));
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
