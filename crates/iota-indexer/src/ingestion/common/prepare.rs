// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Types and associated logic to use while
//! extracting and transforming data from network checkpoints.

use std::collections::BTreeMap;

use iota_types::{
    base_types::{ObjectID, ObjectRef},
    digests::TransactionDigest,
    dynamic_field::{DynamicFieldInfo, DynamicFieldType},
    full_checkpoint_content::CheckpointData,
    messages_checkpoint::CheckpointSequenceNumber,
    object::Object,
};
use move_core_types::language_storage::{StructTag, TypeTag};

use crate::{
    errors::{IndexerError, IndexerResult},
    types::{IndexedDeletedObject, IndexedObject},
};

#[derive(Clone, Debug)]
pub(crate) struct Extractor<'chk> {
    checkpoint: &'chk CheckpointData,
}

impl<'chk> Extractor<'chk> {
    pub fn new(checkpoint: &'chk CheckpointData) -> Self {
        Self { checkpoint }
    }

    pub(crate) fn iter_live_objects(&'chk self) -> impl Iterator<Item = &'chk Object> + 'chk {
        let mut latest_live_objects = BTreeMap::new();
        for tx in self.checkpoint.transactions.iter() {
            for obj in tx.output_objects.iter() {
                latest_live_objects.insert(obj.id(), obj);
            }
            for obj_ref in tx.removed_object_refs_post_version() {
                latest_live_objects.remove(&(obj_ref.0));
            }
        }
        latest_live_objects.into_values()
    }

    pub(crate) fn iter_removed_objects(
        &'chk self,
    ) -> impl Iterator<Item = (ObjectRef, TransactionDigest)> + 'chk {
        let mut eventually_removed_object_refs = BTreeMap::new();
        for tx in self.checkpoint.transactions.iter() {
            let digest = tx.transaction.digest();
            for obj_ref in tx.removed_object_refs_post_version() {
                eventually_removed_object_refs.insert(obj_ref.0, (obj_ref, *digest));
            }
            for obj in tx.output_objects.iter() {
                eventually_removed_object_refs.remove(&(obj.id()));
            }
        }
        eventually_removed_object_refs.into_values()
    }
}

/// If `o` is a dynamic `Field<K, V>`, determine whether it represents a Dynamic
/// Field or a Dynamic Object Field based on its type.
pub(crate) fn extract_df_kind(o: &Object) -> Option<DynamicFieldType> {
    // Skip if not a move object
    let move_object = o.data.try_as_move()?;

    if !move_object.type_().is_dynamic_field() {
        return None;
    }

    let type_: StructTag = move_object.type_().clone().into();
    let [name, _] = type_.type_params.as_slice() else {
        return None;
    };

    Some(
        if matches!(name, TypeTag::Struct(s) if DynamicFieldInfo::is_dynamic_object_field_wrapper(s))
        {
            DynamicFieldType::DynamicObject
        } else {
            DynamicFieldType::DynamicField
        },
    )
}

/// Represent an object that is live at a certain snapshot
/// of the network.
#[derive(Clone, Debug)]
pub(crate) struct LiveObject {
    pub(crate) indexed_object: IndexedObject,
    /// The transaction that mutated the object.
    pub(crate) transaction_digest: TransactionDigest,
}

impl LiveObject {
    pub fn new(
        checkpoint_sequence_number: CheckpointSequenceNumber,
        transaction_digest: TransactionDigest,
        object: Object,
    ) -> IndexerResult<Self> {
        let df_kind = extract_df_kind(&object);
        let indexed_object =
            IndexedObject::from_object(checkpoint_sequence_number, object, df_kind);
        Ok(Self {
            indexed_object,
            transaction_digest,
        })
    }

    pub(crate) fn split(self) -> (IndexedObject, TransactionDigest) {
        (self.indexed_object, self.transaction_digest)
    }

    pub(crate) fn object(&self) -> &Object {
        &self.indexed_object.object
    }

    #[cfg(any(test, feature = "pg_integration", feature = "shared_test_runtime"))]
    fn random() -> Self {
        Self {
            indexed_object: IndexedObject::random(),
            transaction_digest: TransactionDigest::random(),
        }
    }
}

/// Represent an object that is wrapped or deleted at a certain snapshot
/// of the network.
#[derive(Clone, Debug)]
pub(crate) struct RemovedObject {
    pub(crate) indexed_object: IndexedDeletedObject,
    /// The transaction that mutated the object.
    pub(crate) transaction_digest: TransactionDigest,
}

