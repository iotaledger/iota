// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.event.rs");
include!("../../../generated/iota.grpc.v0.event.field_info.rs");

use crate::proto::TryFromProtoError;

// TryFrom implementations for Event
impl TryFrom<&Event> for iota_sdk_types::Event {
    type Error = TryFromProtoError;

    fn try_from(value: &Event) -> Result<Self, Self::Error> {
        let bcs = value
            .bcs
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Event::BCS_FIELD.name))?;

        bcs.deserialize()
            .map_err(|e| TryFromProtoError::invalid(Event::BCS_FIELD.name, e))
    }
}

impl TryFrom<&Events> for Vec<iota_sdk_types::Event> {
    type Error = TryFromProtoError;

    fn try_from(value: &Events) -> Result<Self, Self::Error> {
        value
            .events
            .iter()
            .enumerate()
            .map(|(i, event)| {
                <&Event as TryInto<iota_sdk_types::Event>>::try_into(event)
                    .map_err(|e: TryFromProtoError| e.nested_at(Events::EVENTS_FIELD.name, i))
            })
            .collect()
    }
}

// Convenience methods for Event (delegate to TryFrom)
impl Event {
    /// Deserialize the event from BCS.
    pub fn event(&self) -> Result<iota_sdk_types::Event, TryFromProtoError> {
        self.try_into()
    }
}

// Convenience methods for Events (delegate to TryFrom)
impl Events {
    /// Deserialize all events.
    pub fn events(&self) -> Result<Vec<iota_sdk_types::Event>, TryFromProtoError> {
        self.try_into()
    }
}
