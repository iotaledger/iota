// Copyright 2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::types::block::output::FoundryId;

impl_id!(pub TokenId, 38, "Unique identifiers of native tokens. The TokenId of native tokens minted by a specific foundry is the same as the FoundryId.");

#[cfg(feature = "serde")]
string_serde_impl!(TokenId);

impl From<FoundryId> for TokenId {
    fn from(foundry_id: FoundryId) -> Self {
        Self::new(*foundry_id)
    }
}
