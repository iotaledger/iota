// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashMap, hash_map::Entry},
    sync::Arc,
};

use iota_sdk_types::{ObjectReference, TransactionDigest};
use iota_types::{
    base_types::EpochId,
    error::{IotaError, IotaResult, UserInputError},
    storage::ObjectKey,
    transaction::{
        InputObjectKind, InputObjects, ObjectReadResult, ObjectReadResultKind,
        ReceivingObjectReadResult, ReceivingObjectReadResultKind, ReceivingObjects, TransactionKey,
    },
};
use itertools::izip;
use tracing::instrument;

use crate::{
    authority::{
        authority_per_epoch_store::TxLockGuard, shared_object_version_manager::AssignedVersions,
    },
    execution_cache::ObjectCacheRead,
    post_consensus_input_reader::{
        InputResolution, OwnedVerdict, PackageVerdict, ReceivingObjectsResolution, SharedVerdict,
        reader::CommitIndexedReader,
    },
};

pub(crate) struct TransactionInputLoader {
    cache: Arc<dyn ObjectCacheRead>,
}

impl TransactionInputLoader {
    pub fn new(cache: Arc<dyn ObjectCacheRead>) -> Self {
        Self { cache }
    }
}

impl TransactionInputLoader {
    /// Read the inputs for a transaction that the validator was asked to sign.
    ///
    /// tx_digest is provided so that the inputs can be cached with the
    /// tx_digest and returned with a single hash map lookup when
    /// notify_read_objects_for_execution is called later. TODO: implement
    /// this caching
    #[instrument(level = "trace", skip_all)]
    pub fn read_objects_for_signing(
        &self,
        _tx_digest_for_caching: Option<&TransactionDigest>,
        input_object_kinds: &[InputObjectKind],
        receiving_objects: &[ObjectReference],
        epoch_id: EpochId,
    ) -> IotaResult<(InputObjects, ReceivingObjects)> {
        // Length of input_object_kinds have been checked via validity_check() for
        // ProgrammableTransaction.
        let mut input_results = vec![None; input_object_kinds.len()];
        let mut object_refs = Vec::with_capacity(input_object_kinds.len());
        let mut fetch_indices = Vec::with_capacity(input_object_kinds.len());
        // A transaction and its Move authenticators can name one shared
        // object with different mutability, so one id can come with two
        // kinds. Each shared object is read once and every kind of it gets
        // that read: separate reads can see different versions, while
        // combining the transaction's inputs with the authenticators' needs
        // one object per id.
        let mut shared_objects = HashMap::<_, ObjectReadResultKind>::new();

        for (i, kind) in input_object_kinds.iter().enumerate() {
            match kind {
                // Packages are loaded one at a time via the cache
                InputObjectKind::MovePackage(id) => {
                    let Some(package) = self.cache.try_get_package_object(id)?.map(|o| o.into())
                    else {
                        return Err(IotaError::from(kind.object_not_found_error()));
                    };
                    input_results[i] = Some(ObjectReadResult {
                        input_object_kind: *kind,
                        object: ObjectReadResultKind::Object(package),
                    });
                }
                InputObjectKind::SharedMoveObject { id, .. } => {
                    let object = match shared_objects.entry(*id) {
                        Entry::Occupied(entry) => entry.get().clone(),
                        Entry::Vacant(entry) => entry
                            .insert(self.read_shared_object(kind, epoch_id)?)
                            .clone(),
                    };
                    input_results[i] = Some(ObjectReadResult::new(*kind, object));
                }
                InputObjectKind::ImmOrOwnedMoveObject(objref) => {
                    object_refs.push(*objref);
                    fetch_indices.push(i);
                }
            }
        }

        let objects = self
            .cache
            .try_multi_get_objects_with_more_accurate_error_return(&object_refs)?;
        assert_eq!(objects.len(), object_refs.len());
        for (index, object) in fetch_indices.into_iter().zip(objects.into_iter()) {
            input_results[index] = Some(ObjectReadResult {
                input_object_kind: input_object_kinds[index],
                object: ObjectReadResultKind::Object(object),
            });
        }

        let receiving_results = self.read_receiving_objects(receiving_objects, epoch_id)?;

        Ok((
            input_results
                .into_iter()
                .map(Option::unwrap)
                .collect::<Vec<_>>()
                .into(),
            receiving_results,
        ))
    }

