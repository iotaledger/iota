// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use iota_sdk::types::block::{
    address::{AliasAddress, Ed25519Address},
    output::{
        AliasId, BasicOutputBuilder, Feature, FoundryOutputBuilder, NativeToken, SimpleTokenScheme,
        TokenScheme,
        feature::{Irc30Metadata, MetadataFeature},
        unlock_condition::{
            AddressUnlockCondition, ExpirationUnlockCondition,
            ImmutableAliasAddressUnlockCondition, StorageDepositReturnUnlockCondition,
            TimelockUnlockCondition,
        },
    },
    payload::transaction::TransactionId,
};
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    stardust::{
        coin_type::CoinType, output::basic::BASIC_OUTPUT_MODULE_NAME, stardust_to_iota_address,
    },
};

use crate::stardust::{
    migration::{
        Migration, MigrationTargetNetwork,
        tests::{
            ExpectedAssets, UnlockObjectTestResult, create_foundry, extract_native_tokens_from_bag,
            random_output_header, unlock_object,
        },
    },
    types::{address_swap_map::AddressSwapMap, output_header::OutputHeader},
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

    let mut migration = Migration::new(
        1,
        1_000_000,
        MigrationTargetNetwork::Mainnet,
        CoinType::Iota,
        AddressSwapMap::default(),
    )
    .unwrap();
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

/// Test that basic outputs with expired timelocks and no other unlock condition
/// are migrated to Coins. Test for both vesting reward outputs and regular
/// basic outputs.
#[test]
fn basic_simple_coin_id_with_expired_timelock() {
    for header in [
        random_output_header(),
        OutputHeader::new(
            // A potential vesting reward output transaction ID.
            *TransactionId::from_str(
                "0xb191c4bc825ac6983789e50545d5ef07a1d293a98ad974fc9498cb1812345678",
            )
            .unwrap(),
            Default::default(),
            rand::random(),
            rand::random(),
            rand::random(),
        ),
    ] {
        let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());

        let target_milestone_timestamp_sec: u32 = 100;
        let stardust_basic = BasicOutputBuilder::new_with_amount(1_000_000)
            .add_unlock_condition(AddressUnlockCondition::new(random_address))
            .add_unlock_condition(
                TimelockUnlockCondition::new(target_milestone_timestamp_sec).unwrap(),
            )
            .finish()
            .unwrap();

        let mut migration = Migration::new(
            target_milestone_timestamp_sec,
            1_000_000,
            MigrationTargetNetwork::Mainnet,
            CoinType::Iota,
            AddressSwapMap::default(),
        )
        .unwrap();
        migration
            .run_migration([(header.clone(), stardust_basic.clone().into())])
            .unwrap();
        let created_objects = migration
            .output_objects_map
            .get(&header.output_id())
            .unwrap();
        let migrated_object_id = created_objects.coin().unwrap();
        let expected_object_id = ObjectID::new(header.output_id().hash());
        assert_eq!(expected_object_id, *migrated_object_id);
        // No output should have been created.
        assert!(created_objects.output().is_err())
    }
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

    let mut migration = Migration::new(
        1,
        1_000_000,
        MigrationTargetNetwork::Mainnet,
        CoinType::Iota,
        AddressSwapMap::default(),
    )
    .unwrap();
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
    let mut migration = Migration::new(
        1,
        1_000_000,
        MigrationTargetNetwork::Mainnet,
        CoinType::Iota,
        AddressSwapMap::default(),
    )
    .unwrap();
    migration.run_migration(outputs).unwrap();
}

