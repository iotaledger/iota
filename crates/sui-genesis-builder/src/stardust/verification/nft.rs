// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk::types::block::output::NftOutput;
use sui_types::object::Object;

pub fn verify_nft_output(object: &Object, output: &NftOutput) -> anyhow::Result<()> {
    // TODO: Implementation. Returns Ok for now so the migration can be tested.
    Ok(())
}