    /// Returns the latest version of the shared object that `kind` names, or,
    /// when it was deleted in `epoch_id`, the version and the transaction of
    /// its deletion.
    ///
    /// # Errors
    ///
    /// [`UserInputError::ObjectNotFound`] when the object neither exists nor
    /// was deleted in `epoch_id`, and any error reading the cache.
    fn read_shared_object(
        &self,
        kind: &InputObjectKind,
        epoch_id: EpochId,
    ) -> IotaResult<ObjectReadResultKind> {
        let id = kind.object_id();
        if let Some(object) = self.cache.try_get_object(&id)? {
            return Ok(ObjectReadResultKind::Object(object));
        }

        match self
            .cache
            .try_get_last_shared_object_deletion_info(&id, epoch_id)?
        {
            Some((version, digest)) => {
                Ok(ObjectReadResultKind::DeletedSharedObject(version, digest))
            }
            None => Err(IotaError::from(kind.object_not_found_error())),
        }
    }

    /// Read the inputs for a transaction that is ready to be executed.
    ///
    /// This function panics if any inputs are not available, as
    /// TransactionManager should already have verified that the transaction
    /// is ready to be executed.
    ///
    /// The tx_digest is provided here to support the following optimization
    /// (not yet implemented): All the owned input objects will likely have
    /// been loaded during transaction signing, and can be stored as a group
    /// with the transaction_digest as the key, allowing the lookup to
    /// proceed with only a single hash map lookup. (additional lookups may be
    /// necessary for shared inputs, since the versions are not known at
    /// signing time). Receiving objects could be cached, but only with
    /// appropriate invalidation logic for when an object is received by a
    /// different tx first.
    #[instrument(level = "trace", skip_all)]
    pub fn read_objects_for_execution(
        &self,
        tx_key: &TransactionKey,
        // Important to hold the _tx_lock, otherwise it would be possible for a concurrent
        // execution of the same tx to enter this point after the first execution has
        // finished and the shared locks have been deleted.
        _tx_lock: &TxLockGuard,
        input_object_kinds: &[InputObjectKind],
        assigned_shared_object_versions: &AssignedVersions,
        epoch_id: EpochId,
    ) -> IotaResult<InputObjects> {
        let assigned_shared_versions: BTreeMap<_, _> = assigned_shared_object_versions
            .iter()
            .map(|assignment| (assignment.object_id, assignment.version))
            .collect();

        let mut results = vec![None; input_object_kinds.len()];
        let mut object_keys = Vec::with_capacity(input_object_kinds.len());
        let mut fetches = Vec::with_capacity(input_object_kinds.len());
        let mut gas_object_cancellation = None;
        // Only a transaction without shared inputs can carry a cancellation
        // version on its gas object; every other assignment names a shared object.
        let check_gas_object_cancellation = !input_object_kinds
            .iter()
            .any(|kind| matches!(kind, InputObjectKind::SharedMoveObject { .. }));

        for (i, input) in input_object_kinds.iter().enumerate() {
            match input {
                InputObjectKind::MovePackage(id) => {
                    let package = self.cache.try_get_package_object(id)?.unwrap_or_else(|| {
                        panic!(
                            "Executable transaction {tx_key:?} depends on non-existent package {id}"
                        )
                    });

                    results[i] = Some(ObjectReadResult {
                        input_object_kind: *input,
                        object: ObjectReadResultKind::Object(package.into()),
                    });
                    continue;
                }
                InputObjectKind::ImmOrOwnedMoveObject(objref) => {
                    // A cancelled transaction without shared inputs carries
                    // its cancellation version on the gas object. Unlike a
                    // cancelled shared input, the object is still read
                    // normally: the cancelled execution charges gas to it.
                    if check_gas_object_cancellation {
                        if let Some(version) = assigned_shared_versions.get(&objref.object_id) {
                            assert!(
                                version.is_canceled(),
                                "non-shared input {} of {tx_key:?} has a non-cancellation \
                                    assigned version {version:?}",
                                objref.object_id,
                            );
                            gas_object_cancellation = Some((objref.object_id, *version));
                        }
                    }
                    object_keys.push(objref.into());
                    fetches.push((i, input));
                }
                InputObjectKind::SharedMoveObject { id, .. } => {
                    // If we find a set of assigned versions but an object is missing, it indicates
                    // a serious inconsistency:
                    let version = assigned_shared_versions.get(id).unwrap_or_else(|| {
                        panic!("Shared object version should have been assigned. key: {tx_key:?}, obj id: {id}")
                    });
                    if version.is_canceled() {
                        // Do not need to fetch shared object for cancelled transaction.
                        results[i] = Some(ObjectReadResult {
                            input_object_kind: *input,
                            object: ObjectReadResultKind::CancelledTransactionObject(*version),
                        })
                    } else {
                        object_keys.push(ObjectKey(*id, *version));
                        fetches.push((i, input));
                    }
                }
            }
        }

        let objects = self.cache.try_multi_get_objects_by_key(&object_keys)?;

        assert!(objects.len() == object_keys.len() && objects.len() == fetches.len());

        for (object, key, (index, input)) in izip!(
            objects.into_iter(),
            object_keys.into_iter(),
            fetches.into_iter()
        ) {
            results[index] = Some(match (object, input) {
                (Some(obj), input_object_kind) => ObjectReadResult {
                    input_object_kind: *input_object_kind,
                    object: obj.into(),
                },
                (None, InputObjectKind::SharedMoveObject { id, .. }) => {
                    assert!(key.1.is_valid());
                    // Check if the object was deleted by a concurrently certified tx
                    let version = key.1;
                    if let Some(dependency) = self
                        .cache
                        .try_get_deleted_shared_object_previous_tx_digest(id, version, epoch_id)?
                    {
                        ObjectReadResult {
                            input_object_kind: *input,
                            object: ObjectReadResultKind::DeletedSharedObject(version, dependency),
                        }
                    } else {
                        panic!(
                            "All dependencies of tx {tx_key:?} should have been executed now, but Shared Object id: {}, version: {version} is absent in epoch {epoch_id}",
                            *id
                        );
                    }
                }
                _ => panic!(
                    "All dependencies of tx {tx_key:?} should have been executed now, but obj {key:?} is absent"
                ),
            });
        }

        let mut input_objects: InputObjects = results
            .into_iter()
            .map(Option::unwrap)
            .collect::<Vec<_>>()
            .into();
        if let Some((gas_object_id, version)) = gas_object_cancellation {
            input_objects.set_gas_object_cancellation(gas_object_id, version);
        }
        Ok(input_objects)
    }
}

