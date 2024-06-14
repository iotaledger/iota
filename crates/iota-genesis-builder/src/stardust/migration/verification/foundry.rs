// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use anyhow::{anyhow, ensure, Result};
use iota_sdk::types::block::output::{FoundryOutput, TokenId};
use iota_types::{
    base_types::IotaAddress, coin::CoinMetadata, in_memory_storage::InMemoryStorage, object::Owner,
    Identifier,
};
use move_core_types::language_storage::ModuleId;

use crate::stardust::{
    migration::{
        executor::FoundryLedgerData,
        verification::{
            util::{
                truncate_to_max_allowed_u64_supply, verify_address_owner, verify_coin,
                verify_parent,
            },
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
    total_value: &mut u64,
) -> Result<()> {
    let foundry_data = foundry_data
        .get(&output.token_id())
        .ok_or_else(|| anyhow!("missing foundry data"))?;

    let alias_address = output
        .unlock_conditions()
        .immutable_alias_address()
        .expect("foundry outputs always have an immutable alias address")
        .address();

    // Gas coin value and owner
    let created_gas_coin_obj = created_objects.gas_coin().and_then(|id| {
        storage
            .get_object(id)
            .ok_or_else(|| anyhow!("missing gas coin"))
    })?;
    let created_gas_coin = created_gas_coin_obj
        .as_coin_maybe()
        .ok_or_else(|| anyhow!("expected a gas coin"))?;

    verify_address_owner(alias_address, created_gas_coin_obj, "gas coin")?;
    verify_coin(output.amount(), &created_gas_coin)?;
    *total_value += created_gas_coin.value();

    // Native token coin value
    let native_token_coin_id = created_objects.native_token_coin()?;
    let native_token_coin_obj = storage
        .get_object(native_token_coin_id)
        .ok_or_else(|| anyhow!("missing native token coin"))?;
    let native_token_coin = native_token_coin_obj
        .as_coin_maybe()
        .ok_or_else(|| anyhow!("expected a native token coin"))?;

    // The minted native token coin should be owned by `0x0`
    let expected_owner = Owner::AddressOwner(IotaAddress::default());
    ensure!(
        native_token_coin_obj.owner == expected_owner,
        "native token coin owner mismatch: found {}, expected {}",
        native_token_coin_obj.owner,
        expected_owner
    );

    ensure!(
        foundry_data.native_token_coin_id == *native_token_coin_id,
        "coin ID mismatch: found {}, expected {}",
        foundry_data.native_token_coin_id,
        native_token_coin_id
    );

    ensure!(
        native_token_coin.value() == foundry_data.minted_value,
        "minted coin amount mismatch: found {}, expected {}",
        native_token_coin.value(),
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
    let coin_metadata = created_objects
        .coin_metadata()
        .and_then(|id| {
            storage
                .get_object(id)
                .ok_or_else(|| anyhow!("missing coin metadata"))
        })?
        .to_rust::<CoinMetadata>()
        .ok_or_else(|| anyhow!("expected a coin metadata"))?;

    ensure!(
        coin_metadata.decimals == expected_package_data.module().decimals,
        "coin decimals mismatch: expected {}, found {}",
        expected_package_data.module().decimals,
        coin_metadata.decimals
    );
    ensure!(
        coin_metadata.name == expected_package_data.module().coin_name,
        "coin name mismatch: expected {}, found {}",
        expected_package_data.module().coin_name,
        coin_metadata.name
    );
    ensure!(
        coin_metadata.symbol == expected_package_data.module().symbol,
        "coin symbol mismatch: expected {}, found {}",
        expected_package_data.module().symbol,
        coin_metadata.symbol
    );
    ensure!(
        coin_metadata.description == expected_package_data.module().coin_description,
        "coin description mismatch: expected {}, found {}",
        expected_package_data.module().coin_description,
        coin_metadata.description
    );
    ensure!(
        coin_metadata.icon_url
            == expected_package_data
                .module()
                .icon_url
                .as_ref()
                .map(|u| u.to_string()),
        "coin icon url mismatch: expected {:?}, found {:?}",
        expected_package_data.module().icon_url,
        coin_metadata.icon_url
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
    verify_address_owner(alias_address, max_supply_policy_obj, "max supply policy")?;

    verify_parent(alias_address, storage)?;

    ensure!(
        created_objects.output().is_err(),
        "unexpected output object found"
    );

    Ok(())
}
