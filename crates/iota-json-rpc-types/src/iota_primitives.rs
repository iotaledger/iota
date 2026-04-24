// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::{
    IOTA_CLOCK_ADDRESS, IOTA_FRAMEWORK_ADDRESS, IOTA_SYSTEM_ADDRESS, IOTA_SYSTEM_STATE_ADDRESS,
    MOVE_STDLIB_ADDRESS, STARDUST_ADDRESS, parse_iota_struct_tag, parse_iota_type_tag,
};
use move_core_types::{
    account_address::AccountAddress,
    language_storage::{StructTag as NativeStructTag, TypeTag as NativeTypeTag},
};
use schemars::{
    JsonSchema,
    schema::{InstanceType, Metadata, SchemaObject},
};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer, de::Error as DeError, ser::Error as SerError,
};
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

/// A schema type that defines the JSON representation of the
/// [`ObjectID`](iota_types::base_types::ObjectID) type.
pub struct ObjectID;

impl JsonSchema for ObjectID {
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

/// A schema type that defines the JSON representation of the
/// [`SequenceNumber`](iota_types::base_types::SequenceNumber) type as a string
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
        SequenceNumberString(source.value()).serialize(serializer)
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

/// A schema type that defines the JSON representation of the
/// [`SequenceNumber`](iota_types::base_types::SequenceNumber) type as a u64
/// integer and uses the default serialization.
pub struct SequenceNumberU64;

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

/// A schema type that defines the JSON representation of a Move
/// [`StructTag`](move_core_types::language_storage::StructTag) as a string, and
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

const IOTA_ADDRESSES: [AccountAddress; 7] = [
    AccountAddress::ZERO,
    MOVE_STDLIB_ADDRESS,
    IOTA_FRAMEWORK_ADDRESS,
    IOTA_SYSTEM_ADDRESS,
    STARDUST_ADDRESS,
    IOTA_SYSTEM_STATE_ADDRESS,
    IOTA_CLOCK_ADDRESS,
];

/// Serialize StructTag as a string, retaining the leading zeros in the address.
fn to_iota_struct_tag_string(value: &NativeStructTag) -> Result<String, std::fmt::Error> {
    use std::fmt::Write;
    let mut f = String::new();
    let address = value.address;
    // trim leading zeros if address is in IOTA_ADDRESSES
    let address_str = if IOTA_ADDRESSES.contains(&address) {
        format!("0x{}", address.short_str_lossless())
    } else {
        address.to_canonical_string(/* with_prefix */ true)
    };

    write!(f, "{}::{}::{}", address_str, value.module, value.name)?;
    if let Some(first_ty) = value.type_params.first() {
        write!(f, "<")?;
        write!(f, "{}", to_iota_type_tag_string(first_ty)?)?;
        for ty in value.type_params.iter().skip(1) {
            write!(f, ", {}", to_iota_type_tag_string(ty)?)?;
        }
        write!(f, ">")?;
    }
    Ok(f)
}

impl SerializeAs<NativeStructTag> for StructTag {
    fn serialize_as<S>(value: &NativeStructTag, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let f = to_iota_struct_tag_string(value).map_err(S::Error::custom)?;
        f.serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, NativeStructTag> for StructTag {
    fn deserialize_as<D>(deserializer: D) -> Result<NativeStructTag, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)
            .and_then(|s| parse_iota_struct_tag(&s).map_err(D::Error::custom))
    }
}

/// A schema type that defines the JSON representation of a Move
/// [`TypeTag`](move_core_types::language_storage::TypeTag) as a string, and
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

fn to_iota_type_tag_string(value: &NativeTypeTag) -> Result<String, std::fmt::Error> {
    match value {
        NativeTypeTag::Vector(t) => Ok(format!("vector<{}>", to_iota_type_tag_string(t)?)),
        NativeTypeTag::Struct(s) => to_iota_struct_tag_string(s),
        _ => Ok(value.to_string()),
    }
}

impl SerializeAs<NativeTypeTag> for TypeTag {
    fn serialize_as<S>(value: &NativeTypeTag, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = to_iota_type_tag_string(value).map_err(S::Error::custom)?;
        s.serialize(serializer)
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
