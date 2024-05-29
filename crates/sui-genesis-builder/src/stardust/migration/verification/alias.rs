// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::stardust::migration::verification::created_objects::CreatedObjects;
use crate::stardust::migration::verification::util::{
    verify_issuer_feature, verify_metadata_feature, verify_native_tokens, verify_sender_feature,
};
use crate::stardust::types::{
    stardust_to_sui_address_owner, ALIAS_DYNAMIC_OBJECT_FIELD_KEY,
    ALIAS_DYNAMIC_OBJECT_FIELD_KEY_TYPE,
};
use crate::stardust::types::{Alias, AliasOutput};
use anyhow::{anyhow, bail, ensure};
use iota_sdk::types::block::output::{self as stardust, OutputId};
use sui_types::base_types::ObjectID;
use sui_types::base_types::SuiAddress;
use sui_types::dynamic_field::{derive_dynamic_field_id, DynamicFieldInfo};
use sui_types::in_memory_storage::InMemoryStorage;
use sui_types::object::Owner;
use sui_types::TypeTag;

pub fn verify_alias_output(
    output_id: &OutputId,
    output: &stardust::AliasOutput,
    created_objects: &CreatedObjects,
    storage: &InMemoryStorage,
) -> anyhow::Result<()> {
    let alias_id = ObjectID::new(*output.alias_id_non_null(output_id));

    let alias_output_obj = created_objects.output().and_then(|id| {
        storage
            .get_object(id)
            .ok_or_else(|| anyhow!("missing alias output object"))
    })?;

    let alias_obj = storage
        .get_object(&alias_id)
        .ok_or_else(|| anyhow!("missing alias object"))?;

    // Alias Output Owner
    let expected_alias_output_owner = stardust_to_sui_address_owner(output.governor_address())?;
    ensure!(
        alias_output_obj.owner == expected_alias_output_owner,
        "alias output owner mismatch: found {}, expected {}",
        alias_output_obj.owner,
        expected_alias_output_owner
    );

    //  Alias Owner
    let expected_alias_owner = Owner::ObjectOwner(
        derive_dynamic_field_id(
            alias_output_obj.id(),
            &DynamicFieldInfo::dynamic_object_field_wrapper(
                ALIAS_DYNAMIC_OBJECT_FIELD_KEY_TYPE.parse::<TypeTag>()?,
            )
            .into(),
            &bcs::to_bytes(ALIAS_DYNAMIC_OBJECT_FIELD_KEY)?,
        )?
        .into(),
    );

    ensure!(
        alias_obj.owner == expected_alias_owner,
        "alias owner mismatch: found {}, expected {}",
        alias_obj.owner,
        expected_alias_owner
    );

    let created_alias = alias_obj
        .to_rust::<Alias>()
        .ok_or_else(|| anyhow!("invalid alias object"))?;

    let created_alias_output = alias_output_obj
        .to_rust::<AliasOutput>()
        .ok_or_else(|| anyhow!("invalid alias output object"))?;

    // Amount
    ensure!(
        created_alias_output.iota.value() == output.amount(),
        "amount mismatch: found {}, expected {}",
        created_alias_output.iota.value(),
        output.amount()
    );

    // Native Tokens
    let created_native_token_coins = created_objects.native_tokens().and_then(|ids| {
        ids.iter()
            .map(|id| {
                let obj = storage
                    .get_object(id)
                    .ok_or_else(|| anyhow!("missing native token coin for {id}"))?;
                obj.as_coin_maybe()
                    .ok_or_else(|| anyhow!("expected a native token coin, found {:?}", obj.type_()))
            })
            .collect::<Result<Vec<_>, _>>()
    })?;
    verify_native_tokens(output.native_tokens(), created_native_token_coins)?;

    // Legacy State Controller
    let expected_state_controller = output
        .state_controller_address()
        .to_string()
        .parse::<SuiAddress>()?;
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

    ensure!(created_objects.coin().is_err(), "unexpected coin found");

    ensure!(
        created_objects.package().is_err(),
        "unexpected package found"
    );

    Ok(())
}
