// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use anyhow::{Result, anyhow, ensure};
use iota_sdk::types::block::output::{BasicOutput, OutputId, TokenId};
use iota_types::{
    TypeTag,
    balance::Balance,
    coin::Coin,
    dynamic_field::Field,
    in_memory_storage::InMemoryStorage,
    object::Owner,
    timelock::{
        stardust_upgrade_label::STARDUST_UPGRADE_LABEL_VALUE,
        timelock::{TimeLock, is_timelocked_vested_reward},
    },
};

use crate::stardust::{
    migration::{
        executor::FoundryLedgerData,
        verification::{
            created_objects::CreatedObjects,
            util::{
                TokensAmountCounter, verify_address_owner, verify_coin,
                verify_expiration_unlock_condition, verify_metadata_feature, verify_native_tokens,
                verify_parent, verify_sender_feature, verify_storage_deposit_unlock_condition,
                verify_tag_feature, verify_timelock_unlock_condition,
            },
        },
    },
    types::address_swap_map::AddressSwapMap,
};

pub(super) fn verify_basic_output(
    output_id: OutputId,
    output: &BasicOutput,
    created_objects: &CreatedObjects,
    foundry_data: &HashMap<TokenId, FoundryLedgerData>,
    target_milestone_timestamp: u32,
    storage: &InMemoryStorage,
    tokens_counter: &mut TokensAmountCounter,
    address_swap_map: &AddressSwapMap,
) -> Result<()> {
    // If this is a timelocked vested reward, a `Timelock<Balance>` is created.
    if is_timelocked_vested_reward(output_id, output, target_milestone_timestamp) {
        let created_timelock = created_objects
            .output()
            .and_then(|id| {
                storage
                    .get_object(id)
                    .ok_or_else(|| anyhow!("missing timelock object"))
            })?
            .to_rust::<TimeLock<Balance>>()
            .ok_or_else(|| anyhow!("invalid timelock object"))?;

        // Locked timestamp
        let output_timelock_timestamp =
            output.unlock_conditions().timelock().unwrap().timestamp() as u64 * 1000;
        ensure!(
            created_timelock.expiration_timestamp_ms() == output_timelock_timestamp,
            "timelock timestamp mismatch: found {}, expected {}",
            created_timelock.expiration_timestamp_ms(),
            output_timelock_timestamp
        );

        // Amount
        ensure!(
            created_timelock.locked().value() == output.amount(),
            "locked amount mismatch: found {}, expected {}",
            created_timelock.locked().value(),
            output.amount()
        );
        tokens_counter.update_total_value_for_iota(created_timelock.locked().value());

        // Label
        let label = created_timelock
            .label()
            .as_ref()
            .ok_or_else(|| anyhow!("timelock label must be initialized"))?;
        let expected_label = STARDUST_UPGRADE_LABEL_VALUE;

        ensure!(
            label == expected_label,
            "timelock label mismatch: found {}, expected {}",
            label,
            expected_label
        );

        return Ok(());
    }

    // If the output has multiple unlock conditions or a metadata, tag or sender
    // feature, then a genesis object should have been created.
    if output.unlock_conditions().expiration().is_some()
        || output
            .unlock_conditions()
            .storage_deposit_return()
            .is_some()
        || output
            .unlock_conditions()
            .is_time_locked(target_milestone_timestamp)
        || !output.features().is_empty()
    {
        ensure!(created_objects.coin().is_err(), "unexpected coin created");

        let created_output_obj = created_objects.output().and_then(|id| {
            storage
                .get_object(id)
                .ok_or_else(|| anyhow!("missing basic output object"))
        })?;
        let created_output = created_output_obj
            .to_rust::<iota_types::stardust::output::BasicOutput>()
            .ok_or_else(|| anyhow!("invalid basic output object"))?;

        // Owner
        // If there is an expiration unlock condition, the output is shared.
        if output.unlock_conditions().expiration().is_some() {
            ensure!(
                matches!(created_output_obj.owner, Owner::Shared { .. }),
                "basic output owner mismatch: found {:?}, expected Shared",
                created_output_obj.owner,
            );
        } else {
            verify_address_owner(
                output.address(),
                created_output_obj,
                "basic output",
                address_swap_map,
            )?;
        }

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
        verify_metadata_feature(
            output.features().metadata(),
            created_output.metadata.as_ref(),
        )?;

        // Tag Feature
        verify_tag_feature(output.features().tag(), created_output.tag.as_ref())?;

        // Sender Feature
        verify_sender_feature(output.features().sender(), created_output.sender)?;

    // Otherwise the output contains only an address unlock condition and
    // only a coin and possibly native tokens should have been
    // created.
    } else {
        ensure!(
            created_objects.output().is_err(),
            "unexpected output object created for simple deposit"
        );

        // Gas coin value and owner
        let created_coin_obj = created_objects.coin().and_then(|id| {
            storage
                .get_object(id)
                .ok_or_else(|| anyhow!("missing coin"))
        })?;
        let created_coin = created_coin_obj
            .as_coin_maybe()
            .ok_or_else(|| anyhow!("expected a coin"))?;

        verify_address_owner(output.address(), created_coin_obj, "coin", address_swap_map)?;
        verify_coin(output.amount(), &created_coin)?;
        tokens_counter.update_total_value_for_iota(created_coin.value());

        // Native Tokens
        verify_native_tokens::<(TypeTag, Coin)>(
            output.native_tokens(),
            foundry_data,
            None,
            created_objects.native_tokens().ok(),
            storage,
            tokens_counter,
        )?;
    }

    verify_parent(&output_id, output.address(), storage)?;

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
        "unexpected coin manager cap found"
    );

    ensure!(
        created_objects.package().is_err(),
        "unexpected package found"
    );

    Ok(())
}
