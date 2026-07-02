// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_enum_compat_util::*;
use iota_sdk_types::{Identifier, ObjectId, StructTag};
use iota_types::{
    gas_coin::GasCoin, iota_sdk_types_conversions::struct_tag_sdk_to_core,
    object::bounded_visitor::BoundedVisitor,
};
use move_core_types::{
    account_address::AccountAddress,
    annotated_value::{MoveFieldLayout, MoveStructLayout, MoveTypeLayout},
    ident_str,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{IotaMoveStruct, IotaMoveValue, MoveFunctionName};

#[test]
fn enforce_order_test() {
    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.extend(["tests", "staged", "iota_move_struct.yaml"]);
    check_enum_compat_order::<IotaMoveStruct>(path);

    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.extend(["tests", "staged", "iota_move_value.yaml"]);
    check_enum_compat_order::<IotaMoveValue>(path);
}

#[test]
fn parse_move_function_name() {
    let name = "0x03::wat::call";
    let parsed: MoveFunctionName = name.parse().unwrap();
    assert_eq!(parsed.package, ObjectId::SYSTEM);
    assert_eq!(parsed.module.as_str(), "wat");
    assert_eq!(parsed.function.as_str(), "call");
}

#[test]
fn parse_move_function_name_unsupported_pkg_address() {
    let name = "namedpackage::wat::call";
    let parsed: Result<MoveFunctionName, _> = name.parse();
    assert!(parsed.is_err());
}

#[test]
fn parse_move_function_name_non_ascii_mod() {
    let name = "0x03::βατ::call";
    let parsed: Result<MoveFunctionName, _> = name.parse();
    assert!(parsed.is_err());
}

#[test]
fn parse_move_function_name_non_ascii_fun() {
    let name = "0x03::wat::βατ";
    let parsed: Result<MoveFunctionName, _> = name.parse();
    assert!(parsed.is_err());
}

#[test]
fn test_to_json_value() {
    let move_event = TestEvent {
        creator: AccountAddress::random(),
        name: "test_event".into(),
        data: vec![100, 200, 300],
        coins: vec![
            GasCoin::new(ObjectId::random(), 1000000),
            GasCoin::new(ObjectId::random(), 2000000),
            GasCoin::new(ObjectId::random(), 3000000),
        ],
    };
    let event_bytes = bcs::to_bytes(&move_event).unwrap();
    let iota_move_struct: IotaMoveStruct =
        BoundedVisitor::deserialize_struct(&event_bytes, &TestEvent::layout())
            .unwrap()
            .into();
    let json_value = iota_move_struct.to_json_value();
    assert_eq!(
        Some(&json!("1000000")),
        json_value.pointer("/coins/0/balance")
    );
    assert_eq!(
        Some(&json!("2000000")),
        json_value.pointer("/coins/1/balance")
    );
    assert_eq!(
        Some(&json!("3000000")),
        json_value.pointer("/coins/2/balance")
    );
    assert_eq!(
        Some(&json!(move_event.coins[0].id().to_string())),
        json_value.pointer("/coins/0/id/id")
    );
    assert_eq!(
        Some(&json!(format!("{:#x}", move_event.creator))),
        json_value.pointer("/creator")
    );
    assert_eq!(Some(&json!("100")), json_value.pointer("/data/0"));
    assert_eq!(Some(&json!("200")), json_value.pointer("/data/1"));
    assert_eq!(Some(&json!("300")), json_value.pointer("/data/2"));
    assert_eq!(Some(&json!("test_event")), json_value.pointer("/name"));
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TestEvent {
    creator: AccountAddress,
    name: UTF8String,
    data: Vec<u64>,
    coins: Vec<GasCoin>,
}

impl TestEvent {
    fn type_() -> StructTag {
        StructTag::new(
            ObjectId::FRAMEWORK,
            Identifier::from_static("IOTA"),
            Identifier::from_static("new_foobar"),
            vec![],
        )
    }

    fn layout() -> MoveStructLayout {
        MoveStructLayout {
            type_: struct_tag_sdk_to_core(&Self::type_()),
            fields: vec![
                MoveFieldLayout::new(ident_str!("creator").to_owned(), MoveTypeLayout::Address),
                MoveFieldLayout::new(
                    ident_str!("name").to_owned(),
                    MoveTypeLayout::Struct(Box::new(UTF8String::layout())),
                ),
                MoveFieldLayout::new(
                    ident_str!("data").to_owned(),
                    MoveTypeLayout::Vector(Box::new(MoveTypeLayout::U64)),
                ),
                MoveFieldLayout::new(
                    ident_str!("coins").to_owned(),
                    MoveTypeLayout::Vector(Box::new(MoveTypeLayout::Struct(Box::new(
                        GasCoin::layout(),
                    )))),
                ),
            ],
        }
    }
}

// Rust version of the Move std::string::String type
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
struct UTF8String {
    bytes: String,
}

impl From<&str> for UTF8String {
    fn from(s: &str) -> Self {
        Self {
            bytes: s.to_string(),
        }
    }
}

impl UTF8String {
    fn type_() -> StructTag {
        StructTag::new_string()
    }
    fn layout() -> MoveStructLayout {
        MoveStructLayout {
            type_: struct_tag_sdk_to_core(&Self::type_()),
            fields: vec![MoveFieldLayout::new(
                ident_str!("bytes").to_owned(),
                MoveTypeLayout::Vector(Box::new(MoveTypeLayout::U8)),
            )],
        }
    }
}
