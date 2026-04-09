// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v1.event.rs");
include!("../../../generated/iota.grpc.v1.event.field_info.rs");
include!("../../../generated/iota.grpc.v1.event.accessors.rs");

use crate::{
    proto::{TryFromProtoError, get_inner_field},
    v1::{bcs::BcsData, versioned::VersionedEvent},
};

// TryFrom implementations for Event
impl TryFrom<&Event> for iota_sdk_types::Event {
    type Error = TryFromProtoError;

    fn try_from(value: &Event) -> Result<Self, Self::Error> {
        let bcs = value
            .bcs
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Event::BCS_FIELD.name))?;

        bcs.deserialize::<VersionedEvent>()
            .map_err(|e| TryFromProtoError::invalid(Event::BCS_FIELD.name, e))?
            .try_into_v1()
            .map_err(|_| {
                TryFromProtoError::invalid(Event::BCS_FIELD.name, "unsupported Event version")
            })
    }
}

// Convenience methods for Event (delegate to TryFrom)
impl Event {
    /// Deserialize the full event from BCS.
    ///
    /// **Read mask:** `bcs` relative to this event (see [`EVENT_BCS`]).
    /// Full path depends on context — e.g. `"events.bcs"` for checkpoint
    /// top-level events, `"events.events.bcs"` for transaction events.
    ///
    /// [`EVENT_BCS`]: crate::read_masks::EVENT_BCS
    pub fn event(&self) -> Result<iota_sdk_types::Event, TryFromProtoError> {
        self.try_into()
    }

    /// Get the package ID of the Move module that emitted this event.
    ///
    /// **Read mask:** `package_id` relative to this event (see
    /// [`EVENT_PACKAGE_ID`]).
    ///
    /// [`EVENT_PACKAGE_ID`]: crate::read_masks::EVENT_PACKAGE_ID
    pub fn package_id(&self) -> Result<iota_sdk_types::ObjectId, TryFromProtoError> {
        get_inner_field!(self.package_id, Self::PACKAGE_ID_FIELD, object_id)
    }

    /// Get the module name of the Move module that emitted this event.
    ///
    /// **Read mask:** `module` relative to this event (see [`EVENT_MODULE`]).
    ///
    /// [`EVENT_MODULE`]: crate::read_masks::EVENT_MODULE
    pub fn module_name(&self) -> Result<iota_sdk_types::Identifier, TryFromProtoError> {
        self.module
            .as_deref()
            .ok_or_else(|| TryFromProtoError::missing(Self::MODULE_FIELD.name))?
            .parse()
            .map_err(|_e: iota_sdk_types::TypeParseError| {
                TryFromProtoError::invalid(Self::MODULE_FIELD.name, "invalid identifier format")
            })
    }

    /// Get the sender address of the transaction that emitted this event.
    ///
    /// **Read mask:** `sender` relative to this event (see [`EVENT_SENDER`]).
    ///
    /// [`EVENT_SENDER`]: crate::read_masks::EVENT_SENDER
    pub fn sender(&self) -> Result<iota_sdk_types::Address, TryFromProtoError> {
        get_inner_field!(self.sender, Self::SENDER_FIELD, try_into)
    }

    /// Get the type of the event emitted.
    ///
    /// **Read mask:** `event_type` relative to this event (see
    /// [`EVENT_TYPE`]).
    ///
    /// [`EVENT_TYPE`]: crate::read_masks::EVENT_TYPE
    pub fn type_name(&self) -> Result<iota_sdk_types::StructTag, TryFromProtoError> {
        self.event_type
            .as_deref()
            .ok_or_else(|| TryFromProtoError::missing(Self::EVENT_TYPE_FIELD.name))?
            .parse()
            .map_err(|_e: iota_sdk_types::TypeParseError| {
                TryFromProtoError::invalid(Self::EVENT_TYPE_FIELD.name, "invalid struct tag format")
            })
    }

    /// Get the raw BCS bytes of the event contents/data only.
    ///
    /// This is the serialized event data without the metadata (package, module,
    /// sender, type).
    ///
    /// **Read mask:** `bcs_contents` relative to this event (see
    /// [`EVENT_BCS_CONTENTS`]).
    ///
    /// [`EVENT_BCS_CONTENTS`]: crate::read_masks::EVENT_BCS_CONTENTS
    pub fn bcs_contents(&self) -> Result<&[u8], TryFromProtoError> {
        self.bcs_contents
            .as_ref()
            .map(BcsData::as_bytes)
            .ok_or_else(|| TryFromProtoError::missing(Self::BCS_CONTENTS_FIELD.name))
    }

    /// Get the JSON contents of the event.
    ///
    /// **Read mask:** `json_contents` relative to this event (see
    /// [`EVENT_JSON_CONTENTS`]).
    ///
    /// [`EVENT_JSON_CONTENTS`]: crate::read_masks::EVENT_JSON_CONTENTS
    pub fn json_contents(&self) -> Result<serde_json::Value, TryFromProtoError> {
        self.json_contents
            .as_ref()
            .map(crate::proto::prost_to_json)
            .ok_or_else(|| TryFromProtoError::missing(Self::JSON_CONTENTS_FIELD.name))
    }
}
