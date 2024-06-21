// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_protocol_config::ProtocolConfig;
use iota_sdk::types::block::output::{FoundryOutput, OutputId};
use iota_types::{
    base_types::{ObjectID, SequenceNumber, TxContext},
    id::UID,
    object::Object,
};

use crate::stardust::{
    migration::CoinType,
    types::{output::create_coin, stardust_to_iota_address},
};

pub(crate) fn create_foundry_amount_coin(
    output_id: &OutputId,
    foundry: &FoundryOutput,
    tx_context: &TxContext,
    version: SequenceNumber,
    protocol_config: &ProtocolConfig,
    coin_type: &CoinType,
) -> anyhow::Result<Object> {
    create_coin(
        UID::new(ObjectID::new(output_id.hash())),
        stardust_to_iota_address(*foundry.alias_address())?,
        foundry.amount(),
        tx_context,
        version,
        protocol_config,
        coin_type,
    )
}
