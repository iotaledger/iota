//! Rust types and logic for the Move counterparts in the `stardust` system
//! package.

use anyhow::Result;
use iota_sdk::types::block::address::Address;
use move_core_types::{ident_str, identifier::IdentStr, language_storage::StructTag};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use iota_protocol_config::ProtocolConfig;
use iota_types::{
    balance::Balance,
    base_types::{MoveObjectType, ObjectID, SequenceNumber, IotaAddress, TxContext},
    coin::Coin,
    collection_types::Bag,
    id::UID,
    object::{Data, MoveObject, Object, Owner},
    STARDUST_PACKAGE_ID,
};

use super::{snapshot::OutputHeader, stardust_to_iota_address};

pub const BASIC_OUTPUT_MODULE_NAME: &IdentStr = ident_str!("basic_output");
pub const BASIC_OUTPUT_STRUCT_NAME: &IdentStr = ident_str!("BasicOutput");

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
        expiration_unlock_condition: &iota_sdk::types::block::output::unlock_condition::ExpirationUnlockCondition,
    ) -> anyhow::Result<Self> {
        let owner = stardust_to_iota_address(owner_address)?;
        let return_address = stardust_to_iota_address(expiration_unlock_condition.return_address())?;
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
    /// The unix time (seconds since Unix epoch) starting from which the output
    /// can be consumed.
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

    /// The `Bag` holds native tokens, key-ed by the stringified type of the
    /// asset. Example: key: "0xabcded::soon::SOON", value:
    /// Balance<0xabcded::soon::SOON>.
    pub native_tokens: Bag,

    /// The storage deposit return unlock condition.
    pub storage_deposit_return: Option<StorageDepositReturnUnlockCondition>,
    /// The timelock unlock condition.
    pub timelock: Option<TimelockUnlockCondition>,
    /// The expiration unlock condition.
    pub expiration: Option<ExpirationUnlockCondition>,

    // Possible features, they have no effect and only here to hold data until the object is
    // deleted.
    /// The metadata feature.
    pub metadata: Option<Vec<u8>>,
    /// The tag feature.
    pub tag: Option<Vec<u8>>,
    /// The sender feature.
    pub sender: Option<IotaAddress>,
}

impl BasicOutput {
    /// Construct the basic output with an empty [`Bag`] through the
    /// [`OutputHeader`]
    /// and [`Output`][iota_sdk::types::block::output::BasicOutput].
    pub fn new(
        header: OutputHeader,
        output: &iota_sdk::types::block::output::BasicOutput,
    ) -> Result<Self> {
        let id = UID::new(ObjectID::new(header.output_id().hash()));
        let iota = Balance::new(output.amount());
        let native_tokens = Default::default();
        let unlock_conditions = output.unlock_conditions();
        let storage_deposit_return = unlock_conditions
            .storage_deposit_return()
            .map(|unlock| unlock.try_into())
            .transpose()?;
        let timelock = unlock_conditions.timelock().map(|unlock| unlock.into());
        let expiration = output
            .unlock_conditions()
            .expiration()
            .map(|expiration| ExpirationUnlockCondition::new(output.address(), expiration))
            .transpose()?;

        Ok(BasicOutput {
            id,
            iota,
            native_tokens,
            storage_deposit_return,
            timelock,
            expiration,
            metadata: Default::default(),
            tag: Default::default(),
            sender: Default::default(),
        })
    }

    pub fn type_() -> StructTag {
        StructTag {
            address: STARDUST_PACKAGE_ID.into(),
            module: BASIC_OUTPUT_MODULE_NAME.to_owned(),
            name: BASIC_OUTPUT_STRUCT_NAME.to_owned(),
            type_params: Vec::new(),
        }
    }

    /// Infer whether this object can resolve into a simple coin.
    pub fn is_simple_coin(&self) -> bool {
        !(self.expiration.is_some()
            || self.storage_deposit_return.is_some()
            || self.timelock.is_some())
    }

    pub fn to_genesis_object(
        &self,
        owner: IotaAddress,
        protocol_config: &ProtocolConfig,
        tx_context: &TxContext,
        version: SequenceNumber,
    ) -> Result<Object> {
        let move_object = unsafe {
            // Safety: we know from the definition of `BasicOutput` in the stardust package
            // that it is not publicly transferable (`store` ability is absent).
            MoveObject::new_from_execution(
                Self::type_().into(),
                false,
                version,
                bcs::to_bytes(self)?,
                protocol_config,
            )?
        };
        // Resolve ownership
        let owner = if self.expiration.is_some() {
            Owner::Shared {
                initial_shared_version: version,
            }
        } else {
            Owner::AddressOwner(owner)
        };
        Ok(Object::new_from_genesis(
            Data::Move(move_object),
            owner,
            tx_context.digest(),
        ))
    }

    pub fn into_genesis_coin_object(
        self,
        owner: IotaAddress,
        protocol_config: &ProtocolConfig,
        tx_context: &TxContext,
        version: SequenceNumber,
    ) -> Result<Object> {
        create_gas_coin(
            self.id,
            owner,
            self.iota.value(),
            tx_context,
            version,
            protocol_config,
        )
    }
}

pub(crate) fn create_gas_coin(
    object_id: UID,
    owner: IotaAddress,
    amount: u64,
    tx_context: &TxContext,
    version: SequenceNumber,
    protocol_config: &ProtocolConfig,
) -> Result<Object> {
    let coin = Coin::new(object_id, amount);
    let move_object = unsafe {
        // Safety: we know from the definition of `Coin`
        // that it has public transfer (`store` ability is present).
        MoveObject::new_from_execution(
            MoveObjectType::gas_coin(),
            true,
            version,
            bcs::to_bytes(&coin)?,
            protocol_config,
        )?
    };
    // Resolve ownership
    let owner = Owner::AddressOwner(owner);
    Ok(Object::new_from_genesis(
        Data::Move(move_object),
        owner,
        tx_context.digest(),
    ))
}
