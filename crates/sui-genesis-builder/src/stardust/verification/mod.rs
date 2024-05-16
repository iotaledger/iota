// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The [`verification`] module contains the validation logic to make sure that the stardust outputs are correctly converted to the move objects.

use iota_sdk::types::block::output::Output;
use sui_types::object::Object;

use super::types::snapshot::OutputHeader;

pub mod alias;
pub mod basic;
pub mod foundry;
pub mod nft;

pub fn verify_output(
    object: &Object,
    header: &OutputHeader,
    output: &Output,
) -> anyhow::Result<()> {
    match output {
        Output::Alias(output) => alias::verify_alias_output(object, output),
        Output::Basic(output) => basic::verify_basic_output(object, output),
        Output::Foundry(output) => foundry::verify_foundry_output(object, output),
        Output::Nft(output) => nft::verify_nft_output(object, output),
        Output::Treasury(_) => return Ok(()),
    }
    .map_err(|e| anyhow::anyhow!("error validating output {}: {}", header.output_id(), e))
}