#[test]
fn basic_simple_coin_migration_with_native_tokens() {
    let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let basic_header = random_output_header();
    let basic_output_id = basic_header.output_id();

    let mut outputs = Vec::new();
    let mut basic_builder = BasicOutputBuilder::new_with_amount(1_000_000)
        .add_unlock_condition(AddressUnlockCondition::new(random_address));

    for i in 1..=10 {
        let foundry_header = random_output_header();
        let token_scheme = SimpleTokenScheme::new(100_000, 0, 100_000_000).unwrap();
        let irc_30_metadata = Irc30Metadata::new(format!("Rustcoin{i}"), format!("Rust{i}"), 0);
        let foundry_output =
            FoundryOutputBuilder::new_with_amount(0, i, TokenScheme::Simple(token_scheme))
                .add_unlock_condition(ImmutableAliasAddressUnlockCondition::new(
                    AliasAddress::new(AliasId::null()),
                ))
                .add_immutable_feature(Feature::Metadata(
                    MetadataFeature::new(irc_30_metadata).unwrap(),
                ))
                .finish()
                .unwrap();
        let native_token = NativeToken::new(foundry_output.id().into(), 100).unwrap();
        basic_builder = basic_builder.add_native_token(native_token);
        outputs.push((foundry_header, foundry_output.into()));
    }

    let basic_output = basic_builder.finish().unwrap();
    outputs.push((basic_header, basic_output.into()));

    let mut migration = Migration::new(
        1,
        1_000_000,
        MigrationTargetNetwork::Mainnet,
        CoinType::Iota,
        AddressSwapMap::default(),
    )
    .unwrap();
    migration.run_migration(outputs.clone()).unwrap();

    let created_gas_coin_id = migration
        .output_objects_map
        .get(&basic_output_id)
        .unwrap()
        .coin()
        .unwrap();
    let expected_gas_coin_id = ObjectID::new(basic_output_id.hash());
    assert_eq!(
        created_gas_coin_id, &expected_gas_coin_id,
        "unexpected gas coin id"
    );

    let created_native_token_ids = migration
        .output_objects_map
        .get(&basic_output_id)
        .unwrap()
        .native_tokens()
        .unwrap();
    assert_eq!(
        created_native_token_ids.len(),
        10,
        "unexpected number of created native token objects"
    );
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

    let basic_output = BasicOutputBuilder::new_with_amount(1_000_000)
        .add_unlock_condition(AddressUnlockCondition::new(random_address))
        .add_unlock_condition(
            StorageDepositReturnUnlockCondition::new(random_address, 10, 1000).unwrap(),
        )
        .add_native_token(native_token)
        .finish()
        .unwrap();

    let native_tokens = basic_output.native_tokens().clone();

    let outputs = [
        (foundry_header, foundry_output.into()),
        (header, basic_output.into()),
    ];

    extract_native_tokens_from_bag(
        output_id,
        1_000_000,
        outputs,
        BASIC_OUTPUT_MODULE_NAME,
        native_tokens,
        ExpectedAssets::BalanceBag,
        CoinType::Iota,
    )
    .unwrap();
}

#[test]
fn basic_migration_with_native_tokens() {
    let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let basic_header = random_output_header();
    let basic_output_id = basic_header.output_id();

    let mut outputs = Vec::new();
    let mut basic_builder = BasicOutputBuilder::new_with_amount(1_000_000)
        .add_unlock_condition(AddressUnlockCondition::new(random_address))
        .add_unlock_condition(
            StorageDepositReturnUnlockCondition::new(random_address, 10, 1000).unwrap(),
        );

    for i in 1..=10 {
        let foundry_header = random_output_header();
        let token_scheme = SimpleTokenScheme::new(100_000, 0, 100_000_000).unwrap();
        let irc_30_metadata = Irc30Metadata::new(format!("Rustcoin{i}"), format!("Rust{i}"), 0);
        let foundry_output =
            FoundryOutputBuilder::new_with_amount(0, i, TokenScheme::Simple(token_scheme))
                .add_unlock_condition(ImmutableAliasAddressUnlockCondition::new(
                    AliasAddress::new(AliasId::null()),
                ))
                .add_immutable_feature(Feature::Metadata(
                    MetadataFeature::new(irc_30_metadata).unwrap(),
                ))
                .finish()
                .unwrap();
        let native_token = NativeToken::new(foundry_output.id().into(), 100).unwrap();
        basic_builder = basic_builder.add_native_token(native_token);
        outputs.push((foundry_header, foundry_output.into()));
    }

    let basic_output = basic_builder.finish().unwrap();
    let native_tokens = basic_output.native_tokens().clone();
    outputs.push((basic_header, basic_output.into()));

    extract_native_tokens_from_bag(
        basic_output_id,
        1_000_000,
        outputs,
        BASIC_OUTPUT_MODULE_NAME,
        native_tokens,
        ExpectedAssets::BalanceBag,
        CoinType::Iota,
    )
    .unwrap();
}

