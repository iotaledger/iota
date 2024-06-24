// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_stardust_sdk::types::block::address::Address;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::{base_types::IotaAddress, stardust::stardust_to_iota_address};

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

impl ExpirationUnlockCondition {
    pub(crate) fn new(
        owner_address: &Address,
        expiration_unlock_condition: &iota_stardust_sdk::types::block::output::unlock_condition::ExpirationUnlockCondition,
    ) -> anyhow::Result<Self> {
        let owner = stardust_to_iota_address(owner_address)?;
        let return_address =
            stardust_to_iota_address(expiration_unlock_condition.return_address())?;
        let unix_time = expiration_unlock_condition.timestamp();

        Ok(Self {
            owner,
            return_address,
            unix_time,
        })
    }
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

impl TryFrom<&iota_stardust_sdk::types::block::output::unlock_condition::StorageDepositReturnUnlockCondition>
    for StorageDepositReturnUnlockCondition
{
    type Error = anyhow::Error;

    fn try_from(
        unlock: &iota_stardust_sdk::types::block::output::unlock_condition::StorageDepositReturnUnlockCondition,
    ) -> Result<Self, Self::Error> {
        let return_address = unlock.return_address().to_string().parse()?;
        let return_amount = unlock.amount();
        Ok(Self {
            return_address,
            return_amount,
        })
    }
}

/// Rust version of the stardust timelock unlock condition.
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, JsonSchema)]
pub struct TimelockUnlockCondition {
    /// The unix time (seconds since Unix epoch) starting from which the output
    /// can be consumed.
    pub unix_time: u32,
}

impl From<&iota_stardust_sdk::types::block::output::unlock_condition::TimelockUnlockCondition>
    for TimelockUnlockCondition
{
    fn from(
        unlock: &iota_stardust_sdk::types::block::output::unlock_condition::TimelockUnlockCondition,
    ) -> Self {
        Self {
            unix_time: unlock.timestamp(),
        }
    }
}
