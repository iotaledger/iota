// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.event.rs");
include!("../../../generated/iota.grpc.v0.event.field_info.rs");
include!("../../../generated/iota.grpc.v0.event.accessors.rs");

use iota_json_rpc_types::{IotaEvent, type_and_fields_from_move_event_data};
use iota_types::object::bounded_visitor::BoundedVisitor;
use prost_types::value::Kind;

use crate::{
    field::FieldMaskTree,
    merge::Merge,
    v0::{bcs as grpc_bcs, event as grpc_event, types as grpc_types},
};

// Convert IotaEvent to protobuf Event
impl From<&IotaEvent> for grpc_event::Event {
    fn from(event: &IotaEvent) -> Self {
        grpc_event::Event {
            bcs: Some(grpc_bcs::BcsData {
                data: bcs::to_bytes(&event)
                    .expect("BCS serialization should not fail")
                    .into(),
            }),
            package_id: Some(grpc_types::Address {
                address: event.package_id.into_bytes().to_vec().into(),
            }),
            module: Some(event.transaction_module.to_string()),
            sender: Some(grpc_types::Address {
                address: event.sender.to_vec().into(),
            }),
            event_type: Some(event.type_.to_string()),
            bcs_contents: Some(grpc_bcs::BcsData {
                data: event.bcs.clone().into_bytes().into(),
            }),
            json_contents: None, // TODO: fill in JSON contents
        }
    }
}

// Merge implementation for Events from iota_sdk_types::TransactionEvents
impl Merge<&iota_sdk_types::TransactionEvents> for grpc_event::Events {
    fn merge(
        &mut self,
        source: &iota_sdk_types::TransactionEvents,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(events_mask) = mask.subtree(Self::EVENTS_FIELD.name) {
            // TransactionEvents is a tuple struct with Vec<Event> at index 0
            self.events = source
                .0
                .iter()
                .map(|event| -> Result<_, Box<dyn std::error::Error>> {
                    let mut proto_event = grpc_event::Event::default();
                    proto_event.merge(event, &events_mask)?;
                    Ok(proto_event)
                })
                .collect::<Result<Vec<_>, _>>()?;
        }

        Ok(())
    }
}

// Merge implementation for individual Event from iota_sdk_types::Event
impl Merge<&iota_sdk_types::Event> for grpc_event::Event {
    fn merge(
        &mut self,
        source: &iota_sdk_types::Event,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if mask.contains(Self::BCS_FIELD.name) {
            if let Ok(bcs_bytes) = bcs::to_bytes(source) {
                self.bcs = Some(grpc_bcs::BcsData {
                    data: bcs_bytes.into(),
                });
            }
        }

        if mask.contains(Self::PACKAGE_ID_FIELD.name) {
            self.package_id = Some(grpc_types::Address {
                address: source.package_id.as_bytes().to_vec().into(),
            });
        }

        if mask.contains(Self::MODULE_FIELD.name) {
            self.module = Some(source.module.to_string());
        }

        if mask.contains(Self::SENDER_FIELD.name) {
            self.sender = Some(grpc_types::Address {
                address: source.sender.as_bytes().to_vec().into(),
            });
        }

        if mask.contains(Self::EVENT_TYPE_FIELD.name) {
            self.event_type = Some(source.type_.to_string());
        }

        if mask.contains(Self::BCS_CONTENTS_FIELD.name) {
            self.bcs_contents = Some(grpc_bcs::BcsData {
                data: source.contents.clone().into(),
            });
        }

        Ok(())

        // json_contents is not populated here by default - it requires Move
        // type layout information which is not available at this level.
        // The caller should use `populate_json_contents_with_layout` if
        // json_contents is needed.
    }
}

/// Convert serde_json::Value to prost_types::Value
fn json_to_prost_value(value: serde_json::Value) -> Option<prost_types::Value> {
    let kind = match value {
        serde_json::Value::Null => Kind::NullValue(0),
        serde_json::Value::Bool(b) => Kind::BoolValue(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Kind::NumberValue(i as f64)
            } else if let Some(u) = n.as_u64() {
                Kind::NumberValue(u as f64)
            } else if let Some(f) = n.as_f64() {
                Kind::NumberValue(f)
            } else {
                return None;
            }
        }
        serde_json::Value::String(s) => Kind::StringValue(s),
        serde_json::Value::Array(arr) => {
            let values: Option<Vec<prost_types::Value>> =
                arr.into_iter().map(json_to_prost_value).collect();
            Kind::ListValue(prost_types::ListValue { values: values? })
        }
        serde_json::Value::Object(obj) => {
            let mut fields = std::collections::BTreeMap::new();
            for (k, v) in obj {
                let prost_v = json_to_prost_value(v)?;
                fields.insert(k, prost_v);
            }
            Kind::StructValue(prost_types::Struct { fields })
        }
    };

    Some(prost_types::Value { kind: Some(kind) })
}

impl grpc_event::Event {
    /// Populate json_contents for this event using the provided Move type
    /// layout. This uses `type_and_fields_from_move_event_data` to convert
    /// BCS contents to JSON.
    ///
    /// Returns true if json_contents was successfully populated, false
    /// otherwise.
    pub fn populate_json_contents_with_layout(
        &mut self,
        event: &iota_sdk_types::Event,
        layout: &move_core_types::annotated_value::MoveDatatypeLayout,
    ) -> bool {
        // Deserialize BCS contents using the layout
        let Ok(move_value) =
            BoundedVisitor::deserialize_value(&event.contents, &layout.clone().into_layout())
        else {
            return false;
        };

        // Convert to JSON using type_and_fields_from_move_event_data
        let Ok((_type, json_value)) = type_and_fields_from_move_event_data(move_value) else {
            return false;
        };

        // Convert serde_json::Value to prost_types::Value
        let Some(prost_value) = json_to_prost_value(json_value) else {
            return false;
        };

        self.json_contents = Some(Box::new(prost_value));
        true
    }
}
