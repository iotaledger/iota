// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk::types::block::{
    address::Ed25519Address,
    output::{
        feature::Irc30Metadata,
        unlock_condition::{AddressUnlockCondition, ExpirationUnlockCondition},
        AliasId, BasicOutputBuilder, NativeToken, OutputId, SimpleTokenScheme,
    },
};
use sui_types::base_types::ObjectID;

use super::extract_native_token_from_bag;
use crate::stardust::{
    migration::{
        tests::{create_foundry, random_output_header},
        Migration,
    },
    types::output::BASIC_OUTPUT_MODULE_NAME,
};

/// Test the id of a `BasicOutput` that is transformed to a simple coin.
///
/// Skips checks included in the verification step of the migration.
#[test]
fn basic_simple_coin_id() {
    let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let header = random_output_header();

    let stardust_basic = BasicOutputBuilder::new_with_amount(1_000_000)
        .add_unlock_condition(AddressUnlockCondition::new(random_address))
        .finish()
        .unwrap();

    let mut migration = Migration::new(1).unwrap();
    migration
        .run_migration([(header.clone(), stardust_basic.clone().into())])
        .unwrap();
    let migrated_object_id = migration
        .output_objects_map
        .get(&header.output_id())
        .unwrap()
        .coin()
        .unwrap();
    let expected_object_id = ObjectID::new(header.output_id().hash());
    assert_eq!(expected_object_id, *migrated_object_id);
}

/// Test the id of a `BasicOutput` object.
///
/// Skips checks included in the verification step of the migration.
#[test]
fn basic_id() {
    let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let header = random_output_header();

    let stardust_basic = BasicOutputBuilder::new_with_amount(1_000_000)
        .add_unlock_condition(AddressUnlockCondition::new(random_address))
        .add_unlock_condition(ExpirationUnlockCondition::new(random_address, 1).unwrap())
        .finish()
        .unwrap();

    let mut migration = Migration::new(1).unwrap();
    migration
        .run_migration([(header.clone(), stardust_basic.clone().into())])
        .unwrap();
    let migrated_object_id = migration
        .output_objects_map
        .get(&header.output_id())
        .unwrap()
        .output()
        .unwrap();
    let expected_object_id = ObjectID::new(header.output_id().hash());
    assert_eq!(expected_object_id, *migrated_object_id);
}

#[test]
fn basic_simple_coin_migration_with_native_token() {
    let (foundry_header, foundry_output) = create_foundry(
        0,
        SimpleTokenScheme::new(100_000, 0, 100_000_000).unwrap(),
        Irc30Metadata::new("Rustcoin", "Rust", 0),
        AliasId::null(),
    )
    .unwrap();
    let native_token = NativeToken::new(foundry_output.id().into(), 100).unwrap();

    let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let header = random_output_header();

    let stardust_basic = BasicOutputBuilder::new_with_amount(1_000_000)
        .add_unlock_condition(AddressUnlockCondition::new(random_address))
        .add_native_token(native_token)
        .finish()
        .unwrap();

    let outputs = [
        (foundry_header, foundry_output.into()),
        (header, stardust_basic.into()),
    ];
    let mut migration = Migration::new(1).unwrap();
    migration.run_migration(outputs).unwrap();
}

#[test]
fn basic_migration_with_native_token() {
    let (foundry_header, foundry_output) = create_foundry(
        0,
        SimpleTokenScheme::new(100_000, 0, 100_000_000).unwrap(),
        Irc30Metadata::new("Rustcoin", "Rust", 0),
        AliasId::null(),
    )
    .unwrap();
    let native_token = NativeToken::new(foundry_output.id().into(), 100).unwrap();

    let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let header = random_output_header();
    let output_id = header.output_id();

    let stardust_basic = BasicOutputBuilder::new_with_amount(1_000_000)
        .add_unlock_condition(AddressUnlockCondition::new(random_address))
        .add_unlock_condition(
            StorageDepositReturnUnlockCondition::new(random_address, 10, 1000).unwrap(),
        )
        .add_native_token(native_token)
        .finish()
        .unwrap();

    let outputs = [
        (foundry_header, foundry_output.into()),
        (header, stardust_basic.into()),
    ];
    // let mut migration = Migration::new(1).unwrap();
    // migration.run_migration(outputs.clone()).unwrap();

    extract_native_token_from_bag(
        output_id,
        outputs,
        BASIC_OUTPUT_MODULE_NAME,
        native_token,
        ExpectedAssets::BalanceBag,
    )
    .unwrap();
}
