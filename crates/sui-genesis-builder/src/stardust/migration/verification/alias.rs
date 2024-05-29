// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use iota_sdk::types::block::output::{AliasOutput, TokenId};
use sui_types::in_memory_storage::InMemoryStorage;

use crate::stardust::migration::{
    executor::FoundryLedgerData, verification::created_objects::CreatedObjects,
};

pub(super) fn verify_alias_output(
    _output: &AliasOutput,
    _created_objects: &CreatedObjects,
    _foundry_data: &HashMap<TokenId, FoundryLedgerData>,
    _storage: &InMemoryStorage,
) -> anyhow::Result<()> {
    // TODO: Implementation. Returns Ok for now so the migration can be tested.
    Ok(())
}
