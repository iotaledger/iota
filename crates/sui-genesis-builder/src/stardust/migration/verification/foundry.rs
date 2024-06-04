// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use anyhow::{anyhow, ensure, Result};
use iota_sdk::types::block::output::{FoundryOutput, TokenId};
use move_core_types::language_storage::ModuleId;
use sui_types::{
    base_types::SuiAddress, coin::CoinMetadata, in_memory_storage::InMemoryStorage, object::Owner,
    Identifier,
};

use crate::stardust::{
    migration::{
        executor::FoundryLedgerData,
        verification::{
            util::{truncate_to_max_allowed_u64_supply, verify_parent},
            CreatedObjects,
        },
    },
    native_token::package_data::NativeTokenPackageData,
    types::{capped_coin::MaxSupplyPolicy, token_scheme::SimpleTokenSchemeU64},
};

pub(super) fn verify_foundry_output(
    output: &FoundryOutput,
    created_objects: &CreatedObjects,
    foundry_data: &HashMap<TokenId, FoundryLedgerData>,
    storage: &InMemoryStorage,
) -> Result<()> {
    let foundry_data = foundry_data
        .get(&output.token_id())
        .ok_or_else(|| anyhow!("missing foundry data"))?;

    // Coin value.
    let created_coin = created_objects
        .coin()
        .and_then(|id| {
            storage
                .get_object(id)
                .ok_or_else(|| anyhow!("missing coin"))
        })?
        .as_coin_maybe()
        .ok_or_else(|| anyhow!("expected a coin"))?;
    ensure!(
        created_coin.value() == output.amount(),
        "coin amount mismatch: found {}, expected {}",
        created_coin.value(),
        output.amount()
    );

    // Minted coin value
    let minted_coin_id = created_objects.minted_coin()?;
    let minted_coin = storage
        .get_object(minted_coin_id)
        .ok_or_else(|| anyhow!("missing coin"))?
        .as_coin_maybe()
        .ok_or_else(|| anyhow!("expected a coin"))?;

    ensure!(
        foundry_data.minted_coin_id == *minted_coin_id,
        "coin ID mismatch: found {}, expected {}",
        foundry_data.minted_coin_id,
        minted_coin_id
    );

    ensure!(
        minted_coin.value() == foundry_data.minted_value,
        "minted coin amount mismatch: found {}, expected {}",
        minted_coin.value(),
        foundry_data.minted_value
    );

    // Package
    let package_id = created_objects.package()?;
    let created_package = storage
        .get_object(package_id)
        .ok_or_else(|| anyhow!("missing package"))?
        .data
        .try_as_package()
        .ok_or_else(|| anyhow!("expected a package"))?;

    ensure!(
        foundry_data.package_id == *package_id,
        "foundry data package ID mismatch: found {}, expected {}",
        foundry_data.package_id,
        package_id
    );

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
    ensure!(
        foundry_data.coin_type_origin.module_name == expected_package_data.module().module_name,
        "foundry data module name mismatch: found {}, expected {}",
        foundry_data.coin_type_origin.module_name,
        expected_package_data.module().module_name
    );
    ensure!(
        foundry_data.coin_type_origin.struct_name == expected_package_data.module().otw_name,
        "foundry data OTW struct name mismatch: found {}, expected {}",
        foundry_data.coin_type_origin.struct_name,
        expected_package_data.module().otw_name
    );

    // Adjusted Token Scheme
    let expected_token_scheme_u64 =
        SimpleTokenSchemeU64::try_from(output.token_scheme().as_simple())?;
    ensure!(
        expected_token_scheme_u64 == foundry_data.token_scheme_u64,
        "foundry data token scheme mismatch: found {:?}, expected: {:?}",
        foundry_data.token_scheme_u64,
        expected_token_scheme_u64
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
    let circulating_supply =
        truncate_to_max_allowed_u64_supply(output.token_scheme().as_simple().circulating_supply());
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

    verify_parent(
        output
            .unlock_conditions()
            .immutable_alias_address()
            .expect("foundry outputs always have an immutable alias address")
            .address(),
        storage,
    )?;

    Ok(())
}
