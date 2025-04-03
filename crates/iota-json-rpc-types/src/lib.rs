// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub use balance_changes::*;
use fastcrypto::encoding::{Base58, Base64};
pub use iota_checkpoint::*;
pub use iota_coin::*;
pub use iota_event::*;
pub use iota_extended::*;
pub use iota_governance::*;
pub use iota_move::*;
pub use iota_object::*;
pub use iota_protocol::*;
pub use iota_transaction::*;
use iota_types::base_types::ObjectID;
pub use object_changes::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

#[cfg(test)]
#[path = "unit_tests/rpc_types_tests.rs"]
mod rpc_types_tests;

mod balance_changes;
mod displays;
mod iota_checkpoint;
mod iota_coin;
mod iota_event;
mod iota_extended;
mod iota_governance;
mod iota_move;
mod iota_object;
mod iota_protocol;
mod iota_transaction;
mod object_changes;

pub type DynamicFieldPage = Page<DynamicFieldInfo, ObjectID>;
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

#[serde_with::serde_as]
#[derive(Clone, Serialize, Deserialize, JsonSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DynamicFieldInfo {
    pub name: iota_types::dynamic_field::DynamicFieldName,
    #[serde(flatten)]
    pub bcs_name: BcsName,
    pub type_: iota_types::dynamic_field::DynamicFieldType,
    pub object_type: String,
    pub object_id: ObjectID,
    pub version: iota_types::base_types::SequenceNumber,
    pub digest: iota_types::digests::ObjectDigest,
}

impl From<iota_types::dynamic_field::DynamicFieldInfo> for DynamicFieldInfo {
    fn from(
        iota_types::dynamic_field::DynamicFieldInfo {
            name,
            bcs_name,
            type_,
            object_type,
            object_id,
            version,
            digest,
        }: iota_types::dynamic_field::DynamicFieldInfo,
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

#[serde_as]
#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", tag = "bcsEncoding")]
#[serde(from = "MaybeTaggedBcsName")]
pub enum BcsName {
    Base64 {
        #[serde_as(as = "Base64")]
        #[schemars(with = "Base64")]
        #[serde(rename = "bcsName")]
        bcs_name: Vec<u8>,
    },
    Base58 {
        #[serde_as(as = "Base58")]
        #[schemars(with = "Base58")]
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
