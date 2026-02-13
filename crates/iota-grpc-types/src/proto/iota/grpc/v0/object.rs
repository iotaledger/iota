// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.object.rs");
include!("../../../generated/iota.grpc.v0.object.field_info.rs");

use crate::{
    proto::TryFromProtoError,
    v0::{bcs::BcsData, types::ObjectReference, versioned::VersionedObject},
};

// TryFrom implementations for Object
impl TryFrom<&Object> for iota_sdk_types::Object {
    type Error = TryFromProtoError;

    fn try_from(value: &Object) -> Result<Self, Self::Error> {
        let bcs = value
            .bcs
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Object::BCS_FIELD.name))?;

        bcs.deserialize::<VersionedObject>()
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

        let object_id_str = reference.object_id.as_ref().ok_or_else(|| {
            TryFromProtoError::missing(ObjectReference::OBJECT_ID_FIELD.name)
                .nested(Object::REFERENCE_FIELD.name)
        })?;

        let object_id = object_id_str.parse().map_err(|e| {
            TryFromProtoError::invalid(ObjectReference::OBJECT_ID_FIELD.name, e)
                .nested(Object::REFERENCE_FIELD.name)
        })?;

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
    /// Get the object reference.
    pub fn object_reference(&self) -> Result<iota_sdk_types::ObjectReference, TryFromProtoError> {
        self.try_into()
    }

    /// Deserialize the object from BCS.
    pub fn object(&self) -> Result<iota_sdk_types::Object, TryFromProtoError> {
        self.try_into()
    }

    /// Get the raw BCS bytes of this object.
    pub fn object_bcs(&self) -> Result<&[u8], TryFromProtoError> {
        self.bcs
            .as_ref()
            .map(BcsData::as_bytes)
            .ok_or_else(|| TryFromProtoError::missing(Self::BCS_FIELD.name))
    }

    /// Get the object ID from the reference.
    pub fn object_id(&self) -> Result<iota_sdk_types::ObjectId, TryFromProtoError> {
        let reference = self
            .reference
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::REFERENCE_FIELD.name))?;
        let object_id_str = reference.object_id.as_ref().ok_or_else(|| {
            TryFromProtoError::missing(ObjectReference::OBJECT_ID_FIELD.name)
                .nested(Self::REFERENCE_FIELD.name)
        })?;
        object_id_str.parse().map_err(|e| {
            TryFromProtoError::invalid(ObjectReference::OBJECT_ID_FIELD.name, e)
                .nested(Self::REFERENCE_FIELD.name)
        })
    }

    /// Get the object version from the reference.
    pub fn object_version(&self) -> Result<u64, TryFromProtoError> {
        let reference = self
            .reference
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::REFERENCE_FIELD.name))?;
        reference.version.ok_or_else(|| {
            TryFromProtoError::missing(ObjectReference::VERSION_FIELD.name)
                .nested(Self::REFERENCE_FIELD.name)
        })
    }

    /// Get the object digest from the reference.
    pub fn object_digest(&self) -> Result<iota_sdk_types::Digest, TryFromProtoError> {
        let reference = self
            .reference
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::REFERENCE_FIELD.name))?;
        let digest = reference.digest.as_ref().ok_or_else(|| {
            TryFromProtoError::missing(ObjectReference::DIGEST_FIELD.name)
                .nested(Self::REFERENCE_FIELD.name)
        })?;
        digest
            .try_into()
            .map_err(|e: TryFromProtoError| e.nested(Self::REFERENCE_FIELD.name))
    }
}

// Convenience methods for Objects (delegate to TryFrom)
impl Objects {
    /// Deserialize all objects from BCS.
    pub fn objects(&self) -> Result<Vec<iota_sdk_types::Object>, TryFromProtoError> {
        self.try_into()
    }

    /// Get all object references.
    pub fn object_references(
        &self,
    ) -> Result<Vec<iota_sdk_types::ObjectReference>, TryFromProtoError> {
        self.objects
            .iter()
            .enumerate()
            .map(|(i, o)| {
                o.object_reference()
                    .map_err(|e| e.nested_at(Self::OBJECTS_FIELD.name, i))
            })
            .collect()
    }
}
