// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.event.rs");
include!("../../../generated/iota.grpc.v0.event.field_info.rs");

use crate::v0::event as grpc_event;

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