impl RemovedObject {
    pub fn new(
        checkpoint_sequence_number: CheckpointSequenceNumber,
        transaction_digest: TransactionDigest,
        object_ref: ObjectRef,
    ) -> Self {
        let (object_id, object_version, _) = object_ref;
        let indexed_object = IndexedDeletedObject {
            checkpoint_sequence_number,
            object_id,
            object_version: object_version.into(),
        };
        Self {
            indexed_object,
            transaction_digest,
        }
    }

    pub(crate) fn version(&self) -> u64 {
        self.indexed_object.object_version
    }

    pub(crate) fn object_id(&self) -> ObjectID {
        self.indexed_object.object_id
    }

    #[cfg(any(test, feature = "pg_integration", feature = "shared_test_runtime"))]
    fn random() -> Self {
        Self {
            indexed_object: IndexedDeletedObject::random(),
            transaction_digest: TransactionDigest::random(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct CheckpointObjectChanges {
    pub(crate) changed_objects: Vec<LiveObject>,
    pub(crate) deleted_objects: Vec<RemovedObject>,
}

#[cfg(any(test, feature = "pg_integration", feature = "shared_test_runtime"))]
impl CheckpointObjectChanges {
    pub fn random() -> Self {
        Self {
            changed_objects: vec![LiveObject::random()],
            deleted_objects: vec![RemovedObject::random()],
        }
    }
}

impl TryFrom<&CheckpointData> for CheckpointObjectChanges {
    type Error = IndexerError;
    fn try_from(data: &CheckpointData) -> Result<Self, Self::Error> {
        let checkpoint_seq = data.checkpoint_summary.sequence_number;
        let extractor = Extractor::new(data);

        let deleted_objects = extractor
            .iter_removed_objects()
            .map(|(obj_ref, digest)| RemovedObject::new(checkpoint_seq, digest, obj_ref))
            .collect();

        let changed_objects = extractor
            .iter_live_objects()
            .map(|obj| {
                LiveObject::new(
                    checkpoint_seq,
                    obj.as_inner().previous_transaction,
                    obj.clone(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            changed_objects,
            deleted_objects,
        })
    }
}

/// Retain the live and removed objects with the largest versions from
/// a set of consecutive checkpoints.
pub(crate) fn retain_latest_objects_from_checkpoint_batch(
    checkpoint_batch_object_changes: Vec<CheckpointObjectChanges>,
) -> CheckpointObjectChanges {
    use std::collections::HashMap;

    let mut mutations = HashMap::<ObjectID, LiveObject>::new();
    let mut deletions = HashMap::<ObjectID, RemovedObject>::new();

    for change in checkpoint_batch_object_changes {
        // Remove mutation / deletion with a following deletion / mutation,
        // as we expect that following deletion / mutation has a higher version.
        // Technically, assertions below are not required, double check just in case.
        for mutation in change.changed_objects {
            let id = mutation.object().id();
            let version = mutation.object().version();

            if let Some(existing) = deletions.remove(&id) {
                assert!(
                    existing.version() < version.value(),
                    "mutation version ({version:?}) should be greater than existing deletion version ({:?}) for object {id:?}",
                    existing.version()
                );
            }

            if let Some(existing) = mutations.insert(id, mutation) {
                assert!(
                    existing.object().version() < version,
                    "mutation version ({version:?}) should be greater than existing mutation version ({:?}) for object {id:?}",
                    existing.object().version()
                );
            }
        }
        // Handle deleted objects
        for deletion in change.deleted_objects {
            let id = deletion.object_id();
            let version = deletion.version();

            if let Some(existing) = mutations.remove(&id) {
                assert!(
                    existing.object().version().value() < version,
                    "deletion version ({version:?}) should be greater than existing mutation version ({:?}) for object {id:?}",
                    existing.object().version(),
                );
            }

            if let Some(existing) = deletions.insert(id, deletion) {
                assert!(
                    existing.version() < version,
                    "deletion version ({version:?}) should be greater than existing deletion version ({:?}) for object {id:?}",
                    existing.version()
                );
            }
        }
    }

    CheckpointObjectChanges {
        changed_objects: mutations.into_values().collect(),
        deleted_objects: deletions.into_values().collect(),
    }
}
