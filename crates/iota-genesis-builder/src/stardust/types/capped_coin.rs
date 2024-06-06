// Copyright (c) 2024 IOTA Stiftung
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::{coin::TreasuryCap, id::UID};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The policy wrapper that ensures the supply of a `Coin` never exceeds the
/// maximum supply.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, JsonSchema)]
pub struct MaxSupplyPolicy {
    pub id: UID,
    pub maximum_supply: u64,
    pub treasury_cap: TreasuryCap,
}
