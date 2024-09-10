// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt::Display;

use move_core_types::language_storage::TypeTag;

use crate::{gas_coin::GAS, smr_coin::SMR};

/// The type tag for the outputs used in the migration.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CoinType {
    Iota,
    Shimmer,
}

impl CoinType {
    pub fn to_type_tag(&self) -> TypeTag {
        match self {
            Self::Iota => GAS::type_tag(),
            Self::Shimmer => SMR::type_tag(),
        }
    }
}

impl Display for CoinType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Iota => write!(f, "iota"),
            Self::Shimmer => write!(f, "shimmer"),
        }
    }
}
