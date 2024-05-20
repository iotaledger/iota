// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk::types::block::output::FoundryOutput;
use sui_types::in_memory_storage::InMemoryStorage;

use super::created_objects::CreatedObjects;

pub fn verify_foundry_output(
    output: &FoundryOutput,
    created_objects: &CreatedObjects,
    storage: &InMemoryStorage,
) -> anyhow::Result<()> {
    // TODO: Implementation. Returns Ok for now so the migration can be tested.
    Ok(())
}
