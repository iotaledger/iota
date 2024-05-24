// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk::types::block::output::Output;

use sui_types::object::Object;

use crate::stardust::migration::migration::Migration;
use crate::stardust::types::snapshot::OutputHeader;

mod alias;
mod executor;

fn random_output_header() -> OutputHeader {
    OutputHeader::new_testing(
        rand::random(),
        rand::random(),
        rand::random(),
        rand::random(),
    )
}

fn run_migration(outputs: impl IntoIterator<Item = (OutputHeader, Output)>) -> Vec<Object> {
    let mut snapshot_buffer = Vec::new();
    let mut foundries = Vec::new();
    let mut outputs_without_foundries = Vec::new();

    for (header, output) in outputs.into_iter() {
        match output {
            Output::Foundry(foundry) => {
                foundries.push((header, foundry));
            }
            other => {
                outputs_without_foundries.push((header, other));
            }
        }
    }

    Migration::new()
        .unwrap()
        .run(
            foundries.into_iter(),
            outputs_without_foundries.into_iter(),
            &mut snapshot_buffer,
        )
        .unwrap();

    bcs::from_bytes(&snapshot_buffer).unwrap()
}
