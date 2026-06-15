// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! JSON Schema and serialization adapter types for the IOTA JSON-RPC surface,
//! applied at field sites via `#[schemars(with = "...")]` and
//! `#[serde_as(as = "...")]`. Each adapter owns both the `schemars::JsonSchema`
//! layer and the JSON serialization for its type, so the JSON-RPC wire format
//! is defined in this crate rather than relying on the serde impls of the
//! external `iota-sdk-types` crate.
//!
//! To add a new adapter, prefer a unit marker struct with a manual `JsonSchema`
//! impl (for explicit control over description, format, and shape) plus
//! `SerializeAs` / `DeserializeAs` impls for the target type(s). String-like
//! types reuse `serde_with::DisplayFromStr` so the format matches the type's
//! `Display`/`FromStr`; byte payloads reuse the `fastcrypto` encoders. The Move
//! tag adapters reuse the shared, IOTA-specific formatting/parsing helpers from
//! `iota_types` (which many other crates depend on) rather than duplicating
//! that logic. Newtype wrappers (e.g. `SequenceNumberString(u64)`) are only
//! appropriate when the wrapper itself is the serialised value.

use fastcrypto::{
    encoding::{Base58 as FastCryptoBase58, Base64 as FastCryptoBase64},
    traits::EncodeDecodeBase64,
};
use iota_sdk_types::{
    Digest, Identifier as NativeIdentifier, ObjectId as NativeObjectId,
    StructTag as NativeStructTag, TypeTag as NativeTypeTag,
};
use iota_types::{
    base_types::{IotaAddress as NativeIotaAddress, SequenceNumber},
    iota_serde::{to_iota_struct_tag_string, to_iota_type_tag_string},
    parse_iota_struct_tag, parse_iota_type_tag,
    signature::GenericSignature as NativeGenericSignature,
};
use schemars::{
    JsonSchema,
    schema::{InstanceType, Metadata, NumberValidation, SchemaObject},
};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _, ser::Error as _};
use serde_with::{DeserializeAs, DisplayFromStr, SerializeAs, serde_as};

/// A schema type that defines the JSON representation of the
/// [`IotaAddress`](iota_types::base_types::IotaAddress) type.
pub struct IotaAddress;

impl JsonSchema for IotaAddress {
    fn schema_name() -> String {
        "IotaAddress".to_owned()
    }

    fn json_schema(_: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        SchemaObject {
            metadata: Some(Box::new(Metadata {
                description: Some("IOTA address as a hex string".to_owned()),
                ..Default::default()
            })),
            instance_type: Some(InstanceType::String.into()),
            format: Some("hex".to_owned()),
            ..Default::default()
        }
        .into()
    }
}

impl SerializeAs<NativeIotaAddress> for IotaAddress {
    fn serialize_as<S>(value: &NativeIotaAddress, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        DisplayFromStr::serialize_as(value, serializer)
    }
}

impl<'de> DeserializeAs<'de, NativeIotaAddress> for IotaAddress {
    fn deserialize_as<D>(deserializer: D) -> Result<NativeIotaAddress, D::Error>
    where
        D: Deserializer<'de>,
    {
        DisplayFromStr::deserialize_as(deserializer)
    }
}

/// A schema type that defines the JSON representation of the
/// [`ObjectId`](iota_sdk_types::ObjectId) type.
pub struct ObjectId;

impl JsonSchema for ObjectId {
    fn schema_name() -> String {
        "ObjectID".to_owned()
    }

    fn json_schema(_: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        SchemaObject {
            metadata: Some(Box::new(Metadata {
                description: Some("Object ID as a hex string".to_owned()),
                ..Default::default()
            })),
            instance_type: Some(InstanceType::String.into()),
            format: Some("hex".to_owned()),
            ..Default::default()
        }
        .into()
    }
}

impl SerializeAs<NativeObjectId> for ObjectId {
    fn serialize_as<S>(value: &NativeObjectId, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        DisplayFromStr::serialize_as(value, serializer)
    }
}

impl<'de> DeserializeAs<'de, NativeObjectId> for ObjectId {
    fn deserialize_as<D>(deserializer: D) -> Result<NativeObjectId, D::Error>
    where
        D: Deserializer<'de>,
    {
        DisplayFromStr::deserialize_as(deserializer)
    }
}

