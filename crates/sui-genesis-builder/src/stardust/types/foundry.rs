// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk::types::block::output::FoundryOutput;
use sui_protocol_config::ProtocolConfig;
use sui_types::{
    base_types::{ObjectID, SequenceNumber, TxContext},
    id::UID,
    object::Object,
};

use crate::{
    stardust,
    stardust::types::{snapshot::OutputHeader, stardust_to_sui_address},
};

pub(crate) fn create_foundry_gas_coin(
    header: &OutputHeader,
    foundry: &FoundryOutput,
    tx_context: &TxContext,
    version: SequenceNumber,
    protocol_config: &ProtocolConfig,
) -> anyhow::Result<Object> {
    stardust::types::output::create_gas_coin(
        UID::new(ObjectID::new(header.output_id().hash())),
        stardust_to_sui_address(*foundry.alias_address())?,
        foundry.amount(),
        tx_context,
        version,
        protocol_config,
    )
}
