// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! BCS serialization utilities for IotaEvent.
//!
//! IotaEvent cannot be directly BCS serialized because it uses
//! serde_json::Value, #[serde(flatten)], and #[serde_as] annotations.

use iota_json_rpc_types::IotaEvent;
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    event::EventID,
};
use move_core_types::{identifier::Identifier, language_storage::StructTag};
use serde::{Deserialize, Serialize};

/// Convert IotaEvent to BCS bytes
pub fn try_to_bcs_bytes(event: &IotaEvent) -> Result<Vec<u8>, bcs::Error> {
    let bcs_event = BcsIotaEvent::from(event.clone());
    bcs::to_bytes(&bcs_event)
}

/// Convert BCS bytes back to IotaEvent
pub fn try_from_bcs_bytes(bytes: &[u8]) -> Result<IotaEvent, bcs::Error> {
    let bcs_event: BcsIotaEvent = bcs::from_bytes(bytes)?;
    Ok(IotaEvent::from(bcs_event))
}

/// Internal BCS-compatible version of IotaEvent
#[derive(Clone, Debug, Serialize, Deserialize)]
struct BcsIotaEvent {
    pub id: EventID,
    pub package_id: ObjectID,
    pub transaction_module: Identifier,
    pub sender: IotaAddress,
    pub type_: StructTag,
    pub parsed_json: String,
    pub bcs_data: Vec<u8>,
    pub timestamp_ms: Option<u64>,
}

impl From<IotaEvent> for BcsIotaEvent {
    fn from(event: IotaEvent) -> Self {
        Self {
            id: event.id,
            package_id: event.package_id,
            transaction_module: event.transaction_module,
            sender: event.sender,
            type_: event.type_,
            parsed_json: event.parsed_json.to_string(),
            bcs_data: event.bcs.into_bytes(),
            timestamp_ms: event.timestamp_ms,
        }
    }
}

impl From<BcsIotaEvent> for IotaEvent {
    fn from(bcs_event: BcsIotaEvent) -> Self {
        Self {
            id: bcs_event.id,
            package_id: bcs_event.package_id,
            transaction_module: bcs_event.transaction_module,
            sender: bcs_event.sender,
            type_: bcs_event.type_,
            parsed_json: serde_json::from_str(&bcs_event.parsed_json).unwrap_or_default(),
            bcs: iota_json_rpc_types::BcsEvent::new(bcs_event.bcs_data),
            timestamp_ms: bcs_event.timestamp_ms,
        }
    }
}
