// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::{BTreeMap, BTreeSet};

use iota_sdk_types::TransactionEventsDigest;

use super::{
    ChangedObject, EpochId, ExecutionStatus, GasCostSummary, IdOperation, ObjectId, ObjectIn,
    ObjectOut, ObjectReference, Owner, TransactionDigest, TransactionEffectsV1,
    UnchangedSharedKind, UnchangedSharedObject, Version,
};
use crate::execution::SharedInput;

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

    // A gas object, where there is one, is address-owned and among the changed
    // objects. A system transaction pays no gas and names none.
    if let Some(gas) = TransactionEffectsV1::gas_object(v1) {
        assert!(matches!(gas.owner, Owner::Address(_)));
    }

    for unchanged in &v1.unchanged_shared_objects {
        let id = &unchanged.object_id;
        assert!(
            unique_ids.insert(*id),
            "Duplicate object id: {id:?}\n{v1:#?}"
        );
    }
}
