// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.types.rs");
include!("../../../generated/iota.grpc.v0.types.field_info.rs");

impl From<iota_sdk_types::Digest> for Digest {
    fn from(value: iota_sdk_types::Digest) -> Self {
        Self {
            digest: value.into_inner().to_vec().into(),
        }
    }
}

impl From<iota_types::digests::TransactionDigest> for Digest {
    fn from(value: iota_types::digests::TransactionDigest) -> Self {
        Self {
            digest: value.into_inner().to_vec().into(),
        }
    }
}

impl TryFrom<&Digest> for iota_sdk_types::Digest {
    type Error = crate::proto::TryFromProtoError;

    fn try_from(value: &Digest) -> Result<Self, Self::Error> {
        iota_sdk_types::Digest::from_bytes(&value.digest)
            .map_err(|e| crate::proto::TryFromProtoError::invalid("digest", e))
    }
}

// Address conversions
impl From<iota_sdk_types::Address> for Address {
    fn from(value: iota_sdk_types::Address) -> Self {
        Self {
            address: value.as_bytes().to_vec().into(),
        }
    }
}

impl TryFrom<&Address> for iota_sdk_types::Address {
    type Error = crate::proto::TryFromProtoError;

    fn try_from(value: &Address) -> Result<Self, Self::Error> {
        iota_sdk_types::Address::from_bytes(&value.address)
            .map_err(|e| crate::proto::TryFromProtoError::invalid("address", e))
    }
}

// ObjectReference conversions
impl From<iota_sdk_types::ObjectReference> for ObjectReference {
    fn from(value: iota_sdk_types::ObjectReference) -> Self {
        Self {
            object_id: Some(value.object_id.to_string()),
            version: Some(value.version),
            digest: Some(value.digest.into()),
        }
    }
}

impl TryFrom<&ObjectReference> for iota_sdk_types::ObjectReference {
    type Error = crate::proto::TryFromProtoError;

    fn try_from(value: &ObjectReference) -> Result<Self, Self::Error> {
        let object_id_str = value.object_id.as_ref().ok_or_else(|| {
            crate::proto::TryFromProtoError::missing(ObjectReference::OBJECT_ID_FIELD.name)
        })?;

        let object_id = object_id_str.parse().map_err(|e| {
            crate::proto::TryFromProtoError::invalid(ObjectReference::OBJECT_ID_FIELD.name, e)
        })?;

        let version = value.version.ok_or_else(|| {
            crate::proto::TryFromProtoError::missing(ObjectReference::VERSION_FIELD.name)
        })?;

        let digest = value.digest.as_ref().ok_or_else(|| {
            crate::proto::TryFromProtoError::missing(ObjectReference::DIGEST_FIELD.name)
        })?;

        let digest = digest.try_into().map_err(|e| {
            crate::proto::TryFromProtoError::invalid(ObjectReference::DIGEST_FIELD.name, e)
        })?;

        Ok(iota_sdk_types::ObjectReference {
            object_id,
            version,
            digest,
        })
    }
}

impl Address {
    pub fn address(&self) -> Result<iota_sdk_types::Address, crate::proto::TryFromProtoError> {
        self.try_into()
    }
}

impl Digest {
    pub fn digest(&self) -> Result<iota_sdk_types::Digest, crate::proto::TryFromProtoError> {
        self.try_into()
    }
}

impl ObjectReference {
    pub fn object_reference(
        &self,
    ) -> Result<iota_sdk_types::ObjectReference, crate::proto::TryFromProtoError> {
        self.try_into()
    }
}