#[test]
fn basic_migration_with_timelock_unlocked() {
    let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let return_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let header = random_output_header();

    // The epoch timestamp that the executor will use for the test.
    let epoch_start_timestamp_ms = 100_000;

    let stardust_basic = BasicOutputBuilder::new_with_amount(1_000_000)
        .add_unlock_condition(AddressUnlockCondition::new(random_address))
        .add_unlock_condition(
            TimelockUnlockCondition::new(epoch_start_timestamp_ms / 1000).unwrap(),
        )
        // We need to add a SDRUC because the Basic Output is otherwise migrated to a Coin object.
        .add_unlock_condition(
            StorageDepositReturnUnlockCondition::new(return_address, 1_000, 1_000_000_000).unwrap(),
        )
        .finish()
        .unwrap();

    unlock_object(
        header.output_id(),
        1_000_000,
        [(header, stardust_basic.into())],
        // Sender is not important for this test.
        &IotaAddress::ZERO,
        BASIC_OUTPUT_MODULE_NAME,
        epoch_start_timestamp_ms as u64,
        UnlockObjectTestResult::Success,
        ExpectedAssets::BalanceBag,
        CoinType::Iota,
    )
    .unwrap();
}

/// Test that a Basic Output with an unexpired timelock cannot be unlocked.
///
/// Implicitly also tests that such an output is not migrated to a `Coin`.
#[test]
fn basic_migration_with_timelock_still_locked() {
    let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let header = random_output_header();

    // The epoch timestamp that the executor will use for the test.
    let epoch_start_timestamp_ms = 100_000;

    let stardust_basic = BasicOutputBuilder::new_with_amount(1_000_000)
        .add_unlock_condition(AddressUnlockCondition::new(random_address))
        .add_unlock_condition(
            TimelockUnlockCondition::new((epoch_start_timestamp_ms / 1000) + 1).unwrap(),
        )
        .finish()
        .unwrap();

    unlock_object(
        header.output_id(),
        1_000_000,
        [(header, stardust_basic.into())],
        // Sender is not important for this test.
        &IotaAddress::ZERO,
        BASIC_OUTPUT_MODULE_NAME,
        epoch_start_timestamp_ms as u64,
        UnlockObjectTestResult::ERROR_TIMELOCK_NOT_EXPIRED_FAILURE,
        ExpectedAssets::BalanceBag,
        CoinType::Iota,
    )
    .unwrap();
}

/// Test that a BasicOutput with an expired Expiration Unlock Condition
/// can/cannot be unlocked, depending on the TX sender.
#[test]
fn basic_migration_with_expired_unlock_condition() {
    let owner = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let return_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let iota_owner_address = stardust_to_iota_address(owner).unwrap();
    let iota_return_address = stardust_to_iota_address(return_address).unwrap();
    let header = random_output_header();

    // The epoch timestamp that the executor will use for the test.
    let epoch_start_timestamp_ms = 100_000;

    // Expiration Timestamp is exactly at the epoch start timestamp -> object is
    // expired -> return address can unlock.
    let stardust_basic = BasicOutputBuilder::new_with_amount(1_000_000)
        .add_unlock_condition(AddressUnlockCondition::new(owner))
        .add_unlock_condition(
            ExpirationUnlockCondition::new(return_address, epoch_start_timestamp_ms / 1000)
                .unwrap(),
        )
        .finish()
        .unwrap();

    // Owner Address CANNOT unlock.
    unlock_object(
        header.output_id(),
        1_000_000,
        [(header.clone(), stardust_basic.clone().into())],
        &iota_owner_address,
        BASIC_OUTPUT_MODULE_NAME,
        epoch_start_timestamp_ms as u64,
        UnlockObjectTestResult::ERROR_WRONG_SENDER_FAILURE,
        ExpectedAssets::BalanceBag,
        CoinType::Iota,
    )
    .unwrap();

    // Return Address CAN unlock.
    unlock_object(
        header.output_id(),
        1_000_000,
        [(header, stardust_basic.into())],
        &iota_return_address,
        BASIC_OUTPUT_MODULE_NAME,
        epoch_start_timestamp_ms as u64,
        UnlockObjectTestResult::Success,
        ExpectedAssets::BalanceBag,
        CoinType::Iota,
    )
    .unwrap();
}

