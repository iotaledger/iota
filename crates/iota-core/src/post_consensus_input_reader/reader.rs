// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The reader context and the driver. Holds the horizon and the store
//! handles, walks each input machine from its first lookup to a verdict, and
//! loads a transaction's inputs at a commit for the post-consensus validation
//! entry point.

use std::sync::Arc;

use iota_sdk_types::{ObjectId, ObjectReference, Version};
use iota_types::error::{IotaError, IotaResult};

use super::{
    KeptObject, OwnedVerdict, PackageVerdict, SharedVerdict,
    owned::{
        HandlerRowLookup, HandlerRowRecheck, NeedBytes, OwnedReader, SyncAheadLookup,
        SyncAheadRecheck,
    },
    package::{PackageLookup, PackageReader, PackageRowLookup},
    shared::{
        CreatedObjectLookup, CreationRowLookup, CreationRowRecheck, DeletionInfoLookup,
        DeletionRowLookup, ObjectAbsent, PreSyncObjectLookup, SharedReader, SharedRecordLookup,
        SharedRecordRecheck,
    },
};
use crate::{
    authority::authority_per_epoch_store::{
        AuthorityPerEpochStore, handler_object_state::CommitIndex,
    },
    execution_cache::ObjectCacheRead,
};

/// Distance between the commit being validated and the highest commit whose
/// rows a verdict may trust. A future protocol parameter.
pub const K: CommitIndex = 2;

/// Reads inputs as of one consensus commit. Built once per commit and passed
/// to every transition that touches a store.
pub struct CommitIndexedReader {
    pub(super) cache: Arc<dyn ObjectCacheRead>,
    pub(super) epoch_store: Arc<AuthorityPerEpochStore>,
    /// `C - K`.
    horizon: CommitIndex,
}

impl CommitIndexedReader {
    pub fn new(
        cache: Arc<dyn ObjectCacheRead>,
        epoch_store: Arc<AuthorityPerEpochStore>,
        commit_index: CommitIndex,
    ) -> Self {
        Self {
            cache,
            epoch_store,
            horizon: commit_index.saturating_sub(K),
        }
    }

    /// Rules 1 to 3 for one owned input. Storage errors propagate. They are
    /// never a verdict.
    pub fn read_owned(&self, input: ObjectReference) -> IotaResult<OwnedVerdict> {
        // Rule 1: the handler-processed row at the named version.
        let no_handler_row = match OwnedReader::start(input, self.horizon).read_handler_row(self)? {
            HandlerRowLookup::NeedBytes(need_bytes) => {
                return Ok(OwnedVerdict::Keep(self.load_bytes(need_bytes)?));
            }
            HandlerRowLookup::Missing(reason) => return Ok(OwnedVerdict::Missing(reason)),
            HandlerRowLookup::Drop(reason) => return Ok(OwnedVerdict::Drop(reason)),
            HandlerRowLookup::NoRow(no_handler_row) => no_handler_row,
        };

        // Rule 2: the sync-ahead record for the id.
        let no_sync_ahead_record = match no_handler_row.read_sync_ahead_record(self)? {
            SyncAheadLookup::NeedBytes(need_bytes) => {
                return Ok(OwnedVerdict::Keep(self.load_bytes(need_bytes)?));
            }
            SyncAheadLookup::Missing(reason) => return Ok(OwnedVerdict::Missing(reason)),
            SyncAheadLookup::Drop(reason) => return Ok(OwnedVerdict::Drop(reason)),
            SyncAheadLookup::NoRecord(no_sync_ahead_record) => no_sync_ahead_record,
        };

        // Rule 3: ask the store, hold the answer, read both tables again.
        let store_answered = no_sync_ahead_record.read_store(self)?;

        let store_answered_no_handler_row = match store_answered.reread_handler_row(self)? {
            HandlerRowRecheck::NeedBytes(need_bytes) => {
                return Ok(OwnedVerdict::Keep(self.load_bytes(need_bytes)?));
            }
            HandlerRowRecheck::Missing(reason) => return Ok(OwnedVerdict::Missing(reason)),
            HandlerRowRecheck::Drop(reason) => return Ok(OwnedVerdict::Drop(reason)),
            HandlerRowRecheck::NoRow(store_answered_no_handler_row) => {
                store_answered_no_handler_row
            }
        };
        match store_answered_no_handler_row.reread_sync_ahead_record(self)? {
            SyncAheadRecheck::NeedBytes(need_bytes) => {
                Ok(OwnedVerdict::Keep(self.load_bytes(need_bytes)?))
            }
            SyncAheadRecheck::Missing(reason) => Ok(OwnedVerdict::Missing(reason)),
            SyncAheadRecheck::Drop(reason) => Ok(OwnedVerdict::Drop(reason)),
        }
    }

