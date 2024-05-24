// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, ensure, Result};
use iota_sdk::types::block::output::FoundryOutput;
use move_core_types::language_storage::ModuleId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sui_types::{
    base_types::SuiAddress,
    coin::{CoinMetadata, TreasuryCap},
    in_memory_storage::InMemoryStorage,
    object::Owner,
    Identifier,
};

use crate::stardust::{
    migration::verification::util::truncate_u256_to_u64,
    native_token::package_data::NativeTokenPackageData,
};

use super::created_objects::CreatedObjects;

pub fn verify_foundry_output(
    output: &FoundryOutput,
    created_objects: &CreatedObjects,
    storage: &InMemoryStorage,
) -> Result<()> {
    // Minted coin value
    let minted_coin = created_objects
        .coin()
        .and_then(|id| {
            storage
                .get_object(id)
                .ok_or_else(|| anyhow!("missing coin"))
        })?
        .as_coin_maybe()
        .ok_or_else(|| anyhow!("expected a coin"))?;

    let circulating_supply =
        truncate_u256_to_u64(output.token_scheme().as_simple().circulating_supply());
    ensure!(
        minted_coin.value() == circulating_supply,
        "coin amount mismatch: found {}, expected {}",
        minted_coin.value(),
        circulating_supply
    );

    // Package
    let created_package = created_objects
        .package()
        .and_then(|id| {
            storage
                .get_object(id)
                .ok_or_else(|| anyhow!("missing package"))
        })?
        .data
        .try_as_package()
        .ok_or_else(|| anyhow!("expected a package"))?;

    let expected_package_data = NativeTokenPackageData::try_from(output)?;

    let module_id = ModuleId::new(
        created_package.id().into(),
        Identifier::new(expected_package_data.module().module_name.as_ref())?,
    );

    ensure!(
        created_package.get_module(&module_id).is_some(),
        "package did not create expected module `{}`",
        expected_package_data.module().module_name
    );

    let type_origin_map = created_package.type_origin_map();

    ensure!(
        type_origin_map.contains_key(&(
            expected_package_data.module().module_name.clone(),
            expected_package_data.module().otw_name.clone()
        )),
        "package did not create expected OTW type `{}` within module `{}`",
        expected_package_data.module().otw_name,
        expected_package_data.module().module_name,
    );

    // Coin Metadata
    let minted_coin = created_objects
        .coin_metadata()
        .and_then(|id| {
            storage
                .get_object(id)
                .ok_or_else(|| anyhow!("missing coin metadata"))
        })?
        .to_rust::<CoinMetadata>()
        .ok_or_else(|| anyhow!("expected a coin metadata"))?;

    ensure!(
        minted_coin.decimals == expected_package_data.module().decimals,
        "coin decimals mismatch: expected {}, found {}",
        expected_package_data.module().decimals,
        minted_coin.decimals
    );
    ensure!(
        minted_coin.name == expected_package_data.module().coin_name,
        "coin name mismatch: expected {}, found {}",
        expected_package_data.module().coin_name,
        minted_coin.name
    );
    ensure!(
        minted_coin.symbol == expected_package_data.module().symbol,
        "coin symbol mismatch: expected {}, found {}",
        expected_package_data.module().symbol,
        minted_coin.symbol
    );
    ensure!(
        minted_coin.description == expected_package_data.module().coin_description,
        "coin description mismatch: expected {}, found {}",
        expected_package_data.module().coin_description,
        minted_coin.description
    );
    ensure!(
        minted_coin.icon_url
            == expected_package_data
                .module()
                .icon_url
                .as_ref()
                .map(|u| u.to_string()),
        "coin icon url mismatch: expected {:?}, found {:?}",
        expected_package_data.module().icon_url,
        minted_coin.icon_url
    );

    #[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, JsonSchema)]
    struct MaxSupplyPolicy {
        maximum_supply: u64,
        treasury_cap: TreasuryCap,
    }

    // Maximum Supply
    let max_supply_policy_obj = created_objects.max_supply_policy().and_then(|id| {
        storage
            .get_object(id)
            .ok_or_else(|| anyhow!("missing max supply policy"))
    })?;
    let max_supply_policy = max_supply_policy_obj
        .to_rust::<MaxSupplyPolicy>()
        .ok_or_else(|| anyhow!("expected a max supply policy"))?;

    ensure!(
        max_supply_policy.maximum_supply == expected_package_data.module().maximum_supply,
        "maximum supply mismatch: expected {}, found {}",
        expected_package_data.module().maximum_supply,
        max_supply_policy.maximum_supply
    );
    ensure!(
        max_supply_policy.treasury_cap.total_supply.value == circulating_supply,
        "treasury total supply mismatch: found {}, expected {}",
        max_supply_policy.treasury_cap.total_supply.value,
        circulating_supply
    );

    // Alias Address Unlock Condition
    let alias_address = output.alias_address().to_string().parse::<SuiAddress>()?;
    ensure!(
        max_supply_policy_obj.owner == Owner::AddressOwner(alias_address),
        "unexpected max supply policy owner: expected {}, found {}",
        alias_address,
        max_supply_policy_obj.owner
    );

    Ok(())
}
