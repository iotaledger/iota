// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fmt, fmt::Display, str::FromStr};

use fastcrypto::encoding::{Base58, Base64};
use iota_metrics::monitored_scope;
use iota_types::{
    base_types::{IotaAddress, ObjectID, TransactionDigest},
    error::IotaResult,
    event::{Event, EventEnvelope, EventID},
    iota_serde::{BigInt, IotaStructTag},
};
use json_to_table::json_to_table;
use move_core_types::{
    annotated_value::MoveDatatypeLayout, identifier::Identifier, language_storage::StructTag,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use serde_with::{DisplayFromStr, serde_as};
use tabled::settings::Style as TableStyle;

use crate::{Page, type_and_fields_from_move_event_data};

pub type EventPage = Page<IotaEvent, EventID>;

#[serde_as]
#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename = "Event", rename_all = "camelCase")]
pub struct IotaEvent {
    /// Sequential event ID, ie (transaction seq number, event seq number).
    /// 1) Serves as a unique event ID for each fullnode
    /// 2) Also serves to sequence events for the purposes of pagination and
    ///    querying. A higher id is an event seen later by that fullnode.
    /// This ID is the "cursor" for event querying.
    pub id: EventID,
    /// Move package where this event was emitted.
    pub package_id: ObjectID,
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    /// Move module where this event was emitted.
    pub transaction_module: Identifier,
    /// Sender's IOTA address.
    pub sender: IotaAddress,
    #[schemars(with = "String")]
    #[serde_as(as = "IotaStructTag")]
    /// Move event type.
    pub type_: StructTag,
    /// Parsed json value of the event
    pub parsed_json: Value,
    /// Base64 encoded bcs bytes of the move event
    #[serde(flatten)]
    pub bcs: BcsEvent,
    /// UTC timestamp in milliseconds since epoch (1/1/1970)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<BigInt<u64>>")]
    #[serde_as(as = "Option<BigInt<u64>>")]
    pub timestamp_ms: Option<u64>,
}

#[serde_as]
#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", tag = "bcsEncoding")]
#[serde(from = "MaybeTaggedBcsEvent")]
pub enum BcsEvent {
    Base64 {
        #[serde_as(as = "Base64")]
        #[schemars(with = "Base64")]
        bcs: Vec<u8>,
    },
    Base58 {
        #[serde_as(as = "Base58")]
        #[schemars(with = "Base58")]
        bcs: Vec<u8>,
    },
}

impl BcsEvent {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self::Base64 { bcs: bytes }
    }

    pub fn bytes(&self) -> &[u8] {
        match self {
            BcsEvent::Base64 { bcs } => bcs.as_ref(),
            BcsEvent::Base58 { bcs } => bcs.as_ref(),
        }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        match self {
            BcsEvent::Base64 { bcs } => bcs,
            BcsEvent::Base58 { bcs } => bcs,
        }
    }
}

#[allow(unused)]
#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", untagged)]
enum MaybeTaggedBcsEvent {
    Tagged(TaggedBcsEvent),
    Base58 {
        #[serde_as(as = "Base58")]
        bcs: Vec<u8>,
    },
}

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "bcsEncoding")]
enum TaggedBcsEvent {
    Base64 {
        #[serde_as(as = "Base64")]
        bcs: Vec<u8>,
    },
    Base58 {
        #[serde_as(as = "Base58")]
        bcs: Vec<u8>,
    },
}

impl From<MaybeTaggedBcsEvent> for BcsEvent {
    fn from(event: MaybeTaggedBcsEvent) -> BcsEvent {
        let bcs = match event {
            MaybeTaggedBcsEvent::Tagged(TaggedBcsEvent::Base58 { bcs })
            | MaybeTaggedBcsEvent::Base58 { bcs } => bcs,
            MaybeTaggedBcsEvent::Tagged(TaggedBcsEvent::Base64 { bcs }) => bcs,
        };

        // Bytes are already decoded, force into Base64 variant to avoid serializing to
        // base58
        Self::Base64 { bcs }
    }
}

impl From<EventEnvelope> for IotaEvent {
    fn from(ev: EventEnvelope) -> Self {
        Self {
            id: EventID {
                tx_digest: ev.tx_digest,
                event_seq: ev.event_num,
            },
            package_id: ev.event.package_id,
            transaction_module: ev.event.transaction_module,
            sender: ev.event.sender,
            type_: ev.event.type_,
            parsed_json: ev.parsed_json,
            bcs: BcsEvent::Base64 {
                bcs: ev.event.contents,
            },
            timestamp_ms: Some(ev.timestamp),
        }
    }
}

