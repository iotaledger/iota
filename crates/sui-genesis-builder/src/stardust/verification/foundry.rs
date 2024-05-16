// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk::types::block::output::FoundryOutput;
use sui_types::object::Object;

pub fn verify_foundry_output(object: &Object, output: &FoundryOutput) -> anyhow::Result<()> {
    // TODO: Implementation. Returns Ok for now so the migration can be tested.
    Ok(())
}
