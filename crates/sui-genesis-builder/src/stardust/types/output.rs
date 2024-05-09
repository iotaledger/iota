//! Rust types and logic for the Move counterparts in the `stardust` system package.

use move_core_types::{ident_str, identifier::IdentStr, language_storage::StructTag};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sui_types::{
    balance::Balance,
    base_types::{ObjectID, SuiAddress},
    collection_types::Bag,
    id::UID,
    STARDUST_PACKAGE_ID,
};

use super::snapshot::OutputHeader;

pub const BASIC_OUTPUT_MODULE_NAME: &IdentStr = ident_str!("basic_output");
pub const BASIC_OUTPUT_STRUCT_NAME: &IdentStr = ident_str!("BasicOutput");

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

/// Rust version of the stardust basic output.
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct BasicOutput {
    /// Hash of the `OutputId` that was migrated.
    pub id: UID,

    /// The amount of IOTA coins held by the output.
    pub iota: Balance,

    /// The `Bag` holds native tokens, key-ed by the stringified type of the asset.
    /// Example: key: "0xabcded::soon::SOON", value: Balance<0xabcded::soon::SOON>.
    pub native_tokens: Bag,

    /// The storage deposit return unlock condition.
    pub storage_deposit_return: Option<StorageDepositReturnUnlockCondition>,
    /// The timelock unlock condition.
    pub timelock: Option<TimelockUnlockCondition>,
    /// The expiration unlock condition.
    pub expiration: Option<ExpirationUnlockCondition>,

    // Possible features, they have no effect and only here to hold data until the object is deleted.
    /// The metadata feature.
    pub metadata: Option<Vec<u8>>,
    /// The tag feature.
    pub tag: Option<Vec<u8>>,
    /// The sender feature.
    pub sender: Option<SuiAddress>,
}

impl BasicOutput {
    /// Construct the basic output with an empty [`Bag`] through the [`OutputHeader`]
    /// and [`Output`][iota_sdk::types::block::output::BasicOutput].
    pub fn new(header: OutputHeader, output: &iota_sdk::types::block::output::BasicOutput) -> Self {
        let id = UID::new(ObjectID::new(header.output_id().hash()));
        let iota = Balance::new(output.amount());
        let native_tokens = Default::default();
        let unlock_conditions = output.unlock_conditions();
        let storage_deposit_return = unlock_conditions
            .storage_deposit_return()
            .and_then(|unlock| unlock.try_into().ok());
        let timelock = unlock_conditions.timelock().map(|unlock| unlock.into());
        let expiration = output.try_into().ok();
        BasicOutput {
            id,
            iota,
            native_tokens,
            storage_deposit_return,
            timelock,
            expiration,
            metadata: Default::default(),
            tag: Default::default(),
            sender: Default::default(),
        }
    }

    pub fn type_() -> StructTag {
        StructTag {
            address: STARDUST_PACKAGE_ID.into(),
            name: BASIC_OUTPUT_MODULE_NAME.to_owned(),
            module: BASIC_OUTPUT_STRUCT_NAME.to_owned(),
            type_params: Vec::new(),
        }
    }

    pub fn has_empty_bag(&self) -> bool {
        !(self.expiration.is_some()
            || self.storage_deposit_return.is_some()
            || self.timelock.is_some())
    }
}
