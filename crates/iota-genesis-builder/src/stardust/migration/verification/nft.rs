// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use anyhow::{anyhow, ensure};
use iota_sdk::types::block::output::{NftOutput, OutputId, TokenId};
use iota_types::{
    TypeTag,
    balance::Balance,
    base_types::ObjectID,
    dynamic_field::{DynamicFieldInfo, Field, derive_dynamic_field_id},
    in_memory_storage::InMemoryStorage,
    object::Owner,
    stardust::output::{NFT_DYNAMIC_OBJECT_FIELD_KEY, NFT_DYNAMIC_OBJECT_FIELD_KEY_TYPE},
};

use crate::stardust::{
    migration::{
        executor::FoundryLedgerData,
        verification::{
            created_objects::CreatedObjects,
            util::{
                TokensAmountCounter, verify_address_owner, verify_expiration_unlock_condition,
                verify_issuer_feature, verify_metadata_feature, verify_native_tokens,
                verify_parent, verify_sender_feature, verify_storage_deposit_unlock_condition,
                verify_tag_feature, verify_timelock_unlock_condition,
            },
        },
    },
    types::address_swap_map::AddressSwapMap,
};

pub(super) fn verify_nft_output(
    output_id: OutputId,
    output: &NftOutput,
    created_objects: &CreatedObjects,
    foundry_data: &HashMap<TokenId, FoundryLedgerData>,
    storage: &InMemoryStorage,
    tokens_counter: &mut TokensAmountCounter,
    address_swap_map: &AddressSwapMap,
) -> anyhow::Result<()> {
    let created_output_obj = created_objects.output().and_then(|id| {
        storage
            .get_object(id)
            .ok_or_else(|| anyhow!("missing nft output object for {output_id}"))
    })?;
    let created_output = created_output_obj
        .to_rust::<iota_types::stardust::output::NftOutput>()
        .ok_or_else(|| anyhow!("invalid nft output object for {output_id}"))?;

    let created_nft_obj = storage
        .get_object(&ObjectID::new(*output.nft_id_non_null(&output_id)))
        .ok_or_else(|| anyhow!("missing nft object for {output_id}"))?;
    let created_nft = created_nft_obj
        .to_rust::<iota_types::stardust::output::Nft>()
        .ok_or_else(|| anyhow!("invalid nft object for {output_id}"))?;

    // Output Owner
    // If there is an expiration unlock condition, the NFT is shared.
    if output.unlock_conditions().expiration().is_some() {
        ensure!(
            matches!(created_output_obj.owner, Owner::Shared { .. }),
            "nft output owner mismatch: found {:?}, expected Shared",
            created_output_obj.owner,
        );
    } else {
        verify_address_owner(
            output.address(),
            created_output_obj,
            "nft output",
            address_swap_map,
        )?;
    }

    // NFT Owner
    let expected_nft_owner = Owner::ObjectOwner(
        derive_dynamic_field_id(
            created_output_obj.id(),
            &DynamicFieldInfo::dynamic_object_field_wrapper(
                NFT_DYNAMIC_OBJECT_FIELD_KEY_TYPE.parse::<TypeTag>()?,
            )
            .into(),
            &bcs::to_bytes(NFT_DYNAMIC_OBJECT_FIELD_KEY)?,
        )?
        .into(),
    );

    ensure!(
        created_nft_obj.owner == expected_nft_owner,
        "nft owner mismatch: found {}, expected {}",
        created_nft_obj.owner,
        expected_nft_owner
    );

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
    verify_metadata_feature(output.features().metadata(), created_nft.metadata.as_ref())?;

    // Tag Feature
    verify_tag_feature(output.features().tag(), created_nft.tag.as_ref())?;

    // Sender Feature
    verify_sender_feature(output.features().sender(), created_nft.legacy_sender)?;

    // Issuer Feature
    verify_issuer_feature(
        output.immutable_features().issuer(),
        created_nft.immutable_issuer,
    )?;

    // Immutable Metadata Feature
    ensure!(
        iota_types::stardust::output::Nft::convert_immutable_metadata(output)?
            == created_nft.immutable_metadata,
        "metadata mismatch: found {:x?}, expected {:x?}",
        iota_types::stardust::output::Nft::convert_immutable_metadata(output)?,
        created_nft.immutable_metadata
    );

    verify_parent(&output_id, output.address(), storage)?;

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
