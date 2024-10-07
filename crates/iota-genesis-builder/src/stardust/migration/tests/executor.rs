// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_protocol_config::ProtocolVersion;
use iota_sdk::types::block::{
    address::AliasAddress,
    output::{
        AliasId, FoundryOutputBuilder, NativeToken, NativeTokens, SimpleTokenScheme,
        UnlockCondition, unlock_condition::ImmutableAliasAddressUnlockCondition,
    },
};
use iota_types::{
    balance::Balance,
    dynamic_field::{Field, derive_dynamic_field_id},
    object::Owner,
    stardust::coin_type::CoinType,
};

use crate::stardust::{
    migration::{
        MigrationTargetNetwork, executor::Executor, migration::NATIVE_TOKEN_BAG_KEY_TYPE,
        tests::random_output_header,
    },
    native_token::{
        package_builder,
        package_data::{NativeTokenModuleData, NativeTokenPackageData},
    },
};

#[test]
fn create_bag_with_pt() {
    // Mock the foundry
    let owner = AliasAddress::new(AliasId::new([0; AliasId::LENGTH]));
    let supply = 1_000_000;
    let token_scheme = SimpleTokenScheme::new(supply, 0, supply).unwrap();
    let header = random_output_header();
    let foundry = FoundryOutputBuilder::new_with_amount(1000, 1, token_scheme.into())
        .with_unlock_conditions([UnlockCondition::from(
            ImmutableAliasAddressUnlockCondition::new(owner),
        )])
        .finish()
        .unwrap();
    let foundry_id = foundry.id();
    let foundry_package_data = NativeTokenPackageData::new(
        "wat",
        NativeTokenModuleData::new(
            foundry_id, "wat", "WAT", 0, "WAT", supply, supply, "wat", "wat", None, owner,
        ),
    );
    let foundry_package = package_builder::build_and_compile(foundry_package_data).unwrap();

    // Execution
    let mut executor = Executor::new(
        ProtocolVersion::MAX,
        MigrationTargetNetwork::Mainnet,
        CoinType::Iota,
    )
    .unwrap();
    let object_count = executor.store().objects().len();
    executor
        .create_foundries([(&header, &foundry, foundry_package)])
        .unwrap();
    // Foundry package publication creates five objects
    //
    // * The package
    // * CoinManager
    // * CoinManagerTreasuryCap
    // * The total supply native token coin
    // * The coin held by the foundry which can be a gas coin or a smr coin
    assert_eq!(executor.store().objects().len() - object_count, 5);
    assert!(executor.native_tokens().get(&foundry_id.into()).is_some());
    let initial_supply_coin_object = executor
        .store()
        .objects()
        .values()
        .find(|object| object.is_coin() && !object.is_gas_coin())
        .expect("there should be only a single coin: the total supply of native tokens");
    let coin_type_tag = initial_supply_coin_object.coin_type_maybe().unwrap();
    let initial_supply_coin_data = initial_supply_coin_object.as_coin_maybe().unwrap();

    // Mock the native token
    let token_amount = 10_000;
    let native_token = NativeToken::new(foundry_id.into(), token_amount).unwrap();

    // Create the bag
    let (bag, _, _) = executor
        .create_bag_with_pt(&NativeTokens::from_vec(vec![native_token]).unwrap())
        .unwrap();
    assert!(executor.store().get_object(bag.id.object_id()).is_none());

    // Verify the mutation of the foundry coin with the total supply
    let mutated_supply_coin = executor
        .store()
        .get_object(initial_supply_coin_data.id())
        .unwrap()
        .as_coin_maybe()
        .unwrap();
    assert_eq!(mutated_supply_coin.value(), supply - token_amount);

    // Get the dynamic fields (df)
    let tokens = executor
        .store()
        .objects()
        .values()
        .filter(|object| object.is_child_object())
        .collect::<Vec<_>>();
    assert_eq!(tokens.len(), 1);
    assert_eq!(
        tokens[0].owner,
        Owner::ObjectOwner((*bag.id.object_id()).into())
    );
    let token_as_df = tokens[0].to_rust::<Field<String, Balance>>().unwrap();
    // Verify name
    let expected_name = coin_type_tag.to_canonical_string(false);
    assert_eq!(token_as_df.name, expected_name);
    // Verify value
    let expected_balance = Balance::new(token_amount);
    assert_eq!(token_as_df.value, expected_balance);
    // Verify df id
    let expected_id = derive_dynamic_field_id(
        *bag.id.object_id(),
        &NATIVE_TOKEN_BAG_KEY_TYPE.parse().unwrap(),
        &bcs::to_bytes(&expected_name).unwrap(),
    )
    .unwrap();
    assert_eq!(*token_as_df.id.object_id(), expected_id);
}
