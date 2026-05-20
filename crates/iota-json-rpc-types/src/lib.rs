// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub use balance_changes::*;
use fastcrypto::{
    encoding::{Base58, Base64},
    traits::VerifyingKey,
};
pub use iota_checkpoint::*;
pub use iota_coin::*;
pub use iota_event::*;
pub use iota_extended::*;
pub use iota_gas_cost_summary::*;
pub use iota_governance::*;
pub use iota_indexer::*;
pub use iota_move::*;
pub use iota_object::*;
pub use iota_object_response_error::*;
pub use iota_owner::*;
use iota_primitives::{
    Base58 as Base58Schema, Base64 as Base64Schema, ObjectID as ObjectIDSchema,
    SequenceNumberU64 as SequenceNumberU64Schema, TypeTag as TypeTagSchema,
};
pub use iota_protocol::*;
pub use iota_system_state_summary::*;
pub use iota_transaction::*;
use iota_types::{
    base_types::{ObjectID, TypeTag},
    crypto::{AuthorityPublicKey, AuthorityPublicKeyBytes},
    dynamic_field::{DynamicFieldInfo, DynamicFieldName, DynamicFieldType},
};
pub use object_changes::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeAs, SerializeAs, serde_as};

#[cfg(test)]
#[path = "unit_tests/rpc_types_tests.rs"]
mod rpc_types_tests;

mod balance_changes;
mod displays;
mod iota_checkpoint;
mod iota_coin;
mod iota_event;
mod iota_extended;
mod iota_gas_cost_summary;
mod iota_governance;
mod iota_indexer;
mod iota_move;
mod iota_object;
mod iota_object_response_error;
mod iota_owner;
pub mod iota_primitives;
mod iota_protocol;
mod iota_system_state_summary;
mod iota_transaction;
mod object_changes;

pub type DynamicFieldPage = Page<IotaDynamicFieldInfo, ObjectID>;

/// `next_cursor` points to the last item in the page;
/// Reading with `next_cursor` will start from the next item after `next_cursor`
/// if `next_cursor` is `Some`, otherwise it will start from the first item.
#[derive(Clone, Debug, JsonSchema, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Page<T, C> {
    pub data: Vec<T>,
    pub next_cursor: Option<C>,
    pub has_next_page: bool,
}

impl<T, C> Page<T, C> {
    pub fn empty() -> Self {
        Self {
            data: vec![],
            next_cursor: None,
            has_next_page: false,
        }
    }
}

#[serde_as]
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", rename = "DynamicFieldName")]
pub struct DynamicFieldNameSchema {
    #[schemars(with = "TypeTagSchema")]
    #[serde_as(as = "TypeTagSchema")]
    pub type_: TypeTag,
    // Bincode does not like serde_json::Value, rocksdb will not insert the value without
    // serializing value as string. TODO: investigate if this can be removed after switch to
    // BCS.
    pub value: serde_json::Value,
}

impl SerializeAs<DynamicFieldName> for DynamicFieldNameSchema {
    fn serialize_as<S>(name: &DynamicFieldName, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let schema = DynamicFieldNameSchema::from(name.clone());
        schema.serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, DynamicFieldName> for DynamicFieldNameSchema {
    fn deserialize_as<D>(deserializer: D) -> Result<DynamicFieldName, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let schema = DynamicFieldNameSchema::deserialize(deserializer)?;
        Ok(DynamicFieldName::from(schema))
    }
}

impl From<DynamicFieldName> for DynamicFieldNameSchema {
    fn from(name: DynamicFieldName) -> Self {
        Self {
            type_: name.type_,
            value: name.value,
        }
    }
}

impl From<DynamicFieldNameSchema> for DynamicFieldName {
    fn from(name: DynamicFieldNameSchema) -> Self {
        Self {
            type_: name.type_,
            value: name.value,
        }
    }
}

#[derive(Copy, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename = "DynamicFieldType")]
pub enum DynamicFieldTypeSchema {
    DynamicField,
    DynamicObject,
}

impl SerializeAs<DynamicFieldType> for DynamicFieldTypeSchema {
    fn serialize_as<S>(type_: &DynamicFieldType, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let schema = DynamicFieldTypeSchema::from(*type_);
        schema.serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, DynamicFieldType> for DynamicFieldTypeSchema {
    fn deserialize_as<D>(deserializer: D) -> Result<DynamicFieldType, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let schema = DynamicFieldTypeSchema::deserialize(deserializer)?;
        Ok(DynamicFieldType::from(schema))
    }
}

impl From<DynamicFieldType> for DynamicFieldTypeSchema {
    fn from(type_: DynamicFieldType) -> Self {
        match type_ {
            DynamicFieldType::DynamicField => Self::DynamicField,
            DynamicFieldType::DynamicObject => Self::DynamicObject,
        }
    }
}

impl From<DynamicFieldTypeSchema> for DynamicFieldType {
    fn from(type_: DynamicFieldTypeSchema) -> Self {
        match type_ {
            DynamicFieldTypeSchema::DynamicField => Self::DynamicField,
            DynamicFieldTypeSchema::DynamicObject => Self::DynamicObject,
        }
    }
}

#[serde_as]
#[derive(Clone, Serialize, Deserialize, JsonSchema, Debug)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "DynamicFieldInfo")]
pub struct IotaDynamicFieldInfo {
    #[schemars(with = "DynamicFieldNameSchema")]
    #[serde_as(as = "DynamicFieldNameSchema")]
    pub name: DynamicFieldName,
    #[serde(flatten)]
    pub bcs_name: BcsName,
    #[schemars(with = "DynamicFieldTypeSchema")]
    #[serde_as(as = "DynamicFieldTypeSchema")]
    pub type_: DynamicFieldType,
    pub object_type: String,
    #[schemars(with = "ObjectIDSchema")]
    pub object_id: ObjectID,
    #[schemars(with = "SequenceNumberU64Schema")]
    pub version: iota_types::base_types::SequenceNumber,
    #[schemars(with = "Base58Schema")]
    pub digest: iota_types::digests::ObjectDigest,
}

impl From<DynamicFieldInfo> for IotaDynamicFieldInfo {
    fn from(
        DynamicFieldInfo {
            name,
            bcs_name,
            type_,
            object_type,
            object_id,
            version,
            digest,
        }: DynamicFieldInfo,
    ) -> Self {
        Self {
            name,
            bcs_name: BcsName::new(bcs_name),
            type_,
            object_type,
            object_id,
            version,
            digest,
        }
    }
}

impl From<IotaDynamicFieldInfo> for DynamicFieldInfo {
    fn from(
        IotaDynamicFieldInfo {
            name,
            bcs_name,
            type_,
            object_type,
            object_id,
            version,
            digest,
        }: IotaDynamicFieldInfo,
    ) -> Self {
        Self {
            name,
            bcs_name: bcs_name.into_bytes(),
            type_,
            object_type,
            object_id,
            version,
            digest,
        }
    }
}

#[serde_as]
#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", tag = "bcsEncoding")]
#[serde(from = "MaybeTaggedBcsName")]
pub enum BcsName {
    Base64 {
        #[serde_as(as = "Base64")]
        #[schemars(with = "Base64Schema")]
        #[serde(rename = "bcsName")]
        bcs_name: Vec<u8>,
    },
    Base58 {
        #[serde_as(as = "Base58")]
        #[schemars(with = "Base58Schema")]
        #[serde(rename = "bcsName")]
        bcs_name: Vec<u8>,
    },
}

impl BcsName {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self::Base64 { bcs_name: bytes }
    }

