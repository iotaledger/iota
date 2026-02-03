// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.object.rs");
include!("../../../generated/iota.grpc.v0.object.field_info.rs");

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
        if !mask.contains(Self::REFERENCE_FIELD.name) && !mask.contains(Self::BCS_FIELD.name) {
            // No need to convert if no field is requested
            return Ok(());
        }

        // TODO: wrap Object into a type with a version
        let sdk_object: iota_sdk_types::object::Object = source
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
            self.bcs = BcsData::serialize(source).ok();
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

impl Merge<&Object> for Object {
    fn merge(
        &mut self,
        source: &Object,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if mask.contains(Self::REFERENCE_FIELD.name) {
            self.reference = source.reference.clone();
        }

        if mask.contains(Self::BCS_FIELD.name) {
            self.bcs = source.bcs.clone();
        }

        Ok(())
    }
}

impl Merge<Option<Vec<iota_types::object::Object>>> for Objects {
    fn merge(
        &mut self,
        source: Option<Vec<iota_types::object::Object>>,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Objects is a wrapper message containing a repeated field `objects`.
        // When a user requests the wrapper (e.g., "input_objects"), the mask becomes
        // a wildcard since it's a leaf node. Calling subtree("objects") on a wildcard
        // returns Some(wildcard), which populates the objects array.
        // When a user requests specific fields (e.g., "input_objects.objects.bcs"),
        // subtree("objects") returns the sub-mask with the requested fields.
        if let Some(objects_mask) = mask.subtree(Self::OBJECTS_FIELD.name) {
            if let Some(objects) = source {
                // Merge each object in the source list with the appropriate field mask
                self.objects = objects
                    .into_iter()
                    .map(|obj| Object::merge_from(obj, &objects_mask))
                    .collect::<Result<Vec<_>, _>>()?;
            }
        }

        Ok(())
    }
}

impl Merge<&Objects> for Objects {
    fn merge(
        &mut self,
        source: &Objects,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(objects_mask) = mask.subtree(Self::OBJECTS_FIELD.name) {
            self.objects = source
                .objects
                .iter()
                .map(|obj| Object::merge_from(obj, &objects_mask))
                .collect::<Result<Vec<_>, _>>()?;
        }

        Ok(())
    }
}

// TryFrom implementations for Object
impl TryFrom<&Object> for iota_sdk_types::Object {
    type Error = crate::proto::TryFromProtoError;

    fn try_from(value: &Object) -> Result<Self, Self::Error> {
        let bcs = value
            .bcs
            .as_ref()
            .ok_or_else(|| crate::proto::TryFromProtoError::missing(Object::BCS_FIELD.name))?;

        bcs.deserialize()
            .map_err(|e| crate::proto::TryFromProtoError::invalid(Object::BCS_FIELD.name, e))
    }
}

impl TryFrom<&Object> for iota_sdk_types::ObjectReference {
    type Error = crate::proto::TryFromProtoError;

    fn try_from(value: &Object) -> Result<Self, Self::Error> {
        let reference = value.reference.as_ref().ok_or_else(|| {
            crate::proto::TryFromProtoError::missing(Object::REFERENCE_FIELD.name)
        })?;

        let object_id_str = reference.object_id.as_ref().ok_or_else(|| {
            crate::proto::TryFromProtoError::missing(ObjectReference::OBJECT_ID_FIELD.name)
                .nested(Object::REFERENCE_FIELD.name)
        })?;

        let object_id = object_id_str.parse().map_err(|e| {
            crate::proto::TryFromProtoError::invalid(ObjectReference::OBJECT_ID_FIELD.name, e)
                .nested(Object::REFERENCE_FIELD.name)
        })?;

        let version = reference.version.ok_or_else(|| {
            crate::proto::TryFromProtoError::missing(ObjectReference::VERSION_FIELD.name)
                .nested(Object::REFERENCE_FIELD.name)
        })?;

        let digest = reference.digest.as_ref().ok_or_else(|| {
            crate::proto::TryFromProtoError::missing(ObjectReference::DIGEST_FIELD.name)
                .nested(Object::REFERENCE_FIELD.name)
        })?;

        let digest = digest.try_into().map_err(|e| {
            crate::proto::TryFromProtoError::invalid(ObjectReference::DIGEST_FIELD.name, e)
                .nested(Object::REFERENCE_FIELD.name)
        })?;

        Ok(iota_sdk_types::ObjectReference {
            object_id,
            version,
            digest,
        })
    }
}

impl TryFrom<&Objects> for Vec<iota_sdk_types::Object> {
    type Error = crate::proto::TryFromProtoError;

    fn try_from(value: &Objects) -> Result<Self, Self::Error> {
        value
            .objects
            .iter()
            .enumerate()
            .map(|(i, obj)| {
                <&Object as TryInto<iota_sdk_types::Object>>::try_into(obj).map_err(
                    |e: crate::proto::TryFromProtoError| {
                        e.nested_at(Objects::OBJECTS_FIELD.name, i)
                    },
                )
            })
            .collect()
    }
}

// Convenience methods for Object (delegate to TryFrom)
impl Object {
    pub fn object_reference(
        &self,
    ) -> Result<iota_sdk_types::ObjectReference, crate::proto::TryFromProtoError> {
        self.try_into()
    }

    /// Deserialize the object from BCS.
    pub fn object(&self) -> Result<iota_sdk_types::Object, crate::proto::TryFromProtoError> {
        self.try_into()
    }
}

// Convenience methods for Objects (delegate to TryFrom)
impl Objects {
    /// Deserialize all objects from BCS.
    pub fn objects(&self) -> Result<Vec<iota_sdk_types::Object>, crate::proto::TryFromProtoError> {
        self.try_into()
    }
}
