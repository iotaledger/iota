// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! [`CommitIndexedReader`], the reader for one commit. It holds the horizon
//! and the store handles, and takes each input through its read steps, from
//! the first read to a result.

use std::{cell::RefCell, collections::BTreeSet, sync::Arc};

use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{Address, ObjectId, ObjectReference, Version};
use iota_types::{
    account_abstraction::authenticator_function::{
        AuthenticatorFunctionRef, authenticator_function_ref_v1_from_dynamic_field_object,
        derive_authenticator_function_ref_v1_dynamic_field_id,
    },
    deny_list_v1::coin_deny_list_v1_denial,
    error::{IotaError, IotaResult, UserInputError},
    object::Object,
    storage::{
        BackingPackageStore, ObjectKey, ObjectStore, PackageObject,
        error::{Error as StorageError, Result as StorageResult},
    },
};

use super::{
    AccountVerdict, DropKind, DropReason, DynamicFieldAtHorizon, KeptObject, MissingKind,
    MissingReason, OwnedVerdict, PackageVerdict, SharedVerdict,
    dynamic_field::{DynamicFieldReader, HighestRowLookup, RowLookup},
    owned::{
        LiveAtVersion, NeedBytes, OwnedReader, RowsAboveLookup, SupersededInStore, TablesLookup,
        TablesRecheck,
    },
    package::{PackageLookup, PackageReader, PackageRowLookup},
    shared::{
        CreatedObjectLookup, DeletionRowLookup, ObjectAbsent, SharedReader, SharedTablesLookup,
        SharedTablesRecheck, TypeObjectLookup,
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
        // The row at the named version, when it exists, decides a drop or a
        // missing. A keep from it goes on to the rows above it.
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
                    CreatedObjectLookup::Exists => Ok(SharedVerdict::Exists),
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
                CreatedObjectLookup::Exists => Ok(SharedVerdict::Exists),
                CreatedObjectLookup::Absent(object_absent) => self.check_deletion(object_absent),
            },
            SharedTablesRecheck::Exists => Ok(SharedVerdict::Exists),
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
    /// has not compared the named digest yet. Without the object, a row of
    /// `id` above `V` drops the input. With neither, a kept version is missing
    /// from the store, which is a storage error that stops commit processing,
    /// and never a result.
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
        // Only a version consumed without a lock, by a commit above the
        // horizon, is pruned while a validation can still keep it: a
        // dynamic-field child or a received object. A validator that
        // still holds the bytes rejects either in the input checks, as a
        // child is never an owned input and a received object is owned by
        // the address of the object it was sent to. That holds unless the
        // sender is that address, as an account object's is. The consumer's
        // row is in place before the pruner can remove the version.
        if self
            .epoch_store
            .lowest_handler_processed_row_above(&key.0, key.1)?
            .is_some()
        {
            return Ok(OwnedVerdict::Drop(DropReason(
                DropKind::HandlerRowAbovePrunedVersion,
            )));
        }
        Err(IotaError::Storage(format!(
            "kept input {key:?} has no bytes in the object store and no handler row above it"
        )))
    }

    /// The dynamic field `id` as of the horizon, live or absent. The field
    /// must change only through its shared parent object, as a field of the
    /// coin deny list or an account's authenticator function field does.
    /// Storage errors propagate, and so does a version the read needs but the
    /// store no longer holds.
    pub fn read_dynamic_field(&self, id: ObjectId) -> IotaResult<DynamicFieldAtHorizon> {
        let mut lookup = match DynamicFieldReader::start(id, self.horizon).read_highest_row(self)? {
            HighestRowLookup::Row(lookup) => lookup,
            HighestRowLookup::NoRow(no_row) => {
                no_row.read_object(self)?.reread_highest_row(self)?
            }
        };
        loop {
            lookup = match lookup {
                RowLookup::NeedBytes(need_bytes) => return need_bytes.load_bytes(self),
                RowLookup::Decided(field) => return Ok(field),
                RowLookup::AboveHorizon(above) => above.read_row_below(self)?,
            };
        }
    }

    /// Whether `account_id` had its authenticator function field at the
    /// horizon. An object can receive only while it is not an account, and a
    /// receive takes no lock. So every receive through an account whose
    /// field is at the horizon is at or below it too, where the owned input
    /// read sees what the receive consumed. The field changes only through
    /// the account object, in the order of the commits that change it, as
    /// [`Self::read_dynamic_field`] requires. Storage errors propagate.
    pub fn read_account(&self, account_id: ObjectId) -> IotaResult<AccountVerdict> {
        let field_id = derive_authenticator_function_ref_v1_dynamic_field_id(account_id)?;
        Ok(match self.read_dynamic_field(field_id)? {
            DynamicFieldAtHorizon::Live(field) => AccountVerdict::Account(field),
            DynamicFieldAtHorizon::Absent => {
                AccountVerdict::Missing(MissingReason(MissingKind::AuthenticatorFunctionNotFound))
            }
        })
    }

    /// The authenticate function that `field`, the authenticator function
    /// field [`Self::read_account`] read for the account `account_id`, names,
    /// for the authenticator package deny check at the commit. `None` when
    /// this reader does not see the function's package, so the check never
    /// reports it not found: execution reports it. A package this reader sees
    /// stays visible to it, so the check reads it again through this reader
    /// as its package store.
    ///
    /// # Errors
    ///
    /// Storage errors, and a field that does not decode.
    pub fn visible_authenticator_function_ref(
        &self,
        account_id: ObjectId,
        field: &Object,
    ) -> IotaResult<Option<AuthenticatorFunctionRef>> {
        let function_ref =
            authenticator_function_ref_v1_from_dynamic_field_object(account_id, field)?
                .authenticator_function_ref;
        let package_id = match &function_ref {
            AuthenticatorFunctionRef::V1(v1) => v1.package,
        };
        Ok(match self.read_package(package_id)? {
            PackageVerdict::Visible(_) => Some(function_ref),
            PackageVerdict::Missing(_) => None,
        })
    }

    /// An object of the kept shared input `id` declared at
    /// `initial_shared_version`, for its type. Storage errors propagate, and
    /// so does an input with neither a live object nor the version its
    /// deletion replaced.
    pub fn read_shared_object_for_type(
        &self,
        id: ObjectId,
        initial_shared_version: Version,
    ) -> IotaResult<Object> {
        match SharedReader::start_type_read(id, initial_shared_version, self.horizon)
            .read_object(self)?
        {
            TypeObjectLookup::Live(object) => Ok(object),
            TypeObjectLookup::Deleted(deleted) => deleted.read_deletion_row(self)?.load_bytes(self),
        }
    }
}

