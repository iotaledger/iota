// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk::types::block::output::NativeTokens;
use iota_sdk::types::block::{
    address::AliasAddress,
    output::{
        unlock_condition::ImmutableAliasAddressUnlockCondition, AliasId, FoundryOutputBuilder,
        NativeToken, SimpleTokenScheme, UnlockCondition,
    },
};

use sui_protocol_config::ProtocolVersion;
use sui_types::balance::Balance;
use sui_types::{
    dynamic_field::{derive_dynamic_field_id, Field},
    object::Owner,
};

use crate::stardust::migration::executor::Executor;
use crate::stardust::migration::migration::NATIVE_TOKEN_BAG_KEY_TYPE;
use crate::stardust::migration::tests::random_output_header;
use crate::stardust::native_token::package_builder;
use crate::stardust::native_token::package_data::{NativeTokenModuleData, NativeTokenPackageData};

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
        .finish_with_params(supply)
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
    let mut executor = Executor::new(ProtocolVersion::MAX).unwrap();
    let object_count = executor.store().objects().len();
    executor
        .create_foundries([(&header, &foundry, foundry_package)])
        .unwrap();
    // Foundry package publication creates four objects
    //
    // * The package
    // * Coin metadata
    // * MaxSupplyPolicy
    // * The total supply coin
    assert_eq!(executor.store().objects().len() - object_count, 4);
    assert!(executor.native_tokens().get(&foundry_id.into()).is_some());
    let initial_supply_coin_object = executor
        .store()
        .objects()
        .values()
        .find_map(|object| object.is_coin().then_some(object))
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
        .filter_map(|object| object.is_child_object().then_some(object))
        .collect::<Vec<_>>();
    assert_eq!(tokens.len(), 1);
    assert_eq!(
        tokens[0].owner,
        Owner::ObjectOwner((*bag.id.object_id()).into())
    );
    let token_as_df = tokens[0].to_rust::<Field<String, Balance>>().unwrap();
    // Verify name
    let expected_name = coin_type_tag.to_canonical_string(true);
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
