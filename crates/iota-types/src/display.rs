// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::{StructTag, TypeTag};
use serde::Deserialize;

use crate::{
    collection_types::VecMap,
    event::Event,
    id::{ID, UID},
};

// TODO: add tests to keep in sync
/// Rust version of the Move iota::display::Display type
#[derive(Debug, Deserialize, Clone, Eq, PartialEq)]
pub struct DisplayObject {
    pub id: UID,
    pub fields: VecMap<String, String>,
    pub version: u16,
}

#[derive(Deserialize, Debug)]
/// The event that is emitted when a `Display` version is "released".
/// Serves for Display versioning.
pub struct DisplayVersionUpdatedEvent {
    pub id: ID,
    pub version: u16,
    pub fields: VecMap<String, String>,
}

impl DisplayVersionUpdatedEvent {
    // Checks if the provided `StructTag` is a DisplayVersionUpdatedEvent<T> and
    // returns a reference to the inner type T if so.
    pub fn inner_type(inner: &StructTag) -> Option<&StructTag> {
        if !inner.is_display_version_updated() {
            return None;
        }

        match inner.type_params() {
            [TypeTag::Struct(struct_type)] => Some(struct_type),
            _ => None,
        }
    }

    pub fn try_from_event(event: &Event) -> Option<(&StructTag, Self)> {
        let inner_type = Self::inner_type(&event.type_)?;

        bcs::from_bytes(&event.contents)
            .ok()
            .map(|event| (inner_type, event))
    }
}

#[derive(Deserialize, Debug)]
pub struct DisplayCreatedEvent {
    // The Object ID of Display Object
    pub id: ID,
}