/// The coin deny list as of a commit's horizon, for the deny check at the
/// commit. It reads through the commit's one reader, so every transaction in
/// the commit sees the same deny list. Only latest-object reads are answered:
/// reading an exact version through it is a storage error.
pub struct DenyListAtCommit<'a> {
    reader: &'a CommitIndexedReader,
    /// The reader's error behind the storage error the last failed read
    /// returned, which keeps only its text.
    reader_error: RefCell<Option<IotaError>>,
}

impl<'a> DenyListAtCommit<'a> {
    pub fn new(reader: &'a CommitIndexedReader) -> Self {
        Self {
            reader,
            reader_error: RefCell::new(None),
        }
    }

    /// [`coin_deny_list_v1_denial`] as of the horizon. A failed read returns
    /// the reader's own error, so the end of the epoch stays
    /// `IotaError::EpochEnded`.
    pub fn denial(
        &self,
        address: Address,
        coin_types: BTreeSet<String>,
    ) -> IotaResult<Option<UserInputError>> {
        coin_deny_list_v1_denial(address, coin_types, self)
            .map_err(|error| self.reader_error.take().unwrap_or(error))
    }
}

impl ObjectStore for DenyListAtCommit<'_> {
    fn try_get_object(&self, object_id: &ObjectId) -> StorageResult<Option<Object>> {
        self.reader
            .read_dynamic_field(*object_id)
            .map(|field| match field {
                DynamicFieldAtHorizon::Live(object) => Some(object),
                DynamicFieldAtHorizon::Absent => None,
            })
            .map_err(|error| {
                let storage_error = StorageError::custom(error.to_string());
                *self.reader_error.borrow_mut() = Some(error);
                storage_error
            })
    }

    fn try_get_object_by_key(
        &self,
        object_id: &ObjectId,
        version: Version,
    ) -> StorageResult<Option<Object>> {
        Err(StorageError::custom(format!(
            "the deny list at a commit reads no exact version, asked for {object_id} at {version}"
        )))
    }
}

/// Lets the deny checks load packages as of the commit. A package the reader
/// answers missing reads as absent, which the deny checks report as not
/// found. So validation runs the deny check of the commands only after every
/// package input has loaded, and gives the authenticator package check only
/// functions whose package this reader sees.
impl BackingPackageStore for CommitIndexedReader {
    fn get_package_object(&self, package_id: &ObjectId) -> IotaResult<Option<PackageObject>> {
        Ok(match self.read_package(*package_id)? {
            PackageVerdict::Visible(package) => Some(package),
            PackageVerdict::Missing(_) => None,
        })
    }
}
