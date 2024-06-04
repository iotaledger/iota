// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sui_types::{coin::TreasuryCap, id::UID};

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, JsonSchema)]
pub struct MaxSupplyPolicy {
    pub id: UID,
    pub maximum_supply: u64,
    pub treasury_cap: TreasuryCap,
}
