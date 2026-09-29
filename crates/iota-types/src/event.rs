// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use anyhow::ensure;
use iota_sdk_move_types::iota_system::iota_system_state_inner::{
    SystemEpochInfoEventV1, SystemEpochInfoEventV2,
};
use iota_sdk_types::{Event, TransactionDigest};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_with::serde_as;

use crate::iota_serde::{BigInt, Readable};

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
/// Unique ID of an IOTA Event, the ID is a combination of transaction digest
/// and event seq number.
#[serde_as]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "camelCase")]
pub struct EventID {
    pub tx_digest: TransactionDigest,
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
