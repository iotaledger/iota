// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Result, anyhow};
use iota_protocol_config::ProtocolConfigValue::u64;
use iota_sdk::{
    U256, Url,
    types::block::output::{
        AliasId, FoundryOutput, Output, SimpleTokenScheme, feature::Irc30Metadata,
    },
};
use iota_types::{
    balance::Balance,
    base_types::{IotaAddress, MoveObjectType, ObjectID},
    coin::CoinMetadata,
    coin_manager::CoinManager,
    gas_coin::GAS,
    object::Object,
    smr_coin::{SMR, SmrCoin},
    stardust::{coin_type::CoinType, stardust_to_iota_address, stardust_to_iota_address_owner},
};
use move_core_types::language_storage::TypeTag;

use crate::stardust::{
    migration::tests::{create_foundry, run_migration},
    types::output_header::OutputHeader,
};

type PackageObject = Object;
type CoinObject = Object;
type NativeTokenCoinObject = Object;
type CoinManagerObject = Object;
type CoinManagerTreasuryCapObject = Object;

fn migrate_foundry(
    header: OutputHeader,
    foundry: FoundryOutput,
    coin_type: CoinType,
) -> Result<(
    PackageObject,
    CoinObject,
    NativeTokenCoinObject,
    CoinManagerObject,
    CoinManagerTreasuryCapObject,
    CoinMetadata,
)> {
    let output_id = header.output_id();

    let (executor, objects_map) = run_migration(
        foundry.amount(),
        [(header, Output::Foundry(foundry))],
        coin_type,
    )?;

    let created_objects_ids = objects_map
        .get(&output_id)
        .ok_or(anyhow!("missing created objects"))?;

    let created_objects = executor.into_objects();

    // Foundry package publication creates five objects
    //
    // * The package
    // * CoinManager
    // * CoinManagerTreasuryCap
    // * The total supply native token coin
    // * The coin held by the foundry which can be a gas coin or a smr coin
    assert_eq!(created_objects.len(), 5);

    let package_id = *created_objects_ids.package()?;
    let coin_id = *created_objects_ids.coin()?;
    let native_token_coin_id = *created_objects_ids.native_token_coin()?;
    let coin_manager_id = *created_objects_ids.coin_manager()?;
    let coin_manager_treasury_cap_id = *created_objects_ids.coin_manager_treasury_cap()?;

    let package_object = created_objects
        .iter()
        .find(|object| object.id() == package_id)
        .ok_or(anyhow!("missing package object"))?;
    let coin_object = created_objects
        .iter()
        .find(|object| object.id() == coin_id)
        .ok_or(anyhow!("missing coin object"))?;
    let native_token_coin_object = created_objects
        .iter()
        .find(|object| object.id() == native_token_coin_id)
        .ok_or(anyhow!("missing native token coin object"))?;
    let coin_manager_object = created_objects
        .iter()
        .find(|object| object.id() == coin_manager_id)
        .ok_or(anyhow!("missing coin manager object"))?;
    let coin_manager_treasury_cap_object = created_objects
        .iter()
        .find(|object| object.id() == coin_manager_treasury_cap_id)
        .ok_or(anyhow!("missing treasury cap object"))?;

    let coin_manager: CoinManager = coin_manager_object
        .to_rust()
        .ok_or(anyhow!("expected a coin manager"))?;

    let coin_metadata = coin_manager
        .metadata
        .ok_or(anyhow!("missing coin metadata"))?;

    Ok((
        package_object.clone(),
        coin_object.clone(),
        native_token_coin_object.clone(),
        coin_manager_object.clone(),
        coin_manager_treasury_cap_object.clone(),
        coin_metadata,
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
        native_token_coin_object,
        coin_manager_object,
        coin_manager_treasury_cap_object,
        coin_metadata,
    ) = migrate_foundry(header, foundry, CoinType::Iota)?;

    // Check the package object.
    let type_origin_table = package_object
        .data
        .try_as_package()
        .expect("should be a package object")
        .type_origin_table();
    assert_eq!(type_origin_table.len(), 1);
    let coin_type_origin = type_origin_table[0].clone();
    assert_eq!(coin_type_origin.module_name, "doge");
    assert_eq!(coin_type_origin.datatype_name, "DOGE");

    // Check the coin object.
    let coin = coin_object
        .as_coin_maybe()
        .expect("should be a coin object");
    assert_eq!(
        coin_object.owner.get_owner_address().unwrap().to_string(),
        alias_id.to_string()
    );
    assert_eq!(coin.balance, Balance::new(1_000_000));

    // Check the native token coin object.
    let native_token_coin = native_token_coin_object
        .as_coin_maybe()
        .expect("should be a native token coin object");
    assert_eq!(native_token_coin_object.owner, IotaAddress::ZERO);
    assert_eq!(native_token_coin.balance, Balance::new(100_000));

    // Check the coin metadata object.
    assert_eq!(coin_metadata.decimals, 0);
    assert_eq!(coin_metadata.name, "Dogecoin");
    assert_eq!(coin_metadata.symbol, "doge");
    assert_eq!(coin_metadata.description, "");
    assert!(coin_metadata.icon_url.is_none());

    // Check the CoinManagerTreasuryCap ownership
    assert_eq!(
        coin_manager_treasury_cap_object.owner,
        stardust_to_iota_address_owner(alias_id).unwrap()
    );

    // Check the CoinManager
    let coin_manager = coin_manager_object.to_rust::<CoinManager>().unwrap();

    assert_eq!(coin_manager.maximum_supply.unwrap(), 100_000_000);

    let coin_manager_object_type = coin_manager_object.type_().unwrap();
    assert_eq!(coin_manager_object_type.module().as_str(), "coin_manager");
    assert_eq!(coin_manager_object_type.name().as_str(), "CoinManager");

    let coin_manager_object_type_params = coin_manager_object_type.clone().into_type_params();
    assert_eq!(coin_manager_object_type_params.len(), 1);
    let TypeTag::Struct(type_tag) = &coin_manager_object_type_params[0] else {
        panic!("unexpected type tag")
    };
    assert_eq!(type_tag.module.as_str(), "doge");
    assert_eq!(type_tag.name.as_str(), "DOGE");
    assert_eq!(type_tag.type_params.len(), 0);

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
        native_token_coin_object,
        coin_manager_object,
        coin_manager_treasury_cap_object,
        coin_metadata,
    ) = migrate_foundry(header, foundry, CoinType::Iota)?;

    // Check the package object.
    let type_origin_table = package_object
        .data
        .try_as_package()
        .expect("should be a package object")
        .type_origin_table();
    assert_eq!(type_origin_table.len(), 1);
    let coin_type_origin = type_origin_table[0].clone();
    assert_eq!(coin_type_origin.module_name, "doge");
    assert_eq!(coin_type_origin.datatype_name, "DOGE");

    // Check the coin object.
    let coin = coin_object
        .as_coin_maybe()
        .expect("should be a coin object");
    assert_eq!(
        coin_object.owner.get_owner_address().unwrap().to_string(),
        alias_id.to_string()
    );
    assert_eq!(coin.balance, Balance::new(1_000_000));

    // Check the native token coin object.
    let native_token_coin = native_token_coin_object
        .as_coin_maybe()
        .expect("should be a native token coin object");
    assert_eq!(native_token_coin_object.owner, IotaAddress::ZERO);
    assert_eq!(native_token_coin.balance, Balance::new(u64::MAX - 1));

    // Check the coin metadata object.
    assert_eq!(coin_metadata.decimals, 123);
    assert_eq!(coin_metadata.name, "Dogecoin");
    assert_eq!(coin_metadata.symbol, "doge");
    assert_eq!(coin_metadata.description, "Much wow");
    assert_eq!(
        coin_metadata.icon_url.unwrap().to_string(),
        "https://dogecoin.com/logo.png"
    );

    // Check the CoinManagerTreasuryCap ownership
    assert_eq!(
        coin_manager_treasury_cap_object.owner,
        stardust_to_iota_address_owner(alias_id).unwrap()
    );

    // Check the CoinManager
    let coin_manager = coin_manager_object.to_rust::<CoinManager>().unwrap();

    assert_eq!(coin_manager.maximum_supply.unwrap(), u64::MAX - 1);

    let coin_manager_object_type = coin_manager_object.type_().unwrap();
    assert_eq!(coin_manager_object_type.module().as_str(), "coin_manager");
    assert_eq!(coin_manager_object_type.name().as_str(), "CoinManager");

    let coin_manager_object_type_params = coin_manager_object_type.clone().into_type_params();
    assert_eq!(coin_manager_object_type_params.len(), 1);
    let TypeTag::Struct(type_tag) = &coin_manager_object_type_params[0] else {
        panic!("unexpected type tag")
    };
    assert_eq!(type_tag.module.as_str(), "doge");
    assert_eq!(type_tag.name.as_str(), "DOGE");
    assert_eq!(type_tag.type_params.len(), 0);

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
        native_token_coin_object,
        coin_manager_object,
        coin_manager_treasury_cap_object,
        _coin_metadata_object,
    ) = migrate_foundry(header, foundry, CoinType::Iota)?;

    // Check the owner of the coin object.
    assert_eq!(
        coin_object.owner.get_owner_address().unwrap().to_string(),
        alias_id.to_string()
    );

    // Check the owner of the native token coin object.
    assert_eq!(native_token_coin_object.owner, IotaAddress::ZERO);

    // Check the owner of the coin manager object.
    assert!(coin_manager_object.is_shared());

    // Check if the coin manager object has a public transfer.
    assert!(
        coin_manager_object
            .data
            .try_as_move()
            .unwrap()
            .has_public_transfer()
    );

    // Check the owner of the treasury cap object.
    assert_eq!(
        coin_manager_treasury_cap_object.owner,
        stardust_to_iota_address_owner(alias_id).unwrap()
    );

    Ok(())
}

