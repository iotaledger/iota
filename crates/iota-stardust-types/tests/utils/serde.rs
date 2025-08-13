// Copyright 2023 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use core::str::FromStr;

use iota_stardust_types::types::block::BlockId;
use pretty_assertions::assert_eq;
use serde::{Deserialize, Serialize};

const BLOCK_ID: &str = "0x52fdfc072182654f163f5f0f9a621d729566c74d10037c4d7bbb0407d1e2c649";

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SerdeStringTest {
    #[serde(with = "iota_stardust_types::utils::serde::string")]
    pub block_id: BlockId,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SerdeOptionStringTest {
    #[serde(with = "iota_stardust_types::utils::serde::option_string")]
    pub block_id: Option<BlockId>,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SerdeOptionPrefixHexVecTest {
    #[serde(
        default,
        with = "iota_stardust_types::utils::serde::option_prefix_hex_bytes"
    )]
    pub metadata: Option<Vec<u8>>,
}

#[test]
fn serde_string() {
    let block_id = BlockId::from_str(BLOCK_ID).unwrap();
    let test_1 = SerdeStringTest { block_id };
    let str = serde_json::to_string(&test_1).unwrap();
    let test_2 = serde_json::from_str(&str).unwrap();

    assert_eq!(test_1, test_2);
    assert!(str.contains(BLOCK_ID));
}

#[test]
fn serde_option_string() {
    let block_id = BlockId::from_str(BLOCK_ID).unwrap();
    let test_1 = SerdeOptionStringTest {
        block_id: Some(block_id),
    };
    let str = serde_json::to_string(&test_1).unwrap();
    let test_2 = serde_json::from_str(&str).unwrap();

    assert_eq!(test_1, test_2);
    assert!(str.contains(BLOCK_ID));

    let test_1 = SerdeOptionStringTest { block_id: None };
    let str = serde_json::to_string(&test_1).unwrap();
    let test_2 = serde_json::from_str(&str).unwrap();

    assert_eq!(test_1, test_2);
    assert!(str.contains("null"));
}

#[test]
fn serde_option_prefix_hex_bytes() {
    let metadata = prefix_hex::decode(BLOCK_ID).unwrap();
    let test_1 = SerdeOptionPrefixHexVecTest {
        metadata: Some(metadata),
    };
    let str = serde_json::to_string(&test_1).unwrap();
    let test_2 = serde_json::from_str(&str).unwrap();

    assert_eq!(test_1, test_2);
    assert!(str.contains(BLOCK_ID));

    let test_1 = SerdeOptionPrefixHexVecTest { metadata: None };
    let str = serde_json::to_string(&test_1).unwrap();
    let test_2 = serde_json::from_str(&str).unwrap();

    assert_eq!(test_1, test_2);
    assert!(str.contains("null"));
}
