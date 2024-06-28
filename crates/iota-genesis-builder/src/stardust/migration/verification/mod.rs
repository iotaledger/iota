// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The `verification` module contains the validation logic to make sure that
//! the stardust outputs are correctly converted to the move objects.

use std::collections::HashMap;

use anyhow::{anyhow, ensure};
use iota_sdk::types::block::output::{Output, OutputId, TokenId};
use iota_types::in_memory_storage::InMemoryStorage;

use self::created_objects::CreatedObjects;
use crate::stardust::{migration::executor::FoundryLedgerData, types::output_header::OutputHeader};

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
) -> anyhow::Result<()> {
    let mut total_value = 0;
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
            &mut total_value,
        )?;
    }
    ensure!(
        total_supply == total_value,
        "total supply mismatch: found {total_value}, expected {total_supply}"
    );
    Ok(())
}

fn verify_output(
    header: &OutputHeader,
    output: &Output,
    created_objects: &CreatedObjects,
    foundry_data: &HashMap<TokenId, FoundryLedgerData>,
    target_milestone_timestamp: u32,
    storage: &InMemoryStorage,
    total_value: &mut u64,
) -> anyhow::Result<()> {
    match output {
        Output::Alias(output) => alias::verify_alias_output(
            header.output_id(),
            output,
            created_objects,
            foundry_data,
            storage,
            total_value,
        ),
        Output::Basic(output) => basic::verify_basic_output(
            header.output_id(),
            output,
            created_objects,
            foundry_data,
            target_milestone_timestamp,
            storage,
            total_value,
        ),
        Output::Foundry(output) => foundry::verify_foundry_output(
            header.output_id(),
            output,
            created_objects,
            foundry_data,
            storage,
            total_value,
        ),
        Output::Nft(output) => nft::verify_nft_output(
            header.output_id(),
            output,
            created_objects,
            foundry_data,
            storage,
            total_value,
        ),
        // Treasury outputs aren't used since Stardust, so no need to verify anything here.
        Output::Treasury(_) => return Ok(()),
    }
    .map_err(|e| anyhow!("error verifying output {}: {}", header.output_id(), e))
}
