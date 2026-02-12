// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.types.rs");
include!("../../../generated/iota.grpc.v0.types.field_info.rs");
include!("../../../generated/iota.grpc.v0.types.accessors.rs");

use crate::proto::{TryFromProtoError, get_inner_field};

impl From<iota_sdk_types::Digest> for Digest {
    fn from(value: iota_sdk_types::Digest) -> Self {
        Self {
            digest: value.into_inner().to_vec().into(),
        }
    }
}

impl TryFrom<&Digest> for iota_sdk_types::Digest {
    type Error = TryFromProtoError;

    fn try_from(value: &Digest) -> Result<Self, Self::Error> {
        iota_sdk_types::Digest::from_bytes(&value.digest)
            .map_err(|e| TryFromProtoError::invalid("digest", e))
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
    type Error = TryFromProtoError;

    fn try_from(value: &Address) -> Result<Self, Self::Error> {
        iota_sdk_types::Address::from_bytes(&value.address)
            .map_err(|e| TryFromProtoError::invalid("address", e))
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
    type Error = TryFromProtoError;

    fn try_from(value: &ObjectReference) -> Result<Self, Self::Error> {
        let object_id_str = value
            .object_id
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(ObjectReference::OBJECT_ID_FIELD.name))?;

        let object_id = object_id_str
            .parse()
            .map_err(|e| TryFromProtoError::invalid(ObjectReference::OBJECT_ID_FIELD.name, e))?;

        let version = value
            .version
            .ok_or_else(|| TryFromProtoError::missing(ObjectReference::VERSION_FIELD.name))?;

        let digest = value
            .digest
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(ObjectReference::DIGEST_FIELD.name))?;

        let digest = digest
            .try_into()
            .map_err(|e| TryFromProtoError::invalid(ObjectReference::DIGEST_FIELD.name, e))?;

        Ok(iota_sdk_types::ObjectReference {
            object_id,
            version,
            digest,
        })
    }
}

impl Address {
    /// Deserialize the address to SDK type.
    pub fn address(&self) -> Result<iota_sdk_types::Address, TryFromProtoError> {
        self.try_into()
    }

    /// Get the raw address bytes.
    pub fn address_bytes(&self) -> &[u8] {
        &self.address
    }
}

impl Digest {
    /// Deserialize the digest to SDK type.
    pub fn digest(&self) -> Result<iota_sdk_types::Digest, TryFromProtoError> {
        self.try_into()
    }

    /// Get the raw digest bytes.
    pub fn digest_bytes(&self) -> &[u8] {
        &self.digest
    }
}

impl ObjectReference {
    /// Deserialize the full object reference to SDK type.
    pub fn object_reference(&self) -> Result<iota_sdk_types::ObjectReference, TryFromProtoError> {
        self.try_into()
    }

    /// Get the object ID as a string reference.
    pub fn object_id_str(&self) -> Result<&str, TryFromProtoError> {
        self.object_id
            .as_deref()
            .ok_or_else(|| TryFromProtoError::missing(Self::OBJECT_ID_FIELD.name))
    }

    /// Get the object ID parsed as SDK type.
    pub fn parsed_object_id(&self) -> Result<iota_sdk_types::ObjectId, TryFromProtoError> {
        self.object_id
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::OBJECT_ID_FIELD.name))?
            .parse()
            .map_err(|e| TryFromProtoError::invalid(Self::OBJECT_ID_FIELD.name, e))
    }

    /// Get the object version number.
    pub fn object_version(&self) -> Result<u64, TryFromProtoError> {
        self.version
            .ok_or_else(|| TryFromProtoError::missing(Self::VERSION_FIELD.name))
    }

    /// Get the object digest.
    pub fn object_digest(&self) -> Result<iota_sdk_types::Digest, TryFromProtoError> {
        get_inner_field!(self.digest, Self::DIGEST_FIELD, digest)
    }
}

impl From<&iota_sdk_types::TypeTag> for TypeTag {
    fn from(ty: &iota_sdk_types::TypeTag) -> Self {
        let type_tag = match ty {
            iota_sdk_types::TypeTag::Bool => type_tag::TypeTag::BoolTag(true),
            iota_sdk_types::TypeTag::U8 => type_tag::TypeTag::U8Tag(true),
            iota_sdk_types::TypeTag::U16 => type_tag::TypeTag::U16Tag(true),
            iota_sdk_types::TypeTag::U32 => type_tag::TypeTag::U32Tag(true),
            iota_sdk_types::TypeTag::U64 => type_tag::TypeTag::U64Tag(true),
            iota_sdk_types::TypeTag::U128 => type_tag::TypeTag::U128Tag(true),
            iota_sdk_types::TypeTag::U256 => type_tag::TypeTag::U256Tag(true),
            iota_sdk_types::TypeTag::Address => type_tag::TypeTag::AddressTag(true),
            iota_sdk_types::TypeTag::Signer => type_tag::TypeTag::SignerTag(true),
            iota_sdk_types::TypeTag::Vector(inner) => {
                type_tag::TypeTag::VectorTag(Box::new(TypeTagVector {
                    inner_type: Some(Box::new(inner.as_ref().into())),
                }))
            }
            iota_sdk_types::TypeTag::Struct(struct_tag) => {
                type_tag::TypeTag::StructTag(TypeTagStruct {
                    struct_tag: struct_tag.to_canonical_string(true),
                })
            }
        };

        Self {
            type_tag: Some(type_tag),
        }
    }
}

