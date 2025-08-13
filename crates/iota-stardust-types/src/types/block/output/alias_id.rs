// Copyright 2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::types::block::{
    address::{Address, AliasAddress},
    output::OutputId,
};

impl_id!(pub AliasId, 32, "Unique identifier of an alias, which is the BLAKE2b-256 hash of the Output ID that created it.");

#[cfg(feature = "serde")]
string_serde_impl!(AliasId);

impl From<&OutputId> for AliasId {
    fn from(output_id: &OutputId) -> Self {
        Self::from(output_id.hash())
    }
}

impl AliasId {
    ///
    pub fn or_from_output_id(self, output_id: &OutputId) -> Self {
        if self.is_null() {
            Self::from(output_id)
        } else {
            self
        }
    }
}

impl From<AliasId> for Address {
    fn from(value: AliasId) -> Self {
        Self::Alias(AliasAddress::new(value))
    }
}
