// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use anyhow::ensure;
use move_core_types::{
    account_address::AccountAddress,
    annotated_value::{MoveDatatypeLayout, MoveValue},
    ident_str,
    identifier::{IdentStr, Identifier},
    language_storage::StructTag,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_with::{Bytes, serde_as};

use crate::{
    IOTA_SYSTEM_ADDRESS,
    base_types::{IotaAddress, ObjectID, TransactionDigest},
    error::{IotaError, IotaResult},
    iota_serde::{BigInt, Readable},
    object::bounded_visitor::BoundedVisitor,
};

/// A universal IOTA event type encapsulating different types of events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    /// UTC timestamp in milliseconds since epoch (1/1/1970)
    pub timestamp: u64,
    /// Transaction digest of associated transaction
    pub tx_digest: TransactionDigest,
    /// Consecutive per-tx counter assigned to this event.
    pub event_num: u64,
    /// Specific event type
    pub event: Event,
    /// Move event's json value
    pub parsed_json: Value,
}
/// Unique ID of an IOTA Event, the ID is a combination of tx seq number and
/// event seq number, the ID is local to this particular fullnode and will be
/// different from other fullnode.
#[serde_as]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Hash)]
#[serde(rename_all = "camelCase")]
pub struct EventID {
    pub tx_digest: TransactionDigest,
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub event_seq: u64,
}

impl From<(TransactionDigest, u64)> for EventID {
    fn from((tx_digest_num, event_seq_number): (TransactionDigest, u64)) -> Self {
        Self {
            tx_digest: tx_digest_num as TransactionDigest,
            event_seq: event_seq_number,
        }
    }
}

impl From<EventID> for String {
    fn from(id: EventID) -> Self {
        format!("{:?}:{}", id.tx_digest, id.event_seq)
    }
}

impl TryFrom<String> for EventID {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let values = value.split(':').collect::<Vec<_>>();
        ensure!(values.len() == 2, "Malformed EventID : {value}");
        Ok((
            TransactionDigest::from_str(values[0])?,
            u64::from_str(values[1])?,
        )
            .into())
    }
}

impl EventEnvelope {
    pub fn new(
        timestamp: u64,
        tx_digest: TransactionDigest,
        event_num: u64,
        event: Event,
        move_struct_json_value: Value,
    ) -> Self {
        Self {
            timestamp,
            tx_digest,
            event_num,
            event,
            parsed_json: move_struct_json_value,
        }
    }
}

/// Specific type of event
#[serde_as]
#[derive(PartialEq, Eq, Debug, Clone, Deserialize, Serialize, Hash)]
pub struct Event {
    pub package_id: ObjectID,
    pub transaction_module: Identifier,
    pub sender: IotaAddress,
    pub type_: StructTag,
    #[serde_as(as = "Bytes")]
    pub contents: Vec<u8>,
}

impl Event {
    pub fn new(
        package_id: &AccountAddress,
        module: &IdentStr,
        sender: IotaAddress,
        type_: StructTag,
        contents: Vec<u8>,
    ) -> Self {
        Self {
            package_id: ObjectID::from(*package_id),
            transaction_module: Identifier::from(module),
            sender,
            type_,
            contents,
        }
    }
    pub fn move_event_to_move_value(
        contents: &[u8],
        layout: MoveDatatypeLayout,
    ) -> IotaResult<MoveValue> {
        BoundedVisitor::deserialize_value(contents, &layout.into_layout()).map_err(|e| {
            IotaError::ObjectSerialization {
                error: e.to_string(),
            }
        })
    }

    pub fn is_system_epoch_info_event_v1(&self) -> bool {
        self.type_.address == IOTA_SYSTEM_ADDRESS
            && self.type_.module.as_ident_str() == ident_str!("iota_system_state_inner")
            && self.type_.name.as_ident_str() == ident_str!("SystemEpochInfoEventV1")
    }

    pub fn is_system_epoch_info_event_v2(&self) -> bool {
        self.type_.address == IOTA_SYSTEM_ADDRESS
            && self.type_.module.as_ident_str() == ident_str!("iota_system_state_inner")
            && self.type_.name.as_ident_str() == ident_str!("SystemEpochInfoEventV2")
    }

    pub fn is_system_epoch_info_event(&self) -> bool {
        self.is_system_epoch_info_event_v1() || self.is_system_epoch_info_event_v2()
    }
}

impl Event {
    pub fn random_for_testing() -> Self {
        Self {
            package_id: ObjectID::random(),
            transaction_module: Identifier::new("test").unwrap(),
            sender: AccountAddress::random().into(),
            type_: StructTag {
                address: AccountAddress::random(),
                module: Identifier::new("test").unwrap(),
                name: Identifier::new("test").unwrap(),
                type_params: vec![],
            },
            contents: vec![],
        }
    }
}

#[derive(Deserialize)]
pub enum SystemEpochInfoEvent {
    V1(SystemEpochInfoEventV1),
    V2(SystemEpochInfoEventV2),
}

impl SystemEpochInfoEvent {
    pub fn supply_change(&self) -> i64 {
        match self {
            SystemEpochInfoEvent::V1(event) => {
                event.minted_tokens_amount as i64 - event.burnt_tokens_amount as i64
            }
            SystemEpochInfoEvent::V2(event) => {
                event.minted_tokens_amount as i64 - event.burnt_tokens_amount as i64
            }
        }
    }
}

impl From<Event> for SystemEpochInfoEvent {
    fn from(event: Event) -> Self {
        if event.is_system_epoch_info_event_v2() {
            SystemEpochInfoEvent::V2(
                bcs::from_bytes::<SystemEpochInfoEventV2>(&event.contents)
                    .expect("event deserialization should succeed as type was pre-validated"),
            )
        } else {
            SystemEpochInfoEvent::V1(
                bcs::from_bytes::<SystemEpochInfoEventV1>(&event.contents)
                    .expect("event deserialization should succeed as type was pre-validated"),
            )
        }
    }
}

/// Event emitted in move code `fun advance_epoch` in protocol versions 1 to 3
#[derive(Deserialize)]
pub struct SystemEpochInfoEventV1 {
    pub epoch: u64,
    pub protocol_version: u64,
    pub reference_gas_price: u64,
    pub total_stake: u64,
    pub storage_charge: u64,
    pub storage_rebate: u64,
    pub storage_fund_balance: u64,
    pub total_gas_fees: u64,
    pub total_stake_rewards_distributed: u64,
    pub burnt_tokens_amount: u64,
    pub minted_tokens_amount: u64,
}

/// Event emitted in move code `fun advance_epoch` in protocol versions 5 and
/// later.
/// This second version of the event includes the tips amount to show how much
/// of the gas fees go to the validators when protocol_defined_base_fee is
/// enabled in the protocol config.
#[derive(Deserialize)]
pub struct SystemEpochInfoEventV2 {
    pub epoch: u64,
    pub protocol_version: u64,
    pub total_stake: u64,
    pub storage_charge: u64,
    pub storage_rebate: u64,
    pub storage_fund_balance: u64,
    pub total_gas_fees: u64,
    pub total_stake_rewards_distributed: u64,
    pub burnt_tokens_amount: u64,
    pub minted_tokens_amount: u64,
    pub tips_amount: u64,
}
