// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use iota_json_rpc::name_service::{self, Domain};
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    collection_types::VecMap,
};

#[test]
fn test_parent_extraction() {
    let mut name = Domain::from_str("leaf.node.test.iota").unwrap();

    assert_eq!(name.parent().to_string(), "node.test.iota");

    name = Domain::from_str("node.test.iota").unwrap();

    assert_eq!(name.parent().to_string(), "test.iota");
}

#[test]
fn test_expirations() {
    let system_time: u64 = 100;

    let mut name = name_service::NameRecord {
        nft_id: iota_types::id::ID::new(ObjectID::random()),
        data: VecMap { contents: vec![] },
        target_address: Some(IotaAddress::random_for_testing_only()),
        expiration_timestamp_ms: system_time + 10,
    };

    assert!(!name.is_node_expired(system_time));

    name.expiration_timestamp_ms = system_time - 10;

    assert!(name.is_node_expired(system_time));
}

#[test]
fn test_name_service_outputs() {
    assert_eq!("@test".parse::<Domain>().unwrap().to_string(), "test.iota");
    assert_eq!(
        "test.iota".parse::<Domain>().unwrap().to_string(),
        "test.iota"
    );
    assert_eq!(
        "test@sld".parse::<Domain>().unwrap().to_string(),
        "test.sld.iota"
    );
    assert_eq!(
        "test.test@example".parse::<Domain>().unwrap().to_string(),
        "test.test.example.iota"
    );
    assert_eq!(
        "iota@iota".parse::<Domain>().unwrap().to_string(),
        "iota.iota.iota"
    );

    assert_eq!("@iota".parse::<Domain>().unwrap().to_string(), "iota.iota");

    assert_eq!(
        "test*test@test".parse::<Domain>().unwrap().to_string(),
        "test.test.test.iota"
    );
    assert_eq!(
        "test.test.iota".parse::<Domain>().unwrap().to_string(),
        "test.test.iota"
    );
    assert_eq!(
        "test.test.test.iota".parse::<Domain>().unwrap().to_string(),
        "test.test.test.iota"
    );
}

#[test]
fn test_different_wildcard() {
    assert_eq!("test.iota".parse::<Domain>(), "test*iota".parse::<Domain>(),);

    assert_eq!("@test".parse::<Domain>(), "test*iota".parse::<Domain>(),);
}

#[test]
fn test_invalid_inputs() {
    assert!("*".parse::<Domain>().is_err());
    assert!(".".parse::<Domain>().is_err());
    assert!("@".parse::<Domain>().is_err());
    assert!("@inner.iota".parse::<Domain>().is_err());
    assert!("@inner*iota".parse::<Domain>().is_err());
    assert!("test@".parse::<Domain>().is_err());
    assert!("iota".parse::<Domain>().is_err());
    assert!("test.test@example.iota".parse::<Domain>().is_err());
    assert!("test@test@example".parse::<Domain>().is_err());
}

#[test]
fn output_tests() {
    let mut domain = "test.iota".parse::<Domain>().unwrap();
    assert!(domain.format(name_service::DomainFormat::Dot) == "test.iota");
    assert!(domain.format(name_service::DomainFormat::At) == "@test");

    domain = "test.test.iota".parse::<Domain>().unwrap();
    assert!(domain.format(name_service::DomainFormat::Dot) == "test.test.iota");
    assert!(domain.format(name_service::DomainFormat::At) == "test@test");

    domain = "test.test.test.iota".parse::<Domain>().unwrap();
    assert!(domain.format(name_service::DomainFormat::Dot) == "test.test.test.iota");
    assert!(domain.format(name_service::DomainFormat::At) == "test.test@test");

    domain = "test.test.test.test.iota".parse::<Domain>().unwrap();
    assert!(domain.format(name_service::DomainFormat::Dot) == "test.test.test.test.iota");
    assert!(domain.format(name_service::DomainFormat::At) == "test.test.test@test");
}
