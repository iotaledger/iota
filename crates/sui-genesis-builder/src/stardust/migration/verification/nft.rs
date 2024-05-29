// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use crate::stardust::migration::{
    executor::FoundryLedgerData,
    verification::{
        util::{
            verify_expiration_unlock_condition, verify_issuer_feature, verify_metadata_feature,
            verify_native_tokens, verify_sender_feature, verify_storage_deposit_unlock_condition,
            verify_tag_feature, verify_timelock_unlock_condition,
        },
        {created_objects::CreatedObjects, util::verify_parent},
    },
};
use anyhow::{anyhow, ensure, Result};
use iota_sdk::types::block::output::{NftOutput, OutputId, TokenId};
use sui_types::{
    balance::Balance, base_types::ObjectID, dynamic_field::Field,
    in_memory_storage::InMemoryStorage,
};

pub fn verify_nft_output(
    output_id: OutputId,
    output: &NftOutput,
    created_objects: &CreatedObjects,
    _foundry_data: &HashMap<TokenId, FoundryLedgerData>,
    storage: &InMemoryStorage,
) -> anyhow::Result<()> {
    verify_parent(output.address(), storage)?;

    let created_output = created_objects
        .output()
        .and_then(|id| {
            storage
                .get_object(id)
                .ok_or_else(|| anyhow!("missing object"))
        })?
        .to_rust::<crate::stardust::types::NftOutput>()
        .ok_or_else(|| anyhow!("invalid nft output object"))?;

    let nft = storage
        .get_object(&ObjectID::new(*output.nft_id_non_null(&output_id)))
        .ok_or_else(|| anyhow!("missing object"))?
        .to_rust::<crate::stardust::types::Nft>()
        .ok_or_else(|| anyhow!("invalid nft object"))?;

    // Amount
    ensure!(
        created_output.iota.value() == output.amount(),
        "amount mismatch: found {}, expected {}",
        created_output.iota.value(),
        output.amount()
    );

    // Native Tokens
    ensure!(
        created_output.native_tokens.size == output.native_tokens().len() as u64,
        "native tokens bag length mismatch: found {}, expected {}",
        created_output.native_tokens.size,
        output.native_tokens().len()
    );
    let created_native_token_fields = created_objects.native_tokens().and_then(|ids| {
        ids.iter()
            .map(|id| {
                let obj = storage
                    .get_object(id)
                    .ok_or_else(|| anyhow!("missing native token field for {id}"))?;
                obj.to_rust::<Field<String, Balance>>().ok_or_else(|| {
                    anyhow!("expected a native token field, found {:?}", obj.type_())
                })
            })
            .collect::<Result<Vec<_>, _>>()
    })?;
    verify_native_tokens(output.native_tokens(), created_native_token_fields)?;

    // Storage Deposit Return Unlock Condition
    verify_storage_deposit_unlock_condition(
        output.unlock_conditions().storage_deposit_return(),
        created_output.storage_deposit_return.as_ref(),
    )?;

    // Timelock Unlock Condition
    verify_timelock_unlock_condition(
        output.unlock_conditions().timelock(),
        created_output.timelock.as_ref(),
    )?;

    // Expiration Unlock Condition
    verify_expiration_unlock_condition(
        output.unlock_conditions().expiration(),
        created_output.expiration.as_ref(),
        output.address(),
    )?;

    // Metadata Feature
    verify_metadata_feature(output.features().metadata(), nft.metadata.as_ref())?;

    // Tag Feature
    verify_tag_feature(output.features().tag(), nft.tag.as_ref())?;

    // Sender Feature
    verify_sender_feature(output.features().sender(), nft.legacy_sender)?;

    // Issuer Feature
    verify_issuer_feature(output.immutable_features().issuer(), nft.immutable_issuer)?;

    // Immutable Metadata Feature
    ensure!(
        crate::stardust::types::Nft::convert_immutable_metadata(output)? == nft.immutable_metadata,
        "metadata mismatch: found {:x?}, expected {:x?}",
        crate::stardust::types::Nft::convert_immutable_metadata(output)?,
        nft.immutable_metadata
    );

    ensure!(
        created_objects.package().is_err(),
        "unexpected package found"
    );

    Ok(())
}
