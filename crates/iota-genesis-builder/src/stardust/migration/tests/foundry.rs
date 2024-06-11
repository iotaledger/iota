// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Result};
use iota_protocol_config::ProtocolConfigValue::u64;
use iota_sdk::{
    types::block::output::{
        feature::Irc30Metadata, AliasId, FoundryOutput, Output, SimpleTokenScheme,
    },
    Url, U256,
};
use iota_types::{
    balance::Balance,
    base_types::{IotaAddress, MoveObjectType, ObjectID},
    coin::CoinMetadata,
    gas_coin::GAS,
    object::Object,
};
use move_core_types::language_storage::TypeTag;

use crate::stardust::{
    migration::tests::{create_foundry, run_migration},
    types::{
        capped_coin::MaxSupplyPolicy, snapshot::OutputHeader, stardust_to_iota_address,
        stardust_to_iota_address_owner,
    },
};

type PackageObject = Object;
type CoinObject = Object;
type MintedCoinObject = Object;
type CoinMetadataObject = Object;
type MaxSupplyPolicyObject = Object;

fn migrate_foundry(
    header: OutputHeader,
    foundry: FoundryOutput,
) -> Result<(
    PackageObject,
    CoinObject,
    MintedCoinObject,
    CoinMetadataObject,
    MaxSupplyPolicyObject,
)> {
    let output_id = header.output_id();

    let (executor, objects_map) =
        run_migration(foundry.amount(), [(header, Output::Foundry(foundry))])?;

    let created_objects_ids = objects_map
        .get(&output_id)
        .ok_or(anyhow!("missing created objects"))?;

    let created_objects = executor.into_objects();

    assert_eq!(created_objects.len(), 5);

    let package_id = *created_objects_ids.package()?;
    let coin_id = *created_objects_ids.coin()?;
    let minted_coin_id = *created_objects_ids.minted_coin()?;
    let coin_metadata_id = *created_objects_ids.coin_metadata()?;
    let max_supply_policy_id = *created_objects_ids.max_supply_policy()?;

    let package_object = created_objects
        .iter()
        .find(|object| object.id() == package_id)
        .ok_or(anyhow!("missing package object"))?;
    let coin_object = created_objects
        .iter()
        .find(|object| object.id() == coin_id)
        .ok_or(anyhow!("missing coin object"))?;
    let minted_coin_object = created_objects
        .iter()
        .find(|object| object.id() == minted_coin_id)
        .ok_or(anyhow!("missing minted coin object"))?;
    let coin_metadata_object = created_objects
        .iter()
        .find(|object| object.id() == coin_metadata_id)
        .ok_or(anyhow!("missing coin metadata object"))?;
    let max_supply_policy_object = created_objects
        .iter()
        .find(|object| object.id() == max_supply_policy_id)
        .ok_or(anyhow!("missing max supply policy object"))?;

    Ok((
        package_object.clone(),
        coin_object.clone(),
        minted_coin_object.clone(),
        coin_metadata_object.clone(),
        max_supply_policy_object.clone(),
    ))
}