impl TryFrom<&TypeTag> for iota_sdk_types::TypeTag {
    type Error = TryFromProtoError;

    fn try_from(value: &TypeTag) -> Result<Self, Self::Error> {
        match &value.type_tag {
            Some(type_tag::TypeTag::BoolTag(_)) => Ok(iota_sdk_types::TypeTag::Bool),
            Some(type_tag::TypeTag::U8Tag(_)) => Ok(iota_sdk_types::TypeTag::U8),
            Some(type_tag::TypeTag::U16Tag(_)) => Ok(iota_sdk_types::TypeTag::U16),
            Some(type_tag::TypeTag::U32Tag(_)) => Ok(iota_sdk_types::TypeTag::U32),
            Some(type_tag::TypeTag::U64Tag(_)) => Ok(iota_sdk_types::TypeTag::U64),
            Some(type_tag::TypeTag::U128Tag(_)) => Ok(iota_sdk_types::TypeTag::U128),
            Some(type_tag::TypeTag::U256Tag(_)) => Ok(iota_sdk_types::TypeTag::U256),
            Some(type_tag::TypeTag::AddressTag(_)) => Ok(iota_sdk_types::TypeTag::Address),
            Some(type_tag::TypeTag::SignerTag(_)) => Ok(iota_sdk_types::TypeTag::Signer),
            Some(type_tag::TypeTag::VectorTag(inner)) => {
                let inner_type = inner
                    .inner_type
                    .as_ref()
                    .ok_or_else(|| TryFromProtoError::missing("type_tag.vector.inner_type"))?;
                let inner_sdk: iota_sdk_types::TypeTag = inner_type.as_ref().try_into()?;
                Ok(iota_sdk_types::TypeTag::Vector(Box::new(inner_sdk)))
            }
            Some(type_tag::TypeTag::StructTag(struct_tag)) => {
                let parsed: iota_sdk_types::StructTag = struct_tag
                    .struct_tag
                    .parse()
                    .map_err(|e| TryFromProtoError::invalid("type_tag.struct_tag", e))?;
                Ok(iota_sdk_types::TypeTag::Struct(Box::new(parsed)))
            }
            None => Err(TryFromProtoError::missing("type_tag")),
        }
    }
}

// TypeTag
//

impl TypeTag {
    /// Deserialize the type tag to SDK type.
    pub fn type_tag(&self) -> Result<iota_sdk_types::TypeTag, TryFromProtoError> {
        self.try_into()
    }

    /// Get the inner type tag if this is a vector type tag.
    pub fn vector_inner_type(&self) -> Result<Option<iota_sdk_types::TypeTag>, TryFromProtoError> {
        match &self.type_tag {
            Some(type_tag::TypeTag::VectorTag(inner)) => {
                let inner_type = inner
                    .inner_type
                    .as_ref()
                    .ok_or_else(|| TryFromProtoError::missing("type_tag.vector.inner_type"))?;
                Ok(Some(inner_type.type_tag()?))
            }
            _ => Ok(None),
        }
    }

    /// Get the struct tag if this is a struct type tag.
    pub fn struct_tag(&self) -> Result<Option<iota_sdk_types::StructTag>, TryFromProtoError> {
        match &self.type_tag {
            Some(type_tag::TypeTag::StructTag(s)) => {
                let parsed = s
                    .struct_tag
                    .parse()
                    .map_err(|e| TryFromProtoError::invalid("type_tag.struct_tag", e))?;
                Ok(Some(parsed))
            }
            _ => Ok(None),
        }
    }

    /// Get the struct tag as a string if this is a struct type tag.
    pub fn struct_tag_str(&self) -> Option<&str> {
        match &self.type_tag {
            Some(type_tag::TypeTag::StructTag(s)) => Some(&s.struct_tag),
            _ => None,
        }
    }
}

// TypeTags
//

impl TypeTags {
    /// Deserialize all type tags to SDK types.
    pub fn type_tags(&self) -> Result<Vec<iota_sdk_types::TypeTag>, TryFromProtoError> {
        self.type_tags
            .iter()
            .enumerate()
            .map(|(i, tt)| {
                tt.type_tag()
                    .map_err(|e| e.nested_at(Self::TYPE_TAGS_FIELD.name, i))
            })
            .collect()
    }
}

// TypeTagVector
//

impl TypeTagVector {
    /// Deserialize the inner type to SDK type.
    pub fn inner_type_tag(&self) -> Result<iota_sdk_types::TypeTag, TryFromProtoError> {
        let inner = self
            .inner_type
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing("type_tag_vector.inner_type"))?;
        inner.type_tag()
    }
}

// TypeTagStruct
//

impl TypeTagStruct {
    /// Get the struct tag as a string.
    pub fn struct_tag(&self) -> &str {
        &self.struct_tag
    }

    /// Parse and deserialize the struct tag to SDK type.
    pub fn struct_tag_parsed(&self) -> Result<iota_sdk_types::StructTag, TryFromProtoError> {
        self.struct_tag
            .parse()
            .map_err(|e| TryFromProtoError::invalid("struct_tag", e))
    }
}
