//! Rust types and logic for the Move counterparts in the `stardust` system package.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sui_types::base_types::SuiAddress;

/// Rust version of the stardust expiration unlock condition.
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, JsonSchema)]
pub struct ExpirationUnlockCondition {
    /// The address who owns the output before the timestamp has passed.
    pub owner: SuiAddress,
    /// The address that is allowed to spend the locked funds after the timestamp has passed.
    pub return_address: SuiAddress,
    /// Before this unix time, Address Unlock Condition is allowed to unlock the output, after that only the address defined in Return Address.
    pub unix_time: u32,
}

impl TryFrom<&iota_sdk::types::block::output::BasicOutput> for ExpirationUnlockCondition {
    type Error = anyhow::Error;

    fn try_from(output: &iota_sdk::types::block::output::BasicOutput) -> Result<Self, Self::Error> {
        let Some(address_unlock) = output.unlock_conditions().address() else {
            anyhow::bail!("output does not have address unlock condition");
        };
        let Some(expiration) = output.unlock_conditions().expiration() else {
            anyhow::bail!("output does not have expiration unlock condition");
        };
        let owner = address_unlock.address().to_string().parse()?;
        let return_address = expiration.return_address().to_string().parse()?;
        let unix_time = expiration.timestamp();

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
    /// The address to which the consuming transaction should deposit the amount defined in Return Amount.
    pub return_address: SuiAddress,
    /// The amount of IOTA coins the consuming transaction should deposit to the address defined in Return Address.
    pub return_amount: u64,
}

impl TryFrom<&iota_sdk::types::block::output::unlock_condition::StorageDepositReturnUnlockCondition>
    for StorageDepositReturnUnlockCondition
{
    type Error = anyhow::Error;

    fn try_from(
        unlock: &iota_sdk::types::block::output::unlock_condition::StorageDepositReturnUnlockCondition,
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
    /// The unix time (seconds since Unix epoch) starting from which the output can be consumed.
    pub unix_time: u32,
}

impl From<&iota_sdk::types::block::output::unlock_condition::TimelockUnlockCondition>
    for TimelockUnlockCondition
{
    fn from(
        unlock: &iota_sdk::types::block::output::unlock_condition::TimelockUnlockCondition,
    ) -> Self {
        Self {
            unix_time: unlock.timestamp(),
        }
    }
}