/// Test that a Basic Output with an unexpired Expiration Unlock Condition
/// can/cannot be unlocked, depending on the TX sender.
#[test]
fn basic_migration_with_unexpired_unlock_condition() {
    let owner = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let return_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let iota_owner_address = stardust_to_iota_address(owner).unwrap();
    let iota_return_address = stardust_to_iota_address(return_address).unwrap();
    let header = random_output_header();

    // The epoch timestamp that the executor will use for the test.
    let epoch_start_timestamp_ms = 100_000;

    // Expiration Timestamp is after the epoch start timestamp -> object is not
    // expired -> owner address can unlock.
    let stardust_basic = BasicOutputBuilder::new_with_amount(1_000_000)
        .add_unlock_condition(AddressUnlockCondition::new(owner))
        .add_unlock_condition(
            ExpirationUnlockCondition::new(return_address, (epoch_start_timestamp_ms / 1000) + 1)
                .unwrap(),
        )
        .finish()
        .unwrap();

    // Return Address CANNOT unlock.
    unlock_object(
        header.output_id(),
        1_000_000,
        [(header.clone(), stardust_basic.clone().into())],
        &iota_return_address,
        BASIC_OUTPUT_MODULE_NAME,
        epoch_start_timestamp_ms as u64,
        UnlockObjectTestResult::ERROR_WRONG_SENDER_FAILURE,
        ExpectedAssets::BalanceBag,
        CoinType::Iota,
    )
    .unwrap();

    // Owner Address CAN unlock.
    unlock_object(
        header.output_id(),
        1_000_000,
        [(header, stardust_basic.into())],
        &iota_owner_address,
        BASIC_OUTPUT_MODULE_NAME,
        epoch_start_timestamp_ms as u64,
        UnlockObjectTestResult::Success,
        ExpectedAssets::BalanceBag,
        CoinType::Iota,
    )
    .unwrap();
}

/// Test that a BasicOutput with a Storage Deposit Return Unlock Condition can
/// be unlocked.
#[test]
fn basic_migration_with_storage_deposit_return_unlock_condition() {
    let owner = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let return_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let header = random_output_header();

    let stardust_basic = BasicOutputBuilder::new_with_amount(1_000_000)
        .add_unlock_condition(AddressUnlockCondition::new(owner))
        .add_unlock_condition(
            StorageDepositReturnUnlockCondition::new(return_address, 1_000, 1_000_000_000).unwrap(),
        )
        .finish()
        .unwrap();

    // Simply test that the unlock with the SDRUC succeeds.
    unlock_object(
        header.output_id(),
        1_000_000,
        [(header, stardust_basic.into())],
        // Sender is not important for this test.
        &IotaAddress::ZERO,
        BASIC_OUTPUT_MODULE_NAME,
        // Epoch start time is not important for this test.
        0,
        UnlockObjectTestResult::Success,
        ExpectedAssets::BalanceBag,
        CoinType::Iota,
    )
    .unwrap();
}

#[test]
fn basic_migration_with_incorrect_total_supply() {
    let owner = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let header = random_output_header();

    let total_value = 1_000_000;
    let stardust_basic = BasicOutputBuilder::new_with_amount(total_value)
        .add_unlock_condition(AddressUnlockCondition::new(owner))
        .finish()
        .unwrap();

    // Total supply does not match the total amount on outputs
    let total_supply = 2_000_000;
    let err = unlock_object(
        header.output_id(),
        total_supply,
        [(header, stardust_basic.into())],
        // Sender is not important for this test.
        &IotaAddress::ZERO,
        BASIC_OUTPUT_MODULE_NAME,
        // Epoch start time is not important for this test.
        0,
        UnlockObjectTestResult::Success,
        ExpectedAssets::BalanceBag,
        CoinType::Iota,
    )
    .unwrap_err();

    assert_eq!(
        err.to_string(),
        format!("total supply mismatch: found {total_value}, expected {total_supply}")
    );
}
