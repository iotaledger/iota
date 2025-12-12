// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.event.rs");
include!("../../../generated/iota.grpc.v0.event.field_info.rs");
include!("../../../generated/iota.grpc.v0.event.accessors.rs");

use iota_json_rpc_types::IotaEvent;

use crate::{
    field::FieldMaskTree,
    merge::Merge,
    v0::{bcs as grpc_bcs, event as grpc_event, types as grpc_types},
};

// Convert IotaEvent to protobuf Event
impl From<&IotaEvent> for grpc_event::Event {
    fn from(event: &IotaEvent) -> Self {
        grpc_event::Event {
            bcs: grpc_bcs::BcsData::serialize(&event).ok(),
            package_id: Some(grpc_types::Address {
                address: event.package_id.into_bytes().to_vec().into(),
            }),
            module: Some(event.transaction_module.to_string()),
            sender: Some(grpc_types::Address {
                address: event.sender.to_vec().into(),
            }),
            event_type: Some(event.type_.to_canonical_string(true)),
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
                    Merge::merge_from(event, &events_mask)
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
            self.bcs = grpc_bcs::BcsData::serialize(&source).ok();
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
