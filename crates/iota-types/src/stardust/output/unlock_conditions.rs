// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::base_types::IotaAddress;

/// Rust version of the stardust expiration unlock condition.
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, JsonSchema)]
pub struct ExpirationUnlockCondition {
    /// The address who owns the output before the timestamp has passed.
    pub owner: IotaAddress,
    /// The address that is allowed to spend the locked funds after the
    /// timestamp has passed.
    pub return_address: IotaAddress,
    /// Before this unix time, Address Unlock Condition is allowed to unlock the
    /// output, after that only the address defined in Return Address.
    pub unix_time: u32,
}

/// Rust version of the stardust storage deposit return unlock condition.
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, JsonSchema)]
pub struct StorageDepositReturnUnlockCondition {
    /// The address to which the consuming transaction should deposit the amount
    /// defined in Return Amount.
    pub return_address: IotaAddress,
    /// The amount of IOTA coins the consuming transaction should deposit to the
    /// address defined in Return Address.
    pub return_amount: u64,
}

/// Rust version of the stardust timelock unlock condition.
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, JsonSchema)]
pub struct TimelockUnlockCondition {
    /// The unix time (seconds since Unix epoch) starting from which the output
    /// can be consumed.
    pub unix_time: u32,
}
