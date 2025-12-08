// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.object.rs");
include!("../../../generated/iota.grpc.v0.object.field_info.rs");
include!("../../../generated/iota.grpc.v0.object.accessors.rs");

use iota_types::iota_sdk_types_conversions::SdkTypeConversionError;

use crate::{
    field::FieldMaskTree,
    merge::Merge,
    v0::{bcs::BcsData, types::ObjectReference},
};

impl Merge<iota_types::object::Object> for Object {
    fn merge(
        &mut self,
        source: iota_types::object::Object,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: wrap Object into a type with a version
        let sdk_object = source
            .try_into()
            .map_err(|e: SdkTypeConversionError| format!("Failed to convert SDK object: {}", e))?;

        Merge::merge(self, &sdk_object, mask)
    }
}

// TODO: wrap Object into a type with a version
impl Merge<&iota_sdk_types::object::Object> for Object {
    fn merge(
        &mut self,
        source: &iota_sdk_types::object::Object,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if mask.contains(Self::BCS_FIELD.name) {
            if let Ok(bcs_bytes) = bcs::to_bytes(source) {
                self.bcs = Some(BcsData {
                    data: bcs_bytes.into(),
                });
            }
        }

        if mask.contains(Self::REFERENCE_FIELD.name) {
            let mut reference = ObjectReference::default();

            // Check for nested fields within reference
            if let Some(reference_mask) = mask.subtree(Self::REFERENCE_FIELD.name) {
                if reference_mask.contains(ObjectReference::OBJECT_ID_FIELD.name) {
                    reference.object_id = Some(source.object_id().to_string());
                }

                if reference_mask.contains(ObjectReference::VERSION_FIELD.name) {
                    reference.version = Some(source.version());
                }

                if reference_mask.contains(ObjectReference::DIGEST_FIELD.name) {
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

        Ok(())
    }
}

impl Merge<&[iota_sdk_types::object::Object]> for Objects {
    fn merge(
        &mut self,
        source: &[iota_sdk_types::object::Object],
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Objects is a wrapper message containing a repeated field `objects`.
        // When a user requests the wrapper (e.g., "input_objects"), the mask becomes
        // a wildcard since it's a leaf node. Calling subtree("objects") on a wildcard
        // returns Some(wildcard), which populates the objects array.
        // When a user requests specific fields (e.g., "input_objects.objects.bcs"),
        // subtree("objects") returns the sub-mask with the requested fields.
        if let Some(objects_mask) = mask.subtree(Self::OBJECTS_FIELD.name) {
            // Merge each object in the source list with the appropriate field mask
            self.objects = source
                .iter()
                .map(|obj| -> Result<_, Box<dyn std::error::Error>> {
                    let mut proto_obj = Object::default();
                    proto_obj.merge(obj, &objects_mask)?;
                    Ok(proto_obj)
                })
                .collect::<Result<Vec<_>, _>>()?;
        }

        Ok(())
    }
}
