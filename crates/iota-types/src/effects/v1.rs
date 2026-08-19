// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::{BTreeMap, BTreeSet};

use iota_sdk_types::{
    Address, ObjectDigest, ObjectVersion, OwnedObjectReference, TransactionDigest,
    TransactionEventsDigest,
};

use super::{
    ChangedObject, EpochId, ExecutionStatus, GasCostSummary, IdOperation, InputSharedObject,
    ObjectChange, ObjectId, ObjectIn, ObjectOut, ObjectReference, Owner, TransactionEffectsV1,
    UnchangedSharedKind, UnchangedSharedObject, Version,
};
use crate::{
    effects::{TransactionEffectsAPI, TransactionEffectsAPIForTesting},
    execution::SharedInput,
    object::OBJECT_START_VERSION,
};

impl TransactionEffectsAPI for TransactionEffectsV1 {
    fn status(&self) -> &ExecutionStatus {
        &self.status
    }

    fn into_status(self) -> ExecutionStatus {
        self.status
    }

    fn epoch(&self) -> EpochId {
        self.epoch
    }

    fn modified_at_versions(&self) -> Vec<ObjectVersion> {
        TransactionEffectsV1::modified_at_versions(self)
    }

    fn lamport_version(&self) -> Version {
        self.lamport_version
    }

    fn old_object_metadata(&self) -> Vec<OwnedObjectReference> {
        TransactionEffectsV1::old_object_metadata(self)
    }

    fn input_shared_objects(&self) -> Vec<InputSharedObject> {
        TransactionEffectsV1::input_shared_objects(self)
            .into_iter()
            .map(|shared| match shared {
                iota_sdk_types::InputSharedObject::Mutate(reference) => {
                    InputSharedObject::Mutate(reference)
                }
                iota_sdk_types::InputSharedObject::ReadOnly(reference) => {
                    InputSharedObject::ReadOnly(reference)
                }
                iota_sdk_types::InputSharedObject::ReadDeleted(object) => {
                    InputSharedObject::ReadDeleted(object.object_id, object.version)
                }
                iota_sdk_types::InputSharedObject::MutateDeleted(object) => {
                    InputSharedObject::MutateDeleted(object.object_id, object.version)
                }
                iota_sdk_types::InputSharedObject::Canceled(object) => {
                    InputSharedObject::Cancelled(object.object_id, object.version)
                }
            })
            .collect()
    }

    fn created(&self) -> Vec<OwnedObjectReference> {
        TransactionEffectsV1::created(self)
    }

    fn mutated(&self) -> Vec<OwnedObjectReference> {
        TransactionEffectsV1::mutated(self)
    }

    fn unwrapped(&self) -> Vec<OwnedObjectReference> {
        TransactionEffectsV1::unwrapped(self)
    }

    fn deleted(&self) -> Vec<ObjectReference> {
        TransactionEffectsV1::deleted(self)
    }

    fn unwrapped_then_deleted(&self) -> Vec<ObjectReference> {
        TransactionEffectsV1::unwrapped_then_deleted(self)
    }

    fn wrapped(&self) -> Vec<ObjectReference> {
        TransactionEffectsV1::wrapped(self)
    }

    fn object_changes(&self) -> Vec<ObjectChange> {
        TransactionEffectsV1::object_changes(self)
    }

    fn gas_object(&self) -> OwnedObjectReference {
        // A transaction that needs no gas has no gas object; this reports the
        // dummy reference callers here have always been given for one.
        TransactionEffectsV1::gas_object(self).unwrap_or_else(|| {
            OwnedObjectReference::new(
                ObjectReference::new(ObjectId::ZERO, Version::default(), ObjectDigest::MIN),
                Owner::Address(Address::ZERO),
            )
        })
    }

    fn events_digest(&self) -> Option<&TransactionEventsDigest> {
        self.events_digest.as_ref()
    }

    fn dependencies(&self) -> &[TransactionDigest] {
        &self.dependencies
    }

    fn transaction_digest(&self) -> &TransactionDigest {
        &self.transaction_digest
    }

    fn gas_cost_summary(&self) -> &GasCostSummary {
        &self.gas_cost_summary
    }

    fn unchanged_shared_objects(&self) -> Vec<(ObjectId, UnchangedSharedKind)> {
        self.unchanged_shared_objects
            .iter()
            .map(|unchanged| (unchanged.object_id, unchanged.kind.clone()))
            .collect()
    }
}

impl TransactionEffectsAPIForTesting for TransactionEffectsV1 {
    fn status_mut_for_testing(&mut self) -> &mut ExecutionStatus {
        &mut self.status
    }