    /// Existence and creation metadata for one shared input declared at
    /// `initial_shared_version`. No content read. Storage errors propagate.
    pub fn read_shared(
        &self,
        id: ObjectId,
        initial_shared_version: Version,
    ) -> IotaResult<SharedVerdict> {
        // Creation: the row at the declared initial version. A row at or below
        // the horizon proved the flag, so only the object's presence is left.
        let no_creation_row = match SharedReader::start(id, initial_shared_version, self.horizon)
            .read_creation_row(self)?
        {
            CreationRowLookup::Created(created) => {
                // creation row returned Created, check the store for existence
                return match created.read_object(self)? {
                    CreatedObjectLookup::Exists => Ok(SharedVerdict::Exists),
                    CreatedObjectLookup::Absent(object_absent) => {
                        self.check_deletion(object_absent)
                    }
                };
            }
            CreationRowLookup::Missing(reason) => return Ok(SharedVerdict::Missing(reason)),
            CreationRowLookup::Drop(reason) => return Ok(SharedVerdict::Drop(reason)),
            CreationRowLookup::NoRow(no_creation_row) => no_creation_row,
        };

        // Record: the sync-ahead record for the id.
        let no_record = match no_creation_row.read_sync_ahead_record(self)? {
            SharedRecordLookup::Missing(reason) => return Ok(SharedVerdict::Missing(reason)),
            SharedRecordLookup::PreSyncExisted(pre_sync_existed) => {
                return Ok(match pre_sync_existed.read_object(self)? {
                    PreSyncObjectLookup::Exists => SharedVerdict::Exists,
                    PreSyncObjectLookup::Drop(reason) => SharedVerdict::Drop(reason),
                });
            }
            SharedRecordLookup::NoRecord(no_record) => no_record,
        };

        // Store: hold the latest object, read both tables again.
        let object_answered = no_record.read_object(self)?;
        let object_answered_no_creation_row = match object_answered.reread_creation_row(self)? {
            CreationRowRecheck::Created(created) => {
                return match created.read_object(self)? {
                    CreatedObjectLookup::Exists => Ok(SharedVerdict::Exists),
                    CreatedObjectLookup::Absent(object_absent) => {
                        self.check_deletion(object_absent)
                    }
                };
            }
            CreationRowRecheck::Missing(reason) => return Ok(SharedVerdict::Missing(reason)),
            CreationRowRecheck::Drop(reason) => return Ok(SharedVerdict::Drop(reason)),
            CreationRowRecheck::NoRow(object_answered_no_creation_row) => {
                object_answered_no_creation_row
            }
        };
        let object_absent = match object_answered_no_creation_row.reread_sync_ahead_record(self)? {
            SharedRecordRecheck::Exists => return Ok(SharedVerdict::Exists),
            SharedRecordRecheck::Missing(reason) => return Ok(SharedVerdict::Missing(reason)),
            SharedRecordRecheck::Drop(reason) => return Ok(SharedVerdict::Drop(reason)),
            SharedRecordRecheck::Absent(object_absent) => object_absent,
        };
        self.check_deletion(object_absent)
    }

    /// Deletion: this epoch's marker, then the row at the deleted version.
    /// Reached from a created object that is gone, in either pass, and from a
    /// store answer no table claimed.
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

    /// Visibility of one package input. The store is read first, because a
    /// package input names no version, then the record, then the row. The
    /// package module doc explains the order. Storage errors propagate.
    pub fn read_package(&self, id: ObjectId) -> IotaResult<PackageVerdict> {
        let loaded = match PackageReader::start(id, self.horizon).read_package(self)? {
            PackageLookup::Loaded(loaded) => loaded,
            PackageLookup::Missing(reason) => return Ok(PackageVerdict::Missing(reason)),
        };
        let record_read = loaded.read_sync_ahead_record(self)?;
        Ok(match record_read.read_row(self)? {
            PackageRowLookup::Visible(package) => PackageVerdict::Visible(package),
            PackageRowLookup::Missing(reason) => PackageVerdict::Missing(reason),
        })
    }

    /// The bytes at exactly `(id, V)`, from the store or from the shelter row
    /// once the pruner removed the version. Reachable only with the machine's
    /// keep token. Bytes in neither place break the design's invariant, so
    /// that is a storage error and halts commit processing, never a verdict.
    fn load_bytes(&self, need_bytes: OwnedReader<NeedBytes>) -> IotaResult<KeptObject> {
        let key = need_bytes.into_key();
        if let Some(object) = self.cache.try_get_object_by_key(&key.0, key.1)? {
            return Ok(KeptObject(object));
        }
        self.epoch_store
            .sheltered_object(&key)?
            .map(KeptObject)
            .ok_or_else(|| {
                IotaError::Storage(format!(
                    "kept input {key:?} has bytes in neither the object store nor the shelter"
                ))
            })
    }
}
