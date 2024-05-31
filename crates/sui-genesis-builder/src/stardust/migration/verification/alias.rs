// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk::types::block::output::AliasOutput;
use sui_types::in_memory_storage::InMemoryStorage;

use super::{created_objects::CreatedObjects, util::verify_parent};

pub fn verify_alias_output(
    output: &AliasOutput,
    _created_objects: &CreatedObjects,
    storage: &InMemoryStorage,
) -> anyhow::Result<()> {
    // TODO: Implementation. Returns Ok for now so the migration can be tested.
    verify_parent(output.governor_address(), storage)?;
    Ok(())
}
