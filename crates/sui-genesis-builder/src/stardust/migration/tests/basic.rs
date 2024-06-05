// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk::types::block::{
    address::Ed25519Address,
    output::{
        feature::Irc30Metadata, unlock_condition::AddressUnlockCondition, AliasId,
        BasicOutputBuilder, NativeToken, SimpleTokenScheme,
    },
};

use crate::stardust::migration::{
    tests::{create_foundry, random_output_header},
    Migration,
};

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