    pub fn bytes(&self) -> &[u8] {
        match self {
            BcsName::Base64 { bcs_name } => bcs_name.as_ref(),
            BcsName::Base58 { bcs_name } => bcs_name.as_ref(),
        }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        match self {
            BcsName::Base64 { bcs_name } => bcs_name,
            BcsName::Base58 { bcs_name } => bcs_name,
        }
    }
}

#[allow(unused)]
#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", untagged)]
enum MaybeTaggedBcsName {
    Tagged(TaggedBcsName),
    Base58 {
        #[serde_as(as = "Base58")]
        #[serde(rename = "bcsName")]
        bcs_name: Vec<u8>,
    },
}

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "bcsEncoding")]
enum TaggedBcsName {
    Base64 {
        #[serde_as(as = "Base64")]
        #[serde(rename = "bcsName")]
        bcs_name: Vec<u8>,
    },
    Base58 {
        #[serde_as(as = "Base58")]
        #[serde(rename = "bcsName")]
        bcs_name: Vec<u8>,
    },
}

impl From<MaybeTaggedBcsName> for BcsName {
    fn from(name: MaybeTaggedBcsName) -> BcsName {
        let bcs_name = match name {
            MaybeTaggedBcsName::Tagged(TaggedBcsName::Base58 { bcs_name })
            | MaybeTaggedBcsName::Base58 { bcs_name } => bcs_name,
            MaybeTaggedBcsName::Tagged(TaggedBcsName::Base64 { bcs_name }) => bcs_name,
        };

        // Bytes are already decoded, force into Base64 variant to avoid serializing to
        // base58
        Self::Base64 { bcs_name }
    }
}

/// Defines the compressed version of the public key that we pass around
/// in IOTA.
#[serde_as]
#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[schemars(rename = "AuthorityPublicKeyBytes")]
pub struct IotaAuthorityPublicKeyBytes(
    #[serde_as(as = "Base64")]
    #[schemars(with = "Base64Schema")]
    pub [u8; AuthorityPublicKey::LENGTH],
);

impl From<IotaAuthorityPublicKeyBytes> for AuthorityPublicKeyBytes {
    fn from(value: IotaAuthorityPublicKeyBytes) -> Self {
        Self(value.0)
    }
}

impl From<AuthorityPublicKeyBytes> for IotaAuthorityPublicKeyBytes {
    fn from(value: AuthorityPublicKeyBytes) -> Self {
        Self(value.0)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn bcs_name_test() {
        let bytes = vec![0, 1, 2, 3, 4];
        let untagged_base58 = r#"{"bcsName":"12VfUX"}"#;
        let tagged_base58 = r#"{"bcsEncoding":"base58","bcsName":"12VfUX"}"#;
        let tagged_base64 = r#"{"bcsEncoding":"base64","bcsName":"AAECAwQ="}"#;

        assert_eq!(
            bytes,
            serde_json::from_str::<BcsName>(untagged_base58)
                .unwrap()
                .into_bytes()
        );
        assert_eq!(
            bytes,
            serde_json::from_str::<BcsName>(tagged_base58)
                .unwrap()
                .into_bytes()
        );
        assert_eq!(
            bytes,
            serde_json::from_str::<BcsName>(tagged_base64)
                .unwrap()
                .into_bytes()
        );

        // Roundtrip base64
        let name = serde_json::from_str::<BcsName>(tagged_base64).unwrap();
        let json = serde_json::to_string(&name).unwrap();
        let from_json = serde_json::from_str::<BcsName>(&json).unwrap();
        assert_eq!(name, from_json);

        // Roundtrip base58
        let name = serde_json::from_str::<BcsName>(tagged_base58).unwrap();
        let json = serde_json::to_string(&name).unwrap();
        let from_json = serde_json::from_str::<BcsName>(&json).unwrap();
        assert_eq!(name, from_json);
    }
}
