// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk::types::block::output::AliasOutput;
use sui_types::object::Object;

pub fn verify_alias_output(object: &Object, output: &AliasOutput) -> anyhow::Result<()> {
    // TODO: Implementation. Returns Ok for now so the migration can be tested.
    Ok(())
}
