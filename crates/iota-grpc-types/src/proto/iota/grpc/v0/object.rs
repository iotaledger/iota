// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.object.rs");
include!("../../../generated/iota.grpc.v0.object.field_info.rs");
include!("../../../generated/iota.grpc.v0.object.accessors.rs");

use crate::{
    field::FieldMaskTree,
    merge::Merge,
    v0::{bcs::BcsData, types::ObjectReference},
};

// TODO: Wrap Object into a type with a version
impl Merge<&iota_sdk2::types::object::Object> for Object {
    fn merge(&mut self, source: &iota_sdk2::types::object::Object, mask: &FieldMaskTree) {
        if mask.contains("bcs") {
            if let Ok(bcs_bytes) = bcs::to_bytes(source) {
                self.bcs = Some(BcsData {
                    data: bcs_bytes.into(),
                });
            }
        }

        if mask.contains("reference") {
            let mut reference = ObjectReference::default();

            // Check for nested fields within reference
            if let Some(reference_mask) = mask.subtree("reference") {
                if reference_mask.contains("object_id") {
                    reference.object_id = Some(source.object_id().to_string());
                }

                if reference_mask.contains("version") {
                    reference.version = Some(source.version());
                }

                if reference_mask.contains("digest") {
                    reference.digest = Some(source.digest().into());
                }
            } else {
                // If no subtree, include all reference fields
                reference.object_id = Some(source.object_id().to_string());
                reference.version = Some(source.version());
                reference.digest = Some(source.digest().into());
            }

            self.reference = Some(reference);
        }
    }
}
