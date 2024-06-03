// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The [`verification`] module contains the validation logic to make sure that
//! the stardust outputs are correctly converted to the move objects.

use std::collections::HashMap;

use iota_sdk::types::block::output::{Output, TokenId};
use sui_types::in_memory_storage::InMemoryStorage;

use self::created_objects::CreatedObjects;
use crate::stardust::{migration::executor::FoundryLedgerData, types::snapshot::OutputHeader};

pub mod alias;
pub mod basic;
pub mod created_objects;
pub mod foundry;
pub mod nft;
mod util;

pub(crate) fn verify_output(
    header: &OutputHeader,
    output: &Output,
    created_objects: &CreatedObjects,
    foundry_data: &HashMap<TokenId, FoundryLedgerData>,
    storage: &InMemoryStorage,
) -> anyhow::Result<()> {
    match output {
        Output::Alias(output) => alias::verify_alias_output(
            header.output_id(),
            output,
            created_objects,
            foundry_data,
            storage,
        ),
        Output::Basic(output) => {
            basic::verify_basic_output(output, created_objects, foundry_data, storage)
        }
        Output::Foundry(output) => {
            foundry::verify_foundry_output(output, created_objects, foundry_data, storage)
        }
        Output::Nft(output) => {
            nft::verify_nft_output(output, created_objects, foundry_data, storage)
        }
        // Treasury outputs aren't used since Stardust, so no need to verify anything here.
        Output::Treasury(_) => return Ok(()),
    }
    .map_err(|e| anyhow::anyhow!("error verifying output {}: {}", header.output_id(), e))
}
