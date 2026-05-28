// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::{BTreeMap, BTreeSet};

use iota_sdk_types::Digest;

use super::{
    EffectsObjectChange, EpochId, ExecutionStatus, GasCostSummary, IDOperation, InputSharedObject,
    ObjectChange, ObjectID, ObjectIn, ObjectOut, ObjectRef, Owner, TransactionEffectsV1,
    UnchangedSharedKind, UnchangedSharedObject, Version,
};
use crate::{
    IotaAddress,
    digests::{TransactionDigest, TransactionEventsDigest},
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

    fn modified_at_versions(&self) -> Vec<(ObjectID, Version)> {
        self.changed_objects
            .iter()
            .filter_map(|change| {
                if let ObjectIn::Data { version, .. } = &change.input_state {
                    Some((change.object_id, *version))
                } else {
                    None
                }
            })
            .collect()
    }

    fn lamport_version(&self) -> Version {
        self.lamport_version
    }

    fn old_object_metadata(&self) -> Vec<(ObjectRef, Owner)> {
        self.changed_objects
            .iter()
            .filter_map(|change| {
                if let ObjectIn::Data {
                    version,
                    digest,
                    owner,
                } = change.input_state
                {
                    Some((ObjectRef::new(change.object_id, version, digest), owner))
                } else {
                    None
                }
            })
            .collect()
    }

    fn input_shared_objects(&self) -> Vec<InputSharedObject> {
        self.changed_objects
            .iter()
            .filter_map(|changed| {
                if let ObjectIn::Data {
                    version,
                    digest,
                    owner: Owner::Shared { .. },
                } = changed.input_state
                {
                    Some(InputSharedObject::Mutate(ObjectRef::new(
                        changed.object_id,
                        version,
                        digest,
                    )))
                } else {
                    None
                }
            })
            .chain(self.unchanged_shared_objects.iter().filter_map(
                |unchanged| match unchanged.kind {
                    UnchangedSharedKind::ReadOnlyRoot { version, digest } => {
                        Some(InputSharedObject::ReadOnly(ObjectRef::new(
                            unchanged.object_id,
                            version,
                            digest,
                        )))
                    }
                    UnchangedSharedKind::MutateDeleted { version } => Some(
                        InputSharedObject::MutateDeleted(unchanged.object_id, version),
                    ),
                    UnchangedSharedKind::ReadDeleted { version } => {
                        Some(InputSharedObject::ReadDeleted(unchanged.object_id, version))
                    }
                    UnchangedSharedKind::Cancelled { version } => {
                        Some(InputSharedObject::Cancelled(unchanged.object_id, version))
                    }
                    // We can not expose the per epoch config object as input shared object,
                    // since it does not require sequencing, and hence shall not be considered
                    // as a normal input shared object.
                    UnchangedSharedKind::PerEpochConfig => None,
                    _ => unimplemented!(
                        "a new UnchangedSharedKind enum variant was added and needs to be handled"
                    ),
                },
            ))
            .collect()
    }

    fn created(&self) -> Vec<(ObjectRef, Owner)> {
        self.changed_objects
            .iter()
            .filter_map(|changed| {
                match (
                    &changed.input_state,
                    &changed.output_state,
                    &changed.id_operation,
                ) {
                    (
                        ObjectIn::Missing,
                        ObjectOut::ObjectWrite { digest, owner },
                        IDOperation::Created,
                    ) => Some((
                        ObjectRef::new(changed.object_id, self.lamport_version, *digest),
                        *owner,
                    )),
                    (
                        ObjectIn::Missing,
                        ObjectOut::PackageWrite { version, digest },
                        IDOperation::Created,
                    ) => Some((
                        ObjectRef::new(changed.object_id, *version, *digest),
                        Owner::Immutable,
                    )),
                    _ => None,
                }
            })
            .collect()
    }

    fn mutated(&self) -> Vec<(ObjectRef, Owner)> {
        self.changed_objects
            .iter()
            .filter_map(
                |changed| match (&changed.input_state, &changed.output_state) {
                    (ObjectIn::Data { .. }, ObjectOut::ObjectWrite { digest, owner }) => Some((
                        ObjectRef::new(changed.object_id, self.lamport_version, *digest),
                        *owner,
                    )),
                    (ObjectIn::Data { .. }, ObjectOut::PackageWrite { version, digest }) => Some((
                        ObjectRef::new(changed.object_id, *version, *digest),
                        Owner::Immutable,
                    )),
                    _ => None,
                },
            )
            .collect()
    }

    fn unwrapped(&self) -> Vec<(ObjectRef, Owner)> {
        self.changed_objects
            .iter()
            .filter_map(|changed| {
                match (
                    &changed.input_state,
                    &changed.output_state,
                    &changed.id_operation,
                ) {
                    (
                        ObjectIn::Missing,
                        ObjectOut::ObjectWrite { digest, owner },
                        IDOperation::None,
                    ) => Some((
                        ObjectRef::new(changed.object_id, self.lamport_version, *digest),
                        *owner,
                    )),
                    _ => None,
                }
            })
            .collect()
    }

    fn deleted(&self) -> Vec<ObjectRef> {
        self.changed_objects
            .iter()
            .filter_map(|changed| {
                match (
                    &changed.input_state,
                    &changed.output_state,
                    &changed.id_operation,
                ) {
                    (ObjectIn::Data { .. }, ObjectOut::Missing, IDOperation::Deleted) => {
                        Some(ObjectRef::new(
                            changed.object_id,
                            self.lamport_version,
                            Digest::OBJECT_DELETED,
                        ))
                    }
                    _ => None,
                }
            })
            .collect()
    }

    fn unwrapped_then_deleted(&self) -> Vec<ObjectRef> {
        self.changed_objects
            .iter()
            .filter_map(|changed| {
                match (
                    &changed.input_state,
                    &changed.output_state,
                    &changed.id_operation,
                ) {
                    (ObjectIn::Missing, ObjectOut::Missing, IDOperation::Deleted) => {
                        Some(ObjectRef::new(
                            changed.object_id,
                            self.lamport_version,
                            Digest::OBJECT_DELETED,
                        ))
                    }
                    _ => None,
                }
            })
            .collect()
    }

    fn wrapped(&self) -> Vec<ObjectRef> {
        self.changed_objects
            .iter()
            .filter_map(|changed| {
                match (
                    &changed.input_state,
                    &changed.output_state,
                    &changed.id_operation,
                ) {
                    (ObjectIn::Data { .. }, ObjectOut::Missing, IDOperation::None) => {
                        Some(ObjectRef::new(
                            changed.object_id,
                            self.lamport_version,
                            Digest::OBJECT_WRAPPED,
                        ))
                    }
                    _ => None,
                }
            })
            .collect()
    }

    fn object_changes(&self) -> Vec<ObjectChange> {
        self.changed_objects
            .iter()
            .map(|changed| {
                let input_version_digest = match &changed.input_state {
                    ObjectIn::Missing => None,
                    ObjectIn::Data {
                        version, digest, ..
                    } => Some((version, digest)),
                    _ => unimplemented!(
                        "a new ObjectIn enum variant was added and needs to be handled"
                    ),
                };

                let output_version_digest = match &changed.output_state {
                    ObjectOut::Missing => None,
                    ObjectOut::ObjectWrite { digest, .. } => Some((&self.lamport_version, digest)),
                    ObjectOut::PackageWrite { version, digest } => Some((version, digest)),
                    _ => unimplemented!(
                        "a new ObjectOut enum variant was added and needs to be handled"
                    ),
                };

                ObjectChange {
                    id: changed.object_id,
                    input_version: input_version_digest.map(|k| *k.0),
                    input_digest: input_version_digest.map(|k| *k.1),
                    output_version: output_version_digest.map(|k| *k.0),
                    output_digest: output_version_digest.map(|k| *k.1),
                    id_operation: changed.id_operation,
                }
            })
            .collect()
    }

    fn gas_object(&self) -> (ObjectRef, Owner) {
        if let Some(gas_object_index) = self.gas_object_index {
            let changed = &self.changed_objects[gas_object_index as usize];
            match changed.output_state {
                ObjectOut::ObjectWrite { digest, owner } => (
                    ObjectRef::new(changed.object_id, self.lamport_version, digest),
                    owner,
                ),
                _ => panic!("Gas object must be an ObjectWrite in changed_objects"),
            }
        } else {
            (
                ObjectRef::new(ObjectID::ZERO, Version::default(), Digest::MIN),
                Owner::Address(IotaAddress::ZERO),
            )
        }
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

    fn unchanged_shared_objects(&self) -> Vec<(ObjectID, UnchangedSharedKind)> {
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
                self.changed_objects.push(EffectsObjectChange {
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
                    id_operation: IDOperation::None,
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
                    kind: UnchangedSharedKind::Cancelled { version },
                })
            }
        }
    }

    fn unsafe_add_deleted_live_object_for_testing(&mut self, object_ref: ObjectRef) {
        let (object_id, version, digest) = object_ref.into_parts();
        self.changed_objects.push(EffectsObjectChange {
            object_id,
            input_state: ObjectIn::Data {
                version,
                digest,
                owner: Owner::Address(IotaAddress::ZERO),
            },
            output_state: ObjectOut::ObjectWrite {
                digest,
                owner: Owner::Address(IotaAddress::ZERO),
            },
            id_operation: IDOperation::None,
        })
    }

    fn unsafe_add_object_tombstone_for_testing(&mut self, object_ref: ObjectRef) {
        let (object_id, version, digest) = object_ref.into_parts();
        self.changed_objects.push(EffectsObjectChange {
            object_id,
            input_state: ObjectIn::Data {
                version,
                digest,
                owner: Owner::Address(IotaAddress::ZERO),
            },
            output_state: ObjectOut::Missing,
            id_operation: IDOperation::Deleted,
        })
    }
}

