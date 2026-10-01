// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashSet;

use iota_sdk_types::ObjectId;
use iota_types::effects::{TransactionEffectsAPI, TransactionEffectsExt};

use crate::{
    ingestion::common::prepare::{CheckpointObjectChanges, ValidatedCheckpoint, extract_df_kind},
    metrics::IndexerMetrics,
    models::{obj_indices::StoredObjectVersion, objects::StoredBackwardHistoryObject},
    types::{IndexedObject, IndexedPackage, ObjectStatus},
};

/// The builder of all object data to commit to the database.
#[derive(Debug, Clone, Copy)]
pub(super) struct ObjectsTransformer<'chk> {
    checkpoint: ValidatedCheckpoint<'chk>,
}

impl<'chk> ObjectsTransformer<'chk> {
    pub(super) fn new(checkpoint: ValidatedCheckpoint<'chk>) -> Self {
        Self { checkpoint }
    }

    pub(super) fn transform(self, metrics: &IndexerMetrics) -> ObjectData {
        let checkpoint_objects = {
            let _timer = metrics.indexing_objects_latency.start_timer();
            self.checkpoint.into()
        };

        ObjectData {
            checkpoint_objects,
            object_versions: self.build_object_versions(),
            packages: self.build_packages(metrics),
            history_objects: self.build_history_objects(),
        }
    }

    fn build_packages(self, metrics: &IndexerMetrics) -> Vec<IndexedPackage> {
        let _timer = metrics.indexing_packages_latency.start_timer();
        let checkpoint_sequence_number = self.checkpoint.sequence_number();
        self.checkpoint
            .iter_transactions()
            .flat_map(|tx| &tx.output_objects)
            .filter_map(|object| {
                let iota_sdk_types::ObjectData::Package(package) = object.data() else {
                    return None;
                };
                Some(IndexedPackage::new(
                    package.clone(),
                    checkpoint_sequence_number,
                ))
            })
            .collect()
    }

    /// Builds backward history entries for a checkpoint.
    ///
    /// For each transaction, records the *previous* state of every object that
    /// was superseded, so that a consistent view at an earlier checkpoint can
    /// be reconstructed by applying backward diffs to the current
    /// `checkpointed_objects` snapshot.
    ///
    /// The logic is split into three categories:
    ///
    /// 1. **Input objects that were mutated or removed** (mutate, delete,
    ///    wrap): these had an active prior state available in `input_objects` →
    ///    `ACTIVE` entries with full data.
    ///
    /// 2. **Created objects**: did not exist before → `NOT_YET_CREATED`.
    ///
    /// 3. **Unwrapped / unwrapped-then-deleted objects**: were previously
    ///    wrapped so no prior data is available → `WRAPPED_OR_DELETED` with a
    ///    lamport version approximation.
    fn build_history_objects(self) -> Vec<StoredBackwardHistoryObject> {
        let checkpoint_seq = self.checkpoint.sequence_number() as i64;
        let mut history_objects = Vec::new();

        for tx in self.checkpoint.iter_transactions() {
            let effects = &tx.effects;

            // 1. Input objects that were mutated or removed (deleted/wrapped) had an active
            //    prior state — record it from input_objects. Collect the affected IDs so we
            //    can iterate input_objects once.
            let superseded_ids: HashSet<ObjectId> = effects
                .mutated()
                .into_iter()
                .map(|mutated| mutated.reference().object_id)
                .chain(
                    effects
                        .all_removed_objects()
                        .into_iter()
                        .map(|(r, _)| r.object_id),
                )
                .collect();

            for input_obj in &tx.input_objects {
                if superseded_ids.contains(&input_obj.id()) {
                    let df_kind = extract_df_kind(input_obj);
                    let indexed = IndexedObject::from_object(
                        Some(checkpoint_seq as u64),
                        input_obj.clone(),
                        df_kind,
                    );
                    history_objects.push(StoredBackwardHistoryObject::try_from(indexed).expect(
                        "backward history conversion should not fail for active input objects",
                    ));
                }
            }

            // 2. Created objects did not exist before this transaction. Use lamport version
            //    - 1 so the version is monotonic with other backward-history rows for the
            //    same object.
            for created in effects.created() {
                history_objects.push(StoredBackwardHistoryObject::from_empty(
                    created.reference().object_id,
                    created.reference().version.as_u64() as i64 - 1,
                    ObjectStatus::NotYetCreated,
                    checkpoint_seq,
                ));
            }

            // 3. Unwrapped and unwrapped-then-deleted objects were previously wrapped — no
            //    data available. Use lamport version - 1 as approximation.
            let unwrapped_refs = effects
                .unwrapped()
                .into_iter()
                .map(|unwrapped| *unwrapped.reference());
            let unwrapped_then_deleted_refs = effects.unwrapped_then_deleted().into_iter();
            for r in unwrapped_refs.chain(unwrapped_then_deleted_refs) {
                history_objects.push(StoredBackwardHistoryObject::from_empty(
                    r.object_id,
                    r.version.as_u64() as i64 - 1,
                    ObjectStatus::WrappedOrDeleted,
                    checkpoint_seq,
                ));
            }
        }

        history_objects
    }

    fn build_object_versions(self) -> Vec<StoredObjectVersion> {
        let cp_sequence_number = self.checkpoint.sequence_number() as i64;
        let removed = self
            .checkpoint
            .iter_transactions()
            .flat_map(|tx| tx.removed_object_refs_post_version())
            .map(|obj_ref| StoredObjectVersion {
                object_id: obj_ref.object_id.as_bytes().to_vec(),
                object_version: obj_ref.version.as_u64() as i64,
                cp_sequence_number,
            });
        let output = self
            .checkpoint
            .iter_transactions()
            .flat_map(|tx| &tx.output_objects)
            .map(|o| StoredObjectVersion {
                object_id: o.id().as_bytes().to_vec(),
                object_version: o.version().as_u64() as i64,
                cp_sequence_number,
            });
        removed.chain(output).collect()
    }
}

#[derive(Debug)]
pub(super) struct ObjectData {
    pub(super) checkpoint_objects: CheckpointObjectChanges,
    pub(super) object_versions: Vec<StoredObjectVersion>,
    pub(super) history_objects: Vec<StoredBackwardHistoryObject>,
    pub(super) packages: Vec<IndexedPackage>,
}
