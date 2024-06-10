// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use iota_types::{base_types::ObjectType, iota_serde::IotaStructTag, parse_iota_struct_tag};
use move_core_types::language_storage::StructTag;
use serde::Serialize;
use serde_json::Value;
use serde_with::serde_as;

#[test]
fn test_struct_tag_serde() {
    let tag = parse_iota_struct_tag("0x7f89cdffd8968affa0b47bef91adc5314e19509080470c45bfd434cd83a766b::iotafrens::IotaFren<0x7f89cdffd8968affa0b47bef91adc5314e19509080470c45bfd434cd83a766b::capy::Capy>").unwrap();
    #[serde_as]
    #[derive(Serialize)]
    struct TestStructTag(#[serde_as(as = "IotaStructTag")] StructTag);

    // serialize to json should not trim the leading 0
    let Value::String(json) = serde_json::to_value(&TestStructTag(tag.clone())).unwrap() else {
        panic!()
    };
    assert_eq!(
        json,
        "0x07f89cdffd8968affa0b47bef91adc5314e19509080470c45bfd434cd83a766b::iotafrens::IotaFren<0x07f89cdffd8968affa0b47bef91adc5314e19509080470c45bfd434cd83a766b::capy::Capy>"
    );

    let tag2 = parse_iota_struct_tag(&json).unwrap();
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