impl TransactionInputLoader {
    /// Load the `ReceivingObjects` for a transaction. Used both at signing time
    /// (by [`Self::read_objects_for_signing`]) and at execution time.
    pub(crate) fn read_receiving_objects(
        &self,
        receiving_objects: &[ObjectReference],
        epoch_id: EpochId,
    ) -> IotaResult<ReceivingObjects> {
        let mut receiving_results = Vec::with_capacity(receiving_objects.len());
        for objref in receiving_objects {
            // Note: the digest is checked later in check_transaction_input
            let ObjectReference {
                object_id, version, ..
            } = *objref;

            if self
                .cache
                .try_have_received_object_at_version(&object_id, version, epoch_id)?
            {
                receiving_results.push(ReceivingObjectReadResult::new(
                    *objref,
                    ReceivingObjectReadResultKind::PreviouslyReceivedObject,
                ));
                continue;
            }

            let Some(object) = self.cache.try_get_object(&object_id)? else {
                return Err(UserInputError::ObjectNotFound {
                    object_id,
                    version: Some(version),
                }
                .into());
            };

            receiving_results.push(ReceivingObjectReadResult::new(*objref, object.into()));
        }
        Ok(receiving_results.into())
    }

    /// Reads the inputs of a transaction being validated post-consensus, as of
    /// the commit `reader` was built for. Owned inputs and packages are read
    /// through the commit-indexed reader. A shared input is decided by the
    /// reader, but its object is not returned: a live one is listed as an
    /// `UnreadSharedObject`. Receiving objects are not read. `Err` is a
    /// storage failure, or `UserInputError::MoveObjectAsPackage` when a package
    /// input names a Move object. A dropped or missing input is a result, never
    /// an error.
    #[instrument(level = "trace", skip_all)]
    pub(crate) fn read_objects_at_commit(
        &self,
        reader: &CommitIndexedReader,
        input_object_kinds: &[InputObjectKind],
    ) -> IotaResult<InputResolution> {
        let mut input_results = Vec::with_capacity(input_object_kinds.len());
        for kind in input_object_kinds {
            let object = match kind {
                InputObjectKind::MovePackage(id) => match reader.read_package(*id)? {
                    PackageVerdict::Visible(package) => {
                        ObjectReadResultKind::Object(package.object().clone())
                    }
                    PackageVerdict::Missing(reason) => {
                        return Ok(InputResolution::Missing(*kind, reason));
                    }
                },
                InputObjectKind::SharedMoveObject {
                    id,
                    initial_shared_version,
                    ..
                } => match reader.read_shared(*id, *initial_shared_version)? {
                    SharedVerdict::Exists => ObjectReadResultKind::UnreadSharedObject,
                    SharedVerdict::Deleted(version, digest) => {
                        ObjectReadResultKind::DeletedSharedObject(version, digest)
                    }
                    SharedVerdict::Drop(reason) => return Ok(InputResolution::Drop(*kind, reason)),
                    SharedVerdict::Missing(reason) => {
                        return Ok(InputResolution::Missing(*kind, reason));
                    }
                },
                InputObjectKind::ImmOrOwnedMoveObject(reference) => {
                    match reader.read_owned(*reference)? {
                        OwnedVerdict::Keep(kept) => {
                            ObjectReadResultKind::Object(kept.into_object())
                        }
                        OwnedVerdict::Drop(reason) => {
                            return Ok(InputResolution::Drop(*kind, reason));
                        }
                        OwnedVerdict::Missing(reason) => {
                            return Ok(InputResolution::Missing(*kind, reason));
                        }
                    }
                }
            };
            input_results.push(ObjectReadResult {
                input_object_kind: *kind,
                object,
            });
        }
        Ok(InputResolution::Loaded(input_results.into()))
    }

