// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use anyhow::{anyhow, bail, ensure};
use iota_sdk::types::block::output as stardust;
use iota_types::{
    TypeTag,
    balance::Balance,
    base_types::{IotaAddress, ObjectID},
    dynamic_field::{DynamicFieldInfo, Field, derive_dynamic_field_id},
    in_memory_storage::InMemoryStorage,
    object::Owner,
    stardust::output::{
        ALIAS_DYNAMIC_OBJECT_FIELD_KEY, ALIAS_DYNAMIC_OBJECT_FIELD_KEY_TYPE, Alias, AliasOutput,
    },
};

use crate::stardust::{
    migration::{
        executor::FoundryLedgerData,
        verification::{
            created_objects::CreatedObjects,
            util::{
                TokensAmountCounter, verify_address_owner, verify_issuer_feature,
                verify_metadata_feature, verify_native_tokens, verify_parent,
                verify_sender_feature,
            },
        },
    },
    types::address_swap_map::AddressSwapMap,
};

pub(super) fn verify_alias_output(
    output_id: stardust::OutputId,
    output: &stardust::AliasOutput,
    created_objects: &CreatedObjects,
    foundry_data: &HashMap<stardust::TokenId, FoundryLedgerData>,
    storage: &InMemoryStorage,
    tokens_counter: &mut TokensAmountCounter,
    address_swap_map: &AddressSwapMap,
) -> anyhow::Result<()> {
    let alias_id = ObjectID::new(*output.alias_id_non_null(&output_id));

    let created_output_obj = created_objects.output().and_then(|id| {
        storage
            .get_object(id)
            .ok_or_else(|| anyhow!("missing alias output object"))
    })?;

    let created_alias_obj = storage
        .get_object(&alias_id)
        .ok_or_else(|| anyhow!("missing alias object"))?;

    // Alias Output Owner
    verify_address_owner(
        output.governor_address(),
        created_output_obj,
        "alias output",
        address_swap_map,
    )?;

    // Alias Owner
    let expected_alias_owner = Owner::ObjectOwner(
        derive_dynamic_field_id(
            created_output_obj.id(),
            &DynamicFieldInfo::dynamic_object_field_wrapper(
                ALIAS_DYNAMIC_OBJECT_FIELD_KEY_TYPE.parse::<TypeTag>()?,
            )
            .into(),
            &bcs::to_bytes(ALIAS_DYNAMIC_OBJECT_FIELD_KEY)?,
        )?
        .into(),
    );

    ensure!(
        created_alias_obj.owner == expected_alias_owner,
        "alias owner mismatch: found {}, expected {}",
        created_alias_obj.owner,
        expected_alias_owner
    );

    let created_alias = created_alias_obj
        .to_rust::<Alias>()
        .ok_or_else(|| anyhow!("invalid alias object"))?;

    let created_output = created_output_obj
        .to_rust::<AliasOutput>()
        .ok_or_else(|| anyhow!("invalid alias output object"))?;

    // Amount
    ensure!(
        created_output.balance.value() == output.amount(),
        "amount mismatch: found {}, expected {}",
        created_output.balance.value(),
        output.amount()
    );
    tokens_counter.update_total_value_for_iota(created_output.balance.value());

    // Native Tokens
    verify_native_tokens::<Field<String, Balance>>(
        output.native_tokens(),
        foundry_data,
        created_output.native_tokens,
        created_objects.native_tokens().ok(),
        storage,
        tokens_counter,
    )?;

    // Legacy State Controller
    let expected_state_controller = output
        .state_controller_address()
        .to_string()
        .parse::<IotaAddress>()?;
    ensure!(
        created_alias.legacy_state_controller == expected_state_controller,
        "legacy state controller mismatch: found {}, expected {}",
        created_alias.legacy_state_controller,
        expected_state_controller
    );

    // State Index
    ensure!(
        created_alias.state_index == output.state_index(),
        "state index mismatch: found {}, expected {}",
        created_alias.state_index,
        output.state_index()
    );

    // State Metadata
    if output.state_metadata().is_empty() {
        ensure!(
            created_alias.state_metadata.is_none(),
            "unexpected state metadata found"
        );
    } else {
        let Some(state_metadata) = created_alias.state_metadata.as_ref() else {
            bail!("missing state metadata")
        };

        ensure!(
            state_metadata.as_slice() == output.state_metadata(),
            "state metadata mismatch: found {:?}, expected {:?}",
            state_metadata,
            output.state_metadata()
        );
    }

    // Sender Feature
    verify_sender_feature(output.features().sender(), created_alias.sender)?;

    // Metadata Feature
    verify_metadata_feature(
        output.features().metadata(),
        created_alias.metadata.as_ref(),
    )?;

    // Immutable Issuer Feature
    verify_issuer_feature(
        output.immutable_features().issuer(),
        created_alias.immutable_issuer,
    )?;

    // Immutable Metadata Feature
    verify_metadata_feature(
        output.immutable_features().metadata(),
        created_alias.immutable_metadata.as_ref(),
    )?;

    verify_parent(&output_id, output.governor_address(), storage)?;

    ensure!(created_objects.coin().is_err(), "unexpected coin found");

    ensure!(
        created_objects.native_token_coin().is_err(),
        "unexpected native token coin found"
    );

    ensure!(
        created_objects.coin_manager().is_err(),
        "unexpected coin manager found"
    );

    ensure!(
        created_objects.coin_manager_treasury_cap().is_err(),
        "unexpected coin manager treasury cap found"
    );

    ensure!(
        created_objects.package().is_err(),
        "unexpected package found"
    );

    Ok(())
}