#[test]
fn foundry_with_simple_metadata() -> Result<()> {
    let alias_id = AliasId::new(rand::random());
    let (header, foundry) = create_foundry(
        1_000_000,
        SimpleTokenScheme::new(U256::from(100_000), U256::from(0), U256::from(100_000_000))
            .unwrap(),
        Irc30Metadata::new("Dogecoin", "DOGE", 0),
        alias_id,
    )
    .unwrap();

    let (
        package_object,
        coin_object,
        minted_coin_object,
        coin_metadata_object,
        max_supply_policy_object,
    ) = migrate_foundry(header, foundry)?;

    // Check the package object.
    let type_origin_table = package_object
        .data
        .try_as_package()
        .expect("should be a package object")
        .type_origin_table();
    assert_eq!(type_origin_table.len(), 1);
    let coin_type_origin = type_origin_table[0].clone();
    assert_eq!(coin_type_origin.module_name, "doge");
    assert_eq!(coin_type_origin.struct_name, "DOGE");

    // Check the coin object.
    let coin = coin_object
        .as_coin_maybe()
        .expect("should be a coin object");
    assert_eq!(
        coin_object.owner.get_owner_address().unwrap().to_string(),
        alias_id.to_string()
    );
    assert_eq!(coin.balance, Balance::new(1_000_000));

    // Check the minted coin object.
    let minted_coin = minted_coin_object
        .as_coin_maybe()
        .expect("should be a coin object");
    assert_eq!(minted_coin_object.owner, IotaAddress::ZERO);
    assert_eq!(minted_coin.balance, Balance::new(100_000));

    // Check the coin metadata object.
    let coin_metadata = coin_metadata_object
        .data
        .try_as_move()
        .expect("should be a move object");

    let coin_metadata = CoinMetadata::from_bcs_bytes(coin_metadata.contents()).unwrap();
    assert_eq!(coin_metadata.decimals, 0);
    assert_eq!(coin_metadata.name, "Dogecoin");
    assert_eq!(coin_metadata.symbol, "doge");
    assert_eq!(coin_metadata.description, "");
    assert!(coin_metadata.icon_url.is_none());

    // Check the max supply policy object.
    let max_supply_policy = max_supply_policy_object
        .to_rust::<MaxSupplyPolicy>()
        .unwrap();

    assert_eq!(
        max_supply_policy_object.owner,
        stardust_to_iota_address_owner(alias_id).unwrap()
    );
    assert_eq!(max_supply_policy.maximum_supply, 100_000_000);

    let max_supply_policy_object = max_supply_policy_object.data.try_as_move().unwrap();
    let max_supply_policy_object_type = max_supply_policy_object.type_();
    assert_eq!(
        max_supply_policy_object_type.module().as_str(),
        "capped_coin"
    );
    assert_eq!(
        max_supply_policy_object_type.name().as_str(),
        "MaxSupplyPolicy"
    );

    let max_supply_policy_object_type_params =
        max_supply_policy_object_type.clone().into_type_params();
    assert_eq!(max_supply_policy_object_type_params.len(), 1);
    let TypeTag::Struct(type_tag) = &max_supply_policy_object_type_params[0] else {
        panic!("unexpected type tag")
    };
    assert_eq!(type_tag.module.as_str(), "doge");
    assert_eq!(type_tag.name.as_str(), "DOGE");
    assert_eq!(type_tag.type_params.len(), 0);
    assert!(max_supply_policy_object.has_public_transfer());

    Ok(())
}

