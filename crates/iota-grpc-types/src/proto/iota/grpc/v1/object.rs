// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v1.object.rs");
include!("../../../generated/iota.grpc.v1.object.field_info.rs");

use crate::{proto::TryFromProtoError, v1::types::ObjectReference};

// TryFrom implementations for Object
impl TryFrom<&Object> for iota_sdk_types::Object {
    type Error = TryFromProtoError;

    fn try_from(value: &Object) -> Result<Self, Self::Error> {
        let bcs = value
            .bcs
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Object::BCS_FIELD.name))?;

        bcs.deserialize::<crate::v1::versioned::VersionedObject>()
            .map_err(|e| TryFromProtoError::invalid(Object::BCS_FIELD.name, e))?
            .try_into_v1()
            .map_err(|_| {
                TryFromProtoError::invalid(Object::BCS_FIELD.name, "unsupported Object version")
            })
    }
}

impl TryFrom<&Object> for iota_sdk_types::ObjectReference {
    type Error = TryFromProtoError;

    fn try_from(value: &Object) -> Result<Self, Self::Error> {
        let reference = value
            .reference
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Object::REFERENCE_FIELD.name))?;

        let object_id = reference
            .object_id
            .as_ref()
            .ok_or_else(|| {
                TryFromProtoError::missing(ObjectReference::OBJECT_ID_FIELD.name)
                    .nested(Object::REFERENCE_FIELD.name)
            })?
            .object_id()
            .map_err(|e| e.nested(Object::REFERENCE_FIELD.name))?;

        let version = reference.version.ok_or_else(|| {
            TryFromProtoError::missing(ObjectReference::VERSION_FIELD.name)
                .nested(Object::REFERENCE_FIELD.name)
        })?;

        let digest = reference.digest.as_ref().ok_or_else(|| {
            TryFromProtoError::missing(ObjectReference::DIGEST_FIELD.name)
                .nested(Object::REFERENCE_FIELD.name)
        })?;

        let digest = digest.try_into().map_err(|e| {
            TryFromProtoError::invalid(ObjectReference::DIGEST_FIELD.name, e)
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
    type Error = TryFromProtoError;

    fn try_from(value: &Objects) -> Result<Self, Self::Error> {
        value
            .objects
            .iter()
            .enumerate()
            .map(|(i, obj)| {
                <&Object as TryInto<iota_sdk_types::Object>>::try_into(obj)
                    .map_err(|e: TryFromProtoError| e.nested_at(Objects::OBJECTS_FIELD.name, i))
            })
            .collect()
    }
}

// Convenience methods for Object (delegate to TryFrom)
impl Object {
    /// Get the object reference (object_id, version, digest).
    ///
    /// **Read mask:** `"reference"` (see [`OBJECT_REFERENCE`]).
    /// For objects nested in transactions, prefix accordingly
    /// (e.g. `"input_objects.reference"`, `"output_objects.reference"`).
    ///
    /// [`OBJECT_REFERENCE`]: crate::read_masks::OBJECT_REFERENCE
    pub fn object_reference(&self) -> Result<iota_sdk_types::ObjectReference, TryFromProtoError> {
        self.try_into()
    }

    /// Deserialize the full object from BCS.
    ///
    /// **Read mask:** `"bcs"` (see [`OBJECT_BCS`]).
    /// For objects nested in transactions, prefix accordingly
    /// (e.g. `"input_objects.bcs"`, `"output_objects.bcs"`).
    ///
    /// [`OBJECT_BCS`]: crate::read_masks::OBJECT_BCS
    pub fn object(&self) -> Result<iota_sdk_types::Object, TryFromProtoError> {
        self.try_into()
    }
}
