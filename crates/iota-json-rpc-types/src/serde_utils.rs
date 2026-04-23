// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use fastcrypto::encoding::Base64;
use iota_types::{
    IOTA_CLOCK_ADDRESS, IOTA_FRAMEWORK_ADDRESS, IOTA_SYSTEM_ADDRESS, IOTA_SYSTEM_STATE_ADDRESS,
    MOVE_STDLIB_ADDRESS, STARDUST_ADDRESS,
    base_types::{IotaAddress as NativeIotaAddress, ObjectID as NativeObjectID},
    parse_iota_struct_tag, parse_iota_type_tag,
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

#[derive(JsonSchema)]
pub struct IotaAddress(
    #[expect(unused)]
    #[schemars(with = "String")]
    [u8; 32],
);

impl SerializeAs<NativeIotaAddress> for IotaAddress {
    fn serialize_as<S>(source: &NativeIotaAddress, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        source.serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, NativeIotaAddress> for IotaAddress {
    fn deserialize_as<D>(deserializer: D) -> Result<NativeIotaAddress, D::Error>
    where
        D: Deserializer<'de>,
    {
        NativeIotaAddress::deserialize(deserializer)
    }
}

#[derive(JsonSchema)]
pub struct ObjectID(
    #[expect(unused)]
    #[schemars(with = "String")]
    [u8; NativeObjectID::LENGTH],
);

impl SerializeAs<NativeObjectID> for ObjectID {
    fn serialize_as<S>(source: &NativeObjectID, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        source.serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, NativeObjectID> for ObjectID {
    fn deserialize_as<D>(deserializer: D) -> Result<NativeObjectID, D::Error>
    where
        D: Deserializer<'de>,
    {
        NativeObjectID::deserialize(deserializer)
    }
}

#[serde_as]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct SequenceNumberString(
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    u64,
);

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

#[derive(JsonSchema)]
pub struct SequenceNumberU64(#[expect(unused)] u64);

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

    fn is_referenceable() -> bool {
        false
    }
}

#[derive(JsonSchema)]
pub struct GenericSignature(#[expect(dead_code)] Base64);

pub struct StructTag;

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

pub struct TypeTag;

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