/// Tests the migration of a foundry output with a metadata
/// containing non-ascii characters, overflowing circulating and maximum
/// supplies - basically not the usual cases.
#[test]
fn foundry_with_special_metadata() -> Result<()> {
    let alias_id = AliasId::new(rand::random());
    let (header, foundry) = create_foundry(
        1_000_000,
        SimpleTokenScheme::new(U256::from(u64::MAX), U256::from(0), U256::MAX).unwrap(),
        Irc30Metadata::new("Dogecoin", "DOGE❤", 123)
            .with_description("Much wow")
            .with_url(Url::parse("https://dogecoin.com").unwrap())
            .with_logo_url(Url::parse("https://dogecoin.com/logo.png").unwrap())
            .with_logo("0x54654"),
        alias_id,
    )
    .unwrap();

    let (
        package_object,
        coin_object,
        minted_coin_object,
        coin_metadata_object,
        max_supply_policy_object,
    ) = migrate_foundry(header, foundry)?;

    // Check the package object.
    let type_origin_table = package_object
        .data
        .try_as_package()
        .expect("should be a package object")
        .type_origin_table();
    assert_eq!(type_origin_table.len(), 1);
    let coin_type_origin = type_origin_table[0].clone();
    assert_eq!(coin_type_origin.module_name, "doge");
    assert_eq!(coin_type_origin.struct_name, "DOGE");

    // Check the coin object.
    let coin = coin_object
        .as_coin_maybe()
        .expect("should be a coin object");
    assert_eq!(
        coin_object.owner.get_owner_address().unwrap().to_string(),
        alias_id.to_string()
    );
    assert_eq!(coin.balance, Balance::new(1_000_000));

    // Check the minted coin object.
    let minted_coin = minted_coin_object
        .as_coin_maybe()
        .expect("should be a coin object");
    assert_eq!(minted_coin_object.owner, IotaAddress::ZERO);
    assert_eq!(minted_coin.balance, Balance::new(u64::MAX - 1));

    // Check the coin metadata object.
    let coin_metadata = coin_metadata_object
        .data
        .try_as_move()
        .expect("should be a move object");

    let coin_metadata = CoinMetadata::from_bcs_bytes(coin_metadata.contents()).unwrap();
    assert_eq!(coin_metadata.decimals, 123);
    assert_eq!(coin_metadata.name, "Dogecoin");
    assert_eq!(coin_metadata.symbol, "doge");
    assert_eq!(coin_metadata.description, "Much wow");
    assert_eq!(
        coin_metadata.icon_url.unwrap().to_string(),
        "https://dogecoin.com/logo.png"
    );

    // Check the max supply policy object.
    let max_supply_policy = max_supply_policy_object
        .to_rust::<MaxSupplyPolicy>()
        .unwrap();

    assert_eq!(
        max_supply_policy_object.owner,
        stardust_to_iota_address_owner(alias_id).unwrap()
    );
    assert_eq!(max_supply_policy.maximum_supply, u64::MAX - 1);

    let max_supply_policy_object = max_supply_policy_object.data.try_as_move().unwrap();
    let max_supply_policy_object_type = max_supply_policy_object.type_();
    assert_eq!(
        max_supply_policy_object_type.module().as_str(),
        "capped_coin"
    );
    assert_eq!(
        max_supply_policy_object_type.name().as_str(),
        "MaxSupplyPolicy"
    );

    let max_supply_policy_object_type_params =
        max_supply_policy_object_type.clone().into_type_params();
    assert_eq!(max_supply_policy_object_type_params.len(), 1);
    let TypeTag::Struct(type_tag) = &max_supply_policy_object_type_params[0] else {
        panic!("unexpected type tag")
    };
    assert_eq!(type_tag.module.as_str(), "doge");
    assert_eq!(type_tag.name.as_str(), "DOGE");
    assert_eq!(type_tag.type_params.len(), 0);
    assert!(max_supply_policy_object.has_public_transfer());

    Ok(())
}

#[test]
fn coin_ownership() -> Result<()> {
    let alias_id = AliasId::new(rand::random());
    let (header, foundry) = create_foundry(
        1_000_000,
        SimpleTokenScheme::new(U256::from(100_000), U256::from(0), U256::from(100_000_000))
            .unwrap(),
        Irc30Metadata::new("Dogecoin", "DOGE", 0),
        alias_id,
    )
    .unwrap();

    let (
        _package_object,
        coin_object,
        minted_coin_object,
        _coin_metadata_object,
        max_supply_policy_object,
    ) = migrate_foundry(header, foundry)?;

    // Check the owner of the coin object.
    assert_eq!(
        coin_object.owner.get_owner_address().unwrap().to_string(),
        alias_id.to_string()
    );

    // Check the owner of the minted coin object.
    assert_eq!(minted_coin_object.owner, IotaAddress::ZERO);

    // Check the owner of the max supply policy object.
    assert_eq!(
        max_supply_policy_object.owner,
        stardust_to_iota_address_owner(alias_id).unwrap()
    );

    Ok(())
}

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

    let (executor, _) =
        run_migration(1_000_000, [(foundry_header, foundry_output.into())]).unwrap();
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
        stardust_to_iota_address(alias_address).unwrap()
    );

    assert_eq!(
        *gas_coin_object.type_().unwrap(),
        MoveObjectType::gas_coin()
    );
    assert_eq!(gas_coin_object.coin_type_maybe().unwrap(), GAS::type_tag());
    assert_eq!(coin.value(), 1_000_000);
    assert_eq!(package_object.version(), gas_coin_object.version());
}