    /// Reads the receiving objects of a transaction being validated
    /// post-consensus, as of the commit `reader` was built for. Each goes
    /// through the reader as an owned input does, so a kept one was live at
    /// the declared version on every validator and execution can always run
    /// it. No lock is taken on it and nothing checks that the sender owns it.
    /// A drop or a missing is reported under the owned kind of the reference,
    /// so the caller's error names its id and version.
    #[instrument(level = "trace", skip_all)]
    pub fn read_receiving_objects_at_commit(
        &self,
        reader: &CommitIndexedReader,
        receiving_objects: &[ObjectReference],
    ) -> IotaResult<ReceivingObjectsResolution> {
        let mut receiving_results = Vec::with_capacity(receiving_objects.len());
        for reference in receiving_objects {
            let kind = InputObjectKind::ImmOrOwnedMoveObject(*reference);
            let object = match reader.read_owned(*reference)? {
                OwnedVerdict::Keep(kept) => kept.into_object(),
                OwnedVerdict::Drop(reason) => {
                    return Ok(ReceivingObjectsResolution::Drop(kind, reason));
                }
                OwnedVerdict::Missing(reason) => {
                    return Ok(ReceivingObjectsResolution::Missing(kind, reason));
                }
            };
            receiving_results.push(ReceivingObjectReadResult::new(*reference, object.into()));
        }
        Ok(ReceivingObjectsResolution::Loaded(receiving_results.into()))
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use futures::future::BoxFuture;
    use iota_sdk_types::{ObjectId, ObjectReference, Owner, TransactionDigest, Version};
    use iota_types::{
        base_types::EpochId,
        error::IotaResult,
        iota_system_state::IotaSystemState,
        object::Object,
        storage::{InputKey, MarkerValue, ObjectKey, ObjectOrTombstone, PackageObject},
        transaction::{InputObjectKind, ObjectReadResultKind},
    };

    use super::TransactionInputLoader;
    use crate::{
        authority::{
            authority_per_epoch_store::AuthorityPerEpochStore, authority_store::IotaLockResult,
        },
        execution_cache::ObjectCacheRead,
    };

    /// An object cache that holds one shared object and counts the reads of
    /// it with [`ObjectCacheRead::try_get_object()`]. The first read finds
    /// `versions[0]` and every later read `versions[1]`, as when a transaction
    /// that changes the object executes between two reads. It answers only the
    /// reads that loading shared inputs makes.
    struct OneSharedObjectCache {
        /// `None` once the object is deleted.
        versions: [Option<Object>; 2],
        /// The version and the transaction of the object's deletion, once it
        /// is deleted.
        deletion: Option<(Version, TransactionDigest)>,
        reads: AtomicUsize,
    }

    impl ObjectCacheRead for OneSharedObjectCache {
        fn try_get_object(&self, id: &ObjectId) -> IotaResult<Option<Object>> {
            let read = self.reads.fetch_add(1, Ordering::Relaxed);

            Ok(self.versions[read.min(1)]
                .clone()
                .filter(|object| object.id() == *id))
        }

        fn try_multi_get_objects_by_key(
            &self,
            object_keys: &[ObjectKey],
        ) -> IotaResult<Vec<Option<Object>>> {
            assert!(object_keys.is_empty(), "the test names no owned inputs");

            Ok(vec![])
        }

        fn try_get_package_object(&self, _: &ObjectId) -> IotaResult<Option<PackageObject>> {
            unreachable!("not read when loading shared inputs")
        }

        fn force_reload_system_packages(&self, _: &[ObjectId]) {
            unreachable!("not read when loading shared inputs")
        }

        fn try_get_latest_object_ref_or_tombstone(
            &self,
            _: ObjectId,
        ) -> IotaResult<Option<ObjectReference>> {
            unreachable!("not read when loading shared inputs")
        }

        fn try_get_latest_object_or_tombstone(
            &self,
            _: ObjectId,
        ) -> IotaResult<Option<(ObjectKey, ObjectOrTombstone)>> {
            unreachable!("not read when loading shared inputs")
        }

        fn try_get_object_by_key(&self, _: &ObjectId, _: Version) -> IotaResult<Option<Object>> {
            unreachable!("not read when loading shared inputs")
        }

        fn try_get_object_by_key_with_historic_fallback(
            &self,
            _: &ObjectId,
            _: Version,
            _: EpochId,
        ) -> IotaResult<Option<Object>> {
            unreachable!("not read when loading shared inputs")
        }

        fn try_object_exists_by_key(&self, _: &ObjectId, _: Version) -> IotaResult<bool> {
            unreachable!("not read when loading shared inputs")
        }

        fn try_multi_object_exists_by_key(&self, _: &[ObjectKey]) -> IotaResult<Vec<bool>> {
            unreachable!("not read when loading shared inputs")
        }

        fn multi_input_objects_available_cache_only(&self, _: &[InputKey]) -> Vec<bool> {
            unreachable!("not read when loading shared inputs")
        }

        fn try_multi_transactions_executed(
            &self,
            _: &[TransactionDigest],
        ) -> IotaResult<Vec<bool>> {
            unreachable!("not read when loading shared inputs")
        }

        fn try_find_object_lt_or_eq_version(
            &self,
            _: ObjectId,
            _: Version,
        ) -> IotaResult<Option<Object>> {
            unreachable!("not read when loading shared inputs")
        }

        fn try_get_lock(&self, _: ObjectReference, _: &AuthorityPerEpochStore) -> IotaLockResult {
            unreachable!("not read when loading shared inputs")
        }

        fn _try_get_live_objref(&self, _: ObjectId) -> IotaResult<ObjectReference> {
            unreachable!("not read when loading shared inputs")
        }

        fn try_check_owned_objects_are_live(&self, _: &[ObjectReference]) -> IotaResult {
            unreachable!("not read when loading shared inputs")
        }

        fn try_get_iota_system_state_object_unsafe(&self) -> IotaResult<IotaSystemState> {
            unreachable!("not read when loading shared inputs")
        }

        fn try_find_lowest_version_above_in_epoch(
            &self,
            _: &ObjectId,
            _: Version,
            _: EpochId,
        ) -> IotaResult<Option<(Version, bool)>> {
            unreachable!("not read when loading shared inputs")
        }

        fn try_get_marker_value(
            &self,
            _: &ObjectId,
            _: Version,
            _: EpochId,
        ) -> IotaResult<Option<MarkerValue>> {
            unreachable!("not read when loading shared inputs")
        }

        fn try_get_latest_marker(
            &self,
            _: &ObjectId,
            _: EpochId,
        ) -> IotaResult<Option<(Version, MarkerValue)>> {
            Ok(self
                .deletion
                .map(|(version, digest)| (version, MarkerValue::SharedDeleted(digest))))
        }

        fn notify_read_input_objects<'a>(
            &'a self,
            _: &'a [InputKey],
            _: &'a HashSet<InputKey>,
            _: EpochId,
        ) -> BoxFuture<'a, ()> {
            unreachable!("not read when loading shared inputs")
        }
    }

    /// Loads `kinds` from `cache` for signing. Returns the kind and the read
    /// result of each loaded input, in the loader's order, and the number of
    /// reads of the object.
    fn load_for_signing(
        cache: OneSharedObjectCache,
        kinds: &[InputObjectKind],
    ) -> (Vec<(InputObjectKind, ObjectReadResultKind)>, usize) {
        let cache = Arc::new(cache);
        let loader = TransactionInputLoader::new(cache.clone());
        let (input_objects, _) = loader
            .read_objects_for_signing(None, kinds, &[], 0)
            .unwrap();
        let inputs = input_objects
            .iter()
            .map(|input| (input.input_object_kind, input.object.clone()))
            .collect();

        (inputs, cache.reads.load(Ordering::Relaxed))
    }

    /// A shared object that a transaction takes as mutable and its Move
    /// authenticator as immutable is read once, so both inputs hold the same
    /// version even when the object changes right after the first read. The
    /// inputs keep the kinds they were named with, in the same order.
    #[test]
    fn shared_object_named_with_two_kinds_is_read_once() {
        let id = ObjectId::random();
        let initial_shared_version = Version::from(1);
        let kind = |mutable| InputObjectKind::SharedMoveObject {
            id,
            initial_shared_version,
            mutable,
        };
        let object_at = |version: u64| {
            Object::with_id_owner_version_for_testing(
                id,
                Version::from(version),
                Owner::Shared(initial_shared_version),
            )
        };
        let cache = OneSharedObjectCache {
            versions: [Some(object_at(2)), Some(object_at(3))],
            deletion: None,
            reads: AtomicUsize::new(0),
        };

        let (inputs, reads) = load_for_signing(cache, &[kind(true), kind(false)]);

        let first_read = ObjectReadResultKind::Object(object_at(2));
        assert_eq!(
            inputs,
            [(kind(true), first_read.clone()), (kind(false), first_read)]
        );
        assert_eq!(reads, 1);
    }

    /// Both inputs hold the version and the transaction of the deletion, with
    /// the kinds they were named with, in the same order.
    #[test]
    fn deleted_shared_object_named_with_two_kinds_is_read_once() {
        let id = ObjectId::random();
        let kind = |mutable| InputObjectKind::SharedMoveObject {
            id,
            initial_shared_version: Version::from(1),
            mutable,
        };
        let (version, digest) = (Version::from(2), TransactionDigest::random());
        let cache = OneSharedObjectCache {
            versions: [None, None],
            deletion: Some((version, digest)),
            reads: AtomicUsize::new(0),
        };

        let (inputs, reads) = load_for_signing(cache, &[kind(true), kind(false)]);

        let deleted = ObjectReadResultKind::DeletedSharedObject(version, digest);
        assert_eq!(
            inputs,
            [(kind(true), deleted.clone()), (kind(false), deleted)]
        );
        assert_eq!(reads, 1);
    }
}
