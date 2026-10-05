// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Types and associated logic to use while
//! extracting and transforming data from network checkpoints.

use std::collections::BTreeMap;

use iota_sdk_types::{
    CheckpointSequenceNumber, CheckpointTimestamp, ObjectId, ObjectReference, TransactionDigest,
    TypeTag,
};
use iota_types::{
    dynamic_field::{DynamicFieldInfo, DynamicFieldType},
    fp_ensure,
    full_checkpoint_content::{CheckpointData, CheckpointTransaction},
    messages_checkpoint::CheckpointContentsExt,
    object::Object,
};

use crate::{
    errors::{IndexerError, IndexerResult},
    types::{IndexedDeletedObject, IndexedObject},
};

pub(crate) type TransactionCheckResult<'chk> = IndexerResult<(u64, &'chk CheckpointTransaction)>;

/// Enumerates checkpoint transactions while checking their integrity.
///
/// The returned iterator checks that the transaction order in the checkpoint
/// transactions agrees with the order provided in the checkpoint contents.
///
/// # Errors
///
/// Fails before enumeration if the checkpoint contents and the transactions
/// differ in length.
pub(crate) fn enumerate_checked_transactions(
    checkpoint: &CheckpointData,
) -> IndexerResult<impl Iterator<Item = TransactionCheckResult<'_>>> {
    fp_ensure!(
        checkpoint.checkpoint_contents.len() == checkpoint.transactions.len(),
        IndexerError::FullNodeReading(format!(
            "checkpointContents has different size {} compared to Transactions {} \
            for checkpoint {}",
            checkpoint.checkpoint_contents.len(),
            checkpoint.transactions.len(),
            checkpoint.checkpoint_summary.sequence_number()
        ))
    );
    Ok(checkpoint
        .checkpoint_contents
        .enumerate_transactions(&checkpoint.checkpoint_summary)
        .zip(checkpoint.transactions.iter())
        .map(|((sequence_number, execution_digest), transaction)| {
            let from_contents = execution_digest.transaction;
            let from_transactions = *transaction.transaction.digest();
            fp_ensure!(
                from_contents == from_transactions,
                IndexerError::FullNodeReading(format!(
                    "transactions has different ordering from CheckpointContents, \
                    for checkpoint {}, Mismatch found at {from_contents} v.s. {from_transactions}",
                    checkpoint.checkpoint_summary.sequence_number()
                ))
            );
            Ok((sequence_number, transaction))
        }))
}

/// A checkpoint that guarantees the integrity of its transactions.
#[derive(Clone, Debug, Copy)]
pub(crate) struct ValidatedCheckpoint<'chk> {
    inner: &'chk CheckpointData,
}

impl<'chk> ValidatedCheckpoint<'chk> {
    /// Creates a validated checkpoint from the network data.
    ///
    /// # Errors
    ///
    /// Fails if the checkpoint contents and the transactions differ in length
    /// or in the order of the transactions.
    pub(crate) fn new(checkpoint: &'chk CheckpointData) -> IndexerResult<Self> {
        Self::check_transactions_integrity(checkpoint)?;
        Ok(Self { inner: checkpoint })
    }

    /// Verifies that the transactions have the same order as in the checkpoint
    /// contents.
    ///
    /// # Errors
    ///
    /// Fails if the checkpoint contents and the transactions differ in length,
    /// or at the first transaction whose digest does not match the digest
    /// at the same position in the checkpoint contents.
    fn check_transactions_integrity(checkpoint: &CheckpointData) -> IndexerResult<()> {
        enumerate_checked_transactions(checkpoint)?
            .try_for_each(|check_result| check_result.map(|_| ()))
    }

    pub(crate) fn data(self) -> &'chk CheckpointData {
        self.inner
    }

    pub(crate) fn sequence_number(self) -> CheckpointSequenceNumber {
        self.inner.checkpoint_summary.sequence_number()
    }

    pub(crate) fn timestamp_ms(self) -> CheckpointTimestamp {
        self.inner.checkpoint_summary.timestamp_ms()
    }

    /// Enumerates the checkpoint transactions with their global sequence
    /// numbers.
    pub(crate) fn enumerate_transactions(
        self,
    ) -> impl Iterator<Item = (u64, &'chk CheckpointTransaction)> {
        self.inner
            .checkpoint_contents
            .enumerate_transactions(&self.inner.checkpoint_summary)
            .map(|(seq, _)| seq)
            .zip(&self.inner.transactions)
    }

    /// Iterates over the checkpoint transactions.
    pub(crate) fn iter_transactions(self) -> impl Iterator<Item = &'chk CheckpointTransaction> {
        self.inner.transactions.iter()
    }
}

