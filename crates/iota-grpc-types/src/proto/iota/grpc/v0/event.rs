// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.event.rs");
include!("../../../generated/iota.grpc.v0.event.field_info.rs");

use crate::{
    field::FieldMaskTree,
    merge::Merge,
    v0::{bcs as grpc_bcs, event as grpc_event, types as grpc_types},
};

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

// TryFrom implementations for Event
impl TryFrom<&grpc_event::Event> for iota_sdk_types::Event {
    type Error = crate::proto::TryFromProtoError;

    fn try_from(value: &grpc_event::Event) -> Result<Self, Self::Error> {
        let bcs = value.bcs.as_ref().ok_or_else(|| {
            crate::proto::TryFromProtoError::missing(grpc_event::Event::BCS_FIELD.name)
        })?;

        bcs.deserialize().map_err(|e| {
            crate::proto::TryFromProtoError::invalid(grpc_event::Event::BCS_FIELD.name, e)
        })
    }
}

impl TryFrom<&grpc_event::Events> for Vec<iota_sdk_types::Event> {
    type Error = crate::proto::TryFromProtoError;

    fn try_from(value: &grpc_event::Events) -> Result<Self, Self::Error> {
        value
            .events
            .iter()
            .enumerate()
            .map(|(i, event)| {
                <&grpc_event::Event as TryInto<iota_sdk_types::Event>>::try_into(event).map_err(
                    |e: crate::proto::TryFromProtoError| {
                        e.nested_at(grpc_event::Events::EVENTS_FIELD.name, i)
                    },
                )
            })
            .collect()
    }
}

// Convenience methods for Event (delegate to TryFrom)
impl grpc_event::Event {
    /// Deserialize the event from BCS.
    pub fn event(&self) -> Result<iota_sdk_types::Event, crate::proto::TryFromProtoError> {
        self.try_into()
    }
}

// Convenience methods for Events (delegate to TryFrom)
impl grpc_event::Events {
    /// Deserialize all events.
    pub fn events(&self) -> Result<Vec<iota_sdk_types::Event>, crate::proto::TryFromProtoError> {
        self.try_into()
    }
}
