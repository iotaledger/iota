// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use move_core_types::language_storage::TypeTag;

use crate::{gas_coin::GAS, smr_coin::SMR};

/// The type tag for the outputs used in the migration.
#[derive(Clone, Debug)]
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