    fn gas_cost_summary_mut_for_testing(&mut self) -> &mut GasCostSummary {
        &mut self.gas_cost_summary
    }

    fn transaction_digest_mut_for_testing(&mut self) -> &mut TransactionDigest {
        &mut self.transaction_digest
    }

    fn dependencies_mut_for_testing(&mut self) -> &mut Vec<TransactionDigest> {
        &mut self.dependencies
    }

    fn unsafe_add_input_shared_object_for_testing(&mut self, kind: InputSharedObject) {
        match kind {
            InputSharedObject::Mutate(object_ref) => {
                let (object_id, version, digest) = object_ref.into_parts();
                self.changed_objects.push(ChangedObject {
                    object_id,
                    input_state: ObjectIn::Data {
                        version,
                        digest,
                        owner: Owner::Shared(OBJECT_START_VERSION),
                    },
                    output_state: ObjectOut::ObjectWrite {
                        digest,
                        owner: Owner::Shared(version),
                    },
                    id_operation: IdOperation::None,
                })
            }
            InputSharedObject::ReadOnly(object_ref) => {
                let (object_id, version, digest) = object_ref.into_parts();
                self.unchanged_shared_objects.push(UnchangedSharedObject {
                    object_id,
                    kind: UnchangedSharedKind::ReadOnlyRoot { version, digest },
                })
            }
            InputSharedObject::ReadDeleted(object_id, version) => {
                self.unchanged_shared_objects.push(UnchangedSharedObject {
                    object_id,
                    kind: UnchangedSharedKind::ReadDeleted { version },
                })
            }
            InputSharedObject::MutateDeleted(object_id, version) => {
                self.unchanged_shared_objects.push(UnchangedSharedObject {
                    object_id,
                    kind: UnchangedSharedKind::MutateDeleted { version },
                })
            }
            InputSharedObject::Cancelled(object_id, version) => {
                self.unchanged_shared_objects.push(UnchangedSharedObject {
                    object_id,
                    kind: UnchangedSharedKind::Canceled { version },
                })
            }
        }
    }

    fn unsafe_add_deleted_live_object_for_testing(&mut self, object_ref: ObjectReference) {
        let (object_id, version, digest) = object_ref.into_parts();
        self.changed_objects.push(ChangedObject {
            object_id,
            input_state: ObjectIn::Data {
                version,
                digest,
                owner: Owner::Address(Address::ZERO),
            },
            output_state: ObjectOut::ObjectWrite {
                digest,
                owner: Owner::Address(Address::ZERO),
            },
            id_operation: IdOperation::None,
        })
    }

    fn unsafe_add_object_tombstone_for_testing(&mut self, object_ref: ObjectReference) {
        let (object_id, version, digest) = object_ref.into_parts();
        self.changed_objects.push(ChangedObject {
            object_id,
            input_state: ObjectIn::Data {
                version,
                digest,
                owner: Owner::Address(Address::ZERO),
            },
            output_state: ObjectOut::Missing,
            id_operation: IdOperation::Deleted,
        })
    }
}

pub(crate) fn new_from_execution(
    status: ExecutionStatus,
    epoch: EpochId,
    gas_cost_summary: GasCostSummary,
    shared_objects: Vec<SharedInput>,
    loaded_per_epoch_config_objects: BTreeSet<ObjectId>,
    transaction_digest: TransactionDigest,
    lamport_version: Version,
    changed_objects: BTreeMap<ObjectId, ChangedObject>,
    gas_object: Option<ObjectId>,
    events_digest: Option<TransactionEventsDigest>,
    dependencies: Vec<TransactionDigest>,
) -> TransactionEffectsV1 {
    let unchanged_shared_objects = shared_objects
        .into_iter()
        .filter_map(|shared_input| match shared_input {
            SharedInput::Existing(ObjectReference {
                object_id: id,
                version,
                digest,
            }) => {
                if changed_objects.contains_key(&id) {
                    None
                } else {
                    Some((id, UnchangedSharedKind::ReadOnlyRoot { version, digest }))
                }
            }
            SharedInput::Deleted((id, version, mutable, _)) => {
                debug_assert!(!changed_objects.contains_key(&id));
                if mutable {
                    Some((id, UnchangedSharedKind::MutateDeleted { version }))
                } else {
                    Some((id, UnchangedSharedKind::ReadDeleted { version }))
                }
            }
            SharedInput::Cancelled((id, version)) => {
                debug_assert!(!changed_objects.contains_key(&id));
                Some((id, UnchangedSharedKind::Canceled { version }))
            }
        })
        .chain(
            loaded_per_epoch_config_objects
                .into_iter()
                .map(|id| (id, UnchangedSharedKind::PerEpochConfig)),
        )
        .map(|(object_id, kind)| UnchangedSharedObject { object_id, kind })
        .collect();

    let changed_objects: Vec<_> = changed_objects.into_values().collect();

    let gas_object_index = gas_object.map(|gas_id| {
        changed_objects
            .iter()
            .position(|changed| changed.object_id == gas_id)
            .unwrap() as u32
    });

    let v1 = TransactionEffectsV1 {
        status,
        epoch,
        gas_cost_summary,
        transaction_digest,
        lamport_version,
        changed_objects,
        unchanged_shared_objects,
        gas_object_index,
        events_digest,
        dependencies,
        auxiliary_data_digest: None,
    };

    #[cfg(debug_assertions)]
    check_invariant(&v1);

    v1
}

