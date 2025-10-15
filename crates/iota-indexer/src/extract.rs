// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Types and associated logic to use while extracting
//! from network checkpoints.

use std::collections::BTreeMap;

use iota_types::{
    base_types::ObjectRef, digests::TransactionDigest, full_checkpoint_content::CheckpointData,
    object::Object,
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
