// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.event.rs");
include!("../../../generated/iota.grpc.v0.event.field_info.rs");
include!("../../../generated/iota.grpc.v0.event.accessors.rs");

use crate::{proto::TryFromProtoError, v0::bcs::BcsData};

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

    /// Get the raw BCS bytes of the full event structure.
    ///
    /// This contains the entire `iota_sdk_types::Event` serialized as BCS,
    /// including package ID, module, sender, type, and contents.
    /// Use `event_contents_bcs()` for just the event data/contents.
    pub fn event_bcs(&self) -> Result<&[u8], TryFromProtoError> {
        self.bcs
            .as_ref()
            .map(BcsData::as_bytes)
            .ok_or_else(|| TryFromProtoError::missing(Self::BCS_FIELD.name))
    }

    /// Get the package ID of the Move module that emitted this event.
    pub fn package_id(&self) -> Result<iota_sdk_types::Address, TryFromProtoError> {
        self.package_id
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::PACKAGE_ID_FIELD.name))?
            .try_into()
            .map_err(|e: TryFromProtoError| e.nested(Self::PACKAGE_ID_FIELD.name))
    }

    /// Get the module name of the Move module that emitted this event.
    pub fn module_name(&self) -> Result<&str, TryFromProtoError> {
        self.module
            .as_deref()
            .ok_or_else(|| TryFromProtoError::missing(Self::MODULE_FIELD.name))
    }

    /// Get the sender address of the transaction that emitted this event.
    pub fn sender_address(&self) -> Result<iota_sdk_types::Address, TryFromProtoError> {
        self.sender
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::SENDER_FIELD.name))?
            .try_into()
            .map_err(|e: TryFromProtoError| e.nested(Self::SENDER_FIELD.name))
    }

    /// Get the type of the event emitted.
    pub fn type_name(&self) -> Result<&str, TryFromProtoError> {
        self.event_type
            .as_deref()
            .ok_or_else(|| TryFromProtoError::missing(Self::EVENT_TYPE_FIELD.name))
    }

    /// Get the raw BCS bytes of the event contents/data only.
    ///
    /// This is the serialized event data without the metadata (package, module,
    /// sender, type). Use `event_bcs()` for the full event structure.
    pub fn event_contents_bcs(&self) -> Result<&[u8], TryFromProtoError> {
        self.bcs_contents
            .as_ref()
            .map(BcsData::as_bytes)
            .ok_or_else(|| TryFromProtoError::missing(Self::BCS_CONTENTS_FIELD.name))
    }

    /// Get the JSON contents of the event.
    pub fn json_contents(&self) -> Result<serde_json::Value, TryFromProtoError> {
        self.json_contents
            .as_ref()
            .map(crate::proto::prost_to_json)
            .ok_or_else(|| TryFromProtoError::missing(Self::JSON_CONTENTS_FIELD.name))
    }
}

// Convenience methods for Events (delegate to TryFrom)
impl Events {
    /// Deserialize all events.
    pub fn events(&self) -> Result<Vec<iota_sdk_types::Event>, TryFromProtoError> {
        self.try_into()
    }
}