impl From<IotaEvent> for Event {
    fn from(val: IotaEvent) -> Self {
        Event {
            package_id: val.package_id,
            transaction_module: val.transaction_module,
            sender: val.sender,
            type_: val.type_,
            contents: val.bcs.into_bytes(),
        }
    }
}

impl IotaEvent {
    pub fn try_from(
        event: Event,
        tx_digest: TransactionDigest,
        event_seq: u64,
        timestamp_ms: Option<u64>,
        layout: MoveDatatypeLayout,
    ) -> IotaResult<Self> {
        let Event {
            package_id,
            transaction_module,
            sender,
            type_: _,
            contents,
        } = event;

        let bcs = BcsEvent::Base64 {
            bcs: contents.to_vec(),
        };

        let move_value = Event::move_event_to_move_value(&contents, layout)?;
        let (type_, fields) = type_and_fields_from_move_event_data(move_value)?;

        Ok(IotaEvent {
            id: EventID {
                tx_digest,
                event_seq,
            },
            package_id,
            transaction_module,
            sender,
            type_,
            parsed_json: fields,
            bcs,
            timestamp_ms,
        })
    }
}

impl Display for IotaEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parsed_json = &mut self.parsed_json.clone();
        bytes_array_to_base64(parsed_json);
        let mut table = json_to_table(parsed_json);
        let style = TableStyle::modern();
        table.collapse().with(style);
        write!(
            f,
            " ┌──\n │ EventID: {}:{}\n │ PackageID: {}\n │ Transaction Module: {}\n │ Sender: {}\n │ EventType: {}\n",
            self.id.tx_digest,
            self.id.event_seq,
            self.package_id,
            self.transaction_module,
            self.sender,
            self.type_
        )?;
        if let Some(ts) = self.timestamp_ms {
            writeln!(f, " │ Timestamp: {ts}\n └──")?;
        }
        writeln!(f, " │ ParsedJSON:")?;
        let table_string = table.to_string();
        let table_rows = table_string.split_inclusive('\n');
        for r in table_rows {
            write!(f, " │   {r}")?;
        }

        write!(f, "\n └──")
    }
}

impl IotaEvent {
    pub fn random_for_testing() -> Self {
        Self {
            id: EventID {
                tx_digest: TransactionDigest::random(),
                event_seq: 0,
            },
            package_id: ObjectID::random(),
            transaction_module: Identifier::from_str("random_for_testing").unwrap(),
            sender: IotaAddress::random_for_testing_only(),
            type_: StructTag::from_str("0x6666::random_for_testing::RandomForTesting").unwrap(),
            parsed_json: json!({}),
            bcs: BcsEvent::new(vec![]),
            timestamp_ms: None,
        }
    }
}

/// Convert a json array of bytes to Base64
fn bytes_array_to_base64(v: &mut Value) {
    match v {
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => (),
        Value::Array(vals) => {
            if let Some(vals) = vals.iter().map(try_into_byte).collect::<Option<Vec<_>>>() {
                *v = json!(Base64::from_bytes(&vals).encoded())
            } else {
                for val in vals {
                    bytes_array_to_base64(val)
                }
            }
        }
        Value::Object(map) => {
            for val in map.values_mut() {
                bytes_array_to_base64(val)
            }
        }
    }
}

