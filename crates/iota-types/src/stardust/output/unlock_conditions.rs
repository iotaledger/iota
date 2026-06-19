// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_ext::types::Address;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

/// Rust version of the stardust expiration unlock condition.
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct ExpirationUnlockCondition {
    /// The address who owns the output before the timestamp has passed.
    pub owner: Address,
    /// The address that is allowed to spend the locked funds after the
    /// timestamp has passed.
    pub return_address: Address,
    /// Before this unix time, Address Unlock Condition is allowed to unlock the
    /// output, after that only the address defined in Return Address.
    pub unix_time: u32,
}

/// Rust version of the stardust storage deposit return unlock condition.
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct StorageDepositReturnUnlockCondition {
    /// The address to which the consuming transaction should deposit the amount
    /// defined in Return Amount.
    pub return_address: Address,
    /// The amount of IOTA coins the consuming transaction should deposit to the
    /// address defined in Return Address.
    pub return_amount: u64,
}

/// Rust version of the stardust timelock unlock condition.
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct TimelockUnlockCondition {
    /// The unix time (seconds since Unix epoch) starting from which the output
    /// can be consumed.
    pub unix_time: u32,
}
