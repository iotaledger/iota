// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use core::str::FromStr;

use crate::stardust::migration::verification::created_objects::CreatedObjects;
use crate::stardust::migration::verification::util::{
    verify_issuer_feature, verify_metadata_feature, verify_native_tokens, verify_sender_feature,
};
use crate::stardust::types::stardust_to_sui_address_owner;
use crate::stardust::types::{snapshot::OutputHeader, Alias, AliasOutput};
use anyhow::{anyhow, ensure};
use iota_sdk::types::block::output as stardust;
use sui_types::base_types::SuiAddress;
use sui_types::in_memory_storage::InMemoryStorage;
use sui_types::{base_types::ObjectID, id::UID};

pub fn verify_alias_output(
    header: &OutputHeader,
    output: &stardust::AliasOutput,
    created_objects: &CreatedObjects,
    storage: &InMemoryStorage,
) -> anyhow::Result<()> {
    let alias_id = ObjectID::new(*output.alias_id_non_null(&header.output_id()));

    let alias_output_obj = created_objects.output().and_then(|id| {
        storage
            .get_object(id)
            .ok_or_else(|| anyhow!("missing alias output object"))
    })?;

    let alias_obj = storage
        .get_object(&alias_id)
        .ok_or_else(|| anyhow!("missing alias object"))?;

    // Owner
    let expected_owner = stardust_to_sui_address_owner(output.governor_address())?;
    ensure!(
        alias_output_obj.owner == expected_owner,
        "alias output owner mismatch: found {}, expected {}",
        alias_output_obj.owner,
        expected_owner
    );
    ensure!(
        alias_obj.owner == expected_owner,
        "alias owner mismatch: found {}, expected {}",
        alias_obj.owner,
        expected_owner
    );

    let created_alias = alias_obj
        .to_rust::<Alias>()
        .ok_or_else(|| anyhow!("invalid alias object"))?;

    let created_alias_output = alias_output_obj
        .to_rust::<AliasOutput>()
        .ok_or_else(|| anyhow!("invalid alias output object"))?;

    // ID
    ensure!(
        created_alias.id.object_id() == &alias_id,
        "invalid alias id: found {}, expected {}",
        created_alias.id.object_id(),
        alias_id
    );
    ensure!(
        created_alias_output.id.object_id() != &alias_id,
        "invalid alias object id"
    );

    // Amount
    ensure!(
        created_alias_output.iota.value() == output.amount(),
        "coin amount mismatch: found {}, expected {}",
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

    ensure!(
        created_alias_output.native_tokens.size == output.native_tokens().len() as u64,
        "native token count mismatch: found {}, expected {}",
        created_alias_output.native_tokens.size,
        output.native_tokens().len()
    );

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
    ensure!(
        created_alias.state_metadata.is_some(),
        "missing state metadata for object: {}",
        created_alias.id.object_id()
    );

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
