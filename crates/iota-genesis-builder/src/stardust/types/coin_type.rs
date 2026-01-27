// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt::Display;

use iota_types::gas_coin::GAS;
use move_core_types::language_storage::TypeTag;

/// The type tag for the outputs used in the migration.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CoinType {
    Iota,
}

impl CoinType {
    pub fn to_type_tag(&self) -> TypeTag {
        match self {
            Self::Iota => GAS::type_tag(),
        }
    }
}

impl Display for CoinType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Iota => write!(f, "iota"),
        }
    }
}