/// This function demonstrates what's the invariant of the effects.
/// It also documents the semantics of different combinations in object
/// changes.
#[cfg(debug_assertions)]
fn check_invariant(v1: &TransactionEffectsV1) {
    use std::collections::HashSet;

    let mut unique_ids = HashSet::new();
    for changed in &v1.changed_objects {
        let id = &changed.object_id;
        assert!(unique_ids.insert(*id));
        match (
            &changed.input_state,
            &changed.output_state,
            &changed.id_operation,
        ) {
            (ObjectIn::Missing, ObjectOut::Missing, IdOperation::Created) => {
                // created and then wrapped Move object.
            }
            (ObjectIn::Missing, ObjectOut::Missing, IdOperation::Deleted) => {
                // unwrapped and then deleted Move object.
            }
            (ObjectIn::Missing, ObjectOut::ObjectWrite { owner, .. }, IdOperation::None) => {
                // unwrapped Move object.
                // It's not allowed to make an object shared after unwrapping.
                assert!(!owner.is_shared());
            }
            (ObjectIn::Missing, ObjectOut::ObjectWrite { .. }, IdOperation::Created) => {
                // created Move object.
            }
            (ObjectIn::Missing, ObjectOut::PackageWrite { .. }, IdOperation::Created) => {
                // created Move package or user Move package upgrade.
            }
            (
                ObjectIn::Data {
                    version: old_version,
                    owner: old_owner,
                    ..
                },
                ObjectOut::Missing,
                IdOperation::None,
            ) => {
                // wrapped.
                assert!(*old_version < v1.lamport_version);
                assert!(
                    !old_owner.is_shared() && !old_owner.is_immutable(),
                    "Cannot wrap shared or immutable object"
                );
            }
            (
                ObjectIn::Data {
                    version: old_version,
                    owner: old_owner,
                    ..
                },
                ObjectOut::Missing,
                IdOperation::Deleted,
            ) => {
                // deleted.
                assert!(*old_version < v1.lamport_version);
                assert!(!old_owner.is_immutable(), "Cannot delete immutable object");
            }
            (
                ObjectIn::Data {
                    version: old_version,
                    digest: old_digest,
                    owner: old_owner,
                },
                ObjectOut::ObjectWrite {
                    digest: new_digest,
                    owner: new_owner,
                    ..
                },
                IdOperation::None,
            ) => {
                // mutated.
                assert!(*old_version < v1.lamport_version);
                assert_ne!(old_digest, new_digest);
                assert!(!old_owner.is_immutable(), "Cannot mutate immutable object");
                if old_owner.is_shared() {
                    assert!(new_owner.is_shared(), "Cannot un-share an object");
                } else {
                    assert!(!new_owner.is_shared(), "Cannot share an existing object");
                }
            }
            (
                ObjectIn::Data {
                    version: old_version,
                    digest: old_digest,
                    owner: old_owner,
                },
                ObjectOut::PackageWrite {
                    version: new_version,
                    digest: new_digest,
                    ..
                },
                IdOperation::None,
            ) => {
                // system package upgrade.
                assert!(
                    old_owner.is_immutable() && id.is_system_package(),
                    "Must be a system package"
                );
                assert_eq!(*old_version + 1, *new_version);
                assert_ne!(old_digest, new_digest);
            }
            _ => {
                panic!("Impossible object change: {id:?}, {changed:?}");
            }
        }
    }

    // Make sure that gas object exists in changed_objects.
    let OwnedObjectReference { owner, .. } = TransactionEffectsAPI::gas_object(v1);
    assert!(matches!(owner, Owner::Address(_)));

    for unchanged in &v1.unchanged_shared_objects {
        let id = &unchanged.object_id;
        assert!(
            unique_ids.insert(*id),
            "Duplicate object id: {id:?}\n{v1:#?}"
        );
    }
}
