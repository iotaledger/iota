// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_protocol_config::ProtocolConfig;
use iota_sdk::types::block::output::{FoundryOutput, OutputId};
use iota_types::{
    base_types::{ObjectID, SequenceNumber, TxContext},
    gas_coin::GAS,
    id::UID,
    object::Object,
    smr_coin::SMR,
};
use move_core_types::language_storage::TypeTag;

use crate::{stardust, stardust::types::stardust_to_iota_address};

pub(crate) fn create_output_amount_coin(
    output_id: &OutputId,
    foundry: &FoundryOutput,
    tx_context: &TxContext,
    version: SequenceNumber,
    protocol_config: &ProtocolConfig,
    type_tag: &TypeTag,
) -> anyhow::Result<Object> {
    if type_tag == &GAS::type_tag() {
        return stardust::types::output::create_gas_coin(
            UID::new(ObjectID::new(output_id.hash())),
            stardust_to_iota_address(*foundry.alias_address())?,
            foundry.amount(),
            tx_context,
            version,
            protocol_config,
        );
    } else if type_tag == &SMR::type_tag() {
        return stardust::types::output::create_smr_coin(
            UID::new(ObjectID::new(output_id.hash())),
            stardust_to_iota_address(*foundry.alias_address())?,
            foundry.amount(),
            tx_context,
            version,
            protocol_config,
        );
    }
    anyhow::bail!("unsupported coin type: {:?}", type_tag)
}
