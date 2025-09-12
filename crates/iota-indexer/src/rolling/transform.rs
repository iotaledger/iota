// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Types and associated logic to use while
//! transforming data from network checkpoints.

use iota_types::{
    base_types::{ObjectID, ObjectRef},
    digests::TransactionDigest,
    full_checkpoint_content::CheckpointData,
    messages_checkpoint::CheckpointSequenceNumber,
    object::Object,
};

use crate::{
    handlers::checkpoint_handler::try_extract_df_kind,
    rolling::{
        error::{IndexerError, IndexerResult},
        extract,
    },
    types::{IndexedDeletedObject, IndexedObject},
};
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
        let df_kind = try_extract_df_kind(&object)?;
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
}

#[derive(Clone, Debug, Default)]
pub(crate) struct CheckpointObjectChanges {
    pub(crate) changed_objects: Vec<LiveObject>,
    pub(crate) deleted_objects: Vec<RemovedObject>,
}

impl TryFrom<&CheckpointData> for CheckpointObjectChanges {
    type Error = IndexerError;
    fn try_from(data: &CheckpointData) -> Result<Self, Self::Error> {
        let checkpoint_seq = data.checkpoint_summary.sequence_number;
        let extractor = extract::Extractor::new(data);

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
                    "Mutation version ({version:?}) should be greater than existing deletion version ({:?}) for object {id:?}",
                    existing.version()
                );
            }

            if let Some(existing) = mutations.insert(id, mutation) {
                assert!(
                    existing.object().version() < version,
                    "Mutation version ({version:?}) should be greater than existing mutation version ({:?}) for object {id:?}",
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
                    "Deletion version ({version:?}) should be greater than existing mutation version ({:?}) for object {id:?}",
                    existing.object().version(),
                );
            }

            if let Some(existing) = deletions.insert(id, deletion) {
                assert!(
                    existing.version() < version,
                    "Deletion version ({version:?}) should be greater than existing deletion version ({:?}) for object {id:?}",
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
