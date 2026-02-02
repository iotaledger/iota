// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Extension trait for creating `BasicOutput` from Stardust types during
//! migration.

use anyhow::Result;
use iota_protocol_config::ProtocolConfig;
// Re-export the canonical type from iota-types
pub use iota_types::stardust::output::basic::BasicOutput;
use iota_types::{
    balance::Balance,
    base_types::{IotaAddress, MoveObjectType, ObjectID, SequenceNumber, TxContext},
    coin::Coin,
    collection_types::Bag,
    id::UID,
    object::{Data, MoveObject, Object, Owner},
    stardust::{
        coin_type::CoinType,
        output::unlock_conditions::{
            ExpirationUnlockCondition, StorageDepositReturnUnlockCondition, TimelockUnlockCondition,
        },
    },
};

use super::{
    super::address::stardust_to_iota_address,
    unlock_conditions::{
        ExpirationUnlockConditionExt, StorageDepositReturnUnlockConditionExt,
        TimelockUnlockConditionExt,
    },
};

/// Creates a genesis coin object.
pub fn create_coin(
    object_id: ObjectID,
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

/// Extension trait for creating `BasicOutput` from Stardust types.
pub trait BasicOutputExt {
    /// Construct the basic output with an empty [`Bag`] using the
    /// Output Header ID and Stardust
    /// [`BasicOutput`][iota_stardust_types::block::output::BasicOutput].
    fn new_from_stardust(
        header_object_id: ObjectID,
        output: &iota_stardust_types::block::output::BasicOutput,
    ) -> Result<BasicOutput>;

    /// Creates a genesis object from this basic output.
    fn to_genesis_object(
        &self,
        owner: IotaAddress,
        protocol_config: &ProtocolConfig,
        tx_context: &TxContext,
        version: SequenceNumber,
        coin_type: &CoinType,
    ) -> Result<Object>;

    /// Converts this basic output into a genesis coin object.
    fn into_genesis_coin_object(
        self,
        owner: IotaAddress,
        protocol_config: &ProtocolConfig,
        tx_context: &TxContext,
        version: SequenceNumber,
        coin_type: &CoinType,
    ) -> Result<Object>;

    /// Infer whether this object can resolve into a simple coin.
    ///
    /// Returns `true` in particular when the given milestone timestamp is equal
    /// or past the unix timestamp in a present timelock and no other unlock
    /// condition or metadata, tag, sender feature is present.
    fn is_simple_coin(&self, target_milestone_timestamp_sec: u32) -> bool;
}

impl BasicOutputExt for BasicOutput {
    fn new_from_stardust(
        header_object_id: ObjectID,
        output: &iota_stardust_types::block::output::BasicOutput,
    ) -> Result<BasicOutput> {
        let id = UID::new(header_object_id);
        let balance = Balance::new(output.amount());
        let native_tokens: Bag = Default::default();
        let unlock_conditions = output.unlock_conditions();
        let storage_deposit_return = unlock_conditions
            .storage_deposit_return()
            .map(StorageDepositReturnUnlockCondition::try_from_stardust)
            .transpose()?;
        let timelock = unlock_conditions
            .timelock()
            .map(TimelockUnlockCondition::from_stardust);
        let expiration = output
            .unlock_conditions()
            .expiration()
            .map(|expiration| {
                ExpirationUnlockCondition::new_from_stardust(output.address(), expiration)
            })
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

    fn to_genesis_object(
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

    fn into_genesis_coin_object(
        self,
        owner: IotaAddress,
        protocol_config: &ProtocolConfig,
        tx_context: &TxContext,
        version: SequenceNumber,
        coin_type: &CoinType,
    ) -> Result<Object> {
        create_coin(
            *self.id.object_id(),
            owner,
            self.balance.value(),
            tx_context,
            version,
            protocol_config,
            coin_type,
        )
    }

    fn is_simple_coin(&self, target_milestone_timestamp_sec: u32) -> bool {
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
}
