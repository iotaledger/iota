// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Verification type for output indexes that confines the value to the range
//! [0..128)

use iota_sdk::types::block::output::OUTPUT_INDEX_RANGE;

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
