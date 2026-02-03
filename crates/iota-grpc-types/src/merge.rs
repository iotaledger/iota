// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::error::Error;

pub trait Merge<T> {
    fn merge(
        &mut self,
        source: T,
        mask: &crate::field::FieldMaskTree,
    ) -> Result<(), Box<dyn Error>>;

    fn merge_from(source: T, mask: &crate::field::FieldMaskTree) -> Result<Self, Box<dyn Error>>
    where
        Self: std::default::Default,
    {
        let mut message = Self::default();
        message.merge(source, mask)?;
        Ok(message)
    }
}
