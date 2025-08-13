// Copyright 2023 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::types::block::{
    output::OutputMetadata,
    rand::{
        block::rand_block_id, bool::rand_bool, number::rand_number, option::rand_option,
        output::rand_output_id, transaction::rand_transaction_id,
    },
};

/// Generates a random [`OutputMetadata`].
pub fn rand_output_metadata() -> OutputMetadata {
    OutputMetadata::new(
        rand_block_id(),
        rand_output_id(),
        rand_bool(),
        rand_option(rand_number()),
        rand_option(rand_number()),
        rand_option(rand_transaction_id()),
        rand_number(),
        rand_number(),
        rand_number(),
    )
}
