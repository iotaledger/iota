// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Verification type for output indexes that confines the value to the range
//! [0..128)

use iota_sdk::types::block::output::OUTPUT_INDEX_RANGE;
use rand::{Rng, rngs::StdRng};

#[derive(Copy, Clone, Debug, Default)]
pub struct OutputIndex(u16);

impl OutputIndex {
    pub fn new(index: u16) -> anyhow::Result<Self> {
        if !OUTPUT_INDEX_RANGE.contains(&index) {
            anyhow::bail!("index {index} out of range {OUTPUT_INDEX_RANGE:?}");
        }
        Ok(Self(index))
    }

    pub fn get(&self) -> u16 {
        self.0
    }
}

/// Generates a random, valid output index in the range [0..128)
pub fn random_output_index() -> OutputIndex {
    OutputIndex::new(rand::thread_rng().gen_range(OUTPUT_INDEX_RANGE))
        .expect("range is guaranteed to be valid")
}

/// Generates a random, valid output index in the range [0..128) with a rng
pub fn random_output_index_with_rng(rng: &mut StdRng) -> OutputIndex {
    OutputIndex::new(rng.gen_range(OUTPUT_INDEX_RANGE)).expect("range is guaranteed to be valid")
}