#[test]
fn create_gas_coin() -> Result<()> {
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

    let (
        package_object,
        gas_coin_object,
        _native_token_coin_object,
        _coin_manager_object,
        _coin_manager_treasury_cap_object,
        _coin_metadata_object,
    ) = migrate_foundry(foundry_header, foundry_output, CoinType::Iota)?;

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

    Ok(())
}

#[test]
fn create_smr_coin() -> Result<()> {
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

    let (
        package_object,
        smr_coin_object,
        _native_token_coin_object,
        _coin_manager_object,
        _coin_manager_treasury_cap_object,
        _coin_metadata_object,
    ) = migrate_foundry(foundry_header, foundry_output, CoinType::Shimmer)?;

    // Downcast the smr coin object to get the coin.
    let coin = smr_coin_object.to_rust::<SmrCoin>().unwrap();

    // Check if the gas coin id is the same as the output id.
    assert_eq!(smr_coin_object.id(), ObjectID::new(output_id.hash()));

    // Check if the owner of the gas coin is the package object.
    assert_eq!(
        smr_coin_object.owner.get_owner_address().unwrap(),
        stardust_to_iota_address(alias_address).unwrap()
    );

    assert!(SmrCoin::is_smr_coin(&smr_coin_object.struct_tag().unwrap()));
    assert_eq!(smr_coin_object.coin_type_maybe().unwrap(), SMR::type_tag());
    assert_eq!(coin.value(), 1_000_000);
    assert_eq!(package_object.version(), smr_coin_object.version());

    Ok(())
}