/// Try to convert a json Value object into an u8.
fn try_into_byte(v: &Value) -> Option<u8> {
    let num = v.as_u64()?;
    (num <= 255).then_some(num as u8)
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub enum EventFilter {
    /// Query by sender address.
    Sender(IotaAddress),
    /// Return events emitted by the given transaction.
    Transaction(
        /// digest of the transaction, as base-64 encoded string
        TransactionDigest,
    ),
    /// Return events emitted in a specified Package.
    Package(ObjectID),
    /// Return events emitted in a specified Move module.
    /// If the event is defined in Module A but emitted in a tx with Module B,
    /// query `MoveModule` by module B returns the event.
    /// Query `MoveEventModule` by module A returns the event too.
    MoveModule {
        /// the Move package ID
        package: ObjectID,
        /// the module name
        #[schemars(with = "String")]
        #[serde_as(as = "DisplayFromStr")]
        module: Identifier,
    },
    /// Return events with the given Move event struct name (struct tag).
    /// For example, if the event is defined in `0xabcd::MyModule`, and named
    /// `Foo`, then the struct tag is `0xabcd::MyModule::Foo`.
    MoveEventType(
        #[schemars(with = "String")]
        #[serde_as(as = "IotaStructTag")]
        StructTag,
    ),
    /// Return events with the given Move module name where the event struct is
    /// defined. If the event is defined in Module A but emitted in a tx
    /// with Module B, query `MoveEventModule` by module A returns the
    /// event. Query `MoveModule` by module B returns the event too.
    MoveEventModule {
        /// the Move package ID
        package: ObjectID,
        /// the module name
        #[schemars(with = "String")]
        #[serde_as(as = "DisplayFromStr")]
        module: Identifier,
    },
    MoveEventField {
        path: String,
        value: Value,
    },
    /// Return events emitted in [start_time, end_time] interval
    #[serde(rename_all = "camelCase")]
    TimeRange {
        /// left endpoint of time interval, milliseconds since epoch, inclusive
        #[schemars(with = "BigInt<u64>")]
        #[serde_as(as = "BigInt<u64>")]
        start_time: u64,
        /// right endpoint of time interval, milliseconds since epoch, exclusive
        #[schemars(with = "BigInt<u64>")]
        #[serde_as(as = "BigInt<u64>")]
        end_time: u64,
    },

    All(Vec<EventFilter>),
    Any(Vec<EventFilter>),
    And(Box<EventFilter>, Box<EventFilter>),
    Or(Box<EventFilter>, Box<EventFilter>),
}

impl EventFilter {
    fn try_matches(&self, item: &IotaEvent) -> IotaResult<bool> {
        Ok(match self {
            EventFilter::MoveEventType(event_type) => &item.type_ == event_type,
            EventFilter::MoveEventField { path, value } => {
                matches!(item.parsed_json.pointer(path), Some(v) if v == value)
            }
            EventFilter::Sender(sender) => &item.sender == sender,
            EventFilter::Package(object_id) => &item.package_id == object_id,
            EventFilter::MoveModule { package, module } => {
                &item.transaction_module == module && &item.package_id == package
            }
            EventFilter::All(filters) => filters.iter().all(|f| f.matches(item)),
            EventFilter::Any(filters) => filters.iter().any(|f| f.matches(item)),
            EventFilter::And(f1, f2) => {
                EventFilter::All(vec![*(*f1).clone(), *(*f2).clone()]).matches(item)
            }
            EventFilter::Or(f1, f2) => {
                EventFilter::Any(vec![*(*f1).clone(), *(*f2).clone()]).matches(item)
            }
            EventFilter::Transaction(digest) => digest == &item.id.tx_digest,

            EventFilter::TimeRange {
                start_time,
                end_time,
            } => {
                if let Some(timestamp) = &item.timestamp_ms {
                    start_time <= timestamp && end_time > timestamp
                } else {
                    false
                }
            }
            EventFilter::MoveEventModule { package, module } => {
                &item.type_.module == module && &ObjectID::from(item.type_.address) == package
            }
        })
    }

    pub fn and(self, other_filter: EventFilter) -> Self {
        Self::All(vec![self, other_filter])
    }
    pub fn or(self, other_filter: EventFilter) -> Self {
        Self::Any(vec![self, other_filter])
    }
}

impl Filter<IotaEvent> for EventFilter {
    fn matches(&self, item: &IotaEvent) -> bool {
        let _scope = monitored_scope("EventFilter::matches");
        self.try_matches(item).unwrap_or_default()
    }
}

pub trait Filter<T> {
    fn matches(&self, item: &T) -> bool;
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn bcs_event_test() {
        let bytes = vec![0, 1, 2, 3, 4];
        let untagged_base58 = r#"{"bcs":"12VfUX"}"#;
        let tagged_base58 = r#"{"bcsEncoding":"base58","bcs":"12VfUX"}"#;
        let tagged_base64 = r#"{"bcsEncoding":"base64","bcs":"AAECAwQ="}"#;

        assert_eq!(
            bytes,
            serde_json::from_str::<BcsEvent>(untagged_base58)
                .unwrap()
                .into_bytes()
        );
        assert_eq!(
            bytes,
            serde_json::from_str::<BcsEvent>(tagged_base58)
                .unwrap()
                .into_bytes()
        );
        assert_eq!(
            bytes,
            serde_json::from_str::<BcsEvent>(tagged_base64)
                .unwrap()
                .into_bytes()
        );

        // Roundtrip base64
        let event = serde_json::from_str::<BcsEvent>(tagged_base64).unwrap();
        let json = serde_json::to_string(&event).unwrap();
        let from_json = serde_json::from_str::<BcsEvent>(&json).unwrap();
        assert_eq!(event, from_json);

        // Roundtrip base58
        let event = serde_json::from_str::<BcsEvent>(tagged_base58).unwrap();
        let json = serde_json::to_string(&event).unwrap();
        let from_json = serde_json::from_str::<BcsEvent>(&json).unwrap();
        assert_eq!(event, from_json);
    }
}
