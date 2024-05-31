// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use anyhow::{anyhow, ensure, Result};
use iota_sdk::types::block::output::{BasicOutput, TokenId};
use sui_types::{balance::Balance, dynamic_field::Field, in_memory_storage::InMemoryStorage};

use crate::stardust::migration::{
    executor::FoundryLedgerData,
    verification::{
        created_objects::CreatedObjects,
        util::{
            verify_expiration_unlock_condition, verify_metadata_feature, verify_native_tokens,
            verify_parent, verify_sender_feature, verify_storage_deposit_unlock_condition,
            verify_tag_feature, verify_timelock_unlock_condition,
        },
    },
};

pub(super) fn verify_basic_output(
    output: &BasicOutput,
    created_objects: &CreatedObjects,
    foundry_data: &HashMap<TokenId, FoundryLedgerData>,
    storage: &InMemoryStorage,
) -> Result<()> {
    // If the output has multiple unlock conditions, then a genesis object should
    // have been created.
    if output.unlock_conditions().len() > 1 {
        let created_output = created_objects
            .output()
            .and_then(|id| {
                storage
                    .get_object(id)
                    .ok_or_else(|| anyhow!("missing object"))
            })?
            .to_rust::<crate::stardust::types::output::BasicOutput>()
            .ok_or_else(|| anyhow!("invalid basic output object"))?;

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
        verify_native_tokens(
            output.native_tokens(),
            foundry_data,
            created_native_token_fields,
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
        verify_metadata_feature(
            output.features().metadata(),
            created_output.metadata.as_ref(),
        )?;

        // Tag Feature
        verify_tag_feature(output.features().tag(), created_output.tag.as_ref())?;

        // Sender Feature
        verify_sender_feature(output.features().sender(), created_output.sender)?;

    // Otherwise the output contains only an address unlock condition and only a
    // coin and possibly native tokens should have been created.
    } else {
        ensure!(
            created_objects.output().is_err(),
            "unexpected output object created for simple deposit"
        );

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

        // Native Tokens
        let created_native_token_coins = created_objects.native_tokens().and_then(|ids| {
            ids.iter()
                .map(|id| {
                    let obj = storage
                        .get_object(id)
                        .ok_or_else(|| anyhow!("missing native token coin for {id}"))?;
                    obj.coin_type_maybe()
                        .zip(obj.as_coin_maybe())
                        .ok_or_else(|| {
                            anyhow!("expected a native token coin, found {:?}", obj.type_())
                        })
                })
                .collect::<Result<Vec<_>, _>>()
        })?;
        verify_native_tokens(
            output.native_tokens(),
            foundry_data,
            created_native_token_coins,
        )?;
    }

    ensure!(
        created_objects.package().is_err(),
        "unexpected package found"
    );

    verify_parent(output.address(), storage)?;

    Ok(())
}
