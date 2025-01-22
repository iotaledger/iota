// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Rust types and logic for the Move counterparts in the `stardust` system
//! package.

use anyhow::Result;
use iota_protocol_config::ProtocolConfig;
use move_core_types::{ident_str, identifier::IdentStr, language_storage::StructTag};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use super::unlock_conditions::{
    ExpirationUnlockCondition, StorageDepositReturnUnlockCondition, TimelockUnlockCondition,
};
use crate::{
    STARDUST_PACKAGE_ID, TypeTag,
    balance::Balance,
    base_types::{IotaAddress, MoveObjectType, ObjectID, SequenceNumber, TxContext},
    coin::Coin,
    collection_types::Bag,
    id::UID,
    object::{Data, MoveObject, Object, Owner},
    stardust::{coin_type::CoinType, stardust_to_iota_address},
};

pub const BASIC_OUTPUT_MODULE_NAME: &IdentStr = ident_str!("basic_output");
pub const BASIC_OUTPUT_STRUCT_NAME: &IdentStr = ident_str!("BasicOutput");

/// Rust version of the stardust basic output.
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct BasicOutput {
    /// Hash of the `OutputId` that was migrated.
    pub id: UID,

    /// The amount of coins held by the output.
    pub balance: Balance,

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
    /// Construct the basic output with an empty [`Bag`] using the
    /// Output Header ID and Stardust
    /// [`BasicOutput`][iota_stardust_sdk::types::block::output::BasicOutput].
    pub fn new(
        header_object_id: ObjectID,
        output: &iota_stardust_sdk::types::block::output::BasicOutput,
    ) -> Result<Self> {
        let id = UID::new(header_object_id);
        let balance = Balance::new(output.amount());
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
        let metadata = output
            .features()
            .metadata()
            .map(|metadata| metadata.data().to_vec());
        let tag = output.features().tag().map(|tag| tag.tag().to_vec());
        let sender = output
            .features()
            .sender()
            .map(|sender| stardust_to_iota_address(sender.address()))
            .transpose()?;

        Ok(BasicOutput {
            id,
            balance,
            native_tokens,
            storage_deposit_return,
            timelock,
            expiration,
            metadata,
            tag,
            sender,
        })
    }

    /// Returns the struct tag of the BasicOutput struct
    pub fn tag(type_param: TypeTag) -> StructTag {
        StructTag {
            address: STARDUST_PACKAGE_ID.into(),
            module: BASIC_OUTPUT_MODULE_NAME.to_owned(),
            name: BASIC_OUTPUT_STRUCT_NAME.to_owned(),
            type_params: vec![type_param],
        }
    }

    /// Infer whether this object can resolve into a simple coin.
    ///
    /// Returns `true` in particular when the given milestone timestamp is equal
    /// or past the unix timestamp in a present timelock and no other unlock
    /// condition or metadata, tag, sender feature is present.
    pub fn is_simple_coin(&self, target_milestone_timestamp_sec: u32) -> bool {
        !(self.expiration.is_some()
            || self.storage_deposit_return.is_some()
            || self
                .timelock
                .as_ref()
                .is_some_and(|timelock| target_milestone_timestamp_sec < timelock.unix_time)
            || self.metadata.is_some()
            || self.tag.is_some()
            || self.sender.is_some())
    }

    pub fn to_genesis_object(
        &self,
        owner: IotaAddress,
        protocol_config: &ProtocolConfig,
        tx_context: &TxContext,
        version: SequenceNumber,
        coin_type: &CoinType,
    ) -> Result<Object> {
        let move_object = {
            MoveObject::new_from_execution(
                BasicOutput::tag(coin_type.to_type_tag()).into(),
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
        coin_type: &CoinType,
    ) -> Result<Object> {
        create_coin(
            self.id,
            owner,
            self.balance.value(),
            tx_context,
            version,
            protocol_config,
            coin_type,
        )
    }
}

pub(crate) fn create_coin(
    object_id: UID,
    owner: IotaAddress,
    amount: u64,
    tx_context: &TxContext,
    version: SequenceNumber,
    protocol_config: &ProtocolConfig,
    coin_type: &CoinType,
) -> Result<Object> {
    let coin = Coin::new(object_id, amount);
    let move_object = {
        MoveObject::new_from_execution(
            MoveObjectType::from(Coin::type_(coin_type.to_type_tag())),
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
