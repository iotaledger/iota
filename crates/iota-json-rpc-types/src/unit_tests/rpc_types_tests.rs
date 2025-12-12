// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use anyhow::anyhow;
use iota_types::{
    IdentifierRef, StructTag, TypeTag,
    base_types::{Address, ObjectDigest, ObjectId, Version},
    gas_coin::GasCoin,
    iota_sdk_types_conversions::struct_tag_sdk_to_core,
    object::{MoveObject, Owner},
};
use move_core_types::{
    annotated_value::{MoveStruct, MoveValue},
    ident_str,
};
use serde_json::json;

use crate::{IotaMoveStruct, IotaMoveValue, ObjectChange};

#[test]
fn test_move_value_to_iota_coin() {
    let id = ObjectId::new(rand::random());
    let value = 10000;
    let coin = GasCoin::new(id, value);

    let move_object = MoveObject::new_gas_coin(Version::default(), id, value);
    let layout = GasCoin::layout();

    let move_struct = move_object.to_move_struct(&layout).unwrap();
    let iota_struct = IotaMoveStruct::from(move_struct);
    let gas_coin = GasCoin::try_from(&iota_struct).unwrap();
    assert_eq!(coin.value(), gas_coin.value());
    assert_eq!(coin.id(), gas_coin.id());
}

#[test]
fn test_move_value_to_string() {
    let test_string = "Some test string";
    let bytes = test_string.as_bytes();
    let values = bytes
        .iter()
        .map(|u8| MoveValue::U8(*u8))
        .collect::<Vec<_>>();

    let move_value = MoveValue::Struct(MoveStruct {
        type_: struct_tag_sdk_to_core(&StructTag {
            address: Address::STD_LIB,
            module: IdentifierRef::const_new("string").to_owned(),
            name: IdentifierRef::const_new("String").to_owned(),
            type_params: vec![],
        }),
        fields: vec![(ident_str!("bytes").to_owned(), MoveValue::Vector(values))],
    });

    let iota_value = IotaMoveValue::from(move_value);

    assert!(matches!(iota_value, IotaMoveValue::String(s) if s == test_string));
}

#[test]
fn test_option() {
    // bugfix for https://github.com/iotaledger/iota/issues/4995
    let option = MoveValue::Struct(MoveStruct {
        type_: struct_tag_sdk_to_core(&StructTag {
            address: Address::STD_LIB,
            module: IdentifierRef::const_new("option").to_owned(),
            name: IdentifierRef::const_new("Option").to_owned(),
            type_params: vec![TypeTag::U8],
        }),
        fields: vec![(
            ident_str!("vec").to_owned(),
            MoveValue::Vector(vec![MoveValue::U8(5)]),
        )],
    });
    let iota_value = IotaMoveValue::from(option);
    assert!(matches!(
        iota_value,
        IotaMoveValue::Option(value) if *value == Some(IotaMoveValue::Number(5))
    ));
}

#[test]
fn test_move_value_to_url() {
    let test_url = "http://testing.com";
    let bytes = test_url.as_bytes();
    let values = bytes
        .iter()
        .map(|u8| MoveValue::U8(*u8))
        .collect::<Vec<_>>();

    let string_move_value = MoveValue::Struct(MoveStruct {
        type_: struct_tag_sdk_to_core(&StructTag {
            address: Address::STD_LIB,
            module: IdentifierRef::const_new("string").to_owned(),
            name: IdentifierRef::const_new("String").to_owned(),
            type_params: vec![],
        }),
        fields: vec![(ident_str!("bytes").to_owned(), MoveValue::Vector(values))],
    });

    let url_move_value = MoveValue::Struct(MoveStruct {
        type_: struct_tag_sdk_to_core(&StructTag {
            address: Address::FRAMEWORK,
            module: IdentifierRef::const_new("url").to_owned(),
            name: IdentifierRef::const_new("Url").to_owned(),
            type_params: vec![],
        }),
        fields: vec![(ident_str!("url").to_owned(), string_move_value)],
    });

    let iota_value = IotaMoveValue::from(url_move_value);

    assert!(matches!(iota_value, IotaMoveValue::String(s) if s == test_url));
}

#[test]
fn test_serde() {
    let test_values = [
        IotaMoveValue::Number(u32::MAX),
        IotaMoveValue::UID {
            id: ObjectId::new(rand::random()),
        },
        IotaMoveValue::String("some test string".to_string()),
        IotaMoveValue::Address(Address::new(rand::random())),
        IotaMoveValue::Bool(true),
        IotaMoveValue::Option(Box::new(None)),
        IotaMoveValue::Vector(vec![
            IotaMoveValue::Number(1000000),
            IotaMoveValue::Number(2000000),
            IotaMoveValue::Number(3000000),
        ]),
    ];

    for value in test_values {
        let json = serde_json::to_string(&value).unwrap();
        let serde_value: IotaMoveValue = serde_json::from_str(&json)
            .map_err(|e| anyhow!("Serde failed for [{value:?}], Error msg : {e}"))
            .unwrap();
        assert_eq!(
            value, serde_value,
            "Error converting {value:?} [{json}], got {serde_value:?}",
        )
    }
}

#[test]
fn test_serde_bytearray() {
    // ensure that we serialize byte arrays as number array
    let test_values = MoveValue::Vector(vec![MoveValue::U8(1), MoveValue::U8(2), MoveValue::U8(3)]);
    let iota_move_value = IotaMoveValue::from(test_values);
    let json = serde_json::to_value(&iota_move_value).unwrap();
    assert_eq!(json, json!([1, 2, 3]));
}

#[test]
fn test_serde_number() {
    // ensure that we serialize byte arrays as number array
    let test_values = MoveValue::U8(1);
    let iota_move_value = IotaMoveValue::from(test_values);
    let json = serde_json::to_value(&iota_move_value).unwrap();
    assert_eq!(json, json!(1));
    let test_values = MoveValue::U16(1);
    let iota_move_value = IotaMoveValue::from(test_values);
    let json = serde_json::to_value(&iota_move_value).unwrap();
    assert_eq!(json, json!(1));
    let test_values = MoveValue::U32(1);
    let iota_move_value = IotaMoveValue::from(test_values);
    let json = serde_json::to_value(&iota_move_value).unwrap();
    assert_eq!(json, json!(1));
}

#[test]
fn test_type_tag_struct_tag_devnet_inc_222() {
    let offending_tags = [
        "0x1::address::MyType",
        "0x1::vector::MyType",
        "0x1::address::MyType<0x1::address::OtherType>",
        "0x1::address::MyType<0x1::address::OtherType, 0x1::vector::VecTyper>",
        "0x1::address::address<0x1::vector::address, 0x1::vector::vector>",
    ];

    for tag in offending_tags {
        let oc = ObjectChange::Created {
            sender: Address::ZERO,
            owner: Owner::Immutable,
            object_type: StructTag::from_str(tag).unwrap(),
            object_id: ObjectId::new(rand::random()),
            version: Default::default(),
            digest: ObjectDigest::new(rand::random()),
        };

        let serde_json = serde_json::to_string(&oc).unwrap();
        let deser: ObjectChange = serde_json::from_str(&serde_json).unwrap();
        assert_eq!(oc, deser);
    }
}
