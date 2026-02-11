// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Extension traits for creating `Alias` and `AliasOutput` from Stardust types
//! during migration.

use iota_protocol_config::ProtocolConfig;
use iota_stardust_types::block::output::AliasOutput as StardustAlias;
use iota_types::{
    balance::Balance,
    base_types::{ObjectID, SequenceNumber, TxContext},
    collection_types::Bag,
    id::UID,
    object::{Data, MoveObject, Object, Owner},
    stardust::{
        coin_type::CoinType,
        output::alias::{Alias, AliasOutput},
    },
};

use super::super::address::stardust_to_iota_address;

/// Extension trait for creating `Alias` from Stardust types.
pub trait AliasExt {
    /// Creates the Move-based Alias model from a Stardust-based Alias Output.
    fn try_from_stardust(alias_id: ObjectID, alias: &StardustAlias)
    -> Result<Alias, anyhow::Error>;

    /// Creates a genesis object from this alias.
    fn to_genesis_object(
        &self,
        owner: Owner,
        protocol_config: &ProtocolConfig,
        tx_context: &TxContext,
        version: SequenceNumber,
    ) -> anyhow::Result<Object>;
}

impl AliasExt for Alias {
    fn try_from_stardust(
        alias_id: ObjectID,
        alias: &StardustAlias,
    ) -> Result<Alias, anyhow::Error> {
        if alias_id.as_ref() == [0; 32] {
            anyhow::bail!("alias_id must be non-zeroed");
        }

        let state_metadata: Option<Vec<u8>> = if alias.state_metadata().is_empty() {
            None
        } else {
            Some(alias.state_metadata().to_vec())
        };
        let sender: Option<iota_types::base_types::IotaAddress> = alias
            .features()
            .sender()
            .map(|sender_feat| stardust_to_iota_address(sender_feat.address()))
            .transpose()?;
        let metadata: Option<Vec<u8>> = alias
            .features()
            .metadata()
            .map(|metadata_feat| metadata_feat.data().to_vec());
        let immutable_issuer: Option<iota_types::base_types::IotaAddress> = alias
            .immutable_features()
            .issuer()
            .map(|issuer_feat| stardust_to_iota_address(issuer_feat.address()))
            .transpose()?;
        let immutable_metadata: Option<Vec<u8>> = alias
            .immutable_features()
            .metadata()
            .map(|metadata_feat| metadata_feat.data().to_vec());

        Ok(Alias {
            id: UID::new(alias_id),
            legacy_state_controller: stardust_to_iota_address(alias.state_controller_address())?,
            state_index: alias.state_index(),
            state_metadata,
            sender,
            metadata,
            immutable_issuer,
            immutable_metadata,
        })
    }

    fn to_genesis_object(
        &self,
        owner: Owner,
        protocol_config: &ProtocolConfig,
        tx_context: &TxContext,
        version: SequenceNumber,
    ) -> anyhow::Result<Object> {
        // Construct the Alias object.
        let move_alias_object = {
            MoveObject::new_from_execution(
                Alias::tag().into(),
                version,
                bcs::to_bytes(&self)?,
                protocol_config,
            )?
        };

        let move_alias_object = Object::new_from_genesis(
            Data::Move(move_alias_object),
            // We will later overwrite the owner we set here since this object will be added
            // as a dynamic field on the alias output object.
            owner,
            tx_context.digest(),
        );

        Ok(move_alias_object)
    }
}

/// Extension trait for creating `AliasOutput` from Stardust types.
pub trait AliasOutputExt {
    /// Creates the Move-based Alias Output model from a Stardust-based Alias
    /// Output.
    fn try_from_stardust(
        object_id: ObjectID,
        alias: &StardustAlias,
        native_tokens: Bag,
    ) -> Result<AliasOutput, anyhow::Error>;

    /// Creates a genesis object from this alias output.
    fn to_genesis_object(
        &self,
        owner: Owner,
        protocol_config: &ProtocolConfig,
        tx_context: &TxContext,
        version: SequenceNumber,
        coin_type: CoinType,
    ) -> anyhow::Result<Object>;
}

impl AliasOutputExt for AliasOutput {
    fn try_from_stardust(
        object_id: ObjectID,
        alias: &StardustAlias,
        native_tokens: Bag,
    ) -> Result<AliasOutput, anyhow::Error> {
        Ok(AliasOutput {
            id: UID::new(object_id),
            balance: Balance::new(alias.amount()),
            native_tokens,
        })
    }

    fn to_genesis_object(
        &self,
        owner: Owner,
        protocol_config: &ProtocolConfig,
        tx_context: &TxContext,
        version: SequenceNumber,
        coin_type: CoinType,
    ) -> anyhow::Result<Object> {
        // Construct the Alias Output object.
        let move_alias_output_object = {
            MoveObject::new_from_execution(
                AliasOutput::tag(coin_type.to_type_tag()).into(),
                version,
                bcs::to_bytes(&self)?,
                protocol_config,
            )?
        };

        let move_alias_output_object = Object::new_from_genesis(
            Data::Move(move_alias_output_object),
            owner,
            tx_context.digest(),
        );

        Ok(move_alias_output_object)
    }
}
