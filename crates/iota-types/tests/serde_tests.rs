// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use iota_types::{StructTag, base_types::ObjectType};
use serde::Serialize;
use serde_json::Value;

#[test]
fn test_struct_tag_serde() {
    let tag = StructTag::from_str("0x7f89cdffd8968affa0b47bef91adc5314e19509080470c45bfd434cd83a766b::mymodule::MyStruct<0x7f89cdffd8968affa0b47bef91adc5314e19509080470c45bfd434cd83a766b::othermodule::OtherStruct>").unwrap();
    #[derive(Serialize)]
    struct TestStructTag(StructTag);

    // serialize to json should not trim the leading 0
    let Value::String(json) = serde_json::to_value(TestStructTag(tag.clone())).unwrap() else {
        panic!()
    };
    assert_eq!(
        json,
        "0x07f89cdffd8968affa0b47bef91adc5314e19509080470c45bfd434cd83a766b::mymodule::MyStruct<0x07f89cdffd8968affa0b47bef91adc5314e19509080470c45bfd434cd83a766b::othermodule::OtherStruct>"
    );

    let tag2 = StructTag::from_str(&json).unwrap();
    assert_eq!(tag, tag2);
}

#[test]
fn test_object_type_to_string() {
    let object_type = ObjectType::from_str(
        "0x1a1aa18691be519899bf5187f5ce80af629407dd4f68d4175b99f4dc09497c1::custodian::AccountCap",
    )
    .unwrap();
    assert_eq!(
        object_type.to_string(),
        "0x01a1aa18691be519899bf5187f5ce80af629407dd4f68d4175b99f4dc09497c1::custodian::AccountCap"
    );
}