pub(crate) fn new_from_execution(
    status: ExecutionStatus,
    epoch: EpochId,
    gas_cost_summary: GasCostSummary,
    shared_objects: Vec<SharedInput>,
    loaded_per_epoch_config_objects: BTreeSet<ObjectID>,
    transaction_digest: TransactionDigest,
    lamport_version: Version,
    changed_objects: BTreeMap<ObjectID, EffectsObjectChange>,
    gas_object: Option<ObjectID>,
    events_digest: Option<TransactionEventsDigest>,
    dependencies: Vec<TransactionDigest>,
) -> TransactionEffectsV1 {
    let unchanged_shared_objects = shared_objects
        .into_iter()
        .filter_map(|shared_input| match shared_input {
            SharedInput::Existing(ObjectRef {
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
                Some((id, UnchangedSharedKind::Cancelled { version }))
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
            (ObjectIn::Missing, ObjectOut::Missing, IDOperation::Created) => {
                // created and then wrapped Move object.
            }
            (ObjectIn::Missing, ObjectOut::Missing, IDOperation::Deleted) => {
                // unwrapped and then deleted Move object.
            }
            (ObjectIn::Missing, ObjectOut::ObjectWrite { owner, .. }, IDOperation::None) => {
                // unwrapped Move object.
                // It's not allowed to make an object shared after unwrapping.
                assert!(!owner.is_shared());
            }
            (ObjectIn::Missing, ObjectOut::ObjectWrite { .. }, IDOperation::Created) => {
                // created Move object.
            }
            (ObjectIn::Missing, ObjectOut::PackageWrite { .. }, IDOperation::Created) => {
                // created Move package or user Move package upgrade.
            }
            (
                ObjectIn::Data {
                    version: old_version,
                    owner: old_owner,
                    ..
                },
                ObjectOut::Missing,
                IDOperation::None,
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
                IDOperation::Deleted,
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
                IDOperation::None,
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
                IDOperation::None,
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
    let (_, owner) = v1.gas_object();
    assert!(matches!(owner, Owner::Address(_)));

    for unchanged in &v1.unchanged_shared_objects {
        let id = &unchanged.object_id;
        assert!(
            unique_ids.insert(*id),
            "Duplicate object id: {id:?}\n{v1:#?}"
        );
    }
}
