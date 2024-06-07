// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub mod error;
mod object_store_trait;
mod read_store;
mod shared_in_memory_store;
mod write_store;

use std::{
    collections::BTreeMap,
    fmt::{Display, Formatter},
    sync::Arc,
};

use itertools::Itertools;
use move_binary_format::CompiledModule;
use move_core_types::language_storage::ModuleId;
pub use object_store_trait::ObjectStore;
pub use read_store::ReadStore;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
pub use shared_in_memory_store::{SharedInMemoryStore, SingleCheckpointSharedInMemoryStore};
pub use write_store::WriteStore;

use crate::{
    base_types::{ObjectID, ObjectRef, SequenceNumber, TransactionDigest, VersionNumber},
    committee::EpochId,
    error::{IotaError, IotaResult},
    execution::{DynamicallyLoadedObjectMetadata, ExecutionResults},
    move_package::MovePackage,
    object::Object,
    transaction::{SenderSignedData, TransactionDataAPI, TransactionKey},
};

/// A potential input to a transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum InputKey {
    VersionedObject {
        id: ObjectID,
        version: SequenceNumber,
    },
    Package {
        id: ObjectID,
    },
}

impl InputKey {
    pub fn id(&self) -> ObjectID {
        match self {
            InputKey::VersionedObject { id, .. } => *id,
            InputKey::Package { id } => *id,
        }
    }

    pub fn version(&self) -> Option<SequenceNumber> {
        match self {
            InputKey::VersionedObject { version, .. } => Some(*version),
            InputKey::Package { .. } => None,
        }
    }
}