/// A schema type that defines the JSON representation of the
/// [`SequenceNumber`] type as a string
/// and provides an alternate serialization usable via `#[serde_as]`.
#[serde_as]
#[derive(Serialize, Deserialize)]
pub struct SequenceNumberString(#[serde_as(as = "DisplayFromStr")] u64);

impl JsonSchema for SequenceNumberString {
    fn schema_name() -> String {
        "SequenceNumberString".to_owned()
    }

    fn json_schema(_: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        SchemaObject {
            metadata: Some(Box::new(Metadata {
                description: Some("Sequence number as a string".to_owned()),
                ..Default::default()
            })),
            instance_type: Some(InstanceType::String.into()),
            ..Default::default()
        }
        .into()
    }
}

impl SerializeAs<iota_types::base_types::SequenceNumber> for SequenceNumberString {
    fn serialize_as<S>(
        source: &iota_types::base_types::SequenceNumber,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        SequenceNumberString(source.as_u64()).serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, iota_types::base_types::SequenceNumber> for SequenceNumberString {
    fn deserialize_as<D>(
        deserializer: D,
    ) -> Result<iota_types::base_types::SequenceNumber, D::Error>
    where
        D: Deserializer<'de>,
    {
        let schema = SequenceNumberString::deserialize(deserializer)?;
        Ok(iota_types::base_types::SequenceNumber::from_u64(schema.0))
    }
}

/// JSON representation of a [`SequenceNumber`] as a u64 integer.
///
/// This serializes to a number as opposed to the SDK type that serializes
/// as a string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SequenceNumberU64(SequenceNumber);

impl From<SequenceNumber> for SequenceNumberU64 {
    fn from(value: SequenceNumber) -> Self {
        Self(value)
    }
}

impl From<SequenceNumberU64> for SequenceNumber {
    fn from(value: SequenceNumberU64) -> Self {
        value.0
    }
}

impl std::fmt::Display for SequenceNumberU64 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Serialize for SequenceNumberU64 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.as_u64().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SequenceNumberU64 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(SequenceNumber::from_u64(u64::deserialize(
            deserializer,
        )?)))
    }
}

impl JsonSchema for SequenceNumberU64 {
    fn schema_name() -> String {
        "SequenceNumberU64".to_owned()
    }

    fn json_schema(_: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        SchemaObject {
            metadata: Some(Box::new(Metadata {
                description: Some("Sequence number as a u64 integer".to_owned()),
                ..Default::default()
            })),
            format: Some("uint64".to_owned()),
            number: Some(Box::new(NumberValidation {
                minimum: Some(0.0),
                ..Default::default()
            })),
            instance_type: Some(InstanceType::Integer.into()),
            ..Default::default()
        }
        .into()
    }
}

/// A schema type that defines the JSON representation of the
/// [`ProtocolVersion`](iota_protocol_config::ProtocolVersion) type as a string
/// and provides an alternate serialization usable via `#[serde_as]`.
#[serde_as]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ProtocolVersion(
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    u64,
);

impl SerializeAs<iota_protocol_config::ProtocolVersion> for ProtocolVersion {
    fn serialize_as<S>(
        source: &iota_protocol_config::ProtocolVersion,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ProtocolVersion(source.as_u64()).serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, iota_protocol_config::ProtocolVersion> for ProtocolVersion {
    fn deserialize_as<D>(deserializer: D) -> Result<iota_protocol_config::ProtocolVersion, D::Error>
    where
        D: Deserializer<'de>,
    {
        let schema = ProtocolVersion::deserialize(deserializer)?;
        Ok(iota_protocol_config::ProtocolVersion::new(schema.0))
    }
}

/// A schema type that defines the JSON representation of a Base58 encoded
/// string. A custom JsonSchema impl is necessary to add the "base58" format to
/// the schema.
pub struct Base58;

impl JsonSchema for Base58 {
    fn schema_name() -> String {
        "Base58".to_owned()
    }

    fn json_schema(_: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        SchemaObject {
            metadata: Some(Box::new(Metadata {
                description: Some("Base58 encoded data".to_owned()),
                ..Default::default()
            })),
            instance_type: Some(InstanceType::String.into()),
            format: Some("base58".to_owned()),
            ..Default::default()
        }
        .into()
    }
}

impl SerializeAs<Digest> for Base58 {
    fn serialize_as<S>(value: &Digest, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        DisplayFromStr::serialize_as(value, serializer)
    }
}

impl<'de> DeserializeAs<'de, Digest> for Base58 {
    fn deserialize_as<D>(deserializer: D) -> Result<Digest, D::Error>
    where
        D: Deserializer<'de>,
    {
        DisplayFromStr::deserialize_as(deserializer)
    }
}

impl SerializeAs<Vec<u8>> for Base58 {
    fn serialize_as<S>(value: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        FastCryptoBase58::serialize_as(value, serializer)
    }
}

impl<'de> DeserializeAs<'de, Vec<u8>> for Base58 {
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        FastCryptoBase58::deserialize_as(deserializer)
    }
}

/// A schema type that defines the JSON representation of a Base64 encoded
/// string. A custom JsonSchema impl is necessary to add the "base64" format to
/// the schema.
pub struct Base64;

impl JsonSchema for Base64 {
    fn schema_name() -> String {
        "Base64".to_owned()
    }

