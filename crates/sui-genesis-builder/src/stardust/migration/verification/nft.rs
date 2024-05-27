// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk::types::block::output::NftOutput;
use sui_types::in_memory_storage::InMemoryStorage;

use super::created_objects::CreatedObjects;

pub fn verify_nft_output(
    _output: &NftOutput,
    _created_objects: &CreatedObjects,
    _storage: &InMemoryStorage,
) -> anyhow::Result<()> {
    // TODO: Implementation. Returns Ok for now so the migration can be tested.
    Ok(())
}