impl From<&Object> for InputKey {
    fn from(obj: &Object) -> Self {
        if obj.is_package() {
            InputKey::Package { id: obj.id() }
        } else {
            InputKey::VersionedObject {
                id: obj.id(),
                version: obj.version(),
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum WriteKind {
    /// The object was in storage already but has been modified
    Mutate,
    /// The object was created in this transaction
    Create,
    /// The object was previously wrapped in another object, but has been
    /// restored to storage
    Unwrap,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum DeleteKind {
    /// An object is provided in the call input, and gets deleted.
    Normal,
    /// An object is not provided in the call input, but gets unwrapped
    /// from another object, and then gets deleted.
    UnwrapThenDelete,
    /// An object is provided in the call input, and gets wrapped into another
    /// object.
    Wrap,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum MarkerValue {
    /// An object was received at the given version in the transaction and is no
    /// longer able to be received at that version in subequent
    /// transactions.
    Received,
    /// An owned object was deleted (or wrapped) at the given version, and is no
    /// longer able to be accessed or used in subsequent transactions.
    OwnedDeleted,
    /// A shared object was deleted by the transaction and is no longer able to
    /// be accessed or used in subsequent transactions.
    SharedDeleted(TransactionDigest),
}

/// DeleteKind together with the old sequence number prior to the deletion, if
/// available. For normal deletion and wrap, we always will consult the object
/// store to obtain the old sequence number. For UnwrapThenDelete however, in
/// the old protocol where simplified_unwrap_then_delete is false,
/// we will consult the object store to obtain the old sequence number, which
/// latter will be put in modified_at_versions; in the new protocol where
/// simplified_unwrap_then_delete is true, we will not consult the object store,
/// and hence won't have the old sequence number.
#[derive(Debug)]
pub enum DeleteKindWithOldVersion {
    Normal(SequenceNumber),
    // This variant will be deprecated when we turn on simplified_unwrap_then_delete.
    UnwrapThenDeleteDEPRECATED(SequenceNumber),
    UnwrapThenDelete,
    Wrap(SequenceNumber),
}

impl DeleteKindWithOldVersion {
    pub fn old_version(&self) -> Option<SequenceNumber> {
        match self {
            DeleteKindWithOldVersion::Normal(version)
            | DeleteKindWithOldVersion::UnwrapThenDeleteDEPRECATED(version)
            | DeleteKindWithOldVersion::Wrap(version) => Some(*version),
            DeleteKindWithOldVersion::UnwrapThenDelete => None,
        }
    }

    pub fn to_delete_kind(&self) -> DeleteKind {
        match self {
            DeleteKindWithOldVersion::Normal(_) => DeleteKind::Normal,
            DeleteKindWithOldVersion::UnwrapThenDeleteDEPRECATED(_)
            | DeleteKindWithOldVersion::UnwrapThenDelete => DeleteKind::UnwrapThenDelete,
            DeleteKindWithOldVersion::Wrap(_) => DeleteKind::Wrap,
        }
    }
}

#[derive(Debug)]
pub enum ObjectChange {
    Write(Object, WriteKind),
    // DeleteKind together with the old sequence number prior to the deletion, if available.
    Delete(DeleteKindWithOldVersion),
}

pub trait StorageView: Storage + ParentSync + ChildObjectResolver {}
impl<T: Storage + ParentSync + ChildObjectResolver> StorageView for T {}

/// An abstraction of the (possibly distributed) store for objects. This
/// API only allows for the retrieval of objects, not any state changes
pub trait ChildObjectResolver {
    /// `child` must have an `ObjectOwner` ownership equal to `owner`.
    fn read_child_object(
        &self,
        parent: &ObjectID,
        child: &ObjectID,
        child_version_upper_bound: SequenceNumber,
    ) -> IotaResult<Option<Object>>;

    /// `receiving_object_id` must have an `AddressOwner` ownership equal to
    /// `owner`. `get_object_received_at_version` must be the exact version
    /// at which the object will be received, and it cannot have been
    /// previously received at that version. NB: An object not existing at
    /// that version, and not having valid access to the object will be treated
    /// exactly the same and `Ok(None)` must be returned.
    fn get_object_received_at_version(
        &self,
        owner: &ObjectID,
        receiving_object_id: &ObjectID,
        receive_object_at_version: SequenceNumber,
        epoch_id: EpochId,
    ) -> IotaResult<Option<Object>>;
}

/// An abstraction of the (possibly distributed) store for objects, and (soon)
/// events and transactions
pub trait Storage {
    fn reset(&mut self);

    fn read_object(&self, id: &ObjectID) -> Option<&Object>;

    fn record_execution_results(&mut self, results: ExecutionResults);

    fn save_loaded_runtime_objects(
        &mut self,
        loaded_runtime_objects: BTreeMap<ObjectID, DynamicallyLoadedObjectMetadata>,
    );

    fn save_wrapped_object_containers(
        &mut self,
        wrapped_object_containers: BTreeMap<ObjectID, ObjectID>,
    );
}

pub type PackageFetchResults<Package> = Result<Vec<Package>, Vec<ObjectID>>;

#[derive(Clone, Debug)]
pub struct PackageObject {
    package_object: Object,
}

impl PackageObject {
    pub fn new(package_object: Object) -> Self {
        assert!(package_object.is_package());
        Self { package_object }
    }

    pub fn object(&self) -> &Object {
        &self.package_object
    }

    pub fn move_package(&self) -> &MovePackage {
        self.package_object.data.try_as_package().unwrap()
    }
}

impl From<PackageObject> for Object {
    fn from(package_object_arc: PackageObject) -> Self {
        package_object_arc.package_object
    }
}

pub trait BackingPackageStore {
    fn get_package_object(&self, package_id: &ObjectID) -> IotaResult<Option<PackageObject>>;
}

impl<S: BackingPackageStore> BackingPackageStore for Arc<S> {
    fn get_package_object(&self, package_id: &ObjectID) -> IotaResult<Option<PackageObject>> {
        BackingPackageStore::get_package_object(self.as_ref(), package_id)
    }
}

impl<S: ?Sized + BackingPackageStore> BackingPackageStore for &S {
    fn get_package_object(&self, package_id: &ObjectID) -> IotaResult<Option<PackageObject>> {
        BackingPackageStore::get_package_object(*self, package_id)
    }
}

impl<S: ?Sized + BackingPackageStore> BackingPackageStore for &mut S {
    fn get_package_object(&self, package_id: &ObjectID) -> IotaResult<Option<PackageObject>> {
        BackingPackageStore::get_package_object(*self, package_id)
    }
}

pub fn load_package_object_from_object_store(
    store: &impl ObjectStore,
    package_id: &ObjectID,
) -> IotaResult<Option<PackageObject>> {
    let package = store.get_object(package_id)?;
    if let Some(obj) = &package {
        fp_ensure!(
            obj.is_package(),
            IotaError::BadObjectType {
                error: format!("Package expected, Move object found: {package_id}"),
            }
        );
    }
    Ok(package.map(PackageObject::new))
}

/// Returns Ok(<package object for each package id in `package_ids`>) if all
/// package IDs in `package_id` were found. If any package in `package_ids` was
/// not found it returns a list of any package ids that are unable to be
/// found>).
pub fn get_package_objects<'a>(
    store: &impl BackingPackageStore,
    package_ids: impl IntoIterator<Item = &'a ObjectID>,
) -> IotaResult<PackageFetchResults<PackageObject>> {
    let packages: Vec<Result<_, _>> = package_ids
        .into_iter()
        .map(|id| match store.get_package_object(id) {
            Ok(None) => Ok(Err(*id)),
            Ok(Some(o)) => Ok(Ok(o)),
            Err(x) => Err(x),
        })
        .collect::<IotaResult<_>>()?;

    let (fetched, failed_to_fetch): (Vec<_>, Vec<_>) = packages.into_iter().partition_result();
    if !failed_to_fetch.is_empty() {
        Ok(Err(failed_to_fetch))
    } else {
        Ok(Ok(fetched))
    }
}

pub fn get_module<S: BackingPackageStore>(
    store: S,
    module_id: &ModuleId,
) -> Result<Option<Vec<u8>>, IotaError> {
    Ok(store
        .get_package_object(&ObjectID::from(*module_id.address()))?
        .and_then(|package| {
            package
                .move_package()
                .serialized_module_map()
                .get(module_id.name().as_str())
                .cloned()
        }))
}

pub fn get_module_by_id<S: BackingPackageStore>(
    store: S,
    id: &ModuleId,
) -> anyhow::Result<Option<CompiledModule>, IotaError> {
    Ok(get_module(store, id)?
        .map(|bytes| CompiledModule::deserialize_with_defaults(&bytes).unwrap()))
}

pub trait ParentSync {
    /// This function is only called by older protocol versions.
    /// It creates an explicit dependency to tombstones, which is not desired.
    fn get_latest_parent_entry_ref_deprecated(
        &self,
        object_id: ObjectID,
    ) -> IotaResult<Option<ObjectRef>>;
}

impl<S: ParentSync> ParentSync for std::sync::Arc<S> {
    fn get_latest_parent_entry_ref_deprecated(
        &self,
        object_id: ObjectID,
    ) -> IotaResult<Option<ObjectRef>> {
        ParentSync::get_latest_parent_entry_ref_deprecated(self.as_ref(), object_id)
    }
}

impl<S: ParentSync> ParentSync for &S {
    fn get_latest_parent_entry_ref_deprecated(
        &self,
        object_id: ObjectID,
    ) -> IotaResult<Option<ObjectRef>> {
        ParentSync::get_latest_parent_entry_ref_deprecated(*self, object_id)
    }
}

impl<S: ParentSync> ParentSync for &mut S {
    fn get_latest_parent_entry_ref_deprecated(
        &self,
        object_id: ObjectID,
    ) -> IotaResult<Option<ObjectRef>> {
        ParentSync::get_latest_parent_entry_ref_deprecated(*self, object_id)
    }
}

impl<S: ChildObjectResolver> ChildObjectResolver for std::sync::Arc<S> {
    fn read_child_object(
        &self,
        parent: &ObjectID,
        child: &ObjectID,
        child_version_upper_bound: SequenceNumber,
    ) -> IotaResult<Option<Object>> {
        ChildObjectResolver::read_child_object(
            self.as_ref(),
            parent,
            child,
            child_version_upper_bound,
        )
    }
    fn get_object_received_at_version(
        &self,
        owner: &ObjectID,
        receiving_object_id: &ObjectID,
        receive_object_at_version: SequenceNumber,
        epoch_id: EpochId,
    ) -> IotaResult<Option<Object>> {
        ChildObjectResolver::get_object_received_at_version(
            self.as_ref(),
            owner,
            receiving_object_id,
            receive_object_at_version,
            epoch_id,
        )
    }
}

impl<S: ChildObjectResolver> ChildObjectResolver for &S {
    fn read_child_object(
        &self,
        parent: &ObjectID,
        child: &ObjectID,
        child_version_upper_bound: SequenceNumber,
    ) -> IotaResult<Option<Object>> {
        ChildObjectResolver::read_child_object(*self, parent, child, child_version_upper_bound)
    }
    fn get_object_received_at_version(
        &self,
        owner: &ObjectID,
        receiving_object_id: &ObjectID,
        receive_object_at_version: SequenceNumber,
        epoch_id: EpochId,
    ) -> IotaResult<Option<Object>> {
        ChildObjectResolver::get_object_received_at_version(
            *self,
            owner,
            receiving_object_id,
            receive_object_at_version,
            epoch_id,
        )
    }
}

impl<S: ChildObjectResolver> ChildObjectResolver for &mut S {
    fn read_child_object(
        &self,
        parent: &ObjectID,
        child: &ObjectID,
        child_version_upper_bound: SequenceNumber,
    ) -> IotaResult<Option<Object>> {
        ChildObjectResolver::read_child_object(*self, parent, child, child_version_upper_bound)
    }
    fn get_object_received_at_version(
        &self,
        owner: &ObjectID,
        receiving_object_id: &ObjectID,
        receive_object_at_version: SequenceNumber,
        epoch_id: EpochId,
    ) -> IotaResult<Option<Object>> {
        ChildObjectResolver::get_object_received_at_version(
            *self,
            owner,
            receiving_object_id,
            receive_object_at_version,
            epoch_id,
        )
    }
}

// The primary key type for object storage.
#[serde_as]
#[derive(Eq, PartialEq, Clone, Copy, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
pub struct ObjectKey(pub ObjectID, pub VersionNumber);

impl ObjectKey {
    pub const ZERO: ObjectKey = ObjectKey(ObjectID::ZERO, VersionNumber::MIN);

    pub fn max_for_id(id: &ObjectID) -> Self {
        Self(*id, VersionNumber::MAX)
    }

    pub fn min_for_id(id: &ObjectID) -> Self {
        Self(*id, VersionNumber::MIN)
    }
}

impl From<ObjectRef> for ObjectKey {
    fn from(object_ref: ObjectRef) -> Self {
        ObjectKey::from(&object_ref)
    }
}

impl From<&ObjectRef> for ObjectKey {
    fn from(object_ref: &ObjectRef) -> Self {
        Self(object_ref.0, object_ref.1)
    }
}

pub enum ObjectOrTombstone {
    Object(Object),
    Tombstone(ObjectRef),
}

impl From<Object> for ObjectOrTombstone {
    fn from(object: Object) -> Self {
        ObjectOrTombstone::Object(object)
    }
}

/// Fetch the `ObjectKey`s (IDs and versions) for non-shared input objects.
/// Includes owned, and immutable objects as well as the gas objects, but not
/// move packages or shared objects.
pub fn transaction_input_object_keys(tx: &SenderSignedData) -> IotaResult<Vec<ObjectKey>> {
    use crate::transaction::InputObjectKind as I;
    Ok(tx
        .intent_message()
        .value
        .input_objects()?
        .into_iter()
        .filter_map(|object| match object {
            I::MovePackage(_) | I::SharedMoveObject { .. } => None,
            I::ImmOrOwnedMoveObject(obj) => Some(obj.into()),
        })
        .collect())
}

pub fn transaction_receiving_object_keys(tx: &SenderSignedData) -> Vec<ObjectKey> {
    tx.intent_message()
        .value
        .receiving_objects()
        .into_iter()
        .map(|oref| oref.into())
        .collect()
}

impl Display for DeleteKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DeleteKind::Wrap => write!(f, "Wrap"),
            DeleteKind::Normal => write!(f, "Normal"),
            DeleteKind::UnwrapThenDelete => write!(f, "UnwrapThenDelete"),
        }
    }
}

pub trait BackingStore:
    BackingPackageStore + ChildObjectResolver + ObjectStore + ParentSync
{
    fn as_object_store(&self) -> &dyn ObjectStore;
}

impl<T> BackingStore for T
where
    T: BackingPackageStore,
    T: ChildObjectResolver,
    T: ObjectStore,
    T: ParentSync,
{
    fn as_object_store(&self) -> &dyn ObjectStore {
        self
    }
}

pub trait GetSharedLocks: Send + Sync {
    fn get_shared_locks(
        &self,
        key: &TransactionKey,
    ) -> Result<Vec<(ObjectID, SequenceNumber)>, IotaError>;
}
