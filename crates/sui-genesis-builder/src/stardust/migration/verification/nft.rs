// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use iota_sdk::types::block::output::{NftOutput, TokenId};
use sui_types::in_memory_storage::InMemoryStorage;

use crate::stardust::migration::{
    executor::FoundryLedgerData,
    verification::{created_objects::CreatedObjects, util::verify_parent},
};

pub(super) fn verify_nft_output(
    output: &NftOutput,
    _created_objects: &CreatedObjects,
    _foundry_data: &HashMap<TokenId, FoundryLedgerData>,
    storage: &InMemoryStorage,
) -> anyhow::Result<()> {
    // TODO: Implementation. Returns Ok for now so the migration can be tested.
    verify_parent(output.address(), storage)?;
    Ok(())
}