/// If `o` is a dynamic `Field<K, V>`, determine whether it represents a Dynamic
/// Field or a Dynamic Object Field based on its type.
pub(crate) fn extract_df_kind(o: &Object) -> Option<DynamicFieldType> {
    // Skip if not a move object
    let move_object = o.data.as_opt_struct()?;

    if !move_object.struct_tag().is_dynamic_field() {
        return None;
    }

    let type_ = move_object.struct_tag();
    let [name, _] = type_.type_params() else {
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
    pub fn new(checkpoint_sequence_number: CheckpointSequenceNumber, object: Object) -> Self {
        let transaction_digest = object.as_inner().previous_transaction();
        let df_kind = extract_df_kind(&object);
        let indexed_object =
            IndexedObject::from_object(Some(checkpoint_sequence_number), object, df_kind);
        Self {
            indexed_object,
            transaction_digest,
        }
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
        object_ref: ObjectReference,
    ) -> Self {
        let indexed_object = IndexedDeletedObject {
            checkpoint_sequence_number,
            object_id: object_ref.object_id,
            object_version: object_ref.version.as_u64(),
        };
        Self {
            indexed_object,
            transaction_digest,
        }
    }

    pub(crate) fn version(&self) -> u64 {
        self.indexed_object.object_version
    }

    pub(crate) fn object_id(&self) -> ObjectId {
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

impl<'chk> From<ValidatedCheckpoint<'chk>> for CheckpointObjectChanges {
    fn from(validated_checkpoint: ValidatedCheckpoint<'chk>) -> Self {
        let checkpoint_seq = validated_checkpoint.sequence_number();

        let mut latest_live_objects = BTreeMap::new();
        let mut eventually_removed_object_refs = BTreeMap::new();
        for tx in validated_checkpoint.iter_transactions() {
            let digest = tx.transaction.digest();
            let removed_object_refs = tx.removed_object_refs_post_version().collect::<Vec<_>>();
            for obj_ref in &removed_object_refs {
                eventually_removed_object_refs.insert(obj_ref.object_id, (*obj_ref, *digest));
            }
            for obj in &tx.output_objects {
                latest_live_objects.insert(obj.id(), obj);
                eventually_removed_object_refs.remove(&obj.id());
            }
            for obj_ref in &removed_object_refs {
                latest_live_objects.remove(&obj_ref.object_id);
            }
        }

        let deleted_objects = eventually_removed_object_refs
            .into_values()
            .map(|(obj_ref, digest)| RemovedObject::new(checkpoint_seq, digest, obj_ref))
            .collect();

        let changed_objects = latest_live_objects
            .into_values()
            .map(|obj| LiveObject::new(checkpoint_seq, obj.clone()))
            .collect();
        Self {
            changed_objects,
            deleted_objects,
        }
    }
}

/// Retain the live and removed objects with the largest versions from
/// a set of consecutive checkpoints.
pub(crate) fn retain_latest_objects_from_checkpoint_batch(
    checkpoint_batch_object_changes: Vec<CheckpointObjectChanges>,
) -> CheckpointObjectChanges {
    use std::collections::HashMap;

    let mut mutations = HashMap::<ObjectId, LiveObject>::new();
    let mut deletions = HashMap::<ObjectId, RemovedObject>::new();

    for change in checkpoint_batch_object_changes {
        // Remove mutation / deletion with a following deletion / mutation,
        // as we expect that following deletion / mutation has a higher version.
        // Technically, assertions below are not required, double check just in case.
        for mutation in change.changed_objects {
            let id = mutation.object().id();
            let version = mutation.object().version();

            if let Some(existing) = deletions.remove(&id) {
                assert!(
                    existing.version() < version,
                    "mutation version ({version}) should be greater than existing deletion version ({}) for object {id}",
                    existing.version()
                );
            }

            if let Some(existing) = mutations.insert(id, mutation) {
                assert!(
                    existing.object().version() < version,
                    "mutation version ({version}) should be greater than existing mutation version ({}) for object {id}",
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
                    existing.object().version() < version,
                    "deletion version ({version}) should be greater than existing mutation version ({}) for object {id}",
                    existing.object().version(),
                );
            }

            if let Some(existing) = deletions.insert(id, deletion) {
                assert!(
                    existing.version() < version,
                    "deletion version ({version}) should be greater than existing deletion version ({}) for object {id}",
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
