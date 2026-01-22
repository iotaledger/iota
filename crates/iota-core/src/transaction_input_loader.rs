// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, sync::Arc};

use iota_common::fatal;
use iota_types::{
    account_abstraction::account::AuthenticatorFunctionRefV1Key,
    base_types::{EpochId, ObjectID, ObjectRef, SequenceNumber, TransactionDigest},
    dynamic_field::{self},
    error::{IotaError, IotaResult, UserInputError},
    object::Owner,
    storage::ObjectKey,
    transaction::{
        InputObjectKind, InputObjects, ObjectReadResult, ObjectReadResultKind,
        ReceivingObjectReadResult, ReceivingObjectReadResultKind, ReceivingObjects, TransactionKey,
    },
};
use itertools::izip;
use once_cell::unsync::OnceCell;
use tracing::instrument;

use crate::{
    authority::authority_per_epoch_store::{AuthorityPerEpochStore, CertLockGuard},
    execution_cache::ObjectCacheRead,
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
        receiving_objects: &[ObjectRef],
        epoch_id: EpochId,
    ) -> IotaResult<(InputObjects, ReceivingObjects)> {
        // Length of input_object_kinds have been checked via validity_check() for
        // ProgrammableTransaction.
        let mut input_results = vec![None; input_object_kinds.len()];
        let mut object_refs = Vec::with_capacity(input_object_kinds.len());
        let mut fetch_indices = Vec::with_capacity(input_object_kinds.len());

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
                InputObjectKind::SharedMoveObject { id, .. } => match self
                    .cache
                    .try_get_object(id)?
                {
                    Some(object) => {
                        input_results[i] = Some(ObjectReadResult::new(*kind, object.into()))
                    }
                    None => {
                        if let Some((version, digest)) = self
                            .cache
                            .try_get_last_shared_object_deletion_info(id, epoch_id)?
                        {
                            input_results[i] = Some(ObjectReadResult {
                                input_object_kind: *kind,
                                object: ObjectReadResultKind::DeletedSharedObject(version, digest),
                            });
                        } else {
                            return Err(IotaError::from(kind.object_not_found_error()));
                        }
                    }
                },
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

        let receiving_results =
            self.read_receiving_objects_for_signing(receiving_objects, epoch_id)?;

        let input_results: InputObjects = input_results
            .into_iter()
            .map(Option::unwrap)
            .collect::<Vec<_>>()
            .into();

        self.validate_receiving_objects_not_in_account_objects(&input_results, &receiving_results)?;

        Ok((input_results, receiving_results))
    }

    /// Read the inputs for a transaction that is ready to be executed.
    ///
    /// epoch_store is used to resolve the versions of any shared input objects.
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
        epoch_store: &Arc<AuthorityPerEpochStore>,
        tx_key: &TransactionKey,
        // Important to hold the _tx_lock, otherwise it would be possible for a concurrent
        // execution of the same tx to enter this point after the first execution has
        // finished and the shared locks have been deleted.
        _tx_lock: &CertLockGuard,
        input_object_kinds: &[InputObjectKind],
        epoch_id: EpochId,
    ) -> IotaResult<InputObjects> {
        let assigned_shared_versions_cell: OnceCell<Option<HashMap<_, _>>> = OnceCell::new();

        let mut results = vec![None; input_object_kinds.len()];
        let mut object_keys = Vec::with_capacity(input_object_kinds.len());
        let mut fetches = Vec::with_capacity(input_object_kinds.len());

        for (i, input) in input_object_kinds.iter().enumerate() {
            match input {
                InputObjectKind::MovePackage(id) => {
                    let package = self.cache.try_get_package_object(id)?.unwrap_or_else(|| {
                        panic!("Executable transaction {tx_key:?} depends on non-existent package {id:?}")
                    });

                    results[i] = Some(ObjectReadResult {
                        input_object_kind: *input,
                        object: ObjectReadResultKind::Object(package.into()),
                    });
                    continue;
                }
                InputObjectKind::ImmOrOwnedMoveObject(objref) => {
                    object_keys.push(objref.into());
                    fetches.push((i, input));
                }
                InputObjectKind::SharedMoveObject { id, .. } => {
                    let assigned_shared_versions = assigned_shared_versions_cell
                        .get_or_init(|| {
                            epoch_store
                                .get_assigned_shared_object_versions(tx_key)
                                .map(|versions| versions.into_iter().collect())
                        })
                        .as_ref()
                        .unwrap_or_else(|| {
                            // Important to hold the _tx_lock here - otherwise it would be possible
                            // for a concurrent execution of the same tx to enter this point after
                            // the first execution has finished and the assigned shared versions
                            // have been deleted.
                            fatal!(
                                "Failed to get assigned shared versions for transaction {tx_key:?}"
                            );
                        });
                    // If we find a set of assigned versions but an object is missing, it indicates
                    // a serious inconsistency:
                    let version = assigned_shared_versions.get(id).unwrap_or_else(|| {
                        panic!("Shared object version should have been assigned. key: {tx_key:?}, obj id: {id:?}")
                    });
                    if version.is_cancelled() {
                        // Do not need to fetch shared object for cancelled transaction.
                        results[i] = Some(ObjectReadResult {
                            input_object_kind: *input,
                            object: ObjectReadResultKind::CancelledTransactionSharedObject(
                                *version,
                            ),
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

        Ok(results
            .into_iter()
            .map(Option::unwrap)
            .collect::<Vec<_>>()
            .into())
    }
}

// private methods
impl TransactionInputLoader {
    fn read_receiving_objects_for_signing(
        &self,
        receiving_objects: &[ObjectRef],
        epoch_id: EpochId,
    ) -> IotaResult<ReceivingObjects> {
        let mut receiving_results = Vec::with_capacity(receiving_objects.len());
        for objref in receiving_objects {
            // Note: the digest is checked later in check_transaction_input
            let (object_id, version, _) = objref;

            if self
                .cache
                .try_have_received_object_at_version(object_id, *version, epoch_id)?
            {
                receiving_results.push(ReceivingObjectReadResult::new(
                    *objref,
                    ReceivingObjectReadResultKind::PreviouslyReceivedObject,
                ));
                continue;
            }

            let Some(object) = self.cache.try_get_object(object_id)? else {
                return Err(UserInputError::ObjectNotFound {
                    object_id: *object_id,
                    version: Some(*version),
                }
                .into());
            };

            receiving_results.push(ReceivingObjectReadResult::new(*objref, object.into()));
        }
        Ok(receiving_results.into())
    }

    /// Tries to find receiving objects owned by input objects that have a valid
    /// AuthenticatorFunctionRefV1 dynamic field (i.e., are account objects).
    fn validate_receiving_objects_not_in_account_objects(
        &self,
        input_results: &InputObjects,
        receiving_results: &ReceivingObjects,
    ) -> IotaResult<()> {
        if receiving_results.objects.is_empty() {
            return Ok(());
        }

        // Build a lookup: input object ID -> version
        let input_objects_map: HashMap<ObjectID, SequenceNumber> = input_results
            .iter_objects()
            .map(|obj| (obj.id(), obj.version()))
            .collect();

        // for each receiving object, check if its owner is an account object
        for receiving in receiving_results.iter_objects() {
            let Owner::AddressOwner(owner_address) = receiving.owner() else {
                // if not an address owner, it will fail in later checks
                continue;
            };

            let owner_id: ObjectID = (*owner_address).into();
            let Some(&version) = input_objects_map.get(&owner_id) else {
                continue;
            };

            // Derive the dynamic field ID for AuthenticatorFunctionRefV1
            let auth_field_id = dynamic_field::derive_dynamic_field_id(
                owner_id,
                &AuthenticatorFunctionRefV1Key::tag().into(),
                &AuthenticatorFunctionRefV1Key::default().to_bcs_bytes(),
            )
            .expect("derive_dynamic_field_id should not fail");

            // Try to fetch the authenticator field at or before the shared version
            if let Some(_auth_field_obj) = self
                .cache
                .try_find_object_lt_or_eq_version(auth_field_id, version)
                .ok()
                .flatten()
            {
                // It does not parse and validate the dynamic field to be
                // AuthenticatorFunctionRefV1, it just assumes that the key was generated
                // correctly and uniquely maps a AuthenticatorFunctionRefV1.

                // Finally, we have confirmed that the receiving object's owner is an account
                // object. Raise an error.
                return Err(UserInputError::ReceivingObjectForAccountObject {
                    receiving_object_id: receiving.id(),
                    account_object_id: owner_id,
                }
                .into());
            }
        }

        Ok(())
    }
}
