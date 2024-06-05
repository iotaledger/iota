// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk::{
    types::block::output::{feature::Irc30Metadata, AliasId, SimpleTokenScheme},
    U256,
};
use sui_types::{
    base_types::{MoveObjectType, ObjectID},
    gas_coin::GAS,
};

use crate::stardust::{
    migration::tests::{create_foundry, run_migration},
    types::stardust_to_sui_address,
};

#[test]
fn create_gas_coin() {
    let (foundry_header, foundry_output) = create_foundry(
        1_000_000,
        SimpleTokenScheme::new(U256::from(100_000), U256::from(0), U256::from(100_000_000))
            .unwrap(),
        Irc30Metadata::new("Rustcoin", "Rust", 0),
        AliasId::null(),
    )
    .unwrap();

    let output_id = foundry_header.output_id();
    let alias_address = *foundry_output.alias_address();

    let (executor, _) = run_migration([(foundry_header, foundry_output.into())]).unwrap();
    let objects = executor.into_objects();

    // Foundry package publication creates five objects
    //
    // * The package
    // * Coin metadata
    // * MaxSupplyPolicy
    // * The total supply coin
    // * The foundry gas coin
    assert_eq!(objects.len(), 5);

    // Extract the package object.
    let package_object = objects
        .iter()
        .find(|object| object.is_package())
        .expect("there should be only a single gas coin");

    // Extract the gas coin object.
    let gas_coin_object = objects
        .iter()
        .find(|object| object.is_gas_coin())
        .expect("there should be only a single gas coin");

    // Downcast the gas coin object to get the coin.
    let coin = gas_coin_object.as_coin_maybe().unwrap();

    // Check if the gas coin id is the same as the output id.
    assert_eq!(gas_coin_object.id(), ObjectID::new(output_id.hash()));

    // Check if the owner of the gas coin is the package object.
    assert_eq!(
        gas_coin_object.owner.get_owner_address().unwrap(),
        stardust_to_sui_address(alias_address).unwrap()
    );

    assert_eq!(
        *gas_coin_object.type_().unwrap(),
        MoveObjectType::gas_coin()
    );
    assert_eq!(gas_coin_object.coin_type_maybe().unwrap(), GAS::type_tag());
    assert_eq!(coin.value(), 1_000_000);
    assert_eq!(package_object.version(), gas_coin_object.version());
}