    fn json_schema(_: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        SchemaObject {
            metadata: Some(Box::new(Metadata {
                description: Some("Base64 encoded data".to_owned()),
                ..Default::default()
            })),
            instance_type: Some(InstanceType::String.into()),
            format: Some("base64".to_owned()),
            ..Default::default()
        }
        .into()
    }
}

impl SerializeAs<Vec<u8>> for Base64 {
    fn serialize_as<S>(value: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        FastCryptoBase64::serialize_as(value, serializer)
    }
}

impl<'de> DeserializeAs<'de, Vec<u8>> for Base64 {
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        FastCryptoBase64::deserialize_as(deserializer)
    }
}

/// A schema type that defines the JSON representation of a Base64 encoded
/// signature.
pub struct GenericSignature;

impl JsonSchema for GenericSignature {
    fn schema_name() -> String {
        "GenericSignature".to_owned()
    }

    fn json_schema(_: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        SchemaObject {
            metadata: Some(Box::new(Metadata {
                description: Some("Base64 encoded signature".to_owned()),
                ..Default::default()
            })),
            instance_type: Some(InstanceType::String.into()),
            format: Some("base64".to_owned()),
            ..Default::default()
        }
        .into()
    }
}

impl SerializeAs<NativeGenericSignature> for GenericSignature {
    fn serialize_as<S>(value: &NativeGenericSignature, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.encode_base64().serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, NativeGenericSignature> for GenericSignature {
    fn deserialize_as<D>(deserializer: D) -> Result<NativeGenericSignature, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        NativeGenericSignature::decode_base64(&s).map_err(D::Error::custom)
    }
}

/// A schema type that defines the JSON representation of a Move
/// [`StructTag`](iota_sdk_types::StructTag) as a string, and
/// provides a string serialization usable via `#[serde_as]`.
pub struct StructTag;

impl JsonSchema for StructTag {
    fn schema_name() -> String {
        "StructTag".to_owned()
    }

    fn json_schema(_: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        SchemaObject {
            metadata: Some(Box::new(Metadata {
                description: Some(
                    "Move struct tag, in the format 'address::module::name<type_params>'"
                        .to_owned(),
                ),
                ..Default::default()
            })),
            instance_type: Some(InstanceType::String.into()),
            ..Default::default()
        }
        .into()
    }
}

impl SerializeAs<NativeStructTag> for StructTag {
    fn serialize_as<S>(value: &NativeStructTag, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        to_iota_struct_tag_string(value)
            .map_err(S::Error::custom)?
            .serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, NativeStructTag> for StructTag {
    fn deserialize_as<D>(deserializer: D) -> Result<NativeStructTag, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        parse_iota_struct_tag(&s).map_err(D::Error::custom)
    }
}

/// A schema type that defines the JSON representation of a Move
/// [`TypeTag`](iota_sdk_types::TypeTag) as a string, and
/// provides a string serialization usable via `#[serde_as]`.
pub struct TypeTag;

impl JsonSchema for TypeTag {
    fn schema_name() -> String {
        "TypeTag".to_owned()
    }

    fn json_schema(_: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        SchemaObject {
            metadata: Some(Box::new(Metadata {
                description: Some("Move type tag as a string".to_owned()),
                ..Default::default()
            })),
            instance_type: Some(InstanceType::String.into()),
            ..Default::default()
        }
        .into()
    }
}

impl SerializeAs<NativeTypeTag> for TypeTag {
    fn serialize_as<S>(value: &NativeTypeTag, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        to_iota_type_tag_string(value)
            .map_err(S::Error::custom)?
            .serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, NativeTypeTag> for TypeTag {
    fn deserialize_as<D>(deserializer: D) -> Result<NativeTypeTag, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        parse_iota_type_tag(&s).map_err(D::Error::custom)
    }
}

/// A schema type that defines the JSON representation of a Move identifier,
/// and provides a string serialization usable via `#[serde_as]`.
pub struct Identifier;

impl JsonSchema for Identifier {
    fn schema_name() -> String {
        "Identifier".to_owned()
    }

    fn json_schema(_: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        SchemaObject {
            metadata: Some(Box::new(Metadata {
                description: Some("Move identifier".to_owned()),
                ..Default::default()
            })),
            instance_type: Some(InstanceType::String.into()),
            ..Default::default()
        }
        .into()
    }
}

impl SerializeAs<NativeIdentifier> for Identifier {
    fn serialize_as<S>(value: &NativeIdentifier, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        DisplayFromStr::serialize_as(value, serializer)
    }
}

impl<'de> DeserializeAs<'de, NativeIdentifier> for Identifier {
    fn deserialize_as<D>(deserializer: D) -> Result<NativeIdentifier, D::Error>
    where
        D: Deserializer<'de>,
    {
        DisplayFromStr::deserialize_as(deserializer)
    }
}
