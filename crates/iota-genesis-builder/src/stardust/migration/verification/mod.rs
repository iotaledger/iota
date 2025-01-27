// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The `verification` module contains the validation logic to make sure that
//! the stardust outputs are correctly converted to the move objects.

use std::collections::HashMap;

use anyhow::{anyhow, ensure};
use iota_sdk::types::block::output::{Output, OutputId, TokenId};
use iota_types::in_memory_storage::InMemoryStorage;
use tracing::warn;
use util::{BASE_TOKEN_KEY, TokensAmountCounter};

use self::created_objects::CreatedObjects;
use crate::stardust::{
    migration::executor::FoundryLedgerData,
    types::{address_swap_map::AddressSwapMap, output_header::OutputHeader},
};

pub mod alias;
pub mod basic;
pub mod created_objects;
pub mod foundry;
pub mod nft;
mod util;

pub(crate) fn verify_outputs<'a>(
    outputs: impl IntoIterator<Item = &'a (OutputHeader, Output)>,
    output_objects_map: &HashMap<OutputId, CreatedObjects>,
    foundry_data: &HashMap<TokenId, FoundryLedgerData>,
    target_milestone_timestamp: u32,
    total_supply: u64,
    storage: &InMemoryStorage,
    address_swap_map: &AddressSwapMap,
) -> anyhow::Result<()> {
    let mut tokens_counter = TokensAmountCounter::new(total_supply);
    for (header, output) in outputs {
        let created_objects = output_objects_map
            .get(&header.output_id())
            .ok_or_else(|| anyhow!("missing created objects for output {}", header.output_id()))?;
        verify_output(
            header,
            output,
            created_objects,
            foundry_data,
            target_milestone_timestamp,
            storage,
            &mut tokens_counter,
            address_swap_map,
        )?;
    }
    for (key, (total_value, expected_value)) in tokens_counter.into_inner() {
        if key == BASE_TOKEN_KEY {
            ensure!(
                total_value == expected_value,
                "base token total supply: found {total_value}, expected {expected_value}"
            )
        } else if expected_value != total_value {
            warn!(
                "total supply mismatch for {key}: found {total_value}, expected {expected_value}"
            );
        }
    }
    Ok(())
}

fn verify_output(
    header: &OutputHeader,
    output: &Output,
    created_objects: &CreatedObjects,
    foundry_data: &HashMap<TokenId, FoundryLedgerData>,
    target_milestone_timestamp: u32,
    storage: &InMemoryStorage,
    tokens_counter: &mut TokensAmountCounter,
    address_swap_map: &AddressSwapMap,
) -> anyhow::Result<()> {
    match output {
        Output::Alias(output) => alias::verify_alias_output(
            header.output_id(),
            output,
            created_objects,
            foundry_data,
            storage,
            tokens_counter,
            address_swap_map,
        ),
        Output::Basic(output) => basic::verify_basic_output(
            header.output_id(),
            output,
            created_objects,
            foundry_data,
            target_milestone_timestamp,
            storage,
            tokens_counter,
            address_swap_map,
        ),
        Output::Foundry(output) => foundry::verify_foundry_output(
            header.output_id(),
            output,
            created_objects,
            foundry_data,
            storage,
            tokens_counter,
            address_swap_map,
        ),
        Output::Nft(output) => nft::verify_nft_output(
            header.output_id(),
            output,
            created_objects,
            foundry_data,
            storage,
            tokens_counter,
            address_swap_map,
        ),
        // Treasury outputs aren't used since Stardust, so no need to verify anything here.
        Output::Treasury(_) => return Ok(()),
    }
    .map_err(|e| anyhow!("error verifying output {}: {}", header.output_id(), e))
}
